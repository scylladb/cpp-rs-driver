use crate::argconv::*;
use crate::cass_error::{CassError, CassErrorMessage, CassErrorResult, ToCassError as _};
use crate::cql_types::uuid::CassUuid;
use crate::query_result::{CassNode, CassResult};
use crate::runtime::Runtime;
use crate::statements::prepared::CassPrepared;
use crate::types::*;
use futures::future;
use std::future::Future;
use std::mem;
use std::os::raw::c_void;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use tokio::task::JoinHandle;
use tokio::time::Duration;

#[derive(Debug)]
pub(crate) enum CassResultValue {
    Empty,
    QueryResult(Arc<CassResult>),
    QueryError(Arc<CassErrorResult>),
    Prepared(Arc<CassPrepared>),
}

type CassFutureError = (CassError, String);

pub(crate) type CassFutureResult = Result<CassResultValue, CassFutureError>;

pub type CassFutureCallback = Option<NonNullFutureCallback>;

type NonNullFutureCallback =
    unsafe extern "C" fn(future: CassBorrowedSharedPtr<CassFuture, CMut>, data: *mut c_void);

#[derive(Clone, Copy)]
struct BoundCallback {
    cb: NonNullFutureCallback,
    data: *mut c_void,
}

// *mut c_void is not Send, so Rust will have to take our word
// that we won't screw something up
unsafe impl Send for BoundCallback {}

impl BoundCallback {
    fn invoke(self, fut_ptr: CassBorrowedSharedPtr<CassFuture, CMut>) {
        unsafe {
            (self.cb)(fut_ptr, self.data);
        }
    }
}

#[derive(Default)]
struct CassFutureState {
    /// Optional callback to be executed when the future is resolved.
    callback: Option<BoundCallback>,

    /// Join handle of the tokio task that resolves the future.
    join_handle: Option<JoinHandle<()>>,
}

enum FutureKind {
    /// Future that must be resolved by the tokio runtime.
    Resolvable { fut: ResolvableFuture },

    /// Future that is immediately ready with the result.
    Immediate {
        res: CassFutureResult,
        callback_set: AtomicBool,
    },
}

struct ResolvableFuture {
    /// Runtime used to spawn and execute the future.
    runtime: Arc<Runtime>,

    /// Mutable state of the future that requires synchronized exclusive access
    /// in order to ensure thread safety of the future execution.
    state: Mutex<CassFutureState>,

    /// Result of the future once it is resolved.
    result: OnceLock<CassFutureResult>,

    /// Used to notify threads waiting for the future's result.
    wait_for_value: Condvar,

    #[cfg(cpp_integration_testing)]
    recording_listener: Option<Arc<crate::testing::integration::RecordingHistoryListener>>,
}

pub struct CassFuture {
    /// One of the possible implementations of the future.
    kind: FutureKind,

    /// Required as a place to allocate the stringified error message.
    /// This is needed to support `cass_future_error_message`.
    err_string: OnceLock<String>,
}

impl FFI for CassFuture {
    type Origin = FromArc;
}

/// An error that can appear during `cass_future_wait_timed`.
enum FutureError {
    TimeoutError,
    InvalidDuration,
}

/// The timeout appeared when we tried to await `JoinHandle`.
/// This errors contains the original handle, so it can be awaited later again.
struct JoinHandleTimeout(JoinHandle<()>);

impl CassFuture {
    pub(crate) fn make_ready_raw(res: CassFutureResult) -> CassOwnedSharedPtr<CassFuture, CMut> {
        Self::new_ready(res).into_raw()
    }

    pub(crate) fn make_raw(
        runtime: Arc<Runtime>,
        fut: impl Future<Output = CassFutureResult> + Send + 'static,
        #[cfg(cpp_integration_testing)] recording_listener: Option<
            Arc<crate::testing::integration::RecordingHistoryListener>,
        >,
    ) -> CassOwnedSharedPtr<CassFuture, CMut> {
        Self::new_from_future(
            runtime,
            fut,
            #[cfg(cpp_integration_testing)]
            recording_listener,
        )
        .into_raw()
    }

    pub(crate) fn new_from_future(
        runtime: Arc<Runtime>,
        fut: impl Future<Output = CassFutureResult> + Send + 'static,
        #[cfg(cpp_integration_testing)] recording_listener: Option<
            Arc<crate::testing::integration::RecordingHistoryListener>,
        >,
    ) -> Arc<CassFuture> {
        let cass_fut = Arc::new(CassFuture {
            err_string: OnceLock::new(),
            kind: FutureKind::Resolvable {
                fut: ResolvableFuture {
                    runtime: Arc::clone(&runtime),
                    state: Mutex::new(Default::default()),
                    result: OnceLock::new(),
                    wait_for_value: Condvar::new(),
                    #[cfg(cpp_integration_testing)]
                    recording_listener,
                },
            },
        });
        let cass_fut_clone = Arc::clone(&cass_fut);
        let join_handle = runtime.spawn(async move {
            let resolvable_fut = match cass_fut_clone.kind {
                FutureKind::Resolvable {
                    fut: ref resolvable,
                } => resolvable,
                _ => unreachable!("CassFuture has been created as Resolvable"),
            };

            let r = fut.await;
            let maybe_cb = {
                let guard = resolvable_fut.state.lock().unwrap();
                resolvable_fut
                    .result
                    .set(r)
                    .expect("Tried to resolve future result twice!");

                // Get the callback and call it after releasing the lock.
                // Do not take the callback out, as it prevents other callbacks
                // from being set afterwards, which is needed to match CPP Driver's semantics.
                guard.callback
            };
            if let Some(bound_cb) = maybe_cb {
                let fut_ptr = ArcFFI::as_ptr::<CMut>(&cass_fut_clone);
                // Safety: pointer is valid, because we get it from arc allocation.
                bound_cb.invoke(fut_ptr);
            }

            resolvable_fut.wait_for_value.notify_all();
        });
        {
            let resolvable_fut = match cass_fut.kind {
                FutureKind::Resolvable {
                    fut: ref resolvable,
                } => resolvable,
                _ => unreachable!("CassFuture has been created as Resolvable"),
            };
            let mut lock = resolvable_fut.state.lock().unwrap();
            lock.join_handle = Some(join_handle);
        }
        cass_fut
    }

    pub(crate) fn new_ready(res: CassFutureResult) -> Arc<Self> {
        Arc::new(CassFuture {
            kind: FutureKind::Immediate {
                res,
                callback_set: AtomicBool::new(false),
            },
            err_string: OnceLock::new(),
        })
    }

    /// Awaits the future until completion and exposes the result.
    ///
    /// There are three possible cases:
    /// - result is already available -> we can return.
    /// - no one is currently working on the future -> we take the ownership
    ///   of JoinHandle (future) and poll it until completion.
    /// - some other thread is working on the future -> we wait on the condition
    ///   variable to get an access to the future's state. Once we are notified,
    ///   there are three cases:
    ///     - result is already available -> we can return.
    ///     - JoinHandle is consumed -> some other thread already resolved the future.
    ///       We can return.
    ///     - JoinHandle is Some -> some other thread was working on the future, but
    ///       timed out (see [CassFuture::waited_result_timed]). We need to
    ///       take the ownership of the handle, and complete the work.
    pub(crate) fn waited_result(&self) -> &CassFutureResult {
        let resolvable_fut = match self.kind {
            FutureKind::Resolvable {
                fut: ref resolvable_fut,
            } => resolvable_fut,
            // The future is immediately ready, so we can return the result.
            FutureKind::Immediate { ref res, .. } => return res,
        };

        // Happy path: if the result is already available, we can return it
        // without locking the Mutex.
        if let Some(result) = resolvable_fut.result.get() {
            return result;
        }

        let mut guard = resolvable_fut.state.lock().unwrap();
        loop {
            if let Some(result) = resolvable_fut.result.get() {
                // The result is already available, we can return it.
                return result;
            }
            let handle = guard.join_handle.take();
            if let Some(handle) = handle {
                // No one else has taken the handle, so we are responsible for completing
                // the future.
                mem::drop(guard);
                // unwrap: JoinError appears only when future either panic'ed or canceled.
                resolvable_fut.runtime.block_on(handle).unwrap();

                // Once we are here, the future is resolved.
                // The result is guaranteed to be set.
                return resolvable_fut.result.get().unwrap();
            } else {
                // Someone has taken the handle, so we need to wait for them to complete
                // the future. Once they finish or timeout, we will be notified.
                guard = resolvable_fut
                    .wait_for_value
                    .wait_while(guard, |state| {
                        // There are two cases when we should wake up:
                        // 1. The result is already available, so we can return it.
                        //    In this case, we will see it available in the next iteration
                        //    of the loop, so we will return it.
                        // 2. `join_handle` was None, and now it's Some - some other thread must
                        //    have timed out and returned the handle. We need to take over the work
                        //    of completing the future, because the result is still not available
                        //    and we may be using `current_thread` tokio executor, in which case
                        //    no one else will complete the future, so it's our responsibility.
                        //    In the next iteration we will land in the branch with `block_on`
                        //    and complete the future.
                        resolvable_fut.result.get().is_none() && state.join_handle.is_none()
                    })
                    // unwrap: Error appears only when mutex is poisoned.
                    .unwrap();
            }
        }
    }

    /// Tries to await the future with a given timeout and exposes the result,
    /// if it is available.
    ///
    /// There are three possible cases:
    /// - result is already available -> we can return.
    /// - no one is currently working on the future -> we take the ownership
    ///   of JoinHandle (future) and we try to poll it with given timeout.
    ///   If we timed out, we need to return the unfinished JoinHandle, so
    ///   some other thread can complete the future later.
    /// - some other thread is working on the future -> we wait on the condition
    ///   variable to get an access to the future's state.
    ///   Once we are notified (before the timeout), there are three cases:
    ///     - result is already available -> we can return.
    ///     - JoinHandle is consumed -> some other thread already resolved the future.
    ///       We can return.
    ///     - JoinHandle is Some -> some other thread was working on the future, but
    ///       timed out (see [CassFuture::waited_result_timed]). We need to
    ///       take the ownership of the handle, and continue the work.
    fn waited_result_timed(
        &self,
        timeout_duration: Duration,
    ) -> Result<&CassFutureResult, FutureError> {
        let resolvable_fut = match self.kind {
            FutureKind::Resolvable {
                fut: ref resolvable_fut,
            } => resolvable_fut,
            // The future is immediately ready, so we can return the result.
            FutureKind::Immediate { ref res, .. } => return Ok(res),
        };

        // Happy path: if the result is already available, we can return it
        // without locking the Mutex.
        if let Some(result) = resolvable_fut.result.get() {
            return Ok(result);
        }

        let mut guard = resolvable_fut.state.lock().unwrap();
        let deadline = tokio::time::Instant::now()
            .checked_add(timeout_duration)
            .ok_or(FutureError::InvalidDuration)?;

        loop {
            if let Some(result) = resolvable_fut.result.get() {
                // The result is already available, we can return it.
                return Ok(result);
            }
            let handle = guard.join_handle.take();
            if let Some(handle) = handle {
                // No one else has taken the handle, so we are responsible for completing
                // the future.
                mem::drop(guard);
                // Need to wrap it with async{} block, so the timeout is lazily executed inside the runtime.
                // See mention about panics: https://docs.rs/tokio/latest/tokio/time/fn.timeout.html.
                let timed = async {
                    let sleep_future = tokio::time::sleep_until(deadline);
                    tokio::pin!(sleep_future);
                    let value = future::select(handle, sleep_future).await;
                    match value {
                        future::Either::Left((result, _)) => Ok(result),
                        future::Either::Right((_, handle)) => Err(JoinHandleTimeout(handle)),
                    }
                };
                match resolvable_fut.runtime.block_on(timed) {
                    Err(JoinHandleTimeout(returned_handle)) => {
                        // We timed out. so we can't finish waiting for the future.
                        // The problem is that if current thread executor is used,
                        // then no one will run this future - other threads will
                        // go into the branch with condvar and wait there.
                        // To fix that:
                        //  - Return the join handle, so that next thread can take it
                        //  - Signal one thread, so that if all other consumers are
                        //    already waiting on condvar, one of them wakes up and
                        //    picks up the work.
                        guard = resolvable_fut.state.lock().unwrap();
                        guard.join_handle = Some(returned_handle);
                        resolvable_fut.wait_for_value.notify_one();
                        return Err(FutureError::TimeoutError);
                    }
                    // unwrap: JoinError appears only when future either panic'ed or canceled.
                    Ok(result) => {
                        result.unwrap();

                        // Once we are here, the future is resolved.
                        // The result is guaranteed to be set.
                        return Ok(resolvable_fut.result.get().unwrap());
                    }
                };
            } else {
                // Someone has taken the handle, so we need to wait for them to complete
                // the future. Once they finish or timeout, we will be notified.
                let remaining_timeout = deadline.duration_since(tokio::time::Instant::now());
                let (guard_result, timeout_result) = resolvable_fut
                    .wait_for_value
                    .wait_timeout_while(guard, remaining_timeout, |state| {
                        // There are two cases when we should wake up:
                        // 1. The result is already available, so we can return it.
                        //    In this case, we will see it available in the next iteration
                        //    of the loop, so we will return it.
                        // 2. `join_handle` was None, and now it's Some - some other thread must
                        //    have timed out and returned the handle. We need to take over the work
                        //    of completing the future, because the result is still not available
                        //    and we may be using `current_thread` tokio executor, in which case
                        //    no one else will complete the future, so it's our responsibility.
                        //    In the next iteration we will land in the branch with `block_on`
                        //    and attempt to complete the future.
                        resolvable_fut.result.get().is_none() && state.join_handle.is_none()
                    })
                    // unwrap: Error appears only when mutex is poisoned.
                    .unwrap();
                if timeout_result.timed_out() {
                    return Err(FutureError::TimeoutError);
                }

                guard = guard_result;
            }
        }
    }

    pub(crate) fn into_raw(self: Arc<Self>) -> CassOwnedSharedPtr<Self, CMut> {
        ArcFFI::into_ptr(self)
    }

    #[cfg(cpp_integration_testing)]
    pub(crate) fn attempted_hosts(&self) -> Vec<std::net::SocketAddr> {
        if let FutureKind::Resolvable {
            fut: ref resolvable_fut,
        } = self.kind
            && let Some(listener) = &resolvable_fut.recording_listener
        {
            listener.get_attempted_hosts()
        } else {
            vec![]
        }
    }
}

impl ResolvableFuture {
    unsafe fn set_callback(
        &self,
        self_ptr: CassBorrowedSharedPtr<CassFuture, CMut>,
        cb: NonNullFutureCallback,
        data: *mut c_void,
    ) -> CassError {
        let bound_cb = BoundCallback { cb, data };

        // Check if the callback is already set (in such case we must error out).
        // If it is not set, we store the callback in the state, so that no different
        // callback can be set.
        {
            let mut lock = self.state.lock().unwrap();
            if lock.callback.is_some() {
                // Another callback has been already set
                return CassError::CASS_ERROR_LIB_CALLBACK_ALREADY_SET;
            }

            // Store the callback, so that no other callback can be set from now on.
            // Rationale: only one callback can be set for the whole lifetime of a future.
            lock.callback = Some(bound_cb);
        }

        if self.result.get().is_some() {
            // The value is already available, we need to call the callback ourselves
            bound_cb.invoke(self_ptr);
            return CassError::CASS_OK;
        }
        CassError::CASS_OK
    }
}

// Do not remove; this asserts that `CassFuture` implements Send + Sync,
// which is required by the cpp-driver (saying that `CassFuture` is thread-safe).
#[allow(unused)]
trait CheckSendSync: Send + Sync {}
impl CheckSendSync for CassFuture {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_future_set_callback(
    future_raw: CassBorrowedSharedPtr<CassFuture, CMut>,
    callback: CassFutureCallback,
    data: *mut ::std::os::raw::c_void,
) -> CassError {
    let Some(future) = ArcFFI::as_ref(future_raw.borrow()) else {
        tracing::error!("Provided null future pointer to cass_future_set_callback!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    let Some(callback) = callback else {
        tracing::error!("Provided null callback pointer to cass_future_set_callback!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    match future.kind {
        FutureKind::Resolvable {
            fut: ref resolvable,
        } => {
            // Safety: `callback` is a valid pointer to a function that matches the signature.
            unsafe { resolvable.set_callback(future_raw.borrow(), callback, data) }
        }
        FutureKind::Immediate {
            ref callback_set, ..
        } => {
            if callback_set.swap(true, std::sync::atomic::Ordering::Relaxed) {
                // Another callback has been already set.
                return CassError::CASS_ERROR_LIB_CALLBACK_ALREADY_SET;
            }

            let bound_cb = BoundCallback { cb: callback, data };
            bound_cb.invoke(future_raw.borrow());
            CassError::CASS_OK
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_future_wait(future_raw: CassBorrowedSharedPtr<CassFuture, CMut>) {
    let Some(future) = ArcFFI::as_ref(future_raw) else {
        tracing::error!("Provided null future pointer to cass_future_wait!");
        return;
    };

    future.waited_result();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_future_wait_timed(
    future_raw: CassBorrowedSharedPtr<CassFuture, CMut>,
    timeout_us: cass_duration_t,
) -> cass_bool_t {
    let Some(future) = ArcFFI::as_ref(future_raw) else {
        tracing::error!("Provided null future pointer to cass_future_wait_timed!");
        return cass_false;
    };

    future
        .waited_result_timed(Duration::from_micros(timeout_us))
        .is_ok() as cass_bool_t
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_future_ready(
    future_raw: CassBorrowedSharedPtr<CassFuture, CMut>,
) -> cass_bool_t {
    let Some(future) = ArcFFI::as_ref(future_raw) else {
        tracing::error!("Provided null future pointer to cass_future_ready!");
        return cass_false;
    };

    (match future.kind {
        FutureKind::Resolvable {
            fut: ref resolvable_fut,
        } => resolvable_fut.result.get().is_some(),
        FutureKind::Immediate { .. } => true,
    }) as cass_bool_t
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_future_error_code(
    future_raw: CassBorrowedSharedPtr<CassFuture, CMut>,
) -> CassError {
    let Some(future) = ArcFFI::as_ref(future_raw) else {
        tracing::error!("Provided null future pointer to cass_future_error_code!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    match future.waited_result() {
        Ok(CassResultValue::QueryError(err)) => err.to_cass_error(),
        Err((err, _)) => *err,
        _ => CassError::CASS_OK,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_future_error_message(
    future: CassBorrowedSharedPtr<CassFuture, CMut>,
    message: *mut *const ::std::os::raw::c_char,
    message_length: *mut size_t,
) {
    let Some(future) = ArcFFI::as_ref(future) else {
        tracing::error!("Provided null future pointer to cass_future_error_message!");
        return;
    };

    let value = future.waited_result();
    let msg = match value {
        Ok(CassResultValue::QueryError(err)) => future.err_string.get_or_init(|| err.msg()),
        Err((_, s)) => s,
        _ => "",
    };

    unsafe { write_str_to_c(msg, message, message_length) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_future_free(future_raw: CassOwnedSharedPtr<CassFuture, CMut>) {
    ArcFFI::free(future_raw);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_future_get_result(
    future_raw: CassBorrowedSharedPtr<CassFuture, CMut>,
) -> CassOwnedSharedPtr<CassResult, CConst> {
    let Some(future) = ArcFFI::as_ref(future_raw) else {
        tracing::error!("Provided null future pointer to cass_future_get_result!");
        return ArcFFI::null();
    };

    future
        .waited_result()
        .as_ref()
        .ok()
        .and_then(|r| match r {
            CassResultValue::QueryResult(qr) => Some(Arc::clone(qr)),
            _ => None,
        })
        .map_or(ArcFFI::null(), ArcFFI::into_ptr)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_future_get_error_result(
    future_raw: CassBorrowedSharedPtr<CassFuture, CMut>,
) -> CassOwnedSharedPtr<CassErrorResult, CConst> {
    let Some(future) = ArcFFI::as_ref(future_raw) else {
        tracing::error!("Provided null future pointer to cass_future_get_error_result!");
        return ArcFFI::null();
    };

    future
        .waited_result()
        .as_ref()
        .ok()
        .and_then(|r| match r {
            CassResultValue::QueryError(qr) => Some(Arc::clone(qr)),
            _ => None,
        })
        .map_or(ArcFFI::null(), ArcFFI::into_ptr)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_future_get_prepared(
    future_raw: CassBorrowedSharedPtr<CassFuture, CMut>,
) -> CassOwnedSharedPtr<CassPrepared, CConst> {
    let Some(future) = ArcFFI::as_ref(future_raw) else {
        tracing::error!("Provided null future pointer to cass_future_get_prepared!");
        return ArcFFI::null();
    };

    future
        .waited_result()
        .as_ref()
        .ok()
        .and_then(|r| match r {
            CassResultValue::Prepared(p) => Some(Arc::clone(p)),
            _ => None,
        })
        .map_or(ArcFFI::null(), ArcFFI::into_ptr)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_future_tracing_id(
    future: CassBorrowedSharedPtr<CassFuture, CMut>,
    tracing_id: *mut CassUuid,
) -> CassError {
    let Some(future) = ArcFFI::as_ref(future) else {
        tracing::error!("Provided null future pointer to cass_future_tracing_id!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    match future.waited_result() {
        Ok(CassResultValue::QueryResult(result)) => match result.tracing_id {
            Some(id) => {
                unsafe { *tracing_id = CassUuid::from(id) };
                CassError::CASS_OK
            }
            None => CassError::CASS_ERROR_LIB_NO_TRACING_ID,
        },
        _ => CassError::CASS_ERROR_LIB_INVALID_FUTURE_TYPE,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_future_coordinator(
    future_raw: CassBorrowedSharedPtr<CassFuture, CMut>,
) -> CassBorrowedSharedPtr<CassNode, CConst> {
    let Some(future) = ArcFFI::as_ref(future_raw) else {
        tracing::error!("Provided null future to cass_future_coordinator!");
        return RefFFI::null();
    };

    match future.waited_result() {
        Ok(CassResultValue::QueryResult(result)) => {
            // unwrap: Coordinator is `None` only for tests.
            RefFFI::as_ptr(result.coordinator.as_ref().unwrap())
        }
        _ => RefFFI::null(),
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::utils::{assert_cass_error_eq, assert_cass_future_error_message_eq};

    use super::*;
    use std::{
        os::raw::c_char,
        thread::{self},
        time::Duration,
    };

    fn runtime_for_test() -> Arc<crate::runtime::Runtime> {
        Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .into(),
        )
    }

    // This is not a particularly smart test, but if some thread is granted access the value
    // before it is truly computed, then weird things should happen, even a segfault.
    // In the incorrect implementation that inspired this test to be written, this test
    // results with unwrap on a PoisonError on the CassFuture's mutex.
    #[test]
    #[ntest::timeout(100)]
    fn cass_future_thread_safety() {
        const ERROR_MSG: &str = "NOBODY EXPECTED SPANISH INQUISITION";
        let runtime = runtime_for_test();
        let fut = async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Err((CassError::CASS_OK, ERROR_MSG.into()))
        };
        let cass_fut = CassFuture::make_raw(
            runtime,
            fut,
            #[cfg(cpp_integration_testing)]
            None,
        );

        struct PtrWrapper(CassBorrowedSharedPtr<'static, CassFuture, CMut>);
        unsafe impl Send for PtrWrapper {}
        unsafe {
            // transmute to erase the lifetime to 'static, so the reference
            // can be passed to an async block.
            let static_cass_fut_ref = std::mem::transmute::<
                CassBorrowedSharedPtr<'_, CassFuture, CMut>,
                CassBorrowedSharedPtr<'static, CassFuture, CMut>,
            >(cass_fut.borrow());
            let wrapped_cass_fut = PtrWrapper(static_cass_fut_ref);
            let handle = thread::spawn(move || {
                let wrapper = &wrapped_cass_fut;
                let PtrWrapper(cass_fut) = wrapper;
                assert_cass_future_error_message_eq!(cass_fut, Some(ERROR_MSG));
            });

            assert_cass_future_error_message_eq!(cass_fut, Some(ERROR_MSG));

            handle.join().unwrap();
            cass_future_free(cass_fut);
        }
    }

    // This test makes sure that the future resolves even if timeout happens.
    #[test]
    #[ntest::timeout(200)]
    fn cass_future_resolves_after_timeout() {
        const ERROR_MSG: &str = "NOBODY EXPECTED SPANISH INQUISITION";
        const HUNDRED_MILLIS_IN_MICROS: u64 = 100 * 1000;
        let runtime = runtime_for_test();
        let fut = async move {
            tokio::time::sleep(Duration::from_micros(HUNDRED_MILLIS_IN_MICROS)).await;
            Err((CassError::CASS_OK, ERROR_MSG.into()))
        };
        let cass_fut = CassFuture::make_raw(
            runtime,
            fut,
            #[cfg(cpp_integration_testing)]
            None,
        );

        unsafe {
            // This should timeout on tokio::time::timeout.
            let timed_result =
                cass_future_wait_timed(cass_fut.borrow(), HUNDRED_MILLIS_IN_MICROS / 5);
            assert_eq!(0, timed_result);

            // This should timeout as well.
            let timed_result =
                cass_future_wait_timed(cass_fut.borrow(), HUNDRED_MILLIS_IN_MICROS / 5);
            assert_eq!(0, timed_result);

            // Verify that future eventually resolves, even though timeouts occurred before.
            assert_cass_future_error_message_eq!(cass_fut, Some(ERROR_MSG));

            cass_future_free(cass_fut);
        }
    }

    // This test checks whether the future callback is executed correctly when:
    // - a future is awaited indefinitely
    // - a future is awaited, after the timeout appeared (_wait_timed)
    // - a future is not awaited. We simply sleep, and let the tokio runtime resolve
    //   the future, and execute its callback
    #[test]
    #[ntest::timeout(600)]
    #[allow(clippy::disallowed_methods)]
    fn test_cass_future_callback() {
        const ERROR_MSG: &str = "NOBODY EXPECTED SPANISH INQUISITION";
        const HUNDRED_MILLIS_IN_MICROS: u64 = 100 * 1000;

        let runtime = runtime_for_test();

        let create_future_and_flag = || {
            unsafe extern "C" fn mark_flag_cb(
                _fut: CassBorrowedSharedPtr<CassFuture, CMut>,
                data: *mut c_void,
            ) {
                let flag = data as *mut bool;
                unsafe {
                    *flag = true;
                }
            }

            let fut = async move {
                tokio::time::sleep(Duration::from_micros(HUNDRED_MILLIS_IN_MICROS)).await;
                Err((CassError::CASS_OK, ERROR_MSG.into()))
            };
            let cass_fut = CassFuture::make_raw(
                Arc::clone(&runtime),
                fut,
                #[cfg(cpp_integration_testing)]
                None,
            );
            let flag = Box::new(false);
            let flag_ptr = Box::into_raw(flag);

            unsafe {
                assert_cass_error_eq!(
                    cass_future_set_callback(
                        cass_fut.borrow(),
                        Some(mark_flag_cb),
                        flag_ptr as *mut c_void,
                    ),
                    CassError::CASS_OK
                )
            };

            (cass_fut, flag_ptr)
        };

        // Callback executed after awaiting.
        {
            let (cass_fut, flag_ptr) = create_future_and_flag();
            unsafe { cass_future_wait(cass_fut.borrow()) };

            unsafe {
                assert_cass_future_error_message_eq!(cass_fut, Some(ERROR_MSG));
            }
            assert!(unsafe { *flag_ptr });

            unsafe { cass_future_free(cass_fut) };
            let _ = unsafe { Box::from_raw(flag_ptr) };
        }

        // Future awaited via `assert_cass_future_error_message_eq`.
        {
            let (cass_fut, flag_ptr) = create_future_and_flag();

            unsafe {
                assert_cass_future_error_message_eq!(cass_fut, Some(ERROR_MSG));
            }
            assert!(unsafe { *flag_ptr });

            unsafe { cass_future_free(cass_fut) };
            let _ = unsafe { Box::from_raw(flag_ptr) };
        }

        // Callback executed after timeouts.
        {
            let (cass_fut, flag_ptr) = create_future_and_flag();

            // This should timeout on tokio::time::timeout.
            let timed_result =
                unsafe { cass_future_wait_timed(cass_fut.borrow(), HUNDRED_MILLIS_IN_MICROS / 5) };
            assert_eq!(0, timed_result);
            // This should timeout as well.
            let timed_result =
                unsafe { cass_future_wait_timed(cass_fut.borrow(), HUNDRED_MILLIS_IN_MICROS / 5) };
            assert_eq!(0, timed_result);

            // Await and check result.
            unsafe {
                assert_cass_future_error_message_eq!(cass_fut, Some(ERROR_MSG));
            }
            assert!(unsafe { *flag_ptr });

            unsafe { cass_future_free(cass_fut) };
            let _ = unsafe { Box::from_raw(flag_ptr) };
        }

        // Don't await the future. Just sleep.
        {
            let (cass_fut, flag_ptr) = create_future_and_flag();

            runtime.block_on(async {
                tokio::time::sleep(Duration::from_micros(HUNDRED_MILLIS_IN_MICROS + 10 * 1000))
                    .await
            });

            assert!(unsafe { *flag_ptr });

            unsafe { cass_future_free(cass_fut) };
            let _ = unsafe { Box::from_raw(flag_ptr) };
        }
    }
}
