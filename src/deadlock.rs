use crate::Error;
use std::cell::Cell;

/// A deadlock detector.
pub(crate) struct DLDetector;

impl DLDetector {
    pub(crate) fn lock(&self) -> Result<DLGuard, Error> {
        TASK.try_with(|locked| {
            if locked.get() {
                deadlock_detected()
            } else {
                locked.set(true);
                Ok(())
            }
        })
        .map_err(|_| not_deadlock_check_future())
        .and_then(|r| r.map(|_| DLGuard))
    }
}

pub(crate) struct DLGuard;

impl Drop for DLGuard {
    fn drop(&mut self) {
        let _ = TASK.try_with(|locked| locked.set(false));
    }
}

pub async fn with_deadlock_check<F, R>(f: F) -> R
where
    F: std::future::Future<Output = R>,
{
    if TASK.try_with(|_| ()).is_err() {
        TASK.scope(Default::default(), f).await
    } else {
        f.await
    }
}

tokio::task_local! {
    static TASK: Cell<bool>;
}

fn deadlock_detected() -> Result<(), Error> {
    #[cfg(feature = "telemetry")]
    {
        let _ = tracing::error_span!("deadlock detected").entered();
    }

    Err(Error::DeadlockDetected)
}

/// Gets a count of currently active locks in the task.
pub fn lock_held() -> Result<bool, Error> {
    TASK.try_with(|d| d.get())
        .map_err(|_| Error::NotDeadlockCheckFuture)
}

fn not_deadlock_check_future() -> Error {
    #[cfg(feature = "telemetry")]
    {
        let _ = tracing::error_span!("Not a deadlock check future").entered();
    }

    Error::NotDeadlockCheckFuture
}

/// Log a "lock held" warn in the trace if a lock is currently active.
/// This is useful to prevent a lock from being held while a call api.
#[cfg(feature = "telemetry")]
pub fn warn_lock_held() {
    if lock_held().ok().unwrap_or_default() {
        let _ = tracing::warn_span!("lock held").entered();
    }
}
