use scylla_proxy::{
    Condition, ProxyError, Reaction as _, RequestOpcode, RequestReaction, RequestRule,
    RunningProxy, WorkerError,
};
use scylladb::api::batch::{
    CassBatchType, cass_batch_add_statement, cass_batch_free, cass_batch_new,
    cass_batch_set_execution_profile,
};
use scylladb::api::cluster::{
    cass_cluster_free, cass_cluster_new, cass_cluster_set_contact_points,
    cass_cluster_set_execution_profile,
};
use scylladb::api::error::CassError;
use scylladb::api::execution_profile::{
    cass_execution_profile_free, cass_execution_profile_new,
    cass_execution_profile_set_request_timeout,
};
use scylladb::api::future::{cass_future_error_code, cass_future_free};
use scylladb::api::session::{
    cass_session_close, cass_session_connect, cass_session_execute, cass_session_execute_batch,
    cass_session_free, cass_session_new,
};
use scylladb::api::statement::{
    cass_statement_free, cass_statement_new, cass_statement_set_execution_profile,
};
use scylladb::argconv::CassStrNulTerminated;

use crate::utils::{
    assert_cass_error_eq, cass_future_wait_check_and_free, drop_metadata_queries_rules,
    handshake_rules, make_c_str, proxy_uris_to_contact_points, setup_tracing,
    test_with_3_node_dry_mode_cluster,
};

const SHORT_TIMEOUT_MS: u64 = 1;

/// Rules used only to bring the session up: handshake succeeds, and any query/prepare/batch
/// issued during connection setup (e.g. metadata fetch) is answered with a quick server error,
/// which the driver tolerates by falling back to dummy metadata.
fn setup_rules() -> impl IntoIterator<Item = RequestRule> {
    handshake_rules()
        .into_iter()
        .chain(drop_metadata_queries_rules())
}

/// Replies immediately with a server error - used to prove that, absent a short request
/// timeout, a statement/batch does NOT fail with a client-side timeout.
fn quick_reply_rule() -> RequestRule {
    RequestRule(
        Condition::any([
            Condition::RequestOpcode(RequestOpcode::Query),
            Condition::RequestOpcode(RequestOpcode::Execute),
            Condition::RequestOpcode(RequestOpcode::Batch),
        ])
        .and(Condition::not(Condition::ConnectionRegisteredAnyEvent)),
        RequestReaction::forge().server_error(),
    )
}

/// Drops the request frame entirely, so no response ever arrives - used to deterministically
/// trigger the driver's request timeout, without racing a real sleep against it.
fn drop_frame_rule() -> RequestRule {
    RequestRule(
        Condition::any([
            Condition::RequestOpcode(RequestOpcode::Query),
            Condition::RequestOpcode(RequestOpcode::Execute),
            Condition::RequestOpcode(RequestOpcode::Batch),
        ])
        .and(Condition::not(Condition::ConnectionRegisteredAnyEvent)),
        RequestReaction::drop_frame(),
    )
}

fn set_rule_on_all_nodes(proxy: &mut RunningProxy, rule: RequestRule) {
    proxy
        .running_nodes
        .iter_mut()
        .for_each(|node| node.change_request_rules(Some(vec![rule.clone()])));
}

// Port of ExecutionProfileTest::RequestTimeout (cpp-driver's test_exec_profile.cpp).
//
// The original C++ test used a SleepingHistoryListener to artificially delay statement
// execution, racing a real thread sleep against the configured timeout - this was flaky.
// Here we instead withhold the server response entirely via scylla-proxy's `drop_frame`
// reaction (in dry mode, so no real cluster is needed), which deterministically triggers
// the driver's request timeout, since no response will ever arrive.
#[tokio::test]
#[ntest::timeout(30000)]
async fn request_timeout_on_execution_profile_is_honored() {
    setup_tracing();

    let res = test_with_3_node_dry_mode_cluster(setup_rules, request_timeout_do).await;

    match res {
        Ok(()) => (),
        Err(ProxyError::Worker(WorkerError::DriverDisconnected(_))) => (),
        Err(err) => panic!("{}", err),
    }
}

fn request_timeout_do(proxy_uris: [String; 3], mut proxy: RunningProxy) -> RunningProxy {
    unsafe {
        let mut cluster_raw = cass_cluster_new();
        let contact_points = proxy_uris_to_contact_points(proxy_uris);
        assert_cass_error_eq(
            cass_cluster_set_contact_points(
                cluster_raw.borrow_mut(),
                CassStrNulTerminated::from_cstr(&contact_points),
            ),
            CassError::CASS_OK,
        );

        let mut exec_profile_raw = cass_execution_profile_new();
        let profile_name = make_c_str!("request_timeout");
        assert_cass_error_eq(
            cass_execution_profile_set_request_timeout(
                exec_profile_raw.borrow_mut(),
                SHORT_TIMEOUT_MS,
            ),
            CassError::CASS_OK,
        );
        assert_cass_error_eq(
            cass_cluster_set_execution_profile(
                cluster_raw.borrow_mut(),
                CassStrNulTerminated::from_raw(profile_name),
                exec_profile_raw.borrow_mut(),
            ),
            CassError::CASS_OK,
        );

        let session_raw = cass_session_new();
        cass_future_wait_check_and_free(cass_session_connect(
            session_raw.borrow(),
            cluster_raw.borrow().into_c_const(),
        ));

        let query = make_c_str!("SELECT host_id FROM system.local WHERE key='local'");

        // Case 1: a statement without the "request_timeout" profile should not time out
        // client-side; it should get a (quick) server error instead.
        {
            set_rule_on_all_nodes(&mut proxy, quick_reply_rule());

            let statement_raw = cass_statement_new(CassStrNulTerminated::from_raw(query), 0);
            let fut =
                cass_session_execute(session_raw.borrow(), statement_raw.borrow().into_c_const());
            assert_cass_error_eq(
                cass_future_error_code(fut.borrow()),
                CassError::CASS_ERROR_SERVER_SERVER_ERROR,
            );
            cass_future_free(fut);
            cass_statement_free(statement_raw);
        }

        // Case 2: a statement using the "request_timeout" profile should time out,
        // since no response will ever arrive.
        {
            set_rule_on_all_nodes(&mut proxy, drop_frame_rule());

            let mut statement_raw = cass_statement_new(CassStrNulTerminated::from_raw(query), 0);
            assert_cass_error_eq(
                cass_statement_set_execution_profile(
                    statement_raw.borrow_mut(),
                    CassStrNulTerminated::from_raw(profile_name),
                ),
                CassError::CASS_OK,
            );

            let fut =
                cass_session_execute(session_raw.borrow(), statement_raw.borrow().into_c_const());
            assert_cass_error_eq(
                cass_future_error_code(fut.borrow()),
                CassError::CASS_ERROR_LIB_REQUEST_TIMED_OUT,
            );
            cass_future_free(fut);
            cass_statement_free(statement_raw);
        }

        // Case 3: a batch using the "request_timeout" profile should also time out.
        {
            let statement_raw = cass_statement_new(CassStrNulTerminated::from_raw(query), 0);
            let mut batch_raw = cass_batch_new(CassBatchType::CASS_BATCH_TYPE_LOGGED);
            assert_cass_error_eq(
                cass_batch_add_statement(batch_raw.borrow_mut(), statement_raw.borrow()),
                CassError::CASS_OK,
            );
            assert_cass_error_eq(
                cass_batch_set_execution_profile(
                    batch_raw.borrow_mut(),
                    CassStrNulTerminated::from_raw(profile_name),
                ),
                CassError::CASS_OK,
            );

            let fut =
                cass_session_execute_batch(session_raw.borrow(), batch_raw.borrow().into_c_const());
            assert_cass_error_eq(
                cass_future_error_code(fut.borrow()),
                CassError::CASS_ERROR_LIB_REQUEST_TIMED_OUT,
            );
            cass_future_free(fut);
            cass_batch_free(batch_raw);
            cass_statement_free(statement_raw);
        }

        cass_future_wait_check_and_free(cass_session_close(session_raw.borrow()));
        cass_execution_profile_free(exec_profile_raw);
        cass_session_free(session_raw);
        cass_cluster_free(cluster_raw);
    }

    proxy
}
