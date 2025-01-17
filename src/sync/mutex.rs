use crate::{
    primitives::{LockAwaitGuard, LockData, LockHeldGuard},
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
                _active: LockHeldGuard::new_no_wait(&self.lock_data, "sync_lock")?,
                guard,
            });
        }

        let wait = LockAwaitGuard::new(&self.lock_data, "sync_lock")?;

        match self.mutex.try_lock_for(Duration::from_millis(250)) {
            Some(guard) => Ok(MutexGuard {
                _active: LockHeldGuard::new(wait)?,
                guard,
            }),
            None => Err(Error::SyncLockForTooLong),
        }
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
