use super::acquire_async;
use crate::{
    Result,
    primitives::{LockData, LockHeldGuard, Ops},
};
use std::ops::{Deref, DerefMut};

/// An async RwLock based on [tokio::sync::RwLock] with deadlock detection.
pub struct RwLock<T> {
    lock_data: LockData,
    rwlock: tokio::sync::RwLock<T>,
}

impl<T> RwLock<T> {
    pub const fn new(value: T, name: &'static str) -> Self {
        Self {
            lock_data: LockData::new(name),
            rwlock: tokio::sync::RwLock::const_new(value),
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

    pub async fn read(&self) -> Result<RwLockReadGuard<'_, T>> {
        let rw = &self.rwlock;
        let (_active, guard) =
            acquire_async(&self.lock_data, Ops::Read, rw.try_read().ok(), || rw.read()).await?;

        Ok(RwLockReadGuard { _active, guard })
    }

    pub async fn write(&self) -> Result<RwLockWriteGuard<'_, T>> {
        let rw = &self.rwlock;
        let (_active, guard) =
            acquire_async(&self.lock_data, Ops::Write, rw.try_write().ok(), || {
                rw.write()
            })
            .await?;

        Ok(RwLockWriteGuard { _active, guard })
    }
}

#[must_use = "if unused the RwLock will immediately unlock"]
pub struct RwLockReadGuard<'a, T> {
    _active: LockHeldGuard<'a>,
    guard: tokio::sync::RwLockReadGuard<'a, T>,
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
    guard: tokio::sync::RwLockWriteGuard<'a, T>,
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
    crate::with_deadlock_check(
        async move {
            let rwlock = std::sync::Arc::new(RwLock::new((), "test"));
            let spawn_rwlock = rwlock.clone();
            let g = rwlock.write().await.unwrap();

            let j = crate::spawn_with_deadlock_check(
                async move {
                    assert!(spawn_rwlock.read().await.is_ok());
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
            let g = rwlock.read().await.unwrap();

            assert!(rwlock.read().await.is_ok());
            assert!(rwlock.write().await.is_err());
            assert!(rwlock.read().await.is_ok());

            drop(g);

            let g = rwlock.write().await.unwrap();

            assert!(rwlock.read().await.is_err());
            assert!(rwlock.write().await.is_err());

            drop(g);

            assert!(rwlock.read().await.is_ok());
            assert!(rwlock.write().await.is_ok());
        },
        "test",
    )
    .await
}
