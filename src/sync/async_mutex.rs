use crate::{
    primitives::{LockAwaitGuard, LockData, LockHeldGuard},
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
                _active: LockHeldGuard::new_no_wait(&self.lock_data, "lock")?,
                guard,
            });
        }

        let wait = LockAwaitGuard::new(&self.lock_data, "lock")?;
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
