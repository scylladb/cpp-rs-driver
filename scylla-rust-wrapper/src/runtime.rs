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
