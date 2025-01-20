use crate::{
    is_async,
    primitives::{LockAwaitGuard, LockData, LockHeldGuard, Ops},
    Error, Result,
};
use std::{
    ops::{Deref, DerefMut},
    time::Duration,
};

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

    pub fn get_mut(&mut self) -> &mut T {
        self.rwlock.get_mut()
    }

    pub fn into_inner(self) -> T {
        self.rwlock.into_inner()
    }

    pub fn read(&self) -> Result<RwLockReadGuard<'_, T>> {
        if let Some(guard) = self.rwlock.try_read() {
            return Ok(RwLockReadGuard {
                _active: LockHeldGuard::new_no_wait(&self.lock_data, Ops::Read)?,
                guard,
            });
        }

        let wait = LockAwaitGuard::new(&self.lock_data, Ops::Read)?;

        let guard = if is_async() {
            match self.rwlock.try_read_for(Duration::from_millis(50)) {
                Some(guard) => guard,
                None => return Err(Error::sync_lock_timeout(&self.lock_data, Ops::Read)),
            }
        } else {
            self.rwlock.read()
        };

        Ok(RwLockReadGuard {
            _active: LockHeldGuard::new(wait)?,
            guard,
        })
    }

    pub fn write(&self) -> Result<RwLockWriteGuard<'_, T>> {
        if let Some(guard) = self.rwlock.try_write() {
            return Ok(RwLockWriteGuard {
                _active: LockHeldGuard::new_no_wait(&self.lock_data, Ops::Write)?,
                guard,
            });
        }

        let wait = LockAwaitGuard::new(&self.lock_data, Ops::Write)?;

        let guard = if is_async() {
            match self.rwlock.try_write_for(Duration::from_millis(50)) {
                Some(guard) => guard,
                None => return Err(Error::sync_lock_timeout(&self.lock_data, Ops::Write)),
            }
        } else {
            self.rwlock.write()
        };

        Ok(RwLockWriteGuard {
            _active: LockHeldGuard::new(wait)?,
            guard,
        })
    }
}

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
