use crate::{
    is_async,
    primitives::{LockAwaitGuard, LockData, LockHeldGuard, Ops},
    Error, Result,
};
use std::{
    ops::{Deref, DerefMut},
    time::Duration,
};

pub struct Mutex<T> {
    lock_data: LockData,
    mutex: parking_lot::Mutex<T>,
}

impl<T> Mutex<T> {
    pub const fn new(value: T, name: &'static str) -> Self {
        Self {
            lock_data: LockData::new(name),
            mutex: parking_lot::Mutex::new(value),
        }
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.mutex.get_mut()
    }

    pub fn into_inner(self) -> T {
        self.mutex.into_inner()
    }

    pub fn lock(&self) -> Result<MutexGuard<'_, T>> {
        if let Some(guard) = self.mutex.try_lock() {
            return Ok(MutexGuard {
                _active: LockHeldGuard::new_no_wait(&self.lock_data, Ops::Write)?,
                guard,
            });
        }

        let wait = LockAwaitGuard::new(&self.lock_data, Ops::Write)?;

        let guard = if is_async() {
            match self.mutex.try_lock_for(Duration::from_millis(50)) {
                Some(guard) => guard,
                None => return Err(Error::sync_lock_timeout(&self.lock_data, Ops::Write)),
            }
        } else {
            self.mutex.lock()
        };

        Ok(MutexGuard {
            _active: LockHeldGuard::new(wait)?,
            guard,
        })
    }
}

pub struct MutexGuard<'a, T> {
    _active: LockHeldGuard<'a>,
    guard: parking_lot::MutexGuard<'a, T>,
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
    // even if the test is invalid, we are testing that no deadlock occurs here
    // when used inside async.

    crate::with_deadlock_check(
        async move {
            let mutex = std::sync::Arc::new(Mutex::new((), "test"));
            let spawn_mutex = mutex.clone();
            let g = mutex.lock().unwrap();

            let j = crate::spawn_with_deadlock_check(
                async move {
                    let _ = spawn_mutex.lock();
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

            let _g = mutex.lock().unwrap();

            assert!(mutex.lock().is_err());
        },
        "test",
    )
    .await
}
