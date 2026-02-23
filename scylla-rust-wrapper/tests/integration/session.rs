use std::{
    ffi::{CStr, c_void},
    sync::atomic::{AtomicUsize, Ordering},
};

use libc::c_char;
use rusty_fork::rusty_fork_test;
use scylla::errors::DbError;
use scylla_cpp_driver::{
    api::{
        batch::{
            CassBatch, CassBatchType, cass_batch_add_statement, cass_batch_free, cass_batch_new,
            cass_batch_set_execution_profile, cass_batch_set_retry_policy,
        },
        cluster::{
            cass_cluster_free, cass_cluster_new, cass_cluster_set_client_id,
            cass_cluster_set_contact_points, cass_cluster_set_contact_points_n,
            cass_cluster_set_execution_profile, cass_cluster_set_latency_aware_routing,
            cass_cluster_set_retry_policy,
        },
        error::CassError,
        execution_profile::{
            cass_execution_profile_free, cass_execution_profile_new,
            cass_execution_profile_set_latency_aware_routing,
            cass_execution_profile_set_retry_policy,
        },
        future::{
            CassFuture, cass_future_error_code, cass_future_free, cass_future_set_callback,
            cass_future_wait,
        },
        retry_policy::{
            CassRetryPolicy, cass_retry_policy_default_new, cass_retry_policy_fallthrough_new,
        },
        session::{
            CassSession, cass_session_close, cass_session_connect, cass_session_execute,
            cass_session_execute_batch, cass_session_free, cass_session_get_client_id,
            cass_session_new, cass_session_prepare,
        },
        statement::{
            CassStatement, cass_statement_free, cass_statement_new,
            cass_statement_set_execution_profile, cass_statement_set_retry_policy,
        },
        uuid::CassUuid,
    },
    argconv::{ArcFFI, CConst, CMut, CassBorrowedExclusivePtr, CassBorrowedSharedPtr},
    types::cass_bool_t,
};
use scylla_cql::Consistency;
use scylla_proxy::{
    Condition, ProxyError, RequestOpcode, RequestReaction, RequestRule, RunningProxy, WorkerError,
};
use tracing::instrument::WithSubscriber as _;

use crate::utils::{
    assert_cass_error_eq, cass_future_wait_check_and_free, generic_drop_queries_rules,
    handshake_rules, make_c_str, mock_init_rules, proxy_uris_to_contact_points, setup_tracing,
    str_to_c_str_n, test_with_3_node_dry_mode_cluster,
};

#[tokio::test]
#[ntest::timeout(30000)]
async fn retry_policy_on_statement_and_batch_is_handled_properly() {
    setup_tracing();
    let res = test_with_3_node_dry_mode_cluster(
        retry_policy_on_statement_and_batch_is_handled_properly_rules,
        retry_policy_on_statement_and_batch_is_handled_properly_do,
    )
    .with_current_subscriber()
    .await;

    match res {
        Ok(()) => (),
        Err(ProxyError::Worker(WorkerError::DriverDisconnected(_))) => (),
        Err(err) => panic!("{}", err),
    }
}

fn retry_policy_on_statement_and_batch_is_handled_properly_rules()
-> impl IntoIterator<Item = RequestRule> {
    handshake_rules()
        .into_iter()
        .chain(std::iter::once(RequestRule(
            Condition::any([
                Condition::RequestOpcode(RequestOpcode::Query),
                Condition::RequestOpcode(RequestOpcode::Batch),
                Condition::RequestOpcode(RequestOpcode::Prepare),
            ])
            .and(Condition::not(Condition::ConnectionRegisteredAnyEvent))
            // this 1 differentiates Fallthrough and Default retry policies.
            .and(Condition::TrueForLimitedTimes(1)),
            // We simulate the read timeout error in order to trigger DefaultRetryPolicy's
            // retry on the same node.
            // We don't use the example ReadTimeout error that is included in proxy,
            // because in order to trigger a retry we need data_present=false.
            RequestReaction::forge_with_error(DbError::ReadTimeout {
                consistency: Consistency::All,
                received: 1,
                required: 1,
                data_present: false,
            }),
        )))
        .chain(std::iter::once(RequestRule(
            Condition::any([
                Condition::RequestOpcode(RequestOpcode::Query),
                Condition::RequestOpcode(RequestOpcode::Batch),
                Condition::RequestOpcode(RequestOpcode::Prepare),
            ])
            .and(Condition::not(Condition::ConnectionRegisteredAnyEvent)),
            // We make the second attempt return a hard, nonrecoverable error.
            RequestReaction::forge().read_failure(),
        )))
        .chain(generic_drop_queries_rules())
}

// This test aims to verify that the retry policy emulation works properly,
// in any sequence of actions mutating the retry policy for a query.
//
// Below, the consecutive states of the test case are illustrated:
//     Retry policy set on: ('F' - Fallthrough, 'D' - Default, '-' - no policy set)
//     session default exec profile:   F F F F F F F F F F F F F F
//     per stmt/batch exec profile:    - D - - D D D D D - - - D D
//     stmt/batch (emulated):          - - - F F - F D F F - D D -
fn retry_policy_on_statement_and_batch_is_handled_properly_do(
    proxy_uris: [String; 3],
    mut proxy: RunningProxy,
) -> RunningProxy {
    unsafe {
        let mut cluster_raw = cass_cluster_new();
        let contact_points = proxy_uris_to_contact_points(proxy_uris);

        assert_cass_error_eq(
            cass_cluster_set_contact_points(cluster_raw.borrow_mut(), contact_points.as_ptr()),
            CassError::CASS_OK,
        );

        let fallthrough_policy = cass_retry_policy_fallthrough_new();
        let default_policy = cass_retry_policy_default_new();
        cass_cluster_set_retry_policy(cluster_raw.borrow_mut(), fallthrough_policy.borrow());

        let session_raw = cass_session_new();

        let mut profile_raw = cass_execution_profile_new();
        // A name of a profile that will have been registered in the Cluster.
        let profile_name_c_str = make_c_str!("profile");

        assert_cass_error_eq(
            cass_execution_profile_set_retry_policy(
                profile_raw.borrow_mut(),
                default_policy.borrow(),
            ),
            CassError::CASS_OK,
        );

        let query = make_c_str!("SELECT host_id FROM system.local WHERE key='local'");
        let mut statement_raw = cass_statement_new(query, 0);
        let mut batch_raw = cass_batch_new(CassBatchType::CASS_BATCH_TYPE_LOGGED);
        assert_cass_error_eq(
            cass_batch_add_statement(batch_raw.borrow_mut(), statement_raw.borrow()),
            CassError::CASS_OK,
        );

        assert_cass_error_eq(
            cass_cluster_set_execution_profile(
                cluster_raw.borrow_mut(),
                profile_name_c_str,
                profile_raw.borrow_mut(),
            ),
            CassError::CASS_OK,
        );

        cass_future_wait_check_and_free(cass_session_connect(
            session_raw.borrow(),
            cluster_raw.borrow().into_c_const(),
        ));
        {
            unsafe fn execute_query(
                session_raw: CassBorrowedSharedPtr<CassSession, CMut>,
                statement_raw: CassBorrowedSharedPtr<CassStatement, CConst>,
            ) -> CassError {
                unsafe {
                    cass_future_error_code(
                        cass_session_execute(session_raw, statement_raw).borrow(),
                    )
                }
            }
            unsafe fn execute_batch(
                session_raw: CassBorrowedSharedPtr<CassSession, CMut>,
                batch_raw: CassBorrowedSharedPtr<CassBatch, CConst>,
            ) -> CassError {
                unsafe {
                    cass_future_error_code(
                        cass_session_execute_batch(session_raw, batch_raw).borrow(),
                    )
                }
            }

            fn reset_proxy_rules(proxy: &mut RunningProxy) {
                proxy.running_nodes.iter_mut().for_each(|node| {
                    node.change_request_rules(Some(
                        retry_policy_on_statement_and_batch_is_handled_properly_rules()
                            .into_iter()
                            .collect(),
                    ))
                })
            }

            unsafe fn assert_query_with_fallthrough_policy(
                proxy: &mut RunningProxy,
                session_raw: CassBorrowedSharedPtr<CassSession, CMut>,
                statement_raw: CassBorrowedSharedPtr<CassStatement, CConst>,
                batch_raw: CassBorrowedSharedPtr<CassBatch, CConst>,
            ) {
                reset_proxy_rules(&mut *proxy);
                unsafe {
                    assert_cass_error_eq(
                        execute_query(session_raw.borrow(), statement_raw),
                        CassError::CASS_ERROR_SERVER_READ_TIMEOUT,
                    );
                    reset_proxy_rules(&mut *proxy);
                    assert_cass_error_eq(
                        execute_batch(session_raw, batch_raw),
                        CassError::CASS_ERROR_SERVER_READ_TIMEOUT,
                    );
                }
            }

            unsafe fn assert_query_with_default_policy(
                proxy: &mut RunningProxy,
                session_raw: CassBorrowedSharedPtr<CassSession, CMut>,
                statement_raw: CassBorrowedSharedPtr<CassStatement, CConst>,
                batch_raw: CassBorrowedSharedPtr<CassBatch, CConst>,
            ) {
                reset_proxy_rules(&mut *proxy);
                unsafe {
                    assert_cass_error_eq(
                        execute_query(session_raw.borrow(), statement_raw),
                        CassError::CASS_ERROR_SERVER_READ_FAILURE,
                    );
                    reset_proxy_rules(&mut *proxy);
                    assert_cass_error_eq(
                        execute_batch(session_raw, batch_raw),
                        CassError::CASS_ERROR_SERVER_READ_FAILURE,
                    );
                }
            }

            unsafe fn set_provided_exec_profile(
                name: *const i8,
                statement_raw: CassBorrowedExclusivePtr<CassStatement, CMut>,
                batch_raw: CassBorrowedExclusivePtr<CassBatch, CMut>,
            ) {
                // Set statement/batch exec profile.
                unsafe {
                    assert_cass_error_eq(
                        cass_statement_set_execution_profile(statement_raw, name),
                        CassError::CASS_OK,
                    );
                    assert_cass_error_eq(
                        cass_batch_set_execution_profile(batch_raw, name),
                        CassError::CASS_OK,
                    );
                }
            }
            unsafe fn set_exec_profile(
                profile_name_c_str: *const c_char,
                statement_raw: CassBorrowedExclusivePtr<CassStatement, CMut>,
                batch_raw: CassBorrowedExclusivePtr<CassBatch, CMut>,
            ) {
                unsafe { set_provided_exec_profile(profile_name_c_str, statement_raw, batch_raw) };
            }
            unsafe fn unset_exec_profile(
                statement_raw: CassBorrowedExclusivePtr<CassStatement, CMut>,
                batch_raw: CassBorrowedExclusivePtr<CassBatch, CMut>,
            ) {
                unsafe {
                    set_provided_exec_profile(std::ptr::null::<i8>(), statement_raw, batch_raw)
                };
            }
            unsafe fn set_retry_policy_on_stmt(
                policy: CassBorrowedSharedPtr<CassRetryPolicy, CMut>,
                statement_raw: CassBorrowedExclusivePtr<CassStatement, CMut>,
                batch_raw: CassBorrowedExclusivePtr<CassBatch, CMut>,
            ) {
                unsafe {
                    assert_cass_error_eq(
                        cass_statement_set_retry_policy(statement_raw, policy.borrow()),
                        CassError::CASS_OK,
                    );
                    assert_cass_error_eq(
                        cass_batch_set_retry_policy(batch_raw, policy),
                        CassError::CASS_OK,
                    );
                }
            }
            unsafe fn unset_retry_policy_on_stmt(
                statement_raw: CassBorrowedExclusivePtr<CassStatement, CMut>,
                batch_raw: CassBorrowedExclusivePtr<CassBatch, CMut>,
            ) {
                unsafe { set_retry_policy_on_stmt(ArcFFI::null(), statement_raw, batch_raw) };
            }

            // ### START TESTING

            // With no exec profile nor retry policy set on statement/batch,
            // the default cluster-wide retry policy should be used: in this case, fallthrough.

            // F - -
            assert_query_with_fallthrough_policy(
                &mut proxy,
                session_raw.borrow(),
                statement_raw.borrow().into_c_const(),
                batch_raw.borrow().into_c_const(),
            );

            // F D -
            set_exec_profile(
                profile_name_c_str,
                statement_raw.borrow_mut(),
                batch_raw.borrow_mut(),
            );
            assert_query_with_default_policy(
                &mut proxy,
                session_raw.borrow(),
                statement_raw.borrow().into_c_const(),
                batch_raw.borrow().into_c_const(),
            );

            // F - -
            unset_exec_profile(statement_raw.borrow_mut(), batch_raw.borrow_mut());
            assert_query_with_fallthrough_policy(
                &mut proxy,
                session_raw.borrow(),
                statement_raw.borrow().into_c_const(),
                batch_raw.borrow().into_c_const(),
            );

            // F - F
            set_retry_policy_on_stmt(
                fallthrough_policy.borrow(),
                statement_raw.borrow_mut(),
                batch_raw.borrow_mut(),
            );
            assert_query_with_fallthrough_policy(
                &mut proxy,
                session_raw.borrow(),
                statement_raw.borrow().into_c_const(),
                batch_raw.borrow().into_c_const(),
            );

            // F D F
            set_exec_profile(
                profile_name_c_str,
                statement_raw.borrow_mut(),
                batch_raw.borrow_mut(),
            );
            assert_query_with_fallthrough_policy(
                &mut proxy,
                session_raw.borrow(),
                statement_raw.borrow().into_c_const(),
                batch_raw.borrow().into_c_const(),
            );

            // F D -
            unset_retry_policy_on_stmt(statement_raw.borrow_mut(), batch_raw.borrow_mut());
            assert_query_with_default_policy(
                &mut proxy,
                session_raw.borrow(),
                statement_raw.borrow().into_c_const(),
                batch_raw.borrow().into_c_const(),
            );

            // F D F
            set_retry_policy_on_stmt(
                fallthrough_policy.borrow(),
                statement_raw.borrow_mut(),
                batch_raw.borrow_mut(),
            );
            assert_query_with_fallthrough_policy(
                &mut proxy,
                session_raw.borrow(),
                statement_raw.borrow().into_c_const(),
                batch_raw.borrow().into_c_const(),
            );

            // F D D
            set_retry_policy_on_stmt(
                default_policy.borrow(),
                statement_raw.borrow_mut(),
                batch_raw.borrow_mut(),
            );
            assert_query_with_default_policy(
                &mut proxy,
                session_raw.borrow(),
                statement_raw.borrow().into_c_const(),
                batch_raw.borrow().into_c_const(),
            );

            // F D F
            set_retry_policy_on_stmt(
                fallthrough_policy.borrow(),
                statement_raw.borrow_mut(),
                batch_raw.borrow_mut(),
            );
            assert_query_with_fallthrough_policy(
                &mut proxy,
                session_raw.borrow(),
                statement_raw.borrow().into_c_const(),
                batch_raw.borrow().into_c_const(),
            );

            // F - F
            unset_exec_profile(statement_raw.borrow_mut(), batch_raw.borrow_mut());
            assert_query_with_fallthrough_policy(
                &mut proxy,
                session_raw.borrow(),
                statement_raw.borrow().into_c_const(),
                batch_raw.borrow().into_c_const(),
            );

            // F - -
            unset_retry_policy_on_stmt(statement_raw.borrow_mut(), batch_raw.borrow_mut());
            assert_query_with_fallthrough_policy(
                &mut proxy,
                session_raw.borrow(),
                statement_raw.borrow().into_c_const(),
                batch_raw.borrow().into_c_const(),
            );

            // F - D
            set_retry_policy_on_stmt(
                default_policy.borrow(),
                statement_raw.borrow_mut(),
                batch_raw.borrow_mut(),
            );
            assert_query_with_default_policy(
                &mut proxy,
                session_raw.borrow(),
                statement_raw.borrow().into_c_const(),
                batch_raw.borrow().into_c_const(),
            );

            // F D D
            set_exec_profile(
                profile_name_c_str,
                statement_raw.borrow_mut(),
                batch_raw.borrow_mut(),
            );
            assert_query_with_default_policy(
                &mut proxy,
                session_raw.borrow(),
                statement_raw.borrow().into_c_const(),
                batch_raw.borrow().into_c_const(),
            );

            // F D -
            unset_retry_policy_on_stmt(statement_raw.borrow_mut(), batch_raw.borrow_mut());
            assert_query_with_default_policy(
                &mut proxy,
                session_raw.borrow(),
                statement_raw.borrow().into_c_const(),
                batch_raw.borrow().into_c_const(),
            );
        }

        cass_future_wait_check_and_free(cass_session_close(session_raw.borrow()));
        cass_execution_profile_free(profile_raw);
        cass_statement_free(statement_raw);
        cass_batch_free(batch_raw);
        cass_session_free(session_raw);
        cass_cluster_free(cluster_raw);
    }

    proxy
}

#[test]
#[ntest::timeout(5000)]
fn session_with_latency_aware_load_balancing_does_not_panic() {
    unsafe {
        let mut cluster_raw = cass_cluster_new();

        // An IP with very little chance of having a ScyllaDB node listening
        let ip = "127.0.1.231";
        let (c_ip, c_ip_len) = str_to_c_str_n(ip);

        assert_cass_error_eq(
            cass_cluster_set_contact_points_n(cluster_raw.borrow_mut(), c_ip, c_ip_len),
            CassError::CASS_OK,
        );
        cass_cluster_set_latency_aware_routing(cluster_raw.borrow_mut(), true as cass_bool_t);
        let session_raw = cass_session_new();
        let mut profile_raw = cass_execution_profile_new();
        assert_cass_error_eq(
            cass_execution_profile_set_latency_aware_routing(
                profile_raw.borrow_mut(),
                true as cass_bool_t,
            ),
            CassError::CASS_OK,
        );
        let profile_name = make_c_str!("latency_aware");
        cass_cluster_set_execution_profile(
            cluster_raw.borrow_mut(),
            profile_name,
            profile_raw.borrow_mut(),
        );
        {
            let cass_future =
                cass_session_connect(session_raw.borrow(), cluster_raw.borrow().into_c_const());
            cass_future_wait(cass_future.borrow());
            // The exact outcome is not important, we only test that we don't panic.
        }
        cass_execution_profile_free(profile_raw);
        cass_session_free(session_raw);
        cass_cluster_free(cluster_raw);
    }
}

rusty_fork_test! {
    #![rusty_fork(timeout_ms = 1000)]
    #[test]
    fn cluster_is_not_referenced_by_session_connect_future() {
        // An IP with very little chance of having a ScyllaDB node listening
        let ip = "127.0.1.231";
        let (c_ip, c_ip_len) = str_to_c_str_n(ip);
        let profile_name = make_c_str!("latency_aware");

        unsafe {
            let mut cluster_raw = cass_cluster_new();

            assert_cass_error_eq(
                cass_cluster_set_contact_points_n(cluster_raw.borrow_mut(), c_ip, c_ip_len),
                CassError::CASS_OK
            );
            cass_cluster_set_latency_aware_routing(cluster_raw.borrow_mut(), true as cass_bool_t);
            let session_raw = cass_session_new();
            let mut profile_raw = cass_execution_profile_new();
            assert_cass_error_eq(
                cass_execution_profile_set_latency_aware_routing(profile_raw.borrow_mut(), true as cass_bool_t),
                CassError::CASS_OK
            );
            cass_cluster_set_execution_profile(cluster_raw.borrow_mut(), profile_name, profile_raw.borrow_mut());
            {
                let cass_future = cass_session_connect(session_raw.borrow(), cluster_raw.borrow().into_c_const());

                // This checks that we don't use-after-free the cluster inside the future.
                cass_cluster_free(cluster_raw);

                cass_future_wait(cass_future.borrow());
                // The exact outcome is not important, we only test that we don't segfault.
            }
            cass_execution_profile_free(profile_raw);
            cass_session_free(session_raw);
        }
    }
}

#[tokio::test]
#[ntest::timeout(5000)]
async fn test_cass_session_get_client_id_on_disconnected_session() {
    setup_tracing();
    let res = test_with_3_node_dry_mode_cluster(
        mock_init_rules,
        |proxy_uris: [String; 3], proxy: RunningProxy| {
            unsafe {
                let session_raw = cass_session_new();

                // Check that we can get a client ID from a disconnected session.
                let _random_client_id = cass_session_get_client_id(session_raw.borrow());

                let mut cluster_raw = cass_cluster_new();
                let contact_points = proxy_uris_to_contact_points(proxy_uris);
                assert_cass_error_eq(
                    cass_cluster_set_contact_points(
                        cluster_raw.borrow_mut(),
                        contact_points.as_ptr(),
                    ),
                    CassError::CASS_OK,
                );

                let cluster_client_id = CassUuid {
                    time_and_version: 2137,
                    clock_seq_and_node: 7312,
                };
                cass_cluster_set_client_id(cluster_raw.borrow_mut(), cluster_client_id);

                let connect_fut =
                    cass_session_connect(session_raw.borrow(), cluster_raw.borrow().into_c_const());
                assert_cass_error_eq(cass_future_error_code(connect_fut), CassError::CASS_OK);

                // Verify that the session inherits the client ID from the cluster.
                let session_client_id = cass_session_get_client_id(session_raw.borrow());
                assert_eq!(session_client_id, cluster_client_id);

                // Verify that we can still get a client ID after disconnecting.
                let session_client_id = cass_session_get_client_id(session_raw.borrow());
                assert_eq!(session_client_id, cluster_client_id);

                cass_session_free(session_raw);
                cass_cluster_free(cluster_raw);
            }

            proxy
        },
    )
    .with_current_subscriber()
    .await;

    match res {
        Ok(()) => (),
        Err(ProxyError::Worker(WorkerError::DriverDisconnected(_))) => (),
        Err(err) => panic!("{}", err),
    }
}

#[tokio::test]
#[ntest::timeout(50000)]
async fn session_free_waits_for_requests_to_complete() {
    setup_tracing();
    let res = test_with_3_node_dry_mode_cluster(
        mock_init_rules,
        session_free_waits_for_requests_to_complete_do,
    )
    .with_current_subscriber()
    .await;

    match res {
        Ok(()) => (),
        Err(ProxyError::Worker(WorkerError::DriverDisconnected(_))) => (),
        Err(err) => panic!("{}", err),
    }
}

fn session_free_waits_for_requests_to_complete_do(
    proxy_uris: [String; 3],
    proxy: RunningProxy,
) -> RunningProxy {
    unsafe {
        let mut cluster_raw = cass_cluster_new();
        let contact_points = proxy_uris_to_contact_points(proxy_uris);

        assert_cass_error_eq(
            cass_cluster_set_contact_points(cluster_raw.borrow_mut(), contact_points.as_ptr()),
            CassError::CASS_OK,
        );
        let session_raw = cass_session_new();
        cass_future_wait_check_and_free(cass_session_connect(
            session_raw.borrow(),
            cluster_raw.borrow().into_c_const(),
        ));

        tracing::debug!("Session connected, starting to execute requests...");

        let statement =
            c"SELECT host_id FROM system.local WHERE key='local'" as *const CStr as *const c_char;
        let statement_raw = cass_statement_new(statement, 0);

        let mut batch_raw = cass_batch_new(CassBatchType::CASS_BATCH_TYPE_LOGGED);
        // This batch is obviously invalid, because it contains a SELECT statement. This is OK for us,
        // because we anyway expect the batch to fail. The goal is to have the future set, no matter if it's
        // set with a success or an error.
        cass_batch_add_statement(batch_raw.borrow_mut(), statement_raw.borrow());

        let finished_executions = AtomicUsize::new(0);
        unsafe extern "C" fn finished_execution_callback(
            _future_raw: CassBorrowedSharedPtr<CassFuture, CMut>,
            data: *mut c_void,
        ) {
            let finished_executions = unsafe { &*(data as *const AtomicUsize) };
            finished_executions.fetch_add(1, Ordering::SeqCst);
        }

        const ITERATIONS: usize = 1;
        const EXECUTIONS: usize = 3 * ITERATIONS; // One prepare, one statement and one batch per iteration.

        let futures = (0..ITERATIONS)
            .flat_map(|_| {
                // Prepare a statement
                let prepare_fut = cass_session_prepare(session_raw.borrow(), statement);

                // Execute a statement
                let statement_fut = cass_session_execute(
                    session_raw.borrow(),
                    statement_raw.borrow().into_c_const(),
                );

                // Execute a batch
                let batch_fut = cass_session_execute_batch(
                    session_raw.borrow(),
                    batch_raw.borrow().into_c_const(),
                );
                for fut in [
                    prepare_fut.borrow(),
                    statement_fut.borrow(),
                    batch_fut.borrow(),
                ] {
                    cass_future_set_callback(
                        fut,
                        Some(finished_execution_callback),
                        std::ptr::addr_of!(finished_executions) as _,
                    );
                }

                [prepare_fut, statement_fut, batch_fut]
            })
            .collect::<Vec<_>>();

        tracing::debug!("Started all requests. Now, freeing statements and session...");

        // Free the statement
        cass_statement_free(statement_raw);
        // Free the batch
        cass_batch_free(batch_raw);

        // Session is freed, but the requests may still be in-flight.
        cass_session_free(session_raw);

        tracing::debug!("Session freed.");

        // Assert that the session awaited completion of all requests.
        let actually_finished_executions = finished_executions.load(Ordering::SeqCst);
        assert_eq!(
            actually_finished_executions, EXECUTIONS,
            "Expected {} requests to complete before the session was freed, but only {} did.",
            EXECUTIONS, actually_finished_executions
        );

        futures.into_iter().for_each(|fut| {
            // As per cassandra.h, "a future can be freed anytime".
            cass_future_free(fut);
        });

        cass_cluster_free(cluster_raw);
    }

    proxy
}
