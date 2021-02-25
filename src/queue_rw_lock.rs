use std::ops::{Deref, DerefMut};
use tokio::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Default)]
pub struct QueueRwLock<T> {
    mutex: Mutex<()>,
    rwlock: RwLock<T>,
}

impl<T> QueueRwLock<T> {
    /// Creates a new instance of an `QueueRwLock<T>` which is unlocked.
    pub fn new(val: T) -> Self {
        Self {
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
    #[inline]
    pub fn into_inner(self) -> T {
        self.rwlock.into_inner()
    }

    /// Enqueue to gain access to the write.
    pub async fn queue(&self) -> QueueRwLockQueueGuard<'_, T> {
        QueueRwLockQueueGuard {
            // mutex must be locked first, before the read.
            mutex: self.mutex.lock().await,
            queue: self,
            read: self.rwlock.read().await,
        }
    }

    /// Attempts to acquire the queue, and returns `None` if any
    /// somewhere else is in the queue.
    pub async fn try_queue(&self) -> Option<QueueRwLockQueueGuard<'_, T>> {
        Some(QueueRwLockQueueGuard {
            // mutex must be locked first, before the read.
            mutex: self.mutex.try_lock().ok()?,
            queue: self,
            read: self.rwlock.read().await,
        })
    }

    /// Locks this `RwLock` with shared read access
    #[inline]
    pub async fn read(&self) -> QueueRwLockReadGuard<'_, T> {
        QueueRwLockReadGuard {
            queue: self,
            read: self.rwlock.read().await,
        }
    }
}

pub struct QueueRwLockReadGuard<'a, T> {
    queue: &'a QueueRwLock<T>,
    read: RwLockReadGuard<'a, T>,
}

impl<'a, T> QueueRwLockReadGuard<'a, T> {
    pub async fn queue(self) -> QueueRwLockQueueGuard<'a, T> {
        drop(self.read);
        self.queue.queue().await
    }
}

impl<'a, T, U> AsRef<U> for QueueRwLockReadGuard<'a, T>
where
    T: AsRef<U>,
{
    fn as_ref(&self) -> &U {
        self.read.as_ref()
    }
}

impl<'a, T> Deref for QueueRwLockReadGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.read
    }
}

/// A ticket to obtain a write access to the RwLock.
///
/// While having this guard, you can prepare and do the hard work before
/// obtaining the write access to the RwLock. This makes sure that the
/// RwLock will be held exclusively as short as possible.
pub struct QueueRwLockQueueGuard<'a, T> {
    mutex: MutexGuard<'a, ()>,
    queue: &'a QueueRwLock<T>,
    read: RwLockReadGuard<'a, T>,
}

impl<'a, T> QueueRwLockQueueGuard<'a, T> {
    /// Locks this `RwLock` with exclusive write access, blocking the current
    /// thread until it can be acquired.
    ///
    /// This function will not return while other writers or other readers
    /// currently have access to the lock.
    ///
    /// This will also release the queue so another potential writer will get access.
    #[inline]
    pub async fn write(self) -> QueueRwLockWriteGuard<'a, T> {
        // the read lock must be dropped before trying to acquire write lock.
        drop(self.read);

        let queue = self.queue;
        let write = queue.rwlock.write().await;

        // enphasis here that the mutex must be dropped after the write.
        drop(self.mutex);

        QueueRwLockWriteGuard { queue, write }
    }
}

impl<'a, T, U> AsRef<U> for QueueRwLockQueueGuard<'a, T>
where
    T: AsRef<U>,
{
    fn as_ref(&self) -> &U {
        self.read.as_ref()
    }
}

impl<'a, T> Deref for QueueRwLockQueueGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &*self.read
    }
}

pub struct QueueRwLockWriteGuard<'a, T> {
    queue: &'a QueueRwLock<T>,
    write: RwLockWriteGuard<'a, T>,
}

impl<'a, T> QueueRwLockWriteGuard<'a, T> {
    pub async fn read(self) -> QueueRwLockReadGuard<'a, T> {
        // drop the write lock before trying to acquire the read.
        drop(self.write);

        self.queue.read().await
    }
}

impl<'a, T, U> AsMut<U> for QueueRwLockWriteGuard<'a, T>
where
    T: AsMut<U>,
{
    fn as_mut(&mut self) -> &mut U {
        self.write.as_mut()
    }
}

impl<'a, T, U> AsRef<U> for QueueRwLockWriteGuard<'a, T>
where
    T: AsRef<U>,
{
    fn as_ref(&self) -> &U {
        self.write.as_ref()
    }
}

impl<'a, T> Deref for QueueRwLockWriteGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.write
    }
}

impl<'a, T> DerefMut for QueueRwLockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.write
    }
}
