use crate::{
    deadlock::{DLDetector, DLGuard},
    wait_lock_guard::WaitLockGuard,
    ActiveLockGuard, Error,
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
        let deadlock = self.detector.lock()?;
        let wait = WaitLockGuard::new("queue");
        let mutex = self.mutex.lock().await;
        let read = self.rwlock.read().await;

        Ok(QueueRwLockQueueGuard {
            active: ActiveLockGuard::new(wait),
            deadlock,
            mutex,
            queue: self,
            read,
        })
    }

    /// Locks this `RwLock` with shared read access
    pub async fn read(&self) -> Result<QueueRwLockReadGuard<'_, T>, Error> {
        let deadlock = self.detector.lock()?;
        let wait = WaitLockGuard::new("read");
        let read = self.rwlock.read().await;

        Ok(QueueRwLockReadGuard {
            active: ActiveLockGuard::new(wait),
            deadlock,
            queue: self,
            read,
        })
    }

    /// Attempts to acquire the queue, and returns `None` if any
    /// somewhere else is in the queue.
    pub fn try_queue(&self) -> Option<QueueRwLockQueueGuard<'_, T>> {
        let deadlock = self.detector.lock().ok()?;
        let wait = WaitLockGuard::new("queue");

        // mutex must be locked first, before the read.
        let mutex = self.mutex.try_lock().ok()?;
        let read = self.rwlock.try_read().ok()?;

        Some(QueueRwLockQueueGuard {
            active: ActiveLockGuard::new(wait),
            deadlock,
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
    active: ActiveLockGuard,
    deadlock: DLGuard,
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
        drop(self.deadlock);

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
    active: ActiveLockGuard,
    deadlock: DLGuard,
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

        let deadlock = self.deadlock;
        let queue = self.queue;
        let wait = WaitLockGuard::new("write");
        let write = queue.rwlock.write().await;

        // emphasis here that the mutex must be dropped after the write.
        drop(self.mutex);

        Ok(QueueRwLockWriteGuard {
            active: ActiveLockGuard::new(wait),
            deadlock,
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
    active: ActiveLockGuard,
    deadlock: DLGuard,
    queue: &'a QueueRwLock<T>,
    write: RwLockWriteGuard<'a, T>,
}

impl<'a, T> QueueRwLockWriteGuard<'a, T> {
    pub async fn read(self) -> Result<QueueRwLockReadGuard<'a, T>, Error> {
        // drop the write lock before trying to acquire the read.
        drop(self.write);
        drop(self.deadlock);
        drop(self.active);

        self.queue.read().await
    }

    pub async fn queue(self) -> Result<QueueRwLockQueueGuard<'a, T>, Error> {
        // drop the write lock before trying to acquire the queue.
        drop(self.write);
        drop(self.deadlock);
        drop(self.active);

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
    use crate::deadlock::lock_held;

    crate::with_deadlock_check(async move {
        let lock = QueueRwLock::new(());
        let q = lock.queue().await?;

        assert!(lock_held().unwrap());

        // Cannot queue or read again inside the same task.
        assert!(lock.queue().await.is_err());
        assert!(lock.read().await.is_err());

        let w = q.write().await?;

        assert!(lock_held().unwrap());

        // No queue or read under write
        assert!(lock.queue().await.is_err());
        assert!(lock.read().await.is_err());

        drop(w);

        assert!(!lock_held().unwrap());

        assert!(lock.queue().await.is_ok());

        assert!(!lock_held().unwrap());

        let _v = lock.read().await.unwrap();

        assert!(lock_held().unwrap());

        // can read many time inside the same task.
        assert!(lock.read().await.is_err());

        Ok(())
    })
    .await
}

#[cfg(test)]
#[tokio::test]
async fn should_error_if_run_without_deadlock_check() {
    use crate::deadlock::lock_held;

    let lock = QueueRwLock::new(());

    assert_eq!(
        lock.queue().await.unwrap_err(),
        Error::NotDeadlockCheckFuture
    );

    assert_eq!(
        lock.read().await.unwrap_err(),
        Error::NotDeadlockCheckFuture
    );

    assert_eq!(lock_held().unwrap_err(), Error::NotDeadlockCheckFuture);
}
