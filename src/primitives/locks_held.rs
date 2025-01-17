use super::LockData;
use crate::{Error, Result};
use std::{cell::RefCell, convert::identity, future::Future};
use tokio::{task::futures::TaskLocalFuture, task_local};

task_local! {
    static LOCKS_HELD: RefCell<Vec<u64>>;
}

pub(crate) fn add_lock(lock_id: u64) -> Result<()> {
    debug_assert_ne!(lock_id, 0);

    try_with(|locks_held| locks_held.push(lock_id))
}

pub(crate) fn check_deadlock(lock_data: &LockData, op: &str) -> Result<()> {
    try_with(|locks_held| {
        if locks_held.contains(&lock_data.id()) {
            return Err(Error::recursive_lock(lock_data, op));
        }

        lock_data.check_deadlock(op, locks_held)
    })
    .and_then(identity)
}

#[cfg(any(test, feature = "telemetry"))]
pub(crate) fn has_lock_held() -> bool {
    try_with(|l| !l.is_empty()).unwrap_or_default()
}

pub(crate) fn remove_lock(lock_id: u64) -> Result<()> {
    try_with(|locks_held| {
        if let Some(idx) = locks_held.iter().position(|p| *p == lock_id) {
            locks_held.swap_remove(idx);
        }
    })
}

pub(crate) fn scope<F>(f: F) -> TaskLocalFuture<RefCell<Vec<u64>>, F>
where
    F: Future,
{
    LOCKS_HELD.scope(RefCell::new(Vec::new()), f)
}

fn try_with<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&mut Vec<u64>) -> R,
{
    LOCKS_HELD
        .try_with(|cell| f(&mut cell.borrow_mut()))
        .map_err(Error::not_deadlock_check_future)
}
