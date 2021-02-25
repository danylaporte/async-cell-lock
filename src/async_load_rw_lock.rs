use std::{fmt, future::Future, mem::replace, ops};
pub use tokio::sync::RwLockWriteGuard;
use tokio::sync::{RwLock, RwLockReadGuard};

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
        let o = self.0.get_mut();

        if o.is_none() {
            let v = f.await;
            *o = Some(v);
        }

        o.as_mut().unwrap()
    }

    pub async fn get_mut_or_try_init<F, E>(&mut self, f: F) -> Result<&mut T, E>
    where
        F: Future<Output = Result<T, E>>,
    {
        let o = self.0.get_mut();

        if o.is_none() {
            let v = f.await?;
            *o = Some(v);
        }

        Ok(o.as_mut().unwrap())
    }

    pub async fn read_or_init<F>(&self, f: F) -> AsyncLoadRwLockReadGuard<'_, T>
    where
        F: Future<Output = T>,
    {
        {
            let guard = self.0.read().await;

            if guard.is_some() {
                return AsyncLoadRwLockReadGuard(guard);
            }
        }

        self.write_or_init(f).await.downgrade()
    }

    pub async fn read_or_try_init<F, E>(&self, f: F) -> Result<AsyncLoadRwLockReadGuard<'_, T>, E>
    where
        F: Future<Output = Result<T, E>>,
    {
        {
            let guard = self.0.read().await;

            if guard.is_some() {
                return Ok(AsyncLoadRwLockReadGuard(guard));
            }
        }

        Ok(self.write_or_try_init(f).await?.downgrade())
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

pub struct AsyncLoadRwLockReadGuard<'a, T>(RwLockReadGuard<'a, Option<T>>);

impl<'a, T> fmt::Debug for AsyncLoadRwLockReadGuard<'a, T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T> fmt::Display for AsyncLoadRwLockReadGuard<'a, T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T> ops::Deref for AsyncLoadRwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0.as_ref().unwrap()
    }
}

pub struct AsyncLoadRwLockWriteGuard<'a, T>(RwLockWriteGuard<'a, Option<T>>);

impl<'a, T> AsyncLoadRwLockWriteGuard<'a, T> {
    pub fn downgrade(self) -> AsyncLoadRwLockReadGuard<'a, T> {
        AsyncLoadRwLockReadGuard(self.0.downgrade())
    }
}

impl<'a, T> fmt::Debug for AsyncLoadRwLockWriteGuard<'a, T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T> fmt::Display for AsyncLoadRwLockWriteGuard<'a, T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T> ops::Deref for AsyncLoadRwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0.as_ref().unwrap()
    }
}

impl<T> ops::DerefMut for AsyncLoadRwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.0.as_mut().unwrap()
    }
}
