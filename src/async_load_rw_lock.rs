#![allow(deprecated)]

use std::{fmt, future::Future, mem::replace, ops};
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

#[deprecated(
    note = "unused; use `AsyncOnceCell` or `sync::async_rwlock::RwLock<Option<T>>` instead"
)]
pub struct AsyncLoadRwLock<T>(RwLock<Option<T>>);

impl<T> AsyncLoadRwLock<T> {
    pub const fn new() -> Self {
        Self::with_opt(None)
    }

    pub const fn with_opt(value: Option<T>) -> Self {
        Self(RwLock::const_new(value))
    }

    pub const fn with_val(value: T) -> Self {
        Self::with_opt(Some(value))
    }

    pub fn get_mut(&mut self) -> &mut Option<T> {
        self.0.get_mut()
    }

    pub async fn get_mut_or_init<F>(&mut self, f: F) -> &mut T
    where
        F: Future<Output = T>,
    {
        match self.0.get_mut() {
            Some(v) => v,
            o => o.insert(f.await),
        }
    }

    pub async fn get_mut_or_try_init<F, E>(&mut self, f: F) -> Result<&mut T, E>
    where
        F: Future<Output = Result<T, E>>,
    {
        Ok(match self.0.get_mut() {
            Some(v) => v,
            o => o.insert(f.await?),
        })
    }

    pub async fn read_or_init<F>(&self, f: F) -> AsyncLoadRwLockReadGuard<'_, T>
    where
        F: Future<Output = T>,
    {
        if let Some(guard) = self.try_read_loaded().await {
            return guard;
        }

        self.write_or_init(f).await.downgrade()
    }

    pub async fn read_or_try_init<F, E>(&self, f: F) -> Result<AsyncLoadRwLockReadGuard<'_, T>, E>
    where
        F: Future<Output = Result<T, E>>,
    {
        if let Some(guard) = self.try_read_loaded().await {
            return Ok(guard);
        }

        Ok(self.write_or_try_init(f).await?.downgrade())
    }

    /// Returns a read guard if the value is already loaded, without initializing it.
    async fn try_read_loaded(&self) -> Option<AsyncLoadRwLockReadGuard<'_, T>> {
        let guard = match self.0.try_read() {
            Ok(guard) => guard,
            Err(_) => self.0.read().await,
        };

        RwLockReadGuard::try_map(guard, Option::as_ref)
            .ok()
            .map(AsyncLoadRwLockReadGuard)
    }

    pub fn swap(&mut self, value: Option<T>) -> Option<T> {
        replace(self.0.get_mut(), value)
    }

    pub async fn write_or_init<F>(&self, f: F) -> AsyncLoadRwLockWriteGuard<'_, T>
    where
        F: Future<Output = T>,
    {
        let mut guard = self.0.write().await;

        if guard.is_none() {
            *guard = Some(f.await);
        }

        AsyncLoadRwLockWriteGuard(guard)
    }

    pub async fn write_or_try_init<F, E>(&self, f: F) -> Result<AsyncLoadRwLockWriteGuard<'_, T>, E>
    where
        F: Future<Output = Result<T, E>>,
    {
        let mut guard = self.0.write().await;

        if guard.is_none() {
            *guard = Some(f.await?);
        }

        Ok(AsyncLoadRwLockWriteGuard(guard))
    }
}

impl<T> Default for AsyncLoadRwLock<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[deprecated(
    note = "unused; use `AsyncOnceCell` or `sync::async_rwlock::RwLock<Option<T>>` instead"
)]
pub struct AsyncLoadRwLockReadGuard<'a, T>(RwLockReadGuard<'a, T>);

impl<T> fmt::Debug for AsyncLoadRwLockReadGuard<'_, T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T> fmt::Display for AsyncLoadRwLockReadGuard<'_, T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T> ops::Deref for AsyncLoadRwLockReadGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.0
    }
}

#[deprecated(
    note = "unused; use `AsyncOnceCell` or `sync::async_rwlock::RwLock<Option<T>>` instead"
)]
pub struct AsyncLoadRwLockWriteGuard<'a, T>(RwLockWriteGuard<'a, Option<T>>);

impl<'a, T> AsyncLoadRwLockWriteGuard<'a, T> {
    pub fn downgrade(self) -> AsyncLoadRwLockReadGuard<'a, T> {
        AsyncLoadRwLockReadGuard(RwLockReadGuard::map(self.0.downgrade(), |o| {
            o.as_ref()
                .expect("AsyncLoadRwLockWriteGuard always holds a value")
        }))
    }
}

impl<T> fmt::Debug for AsyncLoadRwLockWriteGuard<'_, T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T> fmt::Display for AsyncLoadRwLockWriteGuard<'_, T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T> ops::Deref for AsyncLoadRwLockWriteGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.0
            .as_ref()
            .expect("AsyncLoadRwLockWriteGuard always holds a value")
    }
}

impl<T> ops::DerefMut for AsyncLoadRwLockWriteGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.0
            .as_mut()
            .expect("AsyncLoadRwLockWriteGuard always holds a value")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test(flavor = "multi_thread")]
    async fn read_or_init_runs_initializer_once() {
        let lock = std::sync::Arc::new(AsyncLoadRwLock::new());
        let calls = std::sync::Arc::new(AtomicUsize::new(0));

        let tasks = (0..16)
            .map(|_| {
                let lock = lock.clone();
                let calls = calls.clone();
                tokio::spawn(async move {
                    *lock
                        .read_or_init(async {
                            calls.fetch_add(1, Ordering::SeqCst);
                            42
                        })
                        .await
                })
            })
            .collect::<Vec<_>>();

        for t in tasks {
            assert_eq!(t.await.unwrap(), 42);
        }

        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn try_init_error_leaves_value_unloaded() {
        let lock = AsyncLoadRwLock::<u32>::new();

        assert!(lock.read_or_try_init(async { Err(()) }).await.is_err());
        assert_eq!(
            *lock
                .read_or_try_init(async { Ok::<_, ()>(1) })
                .await
                .unwrap(),
            1
        );
        assert_eq!(*lock.read_or_init(async { 2 }).await, 1);
    }

    #[tokio::test]
    async fn write_guard_mutates_and_downgrades() {
        let lock = AsyncLoadRwLock::with_val(1);
        let mut g = lock.write_or_init(async { unreachable!() }).await;
        *g += 1;
        assert_eq!(*g.downgrade(), 2);
    }

    #[tokio::test]
    async fn get_mut_or_init_and_swap() {
        let mut lock = AsyncLoadRwLock::new();
        assert_eq!(*lock.get_mut_or_init(async { 1 }).await, 1);
        assert_eq!(
            *lock
                .get_mut_or_try_init(async { Err::<i32, ()>(()) })
                .await
                .unwrap(),
            1
        );
        assert_eq!(lock.swap(None), Some(1));
        assert!(
            lock.get_mut_or_try_init(async { Err::<i32, ()>(()) })
                .await
                .is_err()
        );
        assert_eq!(lock.get_mut(), &None);
    }
}
