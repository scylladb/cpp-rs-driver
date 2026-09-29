//! End-to-end Client Routes coverage through the C API.

use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::net::SocketAddr;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use scylla::cluster::KnownNode;
use scylla_ccm_bridge::CLUSTER_VERSION;
use scylla_ccm_bridge::client_routes::{
    ClientRoutesCluster, FeedbackItem, drain_feedback, run_client_routes_test,
};
use scylla_ccm_bridge::cluster::ClusterOptions;
use scylla_ccm_bridge::node::NodeId;
use tokio::sync::oneshot;

use scylladb::api::cluster::{
    cass_cluster_add_client_routes_proxy, cass_cluster_free, cass_cluster_new,
    cass_cluster_set_contact_points, cass_cluster_set_load_balance_round_robin,
    cass_cluster_set_port,
};
use scylladb::api::error::CassError;
use scylladb::api::future::{cass_future_error_code, cass_future_free, cass_future_wait};
use scylladb::api::session::{
    cass_session_close, cass_session_connect, cass_session_execute, cass_session_free,
    cass_session_new,
};
use scylladb::api::statement::{cass_statement_free, cass_statement_new};
use scylladb::argconv::CassStrNulTerminated;

use crate::utils::{assert_cass_error_eq, setup_tracing};

use super::warn_if_partial_cluster_version;

const CONNECTION_ID: &str = "rust-driver-test-dc0";
const QUERY_BATCH_SIZE: usize = 12;
const FEEDBACK_TIMEOUT: Duration = Duration::from_secs(10);

fn cluster_3_nodes() -> ClusterOptions {
    ClusterOptions {
        name: "cpp_rs_client_routes_3_nodes".to_owned(),
        version: CLUSTER_VERSION.clone(),
        nodes_per_dc: vec![3],
        ..ClusterOptions::default()
    }
}

enum WorkerCommand {
    Execute {
        count: usize,
        done: oneshot::Sender<CassError>,
    },
    Close {
        done: Option<oneshot::Sender<()>>,
    },
}

/// Keeps all C-owned pointers on one blocking thread while the async test
/// changes proxy rules and injects events.
struct CSessionWorker {
    commands: mpsc::Sender<WorkerCommand>,
    thread: Option<thread::JoinHandle<()>>,
}

impl CSessionWorker {
    fn spawn(contact_point: SocketAddr) -> (Self, oneshot::Receiver<CassError>) {
        let (commands, command_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = oneshot::channel();

        let thread = thread::spawn(move || unsafe {
            let mut cluster = cass_cluster_new();
            let contact_host = CString::new(contact_point.ip().to_string()).unwrap();
            let connection_id = CString::new(CONNECTION_ID).unwrap();
            let hostname_override = c"127.0.0.1";

            let contact_result = cass_cluster_set_contact_points(
                cluster.borrow_mut(),
                CassStrNulTerminated::from_cstr(&contact_host),
            );
            let port_result =
                cass_cluster_set_port(cluster.borrow_mut(), contact_point.port().into());
            let routes_result = cass_cluster_add_client_routes_proxy(
                cluster.borrow_mut(),
                CassStrNulTerminated::from_cstr(&connection_id),
                CassStrNulTerminated::from_cstr(hostname_override),
            );
            cass_cluster_set_load_balance_round_robin(cluster.borrow_mut());

            let session = cass_session_new();
            let connect_result = if contact_result != CassError::CASS_OK {
                contact_result
            } else if port_result != CassError::CASS_OK {
                port_result
            } else if routes_result != CassError::CASS_OK {
                routes_result
            } else {
                let future =
                    cass_session_connect(session.borrow(), cluster.borrow().into_c_const());
                cass_future_wait(future.borrow());
                let result = cass_future_error_code(future.borrow());
                cass_future_free(future);
                result
            };

            let _ = ready_tx.send(connect_result);

            if connect_result == CassError::CASS_OK {
                while let Ok(command) = command_rx.recv() {
                    match command {
                        WorkerCommand::Execute { count, done } => {
                            let mut result = CassError::CASS_OK;
                            for _ in 0..count {
                                let statement = cass_statement_new(
                                    CassStrNulTerminated::from_cstr(
                                        c"SELECT host_id FROM system.local",
                                    ),
                                    0,
                                );
                                let future = cass_session_execute(
                                    session.borrow(),
                                    statement.borrow().into_c_const(),
                                );
                                cass_future_wait(future.borrow());
                                result = cass_future_error_code(future.borrow());
                                cass_future_free(future);
                                cass_statement_free(statement);
                                if result != CassError::CASS_OK {
                                    break;
                                }
                            }
                            let _ = done.send(result);
                        }
                        WorkerCommand::Close { done } => {
                            if let Some(done) = done {
                                let _ = done.send(());
                            }
                            break;
                        }
                    }
                }

                let future = cass_session_close(session.borrow());
                cass_future_wait(future.borrow());
                cass_future_free(future);
            }

            cass_session_free(session);
            cass_cluster_free(cluster);
        });

        (
            Self {
                commands,
                thread: Some(thread),
            },
            ready_rx,
        )
    }

    async fn execute(&self, count: usize) -> CassError {
        let (done, result) = oneshot::channel();
        self.commands
            .send(WorkerCommand::Execute { count, done })
            .expect("C session worker stopped before query execution");
        result.await.expect("C session worker dropped query result")
    }

    async fn close(mut self) {
        let (done, closed) = oneshot::channel();
        self.commands
            .send(WorkerCommand::Close { done: Some(done) })
            .expect("C session worker stopped before close");
        closed.await.expect("C session worker dropped close result");
        if let Some(thread) = self.thread.take() {
            tokio::task::spawn_blocking(move || thread.join().unwrap())
                .await
                .unwrap();
        }
    }
}

impl Drop for CSessionWorker {
    fn drop(&mut self) {
        let _ = self.commands.send(WorkerCommand::Close { done: None });
        // Never join here: Drop can run while unwinding on the Tokio runtime.
        // A query in flight may need proxy tasks on that runtime to finish, so
        // waiting for this thread synchronously could deadlock the test.
        let _ = self.thread.take();
    }
}

fn contact_point(plc: &ClientRoutesCluster) -> SocketAddr {
    let known_nodes = plc.make_session_builder().config.known_nodes;
    assert_eq!(
        known_nodes.len(),
        1,
        "single-DC test needs one contact point"
    );
    match known_nodes.into_iter().next().unwrap() {
        KnownNode::Hostname(hostname) => hostname.parse().unwrap(),
        KnownNode::Address(address) => address,
        _ => panic!("unexpected non-exhaustive KnownNode variant"),
    }
}

async fn assert_queries_reach_every_node(
    worker: &CSessionWorker,
    receivers: &mut HashMap<NodeId, tokio::sync::mpsc::UnboundedReceiver<FeedbackItem>>,
) {
    let expected: HashSet<NodeId> = receivers.keys().copied().collect();
    assert_eq!(
        expected.len(),
        3,
        "test harness should expose all three nodes"
    );
    let mut reached = HashSet::new();

    let result = tokio::time::timeout(FEEDBACK_TIMEOUT, async {
        while reached != expected {
            assert_cass_error_eq(CassError::CASS_OK, worker.execute(QUERY_BATCH_SIZE).await);
            let (per_node, _) = drain_feedback(receivers);
            reached.extend(
                per_node
                    .into_iter()
                    .filter_map(|(node_id, count)| (count > 0).then_some(node_id)),
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;

    assert!(
        result.is_ok(),
        "C API queries did not reach every Client Route; reached {reached:?}, expected {expected:?}"
    );
}

async fn wait_for_any_feedback(
    receivers: &mut HashMap<NodeId, tokio::sync::mpsc::UnboundedReceiver<FeedbackItem>>,
) {
    tokio::time::timeout(FEEDBACK_TIMEOUT, async {
        loop {
            if receivers.values_mut().any(|rx| rx.try_recv().is_ok()) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("driver did not re-query system.client_routes after CLIENT_ROUTES_CHANGE");
}

async fn exercise_client_routes_through_c_api(plc: &mut ClientRoutesCluster) {
    // This address is used only to bootstrap. Discovered topology addresses
    // must subsequently be translated using routes selected by CONNECTION_ID.
    let bootstrap = contact_point(plc);
    let (worker, connected) = CSessionWorker::spawn(bootstrap);
    assert_cass_error_eq(
        CassError::CASS_OK,
        connected
            .await
            .expect("C session worker stopped during connect"),
    );

    // Proxy feedback proves that discovered nodes are reached through their
    // NLB routes: the C driver never receives the proxy addresses themselves.
    let mut query_feedback = plc.setup_query_feedback();
    assert_queries_reach_every_node(&worker, &mut query_feedback).await;

    // Prove the C session also installs the Client Routes event path.
    let mut requery_feedback = plc.setup_event_requery_detection();
    let active_nodes = plc.active_node_ids();
    assert_eq!(
        plc.inject_event(&active_nodes),
        1,
        "CLIENT_ROUTES_CHANGE should be injected into the control connection"
    );
    wait_for_any_feedback(&mut requery_feedback).await;

    worker.close().await;
}

#[tokio::test]
async fn client_routes_c_api_end_to_end() {
    setup_tracing();
    warn_if_partial_cluster_version();
    run_client_routes_test(cluster_3_nodes, exercise_client_routes_through_c_api).await;
}
