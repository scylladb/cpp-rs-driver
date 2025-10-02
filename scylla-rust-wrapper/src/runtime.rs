//! Manages tokio runtimes for the application.
//!
//! Runtime is per-cluster and can be changed with `cass_cluster_set_num_threads_io`.

use std::{
    collections::HashMap,
    mem::ManuallyDrop,
    sync::{Arc, Weak},
};

/// This is a wrapper to ensure that the runtime is properly shut down even
/// if dropped in the async context. This is done by using a custom
/// Drop implementation that calls `shutdown_background`, as opposed
/// to the default `Drop` implementation of `Runtime` that calls
/// `shutdown` which panics if called from the async context.
#[repr(transparent)]
pub(crate) struct Runtime {
    inner: ManuallyDrop<tokio::runtime::Runtime>,
}

impl From<tokio::runtime::Runtime> for Runtime {
    fn from(inner: tokio::runtime::Runtime) -> Self {
        Self {
            inner: ManuallyDrop::new(inner),
        }
    }
}

impl std::ops::Deref for Runtime {
    type Target = tokio::runtime::Runtime;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        // SAFETY: We are executing the drop logic only once, here.
        // Drop::drop is guaranteed to be called only once, and we are taking
        // the inner value out of the ManuallyDrop exactly once here.
        let runtime = unsafe { ManuallyDrop::take(&mut self.inner) };

        // We use `shutdown_background()` instead of the usual Runtime Drop impl to avoid panicking
        // if the runtime is dropped in an async context.
        runtime.shutdown_background();
    }
}

/// Manages tokio runtimes for the application.
///
/// Runtime is per-cluster and can be changed with `cass_cluster_set_num_threads_io`.
/// Once a runtime is created, it is cached for future use.
/// Once all `CassSession` instances that reference the runtime are dropped,
/// the runtime is also dropped.
pub(crate) struct Runtimes {
    // Weak pointers are used to make runtimes dropped once all `CassSession` instances
    // that reference them are freed.
    default_runtime: Option<Weak<Runtime>>,
    // This is Option to allow creating a static instance of Runtimes.
    // (`HashMap::new` is not `const`).
    n_thread_runtimes: Option<HashMap<usize, Weak<Runtime>>>,
}

pub(crate) static RUNTIMES: std::sync::Mutex<Runtimes> = {
    std::sync::Mutex::new(Runtimes {
        default_runtime: None,
        n_thread_runtimes: None,
    })
};

impl Runtimes {
    fn cached_or_new_runtime(
        weak_runtime: &mut Weak<Runtime>,
        create_runtime: impl FnOnce() -> Result<Arc<Runtime>, std::io::Error>,
    ) -> Result<Arc<Runtime>, std::io::Error> {
        match weak_runtime.upgrade() {
            Some(cached_runtime) => Ok(cached_runtime),
            None => {
                let runtime = create_runtime()?;
                *weak_runtime = Arc::downgrade(&runtime);
                Ok(runtime)
            }
        }
    }

    /// Returns a default tokio runtime.
    ///
    /// If it's not created yet, it will create a new one with the default configuration
    /// and cache it for future use.
    pub(crate) fn default_runtime(&mut self) -> Result<Arc<Runtime>, std::io::Error> {
        let default_runtime_slot = self.default_runtime.get_or_insert_with(Weak::new);
        Self::cached_or_new_runtime(default_runtime_slot, || {
            tokio::runtime::Runtime::new()
                .map(Runtime::from)
                .map(Arc::new)
        })
    }

    /// Returns a tokio runtime with `n_threads` worker threads.
    ///
    /// If it's not created yet, it will create a new one and cache it for future use.
    pub(crate) fn n_thread_runtime(
        &mut self,
        n_threads: usize,
    ) -> Result<Arc<Runtime>, std::io::Error> {
        let n_thread_runtimes = self.n_thread_runtimes.get_or_insert_with(HashMap::new);
        let n_thread_runtime_slot = n_thread_runtimes.entry(n_threads).or_default();

        Self::cached_or_new_runtime(n_thread_runtime_slot, || {
            match n_threads {
                0 => tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build(),
                n => tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(n)
                    .enable_all()
                    .build(),
            }
            .map(Runtime::from)
            .map(Arc::new)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, process::Command};
    use tracing::instrument::WithSubscriber as _;

    use scylla_proxy::RunningProxy;

    use crate::{
        argconv::str_to_c_str_n,
        cass_error::CassError,
        cluster::{cass_cluster_free, cass_cluster_new, cass_cluster_set_contact_points_n},
        future::cass_future_free,
        session::{cass_session_close, cass_session_connect, cass_session_free, cass_session_new},
        testing::utils::{
            assert_cass_error_eq, cass_future_wait_check_and_free, mock_init_rules,
            rusty_fork_test_with_proxy, setup_tracing, test_with_one_proxy_at_ip,
        },
    };

    rusty_fork_test_with_proxy! {
        #![rusty_fork(timeout_ms = 20000)]
        #[test]
        /// This test aims to verify that the Tokio runtime used by the driver is never dropped
        /// from the async context. Violation of this property results in a panic.
        ///
        /// The property could be violated for example if the last `Arc` reference to the runtime
        /// was held by the tokio task that executes the future wrapped by `CassFuture`. Then,
        /// dropping the future would drop the runtime, which is forbidden by Tokio as dropping
        /// the runtime is a blocking operation.
        ///
        /// The test fails without the fix and passes with it.
        /// It is run with rusty_fork in order to have a fresh process, so that we
        /// can set a custom panic hook that aborts the process instead of
        /// unwinding. This makes catching panics of tokio worker threads reliable.
        ///
        /// Tokio worker threads panic if the runtime is dropped from an async
        /// context, which is made more likely by the test's two properties:
        /// 1. all shared owners of the `Runtime` that are held by the main thread
        ///    are dropped immediately, which makes more likely that the last owner
        ///    is held by a tokio worker thread which executes the future.
        /// 2. the case is repeated 100 times in a loop, which increases the
        ///    likelihood of the runtime being dropped from an async context.
        ///    Without the loop, the test could sometimes pass even without the fix.
        fn runtime_is_never_dropped_in_async_context(ip: IpAddr) {
            setup_tracing();

            std::panic::set_hook(Box::new(|panic_info| {
                let payload = panic_info.payload();
                // Support both &str and String panic payloads.
                let payload_str = payload
                    .downcast_ref::<&str>().copied().or_else(|| payload.downcast_ref::<String>().map(String::as_str));
                payload_str.inspect(|s| eprintln!("Panic occurred: {s}"));
                std::process::abort();
            }));

            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            runtime.block_on(test_with_one_proxy_at_ip(
                runtime_is_never_dropped_in_async_context_do,
                mock_init_rules(),
                ip,
            )
            .with_current_subscriber());
        }
    }

    fn runtime_is_never_dropped_in_async_context_do(
        node_addr: SocketAddr,
        proxy: RunningProxy,
    ) -> RunningProxy {
        for _ in 0..100 {
            unsafe {
                let ip = node_addr.ip().to_string();
                let mut cluster_raw = cass_cluster_new();
                let (c_ip, c_ip_len) = str_to_c_str_n(ip.as_str());

                assert_cass_error_eq!(
                    cass_cluster_set_contact_points_n(cluster_raw.borrow_mut(), c_ip, c_ip_len),
                    CassError::CASS_OK
                );
                let session_raw = cass_session_new();
                cass_future_wait_check_and_free(cass_session_connect(
                    session_raw.borrow(),
                    cluster_raw.borrow().into_c_const(),
                ));

                let session_close_fut = cass_session_close(session_raw.borrow());
                // Intentionally not waiting to increase likelihood of the tokio task dropping the runtime.
                // cass_future_wait(session_close_fut.borrow());
                cass_future_free(session_close_fut);

                cass_cluster_free(cluster_raw);
                cass_session_free(session_raw);
            }
        }
        proxy
    }
}
