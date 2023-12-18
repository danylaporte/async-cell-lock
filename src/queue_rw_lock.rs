use crate::{
    deadlock::{DLDetector, DLGuard},
    Error, LongLock,
};
use std::{
    fmt::{self, Debug, Display, Formatter},
    ops::{Deref, DerefMut},
    time::Duration,
};
use tokio::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub struct QueueRwLock<T> {
    detector: DLDetector,
    mutex: Mutex<()>,
    rwlock: RwLock<T>,
}

impl<T> QueueRwLock<T> {
    /// Creates a new instance of an `QueueRwLock<T>` which is unlocked.
    pub fn new(val: T) -> Self {
        Self {
            detector: DLDetector,
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
        let deadlock = self.detector.write()?;

        // mutex must be locked first, before the read.
        let mutex = self.mutex.lock().await;
        let read = self.rwlock.read().await;

        Ok(QueueRwLockQueueGuard {
            deadlock,
            longlock: LongLock::new(),
            mutex,
            queue: self,
            read,
        })
    }

    /// Locks this `RwLock` with shared read access
    pub async fn read(&self) -> Result<QueueRwLockReadGuard<'_, T>, Error> {
        let deadlock = self.detector.read()?;
        let read = self.rwlock.read().await;

        Ok(QueueRwLockReadGuard {
            deadlock,
            longlock: LongLock::new(),
            queue: self,
            read,
        })
    }

    /// Attempts to acquire the queue, and returns `None` if any
    /// somewhere else is in the queue.
    pub fn try_queue(&self) -> Option<QueueRwLockQueueGuard<'_, T>> {
        let deadlock = self.detector.write().ok()?;

        // mutex must be locked first, before the read.
        let mutex = self.mutex.try_lock().ok()?;
        let read = self.rwlock.try_read().ok()?;

        Some(QueueRwLockQueueGuard {
            deadlock,
            longlock: LongLock::new(),
            mutex,
            queue: self,
            read,
        })
    }
}

impl<T: Default> Default for QueueRwLock<T> {
    fn default() -> Self {
        QueueRwLock::new(T::default())
    }
}

pub struct QueueRwLockReadGuard<'a, T> {
    deadlock: DLGuard,
    longlock: LongLock,
    queue: &'a QueueRwLock<T>,
    read: RwLockReadGuard<'a, T>,
}

impl<'a, T> QueueRwLockReadGuard<'a, T> {
    pub async fn queue(self) -> Result<QueueRwLockQueueGuard<'a, T>, Error> {
        drop(self.read);
        drop(self.deadlock);
        drop(self.longlock);

        self.queue.queue().await
    }
}

impl<'a, T> Debug for QueueRwLockReadGuard<'a, T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.read.deref().fmt(f)
    }
}

impl<'a, T> Deref for QueueRwLockReadGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.read
    }
}

impl<'a, T> Display for QueueRwLockReadGuard<'a, T>
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
    deadlock: DLGuard,
    longlock: LongLock,
    mutex: MutexGuard<'a, ()>,
    queue: &'a QueueRwLock<T>,
    read: RwLockReadGuard<'a, T>,
}

impl<'a, T> QueueRwLockQueueGuard<'a, T> {
    pub fn elapsed(&self) -> Duration {
        self.longlock.elapsed()
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
        drop(self.read);

        let deadlock = self.deadlock;
        let queue = self.queue;
        let write = queue.rwlock.write().await;

        // emphasis here that the mutex must be dropped after the write.
        drop(self.mutex);
        drop(self.longlock);

        Ok(QueueRwLockWriteGuard {
            deadlock,
            longlock: LongLock::new(),
            queue,
            write,
        })
    }
}

impl<'a, T> Debug for QueueRwLockQueueGuard<'a, T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.read.deref().fmt(f)
    }
}

impl<'a, T> Deref for QueueRwLockQueueGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.read
    }
}

impl<'a, T> Display for QueueRwLockQueueGuard<'a, T>
where
    T: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.read.deref().fmt(f)
    }
}

pub struct QueueRwLockWriteGuard<'a, T> {
    deadlock: DLGuard,
    longlock: LongLock,
    queue: &'a QueueRwLock<T>,
    write: RwLockWriteGuard<'a, T>,
}

impl<'a, T> QueueRwLockWriteGuard<'a, T> {
    pub async fn read(self) -> Result<QueueRwLockReadGuard<'a, T>, Error> {
        // drop the write lock before trying to acquire the read.
        drop(self.write);
        drop(self.deadlock);
        drop(self.longlock);

        self.queue.read().await
    }

    pub async fn queue(self) -> Result<QueueRwLockQueueGuard<'a, T>, Error> {
        // drop the write lock before trying to acquire the queue.
        drop(self.write);
        drop(self.deadlock);
        drop(self.longlock);

        self.queue.queue().await
    }
}

impl<'a, T, U> AsMut<U> for QueueRwLockWriteGuard<'a, T>
where
    T: AsMut<U>,
{
    #[inline]
    fn as_mut(&mut self) -> &mut U {
        self.write.as_mut()
    }
}

impl<'a, T> Debug for QueueRwLockWriteGuard<'a, T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.write.deref().fmt(f)
    }
}

impl<'a, T> Deref for QueueRwLockWriteGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.write
    }
}

impl<'a, T> DerefMut for QueueRwLockWriteGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.write
    }
}

impl<'a, T> Display for QueueRwLockWriteGuard<'a, T>
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
    use crate::deadlock::lock_held_count;

    crate::with_deadlock_check(async move {
        let lock = QueueRwLock::new(());

        let q = lock.queue().await?;

        assert_eq!(lock_held_count().unwrap(), 1);

        // Cannot queue or read again inside the same task.
        assert!(lock.queue().await.is_err());
        assert!(lock.read().await.is_err());

        let w = q.write().await?;

        assert_eq!(lock_held_count().unwrap(), 1);

        // No queue or read under write
        assert!(lock.queue().await.is_err());
        assert!(lock.read().await.is_err());

        drop(w);

        assert_eq!(lock_held_count().unwrap(), 0);

        assert!(lock.queue().await.is_ok());

        assert_eq!(lock_held_count().unwrap(), 0);

        let l1 = lock.read().await.unwrap();

        assert_eq!(lock_held_count().unwrap(), 1);

        // can read many time inside the same task.
        let l2 = lock.read().await.unwrap();
        assert_eq!(lock_held_count().unwrap(), 2);

        drop(l1);
        assert_eq!(lock_held_count().unwrap(), 1);

        drop(l2);
        assert_eq!(lock_held_count().unwrap(), 0);

        Ok(())
    })
    .await
}

#[cfg(test)]
#[tokio::test]
async fn should_error_if_run_without_deadlock_check() {
    use crate::deadlock::lock_held_count;

    let lock = QueueRwLock::new(());

    assert_eq!(
        lock.queue().await.unwrap_err(),
        Error::NotDeadlockCheckFuture
    );

    assert_eq!(
        lock.read().await.unwrap_err(),
        Error::NotDeadlockCheckFuture
    );

    assert_eq!(
        lock_held_count().unwrap_err(),
        Error::NotDeadlockCheckFuture
    );
}
