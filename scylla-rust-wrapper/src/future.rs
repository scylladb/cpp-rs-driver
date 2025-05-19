use crate::RUNTIME;
use crate::argconv::*;
use crate::cass_error::CassError;
use crate::cass_error::CassErrorMessage;
use crate::cass_error::ToCassError;
use crate::execution_error::CassErrorResult;
use crate::prepared::CassPrepared;
use crate::query_result::CassResult;
use crate::types::*;
use crate::uuid::CassUuid;
use futures::future;
use std::future::Future;
use std::mem;
use std::os::raw::c_void;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use tokio::task::JoinHandle;
use tokio::time::Duration;

pub enum CassResultValue {
    Empty,
    QueryResult(Arc<CassResult>),
    QueryError(Arc<CassErrorResult>),
    Prepared(Arc<CassPrepared>),
}

type CassFutureError = (CassError, String);

pub type CassFutureResult = Result<CassResultValue, CassFutureError>;

pub type CassFutureCallback = Option<
    unsafe extern "C" fn(future: CassBorrowedSharedPtr<CassFuture, CMut>, data: *mut c_void),
>;

struct BoundCallback {
    pub cb: CassFutureCallback,
    pub data: *mut c_void,
}

// *mut c_void is not Send, so Rust will have to take our word
// that we won't screw something up
unsafe impl Send for BoundCallback {}

impl BoundCallback {
    fn invoke(self, fut_ptr: CassBorrowedSharedPtr<CassFuture, CMut>) {
        unsafe {
            self.cb.unwrap()(fut_ptr, self.data);
        }
    }
}

/// State of the execution of the [CassFuture],
/// together with a join handle of the tokio task that is executing it.
struct CassFutureState {
    execution_state: CassFutureExecution,
    /// Presence of this handle while `execution_state` is not `Completed` indicates
    /// that no thread is currently blocked on the future. This means that it might
    /// not be executed (especially in case of the current-thread executor).
    /// Absence means that some thread has blocked on the future, so it is necessarily
    /// being executed.
    join_handle: Option<JoinHandle<()>>,
}

/// State of the execution of the [CassFuture].
enum CassFutureExecution {
    RunningWithoutCallback,
    RunningWithCallback { callback: BoundCallback },
    Completed(CassFutureCompleted),
}

impl CassFutureExecution {
    fn completed(&self) -> bool {
        match self {
            Self::Completed(_) => true,
            Self::RunningWithCallback { .. } | Self::RunningWithoutCallback => false,
        }
    }

    /// Sets callback for the [CassFuture]. If the future has not completed yet,
    /// the callback will be invoked once the future is completed, by the executor thread.
    /// If the future has already completed, the callback will be invoked immediately.
    unsafe fn set_callback(
        mut state_lock: MutexGuard<CassFutureState>,
        fut_ptr: CassBorrowedSharedPtr<CassFuture, CMut>,
        cb: CassFutureCallback,
        data: *mut c_void,
    ) -> CassError {
        let bound_cb = BoundCallback { cb, data };

        match state_lock.execution_state {
            Self::RunningWithoutCallback => {
                // Store the callback.
                state_lock.execution_state = Self::RunningWithCallback { callback: bound_cb };
                CassError::CASS_OK
            }
            Self::RunningWithCallback { .. } =>
            // Another callback has been already set.
            {
                CassError::CASS_ERROR_LIB_CALLBACK_ALREADY_SET
            }
            Self::Completed { .. } => {
                // The value is already available, we need to call the callback ourselves.
                mem::drop(state_lock);
                bound_cb.invoke(fut_ptr);
                CassError::CASS_OK
            }
        }
    }

    /// Sets the [CassFuture] as completed. This function is called by the executor thread
    /// once it completes the underlying Rust future. If there's a callback set,
    /// it will be invoked immediately.
    fn complete(
        mut state_lock: MutexGuard<CassFutureState>,
        value: CassFutureResult,
        cass_fut: &Arc<CassFuture>,
    ) {
        let prev_state = mem::replace(
            &mut state_lock.execution_state,
            Self::Completed(CassFutureCompleted::new(value)),
        );

        // This is because we mustn't hold the lock while invoking the callback.
        mem::drop(state_lock);

        let maybe_cb = match prev_state {
            Self::RunningWithoutCallback => None,
            Self::RunningWithCallback { callback } => Some(callback),
            Self::Completed { .. } => unreachable!(
                "Exactly one dedicated tokio task is expected to execute and complete the CassFuture."
            ),
        };

        if let Some(bound_cb) = maybe_cb {
            let fut_ptr = ArcFFI::as_ptr::<CMut>(cass_fut);
            // Safety: pointer is valid, because we get it from arc allocation.
            bound_cb.invoke(fut_ptr);
        }
    }
}

/// The result of a completed [CassFuture].
struct CassFutureCompleted {
    /// The result of the future, either a value or an error.
    value: CassFutureResult,
    /// Just a cache for the error message. Needed because the C API exposes a pointer to the
    /// error message, and we need to keep it alive until the future is freed.
    /// Initially, it's `None`, and it is set to `Some` when the error message is requested
    /// by `cass_future_error_message()`.
    cached_err_string: Option<String>,
}

impl CassFutureCompleted {
    fn new(value: CassFutureResult) -> Self {
        Self {
            value,
            cached_err_string: None,
        }
    }
}

/// The C-API representation of a future. Implemented as a wrapper around a Rust future
/// that can be awaited and has a callback mechanism. It's **eager** in a way that
/// its execution starts possibly immediately (unless the executor thread pool is nempty,
/// which is the case for the current-thread executor).
pub struct CassFuture {
    state: Mutex<CassFutureState>,
    wait_for_value: Condvar,
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
    pub fn make_raw(
        fut: impl Future<Output = CassFutureResult> + Send + 'static,
    ) -> CassOwnedSharedPtr<CassFuture, CMut> {
        Self::new_from_future(fut).into_raw()
    }

    pub fn new_from_future(
        fut: impl Future<Output = CassFutureResult> + Send + 'static,
    ) -> Arc<CassFuture> {
        let cass_fut = Arc::new(CassFuture {
            state: Mutex::new(CassFutureState {
                join_handle: None,
                execution_state: CassFutureExecution::RunningWithoutCallback,
            }),
            wait_for_value: Condvar::new(),
        });
        let cass_fut_clone = Arc::clone(&cass_fut);
        let join_handle = RUNTIME.spawn(async move {
            let r = fut.await;

            let guard = cass_fut_clone.state.lock().unwrap();
            CassFutureExecution::complete(guard, r, &cass_fut_clone);

            cass_fut_clone.wait_for_value.notify_all();
        });
        {
            let mut lock = cass_fut.state.lock().unwrap();
            lock.join_handle = Some(join_handle);
        }
        cass_fut
    }

    pub fn new_ready(r: CassFutureResult) -> Arc<Self> {
        Arc::new(CassFuture {
            state: Mutex::new(CassFutureState {
                join_handle: None,
                execution_state: CassFutureExecution::Completed(CassFutureCompleted::new(r)),
            }),
            wait_for_value: Condvar::new(),
        })
    }

    pub fn with_waited_result<T>(&self, f: impl FnOnce(&mut CassFutureResult) -> T) -> T {
        self.with_waited_state(|s| f(&mut s.value))
    }

    /// Awaits the future until completion.
    ///
    /// There are two possible cases:
    /// - noone is currently working on the future -> we take the ownership
    ///   of JoinHandle (future) and we poll it until completion.
    /// - some other thread is working on the future -> we wait on the condition
    ///   variable to get an access to the future's state. Once we are notified,
    ///   there are two cases:
    ///     - JoinHandle is consumed -> some other thread already resolved the future.
    ///       We can return.
    ///     - JoinHandle is Some -> some other thread was working on the future, but
    ///       timed out (see [CassFuture::with_waited_state_timed]). We need to
    ///       take the ownership of the handle, and complete the work.
    fn with_waited_state<T>(&self, f: impl FnOnce(&mut CassFutureCompleted) -> T) -> T {
        let mut guard = self.state.lock().unwrap();
        loop {
            let handle = guard.join_handle.take();
            if let Some(handle) = handle {
                mem::drop(guard);
                // unwrap: JoinError appears only when future either panic'ed or canceled.
                RUNTIME.block_on(handle).unwrap();
                guard = self.state.lock().unwrap();
            } else {
                guard = self
                    .wait_for_value
                    .wait_while(guard, |state| {
                        !state.execution_state.completed() && state.join_handle.is_none()
                    })
                    // unwrap: Error appears only when mutex is poisoned.
                    .unwrap();
                if guard.join_handle.is_some() {
                    // join_handle was none, and now it isn't - some other thread must
                    // have timed out and returned the handle. We need to take over
                    // the work of completing the future. To do that, we go into
                    // another iteration so that we land in the branch with block_on.
                    continue;
                }
            }

            // If we had ended up in either the handle's or with the condvar's `if` branch,
            // we awaited the future and it is now completed.
            let completed = match &mut guard.execution_state {
                CassFutureExecution::RunningWithoutCallback
                | CassFutureExecution::RunningWithCallback { .. } => unreachable!(),
                CassFutureExecution::Completed(completed) => completed,
            };
            return f(completed);
        }
    }

    fn with_waited_result_timed<T>(
        &self,
        f: impl FnOnce(&mut CassFutureResult) -> T,
        timeout_duration: Duration,
    ) -> Result<T, FutureError> {
        self.with_waited_state_timed(|s| f(&mut s.value), timeout_duration)
    }

    /// Tries to await the future with a given timeout.
    ///
    /// There are two possible cases:
    /// - noone is currently working on the future -> we take the ownership
    ///   of JoinHandle (future) and we try to poll it with given timeout.
    ///   If we timed out, we need to return the unfinished JoinHandle, so
    ///   some other thread can complete the future later.
    /// - some other thread is working on the future -> we wait on the condition
    ///   variable to get an access to the future's state.
    ///   Once we are notified (before the timeout), there are two cases.
    ///     - JoinHandle is consumed -> some other thread already resolved the future.
    ///       We can return.
    ///     - JoinHandle is Some -> some other thread was working on the future, but
    ///       timed out (see [CassFuture::with_waited_state_timed]). We need to
    ///       take the ownership of the handle, and continue the work.
    fn with_waited_state_timed<T>(
        &self,
        f: impl FnOnce(&mut CassFutureCompleted) -> T,
        timeout_duration: Duration,
    ) -> Result<T, FutureError> {
        let mut guard = self.state.lock().unwrap();
        let deadline = tokio::time::Instant::now()
            .checked_add(timeout_duration)
            .ok_or(FutureError::InvalidDuration)?;

        loop {
            let handle = guard.join_handle.take();
            if let Some(handle) = handle {
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
                match RUNTIME.block_on(timed) {
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
                        guard = self.state.lock().unwrap();
                        guard.join_handle = Some(returned_handle);
                        self.wait_for_value.notify_one();
                        return Err(FutureError::TimeoutError);
                    }
                    // unwrap: JoinError appears only when future either panic'ed or canceled.
                    Ok(result) => result.unwrap(),
                };
                guard = self.state.lock().unwrap();
            } else {
                let remaining_timeout = deadline.duration_since(tokio::time::Instant::now());
                let (guard_result, timeout_result) = self
                    .wait_for_value
                    .wait_timeout_while(guard, remaining_timeout, |state| {
                        !state.execution_state.completed() && state.join_handle.is_none()
                    })
                    // unwrap: Error appears only when mutex is poisoned.
                    .unwrap();
                if timeout_result.timed_out() {
                    return Err(FutureError::TimeoutError);
                }
                guard = guard_result;
                if guard.join_handle.is_some() {
                    // join_handle was none, and now it isn't - some other thread must
                    // have timed out and returned the handle. We need to take over
                    // the work of completing the future. To do that, we go into
                    // another iteration so that we land in the branch with block_on.
                    continue;
                }
            }

            // If we had ended up in either the handle's or with the condvar's `if` branch
            // and we didn't return `TimeoutError`, we awaited the future and it is now completed.
            let completed = match &mut guard.execution_state {
                CassFutureExecution::RunningWithoutCallback
                | CassFutureExecution::RunningWithCallback { .. } => unreachable!(),
                CassFutureExecution::Completed(completed) => completed,
            };
            return Ok(f(completed));
        }
    }

    pub unsafe fn set_callback(
        &self,
        self_ptr: CassBorrowedSharedPtr<CassFuture, CMut>,
        cb: CassFutureCallback,
        data: *mut c_void,
    ) -> CassError {
        let lock = self.state.lock().unwrap();
        unsafe { CassFutureExecution::set_callback(lock, self_ptr, cb, data) }
    }

    fn into_raw(self: Arc<Self>) -> CassOwnedSharedPtr<Self, CMut> {
        ArcFFI::into_ptr(self)
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

    unsafe { future.set_callback(future_raw.borrow(), callback, data) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_future_wait(future_raw: CassBorrowedSharedPtr<CassFuture, CMut>) {
    let Some(future) = ArcFFI::as_ref(future_raw) else {
        tracing::error!("Provided null future pointer to cass_future_wait!");
        return;
    };

    future.with_waited_result(|_| ());
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
        .with_waited_result_timed(|_| (), Duration::from_micros(timeout_us))
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

    let state_guard = future.state.lock().unwrap();
    state_guard.execution_state.completed() as cass_bool_t
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_future_error_code(
    future_raw: CassBorrowedSharedPtr<CassFuture, CMut>,
) -> CassError {
    let Some(future) = ArcFFI::as_ref(future_raw) else {
        tracing::error!("Provided null future pointer to cass_future_error_code!");
        return CassError::CASS_ERROR_LIB_BAD_PARAMS;
    };

    future.with_waited_result(|r: &mut CassFutureResult| match r {
        Ok(CassResultValue::QueryError(err)) => err.to_cass_error(),
        Err((err, _)) => *err,
        _ => CassError::CASS_OK,
    })
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

    future.with_waited_state(|completed: &mut CassFutureCompleted| {
        let msg = completed
            .cached_err_string
            .get_or_insert_with(|| match &completed.value {
                Ok(CassResultValue::QueryError(err)) => err.msg(),
                Err((_, s)) => s.msg(),
                _ => "".to_string(),
            });
        unsafe { write_str_to_c(msg.as_str(), message, message_length) };
    });
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
        .with_waited_result(|r: &mut CassFutureResult| -> Option<Arc<CassResult>> {
            match r.as_ref().ok()? {
                CassResultValue::QueryResult(qr) => Some(Arc::clone(qr)),
                _ => None,
            }
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
        .with_waited_result(|r: &mut CassFutureResult| -> Option<Arc<CassErrorResult>> {
            match r.as_ref().ok()? {
                CassResultValue::QueryError(qr) => Some(Arc::clone(qr)),
                _ => None,
            }
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
        .with_waited_result(|r: &mut CassFutureResult| -> Option<Arc<CassPrepared>> {
            match r.as_ref().ok()? {
                CassResultValue::Prepared(p) => Some(Arc::clone(p)),
                _ => None,
            }
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

    future.with_waited_result(|r: &mut CassFutureResult| match r {
        Ok(CassResultValue::QueryResult(result)) => match result.tracing_id {
            Some(id) => {
                unsafe { *tracing_id = CassUuid::from(id) };
                CassError::CASS_OK
            }
            None => CassError::CASS_ERROR_LIB_NO_TRACING_ID,
        },
        _ => CassError::CASS_ERROR_LIB_INVALID_FUTURE_TYPE,
    })
}

#[cfg(test)]
mod tests {
    use crate::testing::{assert_cass_error_eq, assert_cass_future_error_message_eq};

    use super::*;
    use std::{
        os::raw::c_char,
        thread::{self},
        time::Duration,
    };

    // This is not a particularly smart test, but if some thread is granted access the value
    // before it is truly computed, then weird things should happen, even a segfault.
    // In the incorrect implementation that inspired this test to be written, this test
    // results with unwrap on a PoisonError on the CassFuture's mutex.
    #[test]
    #[ntest::timeout(100)]
    fn cass_future_thread_safety() {
        const ERROR_MSG: &str = "NOBODY EXPECTED SPANISH INQUISITION";
        let fut = async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Err((CassError::CASS_OK, ERROR_MSG.into()))
        };
        let cass_fut = CassFuture::make_raw(fut);

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
        let fut = async move {
            tokio::time::sleep(Duration::from_micros(HUNDRED_MILLIS_IN_MICROS)).await;
            Err((CassError::CASS_OK, ERROR_MSG.into()))
        };
        let cass_fut = CassFuture::make_raw(fut);

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
            let cass_fut = CassFuture::make_raw(fut);
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

            RUNTIME.block_on(async {
                tokio::time::sleep(Duration::from_micros(HUNDRED_MILLIS_IN_MICROS + 10 * 1000))
                    .await
            });

            assert!(unsafe { *flag_ptr });

            unsafe { cass_future_free(cass_fut) };
            let _ = unsafe { Box::from_raw(flag_ptr) };
        }
    }
}
