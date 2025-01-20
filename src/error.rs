use crate::primitives::{LockData, Ops};
use std::{
    error,
    fmt::{self, Formatter},
};

#[derive(Clone, Copy)]
pub enum Error {
    DeadlockDetected,
    RecursiveLock,
    NotDeadlockCheckFuture,
    SyncLockTimeout,
}

impl Error {
    pub(crate) fn not_deadlock_check_future<E>(_: E) -> Self {
        Self::NotDeadlockCheckFuture
    }

    #[allow(unused_variables)]
    pub(crate) fn deadlock_detected(lock_data: &LockData, op: Ops, locked_task: &str) -> Self {
        #[cfg(feature = "telemetry")]
        {
            let _ = crate::primitives::task::try_with(|task| {
                tracing::error!(
                    lock = lock_data.name,
                    op = op.as_str(),
                    await_task = task.name,
                    locked_task = locked_task,
                    "deadlock detected"
                );

                let _ = tracing::error_span!(parent: None, "deadlock detected", lock = lock_data.name, op = op.as_str(), await_task = task.name, locked_task = locked_task)
                    .entered();

                create_counter(lock_data, op, task, "deadlock_detected");
            });
        }

        Self::DeadlockDetected
    }

    #[allow(unused_variables)]
    pub(crate) fn recursive_lock(lock_data: &LockData, op: Ops) -> Self {
        #[cfg(feature = "telemetry")]
        {
            let _ = crate::primitives::task::try_with(|task| {
                tracing::error!(
                    lock = lock_data.name,
                    op = op.as_str(),
                    task = task.name,
                    "recursive lock",
                );

                let _ = tracing::error_span!(
                    parent: None,
                    "recursive lock",
                    lock = lock_data.name,
                    op = op.as_str(),
                    task = task.name
                )
                .entered();

                create_counter(lock_data, op, task, "recursive_lock");
            });
        }

        Self::RecursiveLock
    }

    #[allow(unused_variables)]
    pub(crate) fn sync_lock_timeout(lock_data: &LockData, op: Ops) -> Self {
        #[cfg(feature = "telemetry")]
        {
            let _ = crate::primitives::task::try_with(|task| {
                tracing::error!(
                    lock = lock_data.name,
                    op = op.as_str(),
                    task = task.name,
                    "sync lock timeout",
                );

                let _ = tracing::error_span!(
                    parent: None,
                    "sync lock timeout",
                    lock = lock_data.name,
                    op = op.as_str(),
                    task = task.name
                )
                .entered();

                create_counter(lock_data, op, task, "sync_lock_timeout");
            });
        }

        Self::SyncLockTimeout
    }
}

#[cfg(feature = "telemetry")]
fn create_counter(
    lock_data: &LockData,
    op: Ops,
    task: &crate::primitives::Task,
    error: &'static str,
) {
    metrics::counter!("lock_error_count", "error" => error, "lock_name" => lock_data.name, "op" => op, "task" => task.name.clone())
    .increment(1);
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeadlockDetected => f.write_str("Deadlock detected."),
            Self::NotDeadlockCheckFuture => {
                f.write_str("Must run inside a with_deadlock_check future.")
            }
            Self::RecursiveLock => f.write_str("Recursive lock."),
            Self::SyncLockTimeout => f.write_str("Synchronous lock for too long"),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self, f)
    }
}

impl error::Error for Error {}

impl Eq for Error {}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        *self as u8 == *other as u8
    }
}
