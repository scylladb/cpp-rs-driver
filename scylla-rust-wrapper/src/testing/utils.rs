use scylla_proxy::{
    Condition, DoorkeeperError, Node, Proxy, Reaction as _, RequestFrame, RequestOpcode,
    RequestReaction, RequestRule, ResponseFrame, RunningProxy,
};
use std::{
    collections::HashMap,
    ffi::c_char,
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

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

pub(crate) async fn test_with_one_proxy_at_ip(
    test: impl FnOnce(SocketAddr, RunningProxy) -> RunningProxy + Send + 'static,
    rules: impl IntoIterator<Item = RequestRule>,
    ip: IpAddr,
) {
    let proxy_addr = SocketAddr::new(ip, 9042);

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

/// Maximum number of attempts to start proxy with different addresses.
/// This handles the case where addresses returned by `get_exclusive_local_address()`
/// are already in use when tests run in parallel.
const MAX_PROXY_BIND_ATTEMPTS: usize = 10;

/// Checks if a DoorkeeperError is caused by an address already being in use.
fn is_addr_in_use_error(error: &DoorkeeperError) -> bool {
    let io_error = match error {
        DoorkeeperError::Listen(_, e)
        | DoorkeeperError::DriverConnectionAttempt(_, e)
        | DoorkeeperError::NodeConnectionAttempt(_, e)
        | DoorkeeperError::SocketCreate(e)
        | DoorkeeperError::SocketBind(e)
        | DoorkeeperError::ObtainingShardNumber(e) => e,
        _ => return false,
    };

    io_error.kind() == std::io::ErrorKind::AddrInUse
}

pub(crate) async fn test_with_one_proxy(
    test: impl FnOnce(SocketAddr, RunningProxy) -> RunningProxy + Send + 'static,
    rules: impl IntoIterator<Item = RequestRule>,
) {
    let rules: Vec<_> = rules.into_iter().collect();

    for attempt in 0..MAX_PROXY_BIND_ATTEMPTS {
        let ip = scylla_proxy::get_exclusive_local_address();
        let proxy_addr = SocketAddr::new(ip, 9042);

        let proxy_result = Proxy::builder()
            .with_node(
                Node::builder()
                    .proxy_address(proxy_addr)
                    .request_rules(rules.clone())
                    .build_dry_mode(),
            )
            .build()
            .run()
            .await;

        match proxy_result {
            Ok(proxy) => {
                // This is required to avoid the clash of a runtime built inside another runtime
                // (the test runs one runtime to drive the proxy, and CassFuture implementation uses another)
                let proxy = tokio::task::spawn_blocking(move || test(proxy_addr, proxy))
                    .await
                    .expect("Test thread panicked");

                let _ = proxy.finish().await;
                return;
            }
            Err(e) => {
                if is_addr_in_use_error(&e) && attempt < MAX_PROXY_BIND_ATTEMPTS - 1 {
                    eprintln!(
                        "Proxy bind attempt {} failed for {}: {}. Retrying with new address...",
                        attempt + 1,
                        proxy_addr,
                        e
                    );
                    continue;
                }

                panic!(
                    "Failed to start proxy after {} attempts. Last error at {}: {}",
                    attempt + 1,
                    proxy_addr,
                    e
                );
            }
        }
    }
}

/// Run a Rust test in a subprocess, setting up the proxy
/// with a correct unique address.
///
/// This is intended to coordinate in address allocation between
/// the parent process and the subprocesses, so that no address
/// collisions occur when tests are run in multiple processes.
///
/// The basic usage is to simply put this macro around your `#[test]`
/// function and add one argument of type `IpAddr` to it, like so:
///
/// ```no_run
/// use crate::testing::rusty_fork_test_with_proxy;
///
/// rusty_fork_test_with_proxy! {
///     #[test]
///     fn my_test(ip: IpAddr) {
///         // Use the `ip` variable to set up the proxy.
///         let fut = test_with_one_proxy_at_ip(
///             test_body,
///             proxy_rules,
///             ip,
///         );
///
///         // Run the future to completion.
///     }
/// }
/// ```
///
/// The test will be run in its own process. If the subprocess exits
/// unsuccessfully for any reason, including due to signals, the test fails.
///
/// It is also possible to specify a timeout which is applied to the test in
/// the block, by adding the following line straight before the test:
///
/// ```no_run
/// #![rusty_fork(timeout_ms = 1000)]
/// ```
///
/// For more details, see the documentation of the `rusty_fork_test` macro
/// in the [rusty_fork] crate.
/// ```
macro_rules! rusty_fork_test_with_proxy {
    (#![rusty_fork(timeout_ms = $timeout:expr)]
         $(#[$meta:meta])*
         fn $test_name:ident($ip:ident: IpAddr) $body:block
    ) => {
        $(#[$meta])*
        fn $test_name() {
            // Eagerly convert everything to function pointers so that all
            // tests use the same instantiation of `fork`.
            let supervise_fn = |child: &mut rusty_fork::ChildWrapper, _file: &mut std::fs::File| {
                rusty_fork::fork_test::supervise_child(child, $timeout);
            };

            const ENV_KEY: &str = "PROXY_RUSTY_FORK_IP";

            let process_modifier = |child: &mut Command| {
                child.env(
                    ENV_KEY,
                    scylla_proxy::get_exclusive_local_address().to_string(),
                );
            };

            let test = || {
                // Body expects the ip variable in scope.
                let $ip: ::std::net::IpAddr = std::env::var(ENV_KEY)
                    .expect("Parent should have set the proxy address as an env variable.")
                    .parse()
                    .expect("Error parsing the proxy address from env variable.");

                $body
            };

            rusty_fork::fork(
                rusty_fork::rusty_fork_test_name!($test_name),
                rusty_fork::rusty_fork_id!(),
                process_modifier,
                supervise_fn,
                test,
            )
            .expect("forking test failed")
        }
    };

    (
         $(#[$meta:meta])*
         fn $test_name:ident() $body:block
    ) => {
        rusty_fork_test! {
            #![rusty_fork(timeout_ms = 0)]

            $(#[$meta])* fn $test_name() $body
        }
    };
}
pub(crate) use rusty_fork_test_with_proxy;
