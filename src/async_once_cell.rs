use crate::{
    Error,
    primitives::{LockAwaitGuard, LockData, LockHeldGuard, Ops},
};
use once_cell::sync::OnceCell;
use std::{convert::Infallible, future::Future, mem};
use tokio::sync::{Mutex, MutexGuard};

/// A cell that is initialized only once, by either an async or a sync initializer.
///
/// Reading an initialized value is a single atomic load. Concurrent async initializers are
/// serialized so the initializer runs once. Inside a deadlock-check scope the initialization
/// lock is tracked by the deadlock detector; on a recursive or deadlocking initialization, the
/// cell falls back to an unsynchronized initialization (first value stored wins) instead of
/// hanging.
pub struct AsyncOnceCell<T> {
    cell: OnceCell<T>,
    init_lock: Mutex<()>,
    lock_data: LockData,
}

impl<T> AsyncOnceCell<T> {
    pub const fn new() -> Self {
        Self::from_cell(OnceCell::new())
    }

    pub const fn with_val(val: T) -> Self {
        Self::from_cell(OnceCell::with_value(val))
    }

    const fn from_cell(cell: OnceCell<T>) -> Self {
        Self {
            cell,
            init_lock: Mutex::const_new(()),
            lock_data: LockData::new("async-once-cell"),
        }
    }

    #[inline]
    pub fn get(&self) -> Option<&T> {
        self.cell.get()
    }

    #[inline]
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.cell.get_mut()
    }

    pub async fn get_or_init<F>(&self, f: F) -> &T
    where
        F: Future<Output = T>,
    {
        if let Some(v) = self.cell.get() {
            return v;
        }

        let Ok(v) = self.init(async { Ok::<_, Infallible>(f.await) }).await;
        v
    }

    pub fn get_or_init_sync<F: FnOnce() -> T>(&self, f: F) -> &T {
        self.cell.get_or_init(f)
    }

    pub async fn get_or_try_init<F, E>(&self, f: F) -> Result<&T, E>
    where
        F: Future<Output = Result<T, E>>,
    {
        if let Some(v) = self.cell.get() {
            return Ok(v);
        }

        self.init(f).await
    }

    pub fn get_or_try_init_sync<E, F: FnOnce() -> Result<T, E>>(&self, f: F) -> Result<&T, E> {
        self.cell.get_or_try_init(f)
    }

    pub fn into_inner(self) -> Option<T> {
        self.cell.into_inner()
    }

    pub fn swap(&mut self, value: Option<T>) -> Option<T> {
        let new = value.map_or_else(OnceCell::new, OnceCell::with_value);
        mem::replace(&mut self.cell, new).into_inner()
    }

    pub fn take(&mut self) -> Option<T> {
        self.cell.take()
    }

    async fn init<F, E>(&self, f: F) -> Result<&T, E>
    where
        F: Future<Output = Result<T, E>>,
    {
        let _guard = self.lock_init().await;

        if let Some(v) = self.cell.get() {
            return Ok(v);
        }

        let v = f.await?;

        // A sync initializer may have won the race; keep its value.
        Ok(self.cell.get_or_init(|| v))
    }

    async fn lock_init(&self) -> Option<InitGuard<'_>> {
        match LockAwaitGuard::new(&self.lock_data, Ops::Write) {
            Ok(wait) => {
                let guard = self.init_lock.lock().await;

                Some(InitGuard {
                    _held: LockHeldGuard::new(wait).ok(),
                    _guard: guard,
                })
            }
            Err(Error::NotDeadlockCheckFuture) => Some(InitGuard {
                _held: None,
                _guard: self.init_lock.lock().await,
            }),
            // Recursive or deadlocking initialization: do not wait.
            Err(_) => None,
        }
    }
}

impl<T> Default for AsyncOnceCell<T> {
    fn default() -> Self {
        Self::new()
    }
}

struct InitGuard<'a> {
    // Declared first so the lock is reported released before the mutex is unlocked.
    _held: Option<LockHeldGuard<'a>>,
    _guard: MutexGuard<'a, ()>,
}

#[cfg(test)]
mod tests {
    use super::AsyncOnceCell;
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering::SeqCst},
        },
        time::Duration,
    };

    async fn concurrent_init_runs_once(in_scope: bool) {
        let cell = Arc::new(AsyncOnceCell::new());
        let calls = Arc::new(AtomicUsize::new(0));

        let handles = (0..8)
            .map(|_| {
                let cell = Arc::clone(&cell);
                let calls = Arc::clone(&calls);

                let f = async move {
                    *cell
                        .get_or_init(async {
                            calls.fetch_add(1, SeqCst);
                            tokio::time::sleep(Duration::from_millis(10)).await;
                            42
                        })
                        .await
                };

                if in_scope {
                    crate::spawn_with_deadlock_check(f, "init")
                } else {
                    tokio::spawn(f)
                }
            })
            .collect::<Vec<_>>();

        for h in handles {
            assert_eq!(h.await.unwrap(), 42);
        }

        assert_eq!(calls.load(SeqCst), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn concurrent_init_runs_once_in_scope() {
        concurrent_init_runs_once(true).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn concurrent_init_runs_once_outside_scope() {
        concurrent_init_runs_once(false).await;
    }

    #[crate::tokio_test]
    async fn try_init_error_does_not_initialize() {
        let cell = AsyncOnceCell::new();

        assert_eq!(cell.get_or_try_init(async { Err("err") }).await, Err("err"));
        assert!(cell.get().is_none());
        assert_eq!(cell.get_or_try_init(async { Ok::<_, ()>(1) }).await, Ok(&1));
        assert_eq!(cell.get_or_try_init(async { Ok::<_, ()>(2) }).await, Ok(&1));
    }

    #[crate::tokio_test]
    async fn recursive_init_does_not_hang() {
        let cell = AsyncOnceCell::new();

        let v = cell
            .get_or_init(async { *Box::pin(cell.get_or_init(async { 1 })).await + 1 })
            .await;

        // The inner initializer wins; the outer value is discarded.
        assert_eq!(*v, 1);
    }

    #[test]
    fn swap_and_take() {
        let mut cell = AsyncOnceCell::with_val(1);

        assert_eq!(cell.swap(Some(2)), Some(1));
        assert_eq!(cell.get(), Some(&2));
        assert_eq!(cell.swap(None), Some(2));
        assert_eq!(cell.get(), None);
        assert_eq!(cell.get_or_init_sync(|| 3), &3);
        assert_eq!(cell.take(), Some(3));
        assert_eq!(cell.into_inner(), None);
    }
}
