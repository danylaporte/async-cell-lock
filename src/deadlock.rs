use crate::Error;
use std::cell::Cell;
use tracing::{error_span, warn_span};

/// A deadlock detector.
pub(crate) struct DLDetector;

impl DLDetector {
    pub fn read(&self) -> Result<DLGuard, Error> {
        TASK.try_with(|task| {
            task.set(task.get().read()?);
            Ok(())
        })
        .map_err(|_| not_deadlock_check_future())
        .and_then(|r| r.map(|_| DLGuard))
    }

    pub fn write(&self) -> Result<DLGuard, Error> {
        TASK.try_with(|task| {
            task.set(task.get().write()?);
            Ok(())
        })
        .map_err(|_| not_deadlock_check_future())
        .and_then(|r| r.map(|_| DLGuard))
    }
}

pub(crate) struct DLGuard;

impl Drop for DLGuard {
    fn drop(&mut self) {
        let _ = TASK.try_with(|task| task.set(task.get().unlock()));
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
    static TASK: Cell<TaskData>;
}

#[derive(Clone, Copy)]
enum TaskData {
    Read(usize),
    Write,
}

impl TaskData {
    fn read(&self) -> Result<TaskData, Error> {
        match self {
            Self::Read(v) => Ok(Self::Read(v + 1)),
            Self::Write => deadlock_detected(),
        }
    }

    fn count(self) -> usize {
        match self {
            Self::Read(v) => v,
            Self::Write => 1,
        }
    }

    fn unlock(self) -> Self {
        match self {
            Self::Read(v) => Self::Read(v.saturating_sub(1)),
            Self::Write => Self::Read(0),
        }
    }

    fn write(&self) -> Result<TaskData, Error> {
        match self {
            Self::Read(0) => Ok(Self::Write),
            Self::Read(_) | Self::Write => deadlock_detected(),
        }
    }
}

impl Default for TaskData {
    fn default() -> Self {
        Self::Read(0)
    }
}

fn deadlock_detected() -> Result<TaskData, Error> {
    let _ = error_span!("deadlock detected").entered();
    Err(Error::DeadlockDetected)
}

/// Gets a count of currently active locks in the task.
pub(crate) fn lock_held_count() -> Result<usize, Error> {
    TASK.try_with(|d| d.get().count())
        .map_err(|_| Error::NotDeadlockCheckFuture)
}

fn not_deadlock_check_future() -> Error {
    let _ = error_span!("Not a deadlock check future").entered();
    Error::NotDeadlockCheckFuture
}

/// Log a "lock held" warn in the trace if a lock is currently active.
/// This is useful to prevent a lock from being held while a call api.
pub fn warn_lock_held() {
    if lock_held_count().ok().filter(|v| *v > 0).is_some() {
        let _ = warn_span!("lock held").entered();
    }
}
