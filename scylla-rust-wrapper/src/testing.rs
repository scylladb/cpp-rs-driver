use scylla_proxy::{
    Condition, Node, Proxy, Reaction as _, RequestFrame, RequestOpcode, RequestReaction,
    RequestRule, ResponseFrame, RunningProxy,
};
use std::{collections::HashMap, ffi::c_char, net::SocketAddr, sync::Arc};

use crate::{
    argconv::{CMut, CassOwnedSharedPtr, ptr_to_cstr_n},
    cass_error::CassError,
    future::{
        CassFuture, cass_future_error_code, cass_future_error_message, cass_future_free,
        cass_future_wait,
    },
    types::size_t,
};

pub(crate) fn setup_tracing() {
    let _ = tracing_subscriber::fmt::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(tracing_subscriber::fmt::TestWriter::new())
        .try_init();
}

macro_rules! assert_cass_error_eq {
    ($expr:expr, $error:expr $(,)?) => {{
        use crate::api::error::cass_error_desc;
        use crate::argconv::ptr_to_cstr;
        let ___x = $expr;
        assert_eq!(
            ___x,
            $error,
            "expected \"{}\", instead got \"{}\"",
            ptr_to_cstr(cass_error_desc($error)).unwrap(),
            ptr_to_cstr(cass_error_desc(___x)).unwrap()
        );
    }};
}
pub(crate) use assert_cass_error_eq;

macro_rules! assert_cass_future_error_message_eq {
    ($cass_fut:ident, $error_msg_opt:expr) => {
        use crate::argconv::ptr_to_cstr_n;
        use crate::future::cass_future_error_message;

        let mut ___message: *const c_char = ::std::ptr::null();
        let mut ___msg_len: size_t = 0;
        cass_future_error_message($cass_fut.borrow(), &mut ___message, &mut ___msg_len);
        assert_eq!(ptr_to_cstr_n(___message, ___msg_len), $error_msg_opt);
    };
}
pub(crate) use assert_cass_future_error_message_eq;

pub(crate) unsafe fn cass_future_wait_check_and_free(fut: CassOwnedSharedPtr<CassFuture, CMut>) {
    unsafe { cass_future_wait(fut.borrow()) };
    if unsafe { cass_future_error_code(fut.borrow()) } != CassError::CASS_OK {
        let mut message: *const c_char = std::ptr::null();
        let mut message_len: size_t = 0;
        unsafe { cass_future_error_message(fut.borrow(), &mut message, &mut message_len) };
        eprintln!("{:?}", unsafe { ptr_to_cstr_n(message, message_len) });
    }
    unsafe {
        assert_cass_error_eq!(cass_future_error_code(fut.borrow()), CassError::CASS_OK);
    }
    unsafe { cass_future_free(fut) };
}

/// A set of rules that are needed to negotiate connections.
// All connections are successfully negotiated.
pub(crate) fn handshake_rules() -> impl IntoIterator<Item = RequestRule> {
    [
        RequestRule(
            Condition::RequestOpcode(RequestOpcode::Options),
            RequestReaction::forge_response(Arc::new(move |frame: RequestFrame| {
                ResponseFrame::forged_supported(frame.params, &HashMap::new()).unwrap()
            })),
        ),
        RequestRule(
            Condition::RequestOpcode(RequestOpcode::Startup)
                .or(Condition::RequestOpcode(RequestOpcode::Register)),
            RequestReaction::forge_response(Arc::new(move |frame: RequestFrame| {
                ResponseFrame::forged_ready(frame.params)
            })),
        ),
    ]
}

// As these are very generic, they should be put last in the rules Vec.
pub(crate) fn generic_drop_queries_rules() -> impl IntoIterator<Item = RequestRule> {
    [RequestRule(
        Condition::RequestOpcode(RequestOpcode::Query),
        // We won't respond to any queries (including metadata fetch),
        // but the driver will manage to continue with dummy metadata.
        RequestReaction::forge().server_error(),
    )]
}

/// A set of rules that are needed to finish session initialization.
// They are used in tests that require a session to be connected.
// All connections are successfully negotiated.
// All requests are replied with a server error.
pub(crate) fn mock_init_rules() -> impl IntoIterator<Item = RequestRule> {
    handshake_rules()
        .into_iter()
        .chain(std::iter::once(RequestRule(
            Condition::RequestOpcode(RequestOpcode::Query)
                .or(Condition::RequestOpcode(RequestOpcode::Prepare))
                .or(Condition::RequestOpcode(RequestOpcode::Batch)),
            // We won't respond to any queries (including metadata fetch),
            // but the driver will manage to continue with dummy metadata.
            RequestReaction::forge().server_error(),
        )))
}

pub(crate) async fn test_with_one_proxy(
    test: impl FnOnce(SocketAddr, RunningProxy) -> RunningProxy + Send + 'static,
    rules: impl IntoIterator<Item = RequestRule>,
) {
    let proxy_addr = SocketAddr::new(scylla_proxy::get_exclusive_local_address(), 9042);

    let proxy = Proxy::builder()
        .with_node(
            Node::builder()
                .proxy_address(proxy_addr)
                .request_rules(rules.into_iter().collect())
                .build_dry_mode(),
        )
        .build()
        .run()
        .await
        .unwrap();

    // This is required to avoid the clash of a runtime built inside another runtime
    // (the test runs one runtime to drive the proxy, and CassFuture implementation uses another)
    let proxy = tokio::task::spawn_blocking(move || test(proxy_addr, proxy))
        .await
        .expect("Test thread panicked");

    let _ = proxy.finish().await;
}
