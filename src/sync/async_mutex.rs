use crate::{
    primitives::{LockAwaitGuard, LockData, LockHeldGuard, Ops},
    Result,
};
use std::ops::{Deref, DerefMut};

pub struct Mutex<T> {
    lock_data: LockData,
    mutex: tokio::sync::Mutex<T>,
}

impl<T> Mutex<T> {
    pub const fn new(value: T, name: &'static str) -> Self {
        Self {
            lock_data: LockData::new(name),
            mutex: tokio::sync::Mutex::const_new(value),
        }
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.mutex.get_mut()
    }

    pub fn into_inner(self) -> T {
        self.mutex.into_inner()
    }

    pub async fn lock(&self) -> Result<MutexGuard<'_, T>> {
        if let Ok(guard) = self.mutex.try_lock() {
            return Ok(MutexGuard {
                _active: LockHeldGuard::new_no_wait(&self.lock_data, Ops::Write)?,
                guard,
            });
        }

        let wait = LockAwaitGuard::new(&self.lock_data, Ops::Write)?;
        let guard = self.mutex.lock().await;
        let _active = LockHeldGuard::new(wait)?;

        Ok(MutexGuard { _active, guard })
    }
}

pub struct MutexGuard<'a, T> {
    _active: LockHeldGuard<'a>,
    guard: tokio::sync::MutexGuard<'a, T>,
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
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
            let rwlock = std::sync::Arc::new(Mutex::new((), "test"));
            let spawn_rwlock = rwlock.clone();
            let g = rwlock.lock().await.unwrap();

            let j = crate::spawn_with_deadlock_check(
                async move {
                    assert!(spawn_rwlock.lock().await.is_ok());
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
            let mutex = Mutex::new((), "test");

            let _g = mutex.lock().await.unwrap();

            assert!(mutex.lock().await.is_err());
        },
        "test",
    )
    .await
}
