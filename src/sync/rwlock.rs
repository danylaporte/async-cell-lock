use super::acquire_sync;
use crate::{
    Result,
    primitives::{LockData, LockHeldGuard, Ops},
};
use std::ops::{Deref, DerefMut};

/// This is a sync RwLock based on [parking_lot::RwLock] that should not deadlock
/// even when used inside async context across await point.
pub struct RwLock<T> {
    lock_data: LockData,
    rwlock: parking_lot::RwLock<T>,
}

impl<T> RwLock<T> {
    pub const fn new(value: T, name: &'static str) -> Self {
        Self {
            lock_data: LockData::new(name),
            rwlock: parking_lot::RwLock::new(value),
        }
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.rwlock.get_mut()
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.rwlock.into_inner()
    }

    pub fn read(&self) -> Result<RwLockReadGuard<'_, T>> {
        let rw = &self.rwlock;
        let (_active, guard) = acquire_sync(
            &self.lock_data,
            Ops::Read,
            || rw.try_read(),
            |timeout| rw.try_read_for(timeout),
            || rw.read(),
        )?;

        Ok(RwLockReadGuard { _active, guard })
    }

    pub fn write(&self) -> Result<RwLockWriteGuard<'_, T>> {
        let rw = &self.rwlock;
        let (_active, guard) = acquire_sync(
            &self.lock_data,
            Ops::Write,
            || rw.try_write(),
            |timeout| rw.try_write_for(timeout),
            || rw.write(),
        )?;

        Ok(RwLockWriteGuard { _active, guard })
    }
}

#[must_use = "if unused the RwLock will immediately unlock"]
pub struct RwLockReadGuard<'a, T> {
    _active: LockHeldGuard<'a>,
    guard: parking_lot::RwLockReadGuard<'a, T>,
}

impl<T> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

#[must_use = "if unused the RwLock will immediately unlock"]
pub struct RwLockWriteGuard<'a, T> {
    _active: LockHeldGuard<'a>,
    guard: parking_lot::RwLockWriteGuard<'a, T>,
}

impl<T> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<T> DerefMut for RwLockWriteGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

#[cfg(test)]
#[tokio::test]
async fn can_keep_lock_across_await_point() {
    // even if the test is invalid, we are testing that no deadlock occurs here
    // when used inside async.

    crate::with_deadlock_check(
        async move {
            let rwlock = std::sync::Arc::new(RwLock::new((), "test"));
            let spawn_rwlock = rwlock.clone();
            let g = rwlock.write().unwrap();

            let j = crate::spawn_with_deadlock_check(
                async move {
                    let _ = spawn_rwlock.read();
                },
                "test",
            );

            tokio::time::sleep(std::time::Duration::from_millis(10)).await;

            drop(g);

            j.await.unwrap();
        },
        "test",
    )
    .await
}

#[cfg(test)]
#[tokio::test]
async fn recursive_call_returns_an_error() {
    crate::with_deadlock_check(
        async move {
            let rwlock = RwLock::new((), "test");
            let g = rwlock.read().unwrap();

            assert!(rwlock.read().is_ok());
            assert!(rwlock.write().is_err());
            assert!(rwlock.read().is_ok());

            drop(g);

            let g = rwlock.write().unwrap();

            assert!(rwlock.read().is_err());
            assert!(rwlock.write().is_err());

            drop(g);

            assert!(rwlock.read().is_ok());
            assert!(rwlock.write().is_ok());
        },
        "test",
    )
    .await
}
