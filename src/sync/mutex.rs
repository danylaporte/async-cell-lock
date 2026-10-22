use super::acquire_sync;
use crate::{
    Result,
    primitives::{LockData, LockHeldGuard, Ops},
};
use std::ops::{Deref, DerefMut};

/// This is a sync Mutex based on [parking_lot::Mutex] that should not deadlock
/// even when used inside async context across await point.
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

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.mutex.get_mut()
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.mutex.into_inner()
    }

    pub fn lock(&self) -> Result<MutexGuard<'_, T>> {
        let m = &self.mutex;
        let (_active, guard) = acquire_sync(
            &self.lock_data,
            Ops::Write,
            || m.try_lock(),
            |timeout| m.try_lock_for(timeout),
            || m.lock(),
        )?;

        Ok(MutexGuard { _active, guard })
    }
}

#[must_use = "if unused the Mutex will immediately unlock"]
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
