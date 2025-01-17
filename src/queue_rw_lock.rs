use crate::{
    primitives::{LockAwaitGuard, LockData, LockHeldGuard},
    Error,
};
use std::{
    fmt::{self, Debug, Display, Formatter},
    ops::{Deref, DerefMut},
    time::Duration,
};
use tokio::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub struct QueueRwLock<T> {
    lock_data: LockData,
    mutex: Mutex<()>,
    rwlock: RwLock<T>,
}

impl<T> QueueRwLock<T> {
    /// Creates a new instance of an `QueueRwLock<T>` which is unlocked.
    pub fn new(val: T, lock_name: &'static str) -> Self {
        Self {
            lock_data: LockData::new(lock_name),
            mutex: Default::default(),
            rwlock: RwLock::new(val),
        }
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this call borrows the `RwLock` mutably, no actual locking needs to
    /// take place---the mutable borrow statically guarantees no locks exist.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.rwlock.get_mut()
    }

    /// Consumes this `RwLock`, returning the underlying data.
    pub fn into_inner(self) -> T {
        self.rwlock.into_inner()
    }

    /// Enqueue to gain access to the write.
    pub async fn queue(&self) -> Result<QueueRwLockQueueGuard<'_, T>, Error> {
        if let Ok(mutex) = self.mutex.try_lock() {
            if let Ok(read) = self.rwlock.try_read() {
                return Ok(QueueRwLockQueueGuard {
                    active: LockHeldGuard::new_no_wait(&self.lock_data, "queue")?,
                    mutex,
                    queue: self,
                    read,
                });
            }
        }

        let wait = LockAwaitGuard::new(&self.lock_data, "queue")?;
        let mutex = self.mutex.lock().await;
        let read = self.rwlock.read().await;

        Ok(QueueRwLockQueueGuard {
            active: LockHeldGuard::new(wait)?,
            mutex,
            queue: self,
            read,
        })
    }

    /// Locks this `RwLock` with shared read access
    pub async fn read(&self) -> Result<QueueRwLockReadGuard<'_, T>, Error> {
        if let Ok(read) = self.rwlock.try_read() {
            return Ok(QueueRwLockReadGuard {
                active: LockHeldGuard::new_no_wait(&self.lock_data, "read")?,
                queue: self,
                read,
            });
        }

        let wait = LockAwaitGuard::new(&self.lock_data, "read")?;
        let read = self.rwlock.read().await;

        Ok(QueueRwLockReadGuard {
            active: LockHeldGuard::new(wait)?,
            queue: self,
            read,
        })
    }

    /// Attempts to acquire the queue, and returns `None` if any
    /// somewhere else is in the queue.
    pub fn try_queue(&self) -> Option<QueueRwLockQueueGuard<'_, T>> {
        // mutex must be locked first, before the read.
        let mutex = self.mutex.try_lock().ok()?;
        let read = self.rwlock.try_read().ok()?;
        let active = LockHeldGuard::new_no_wait(&self.lock_data, "queue").ok()?;

        Some(QueueRwLockQueueGuard {
            active,
            mutex,
            queue: self,
            read,
        })
    }
}

impl<T: Default> Default for QueueRwLock<T> {
    fn default() -> Self {
        QueueRwLock::new(T::default(), stringify!(QueueRwLock<T>))
    }
}

pub struct QueueRwLockReadGuard<'a, T> {
    active: LockHeldGuard<'a>,
    queue: &'a QueueRwLock<T>,
    read: RwLockReadGuard<'a, T>,
}

impl<'a, T> QueueRwLockReadGuard<'a, T> {
    pub fn elapsed(&self) -> Duration {
        self.active.elapsed()
    }

    pub async fn queue(self) -> Result<QueueRwLockQueueGuard<'a, T>, Error> {
        drop(self.active);
        drop(self.read);

        self.queue.queue().await
    }
}

impl<T> Debug for QueueRwLockReadGuard<'_, T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.read.deref().fmt(f)
    }
}

impl<T> Deref for QueueRwLockReadGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.read
    }
}

impl<T> Display for QueueRwLockReadGuard<'_, T>
where
    T: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.read.deref().fmt(f)
    }
}

/// A ticket to obtain a write access to the RwLock.
///
/// While having this guard, you can prepare and do the hard work before
/// obtaining the write access to the RwLock. This makes sure that the
/// RwLock will be held exclusively as short as possible.
pub struct QueueRwLockQueueGuard<'a, T> {
    active: LockHeldGuard<'a>,
    mutex: MutexGuard<'a, ()>,
    queue: &'a QueueRwLock<T>,
    read: RwLockReadGuard<'a, T>,
}

impl<'a, T> QueueRwLockQueueGuard<'a, T> {
    pub fn elapsed(&self) -> Duration {
        self.active.elapsed()
    }

    /// Locks this `RwLock` with exclusive write access, blocking the current
    /// thread until it can be acquired.
    ///
    /// This function will not return while other writers or other readers
    /// currently have access to the lock.
    ///
    /// This will also release the queue so another potential writer will get access.
    pub async fn write(self) -> Result<QueueRwLockWriteGuard<'a, T>, Error> {
        // the read lock must be dropped before trying to acquire write lock.
        drop(self.active);
        drop(self.read);

        let queue = self.queue;

        if let Ok(write) = queue.rwlock.try_write() {
            // emphasis here that the mutex must be dropped after the write.
            drop(self.mutex);

            return Ok(QueueRwLockWriteGuard {
                active: LockHeldGuard::new_no_wait(&queue.lock_data, "write")?,
                queue,
                write,
            });
        }

        let wait = LockAwaitGuard::new(&queue.lock_data, "write")?;
        let write = queue.rwlock.write().await;

        // emphasis here that the mutex must be dropped after the write.
        drop(self.mutex);

        Ok(QueueRwLockWriteGuard {
            active: LockHeldGuard::new(wait)?,
            queue,
            write,
        })
    }
}

impl<T> Debug for QueueRwLockQueueGuard<'_, T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.read.deref().fmt(f)
    }
}

impl<T> Deref for QueueRwLockQueueGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.read
    }
}

impl<T> Display for QueueRwLockQueueGuard<'_, T>
where
    T: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.read.deref().fmt(f)
    }
}

pub struct QueueRwLockWriteGuard<'a, T> {
    active: LockHeldGuard<'a>,
    queue: &'a QueueRwLock<T>,
    write: RwLockWriteGuard<'a, T>,
}

impl<'a, T> QueueRwLockWriteGuard<'a, T> {
    pub async fn read(self) -> Result<QueueRwLockReadGuard<'a, T>, Error> {
        // drop the write lock before trying to acquire the read.
        drop(self.write);
        drop(self.active);

        self.queue.read().await
    }

    pub async fn queue(self) -> Result<QueueRwLockQueueGuard<'a, T>, Error> {
        // drop the write lock before trying to acquire the queue.
        drop(self.write);
        drop(self.active);

        self.queue.queue().await
    }
}

impl<T, U> AsMut<U> for QueueRwLockWriteGuard<'_, T>
where
    T: AsMut<U>,
{
    #[inline]
    fn as_mut(&mut self) -> &mut U {
        self.write.as_mut()
    }
}

impl<T> Debug for QueueRwLockWriteGuard<'_, T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.write.deref().fmt(f)
    }
}

impl<T> Deref for QueueRwLockWriteGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.write
    }
}

impl<T> DerefMut for QueueRwLockWriteGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.write
    }
}

impl<T> Display for QueueRwLockWriteGuard<'_, T>
where
    T: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.write.deref().fmt(f)
    }
}

#[cfg(test)]
#[tokio::test]
async fn check_deadlock() -> Result<(), Error> {
    use crate::primitives::locks_held::has_lock_held;

    crate::with_deadlock_check(
        async move {
            let lock = QueueRwLock::new((), "main_lock");
            let q = lock.queue().await?;

            assert!(has_lock_held());

            // Cannot queue or read again inside the same task.
            assert!(lock.queue().await.is_err());
            assert!(lock.read().await.is_ok());

            let w = q.write().await?;

            assert!(has_lock_held());

            // No queue or read under write
            assert!(lock.queue().await.is_err());
            assert!(lock.read().await.is_err());

            drop(w);

            assert!(!has_lock_held());

            assert!(lock.queue().await.is_ok());

            assert!(!has_lock_held());

            let _v = lock.read().await.unwrap();

            assert!(has_lock_held());

            // can read many time inside the same task.
            assert!(lock.read().await.is_ok());

            Ok(())
        },
        "lock_test".into(),
    )
    .await
}

#[cfg(test)]
#[tokio::test]
async fn should_error_if_run_without_deadlock_check() {
    use crate::primitives::locks_held::has_lock_held;

    let lock = QueueRwLock::new((), "main_lock");

    assert_eq!(
        lock.queue().await.unwrap_err(),
        Error::NotDeadlockCheckFuture
    );

    assert_eq!(
        lock.read().await.unwrap_err(),
        Error::NotDeadlockCheckFuture
    );

    assert!(!has_lock_held());
}
