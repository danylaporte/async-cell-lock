use crate::{
    primitives::{LockAwaitGuard, LockData, LockHeldGuard, Ops},
    Result,
};
use std::ops::{Deref, DerefMut};

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

    pub fn get_mut(&mut self) -> &mut T {
        self.rwlock.get_mut()
    }

    pub fn into_inner(self) -> T {
        self.rwlock.into_inner()
    }

    pub async fn read(&self) -> Result<RwLockReadGuard<'_, T>> {
        if let Ok(guard) = self.rwlock.try_read() {
            return Ok(RwLockReadGuard {
                _active: LockHeldGuard::new_no_wait(&self.lock_data, Ops::Read)?,
                guard,
            });
        }

        let wait = LockAwaitGuard::new(&self.lock_data, Ops::Read)?;
        let guard = self.rwlock.read().await;
        let _active = LockHeldGuard::new(wait)?;

        Ok(RwLockReadGuard { _active, guard })
    }

    pub async fn write(&self) -> Result<RwLockWriteGuard<'_, T>> {
        if let Ok(guard) = self.rwlock.try_write() {
            return Ok(RwLockWriteGuard {
                _active: LockHeldGuard::new_no_wait(&self.lock_data, Ops::Write)?,
                guard,
            });
        }

        let wait = LockAwaitGuard::new(&self.lock_data, Ops::Write)?;
        let guard = self.rwlock.write().await;
        let _active = LockHeldGuard::new(wait)?;

        Ok(RwLockWriteGuard { _active, guard })
    }
}

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
