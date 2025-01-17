use crate::primitives::LockData;
use std::{
    error,
    fmt::{self, Formatter},
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Error {
    DeadlockDetected,
    RecursiveLock,
    NotDeadlockCheckFuture,
    SyncLockForTooLong,
}

impl Error {
    pub(crate) fn not_deadlock_check_future<E>(_: E) -> Self {
        Self::NotDeadlockCheckFuture
    }

    #[allow(unused_variables)]
    pub(crate) fn deadlock_detected(lock_data: &LockData, op: &str, locked_task: &str) -> Self {
        #[cfg(feature = "telemetry")]
        {
            let _ = crate::primitives::task::try_with(|task| {
                tracing::error!(
                    lock = lock_data.name,
                    op = op,
                    await_task = task.name,
                    locked_task = locked_task,
                    "deadlock detected"
                );

                let _ = tracing::error_span!(parent: None, "deadlock detected", lock = lock_data.name, op = op, await_task = task.name, locked_task = locked_task)
                    .entered();
            });
        }

        Self::DeadlockDetected
    }

    #[allow(unused_variables)]
    pub(crate) fn recursive_lock(lock_data: &LockData, op: &str) -> Self {
        #[cfg(feature = "telemetry")]
        {
            let _ = crate::primitives::task::try_with(|task| {
                tracing::error!(
                    lock = lock_data.name,
                    op = op,
                    task = task.name,
                    "recursive lock",
                );

                let _ = tracing::error_span!(
                    parent: None,
                    "recursive lock",
                    lock = lock_data.name,
                    op = op,
                    task = task.name
                )
                .entered();
            });
        }

        Self::RecursiveLock
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeadlockDetected => f.write_str("Deadlock detected."),
            Self::NotDeadlockCheckFuture => {
                f.write_str("Must run inside a with_deadlock_check future.")
            }
            Self::RecursiveLock => f.write_str("Recursive lock."),
            Self::SyncLockForTooLong => f.write_str("Synchronous lock for too long"),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self, f)
    }
}

impl error::Error for Error {}
