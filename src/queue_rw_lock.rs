use tokio::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Default)]
pub struct QueueRwLock<T> {
    lock: RwLock<T>,
    queue: Mutex<()>,
}

impl<T> QueueRwLock<T> {
    /// Creates a new instance of an `QueueRwLock<T>` which is unlocked.
    pub fn new(val: T) -> Self {
        Self {
            lock: RwLock::new(val),
            queue: Default::default(),
        }
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this call borrows the `RwLock` mutably, no actual locking needs to
    /// take place---the mutable borrow statically guarantees no locks exist.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.lock.get_mut()
    }

    /// Consumes this `RwLock`, returning the underlying data.
    #[inline]
    pub fn into_inner(self) -> T {
        self.lock.into_inner()
    }

    /// Enqueue to gain access to the write.
    pub async fn queue(&self) -> QueueWriteGuard<'_, T> {
        QueueWriteGuard {
            _queue: self.queue.lock().await,
            lock: &self.lock,
        }
    }

    /// Attempts to acquire the queue, and returns `None` if any
    /// somewhere else is in the queue.
    pub fn try_queue(&self) -> Option<QueueWriteGuard<'_, T>> {
        Some(QueueWriteGuard {
            _queue: self.queue.try_lock().ok()?,
            lock: &self.lock,
        })
    }

    /// Locks this `RwLock` with shared read access
    #[inline]
    pub async fn read(&self) -> RwLockReadGuard<'_, T> {
        self.lock.read().await
    }
}

/// A ticket to obtain a write access to the RwLock.
///
/// While having this guard, you can prepare and do the hard work before
/// obtaining the write access to the RwLock. This makes sure that the
/// RwLock will be held exclusively as short as possible.
pub struct QueueWriteGuard<'a, T> {
    _queue: MutexGuard<'a, ()>,
    lock: &'a RwLock<T>,
}

impl<'a, T> QueueWriteGuard<'a, T> {
    #[inline]
    pub async fn read(&self) -> RwLockReadGuard<'a, T> {
        self.lock.read().await
    }

    /// Locks this `RwLock` with exclusive write access, blocking the current
    /// thread until it can be acquired.
    ///
    /// This function will not return while other writers or other readers
    /// currently have access to the lock.
    ///
    /// This will also release the queue so another potential writer will get access.
    #[inline]
    pub async fn write(self) -> RwLockWriteGuard<'a, T> {
        self.lock.write().await
    }
}
