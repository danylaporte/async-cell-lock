//! Deadlock-aware wrappers around [parking_lot] (sync) and [tokio::sync] (async) locks.

pub mod async_mutex;
pub mod async_rwlock;
pub mod mutex;
pub mod rwlock;

use crate::{
    Error, Result, is_async,
    primitives::{LockAwaitGuard, LockData, LockHeldGuard, Ops},
};
use std::{future::Future, time::Duration};

/// Maximum time a sync lock may block a thread that is driving an async runtime.
const SYNC_LOCK_TIMEOUT: Duration = Duration::from_millis(50);

/// Acquires a sync lock, registering it for deadlock detection.
///
/// The uncontended path is kept small so it can be inlined; waiting is delegated
/// to a cold, out-of-line function.
#[inline]
fn acquire_sync<'a, G>(
    lock_data: &'a LockData,
    op: Ops,
    try_lock: impl FnOnce() -> Option<G>,
    try_lock_for: impl FnOnce(Duration) -> Option<G>,
    lock: impl FnOnce() -> G,
) -> Result<(LockHeldGuard<'a>, G)> {
    match try_lock() {
        Some(guard) => Ok((LockHeldGuard::new_no_wait(lock_data, op)?, guard)),
        None => acquire_sync_slow(lock_data, op, try_lock_for, lock),
    }
}

#[cold]
#[inline(never)]
fn acquire_sync_slow<'a, G>(
    lock_data: &'a LockData,
    op: Ops,
    try_lock_for: impl FnOnce(Duration) -> Option<G>,
    lock: impl FnOnce() -> G,
) -> Result<(LockHeldGuard<'a>, G)> {
    let wait = LockAwaitGuard::new(lock_data, op)?;

    // Blocking a runtime worker indefinitely could starve the task holding the lock.
    let guard = if is_async() {
        try_lock_for(SYNC_LOCK_TIMEOUT).ok_or_else(|| Error::sync_lock_timeout(lock_data, op))?
    } else {
        lock()
    };

    Ok((LockHeldGuard::new(wait)?, guard))
}

/// Acquires an async lock, registering it for deadlock detection.
#[inline]
async fn acquire_async<'a, G, F>(
    lock_data: &'a LockData,
    op: Ops,
    fast: Option<G>,
    lock: impl FnOnce() -> F,
) -> Result<(LockHeldGuard<'a>, G)>
where
    F: Future<Output = G>,
{
    if let Some(guard) = fast {
        return Ok((LockHeldGuard::new_no_wait(lock_data, op)?, guard));
    }

    let wait = LockAwaitGuard::new(lock_data, op)?;
    let guard = lock().await;

    Ok((LockHeldGuard::new(wait)?, guard))
}
