use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::Duration;
use tokio::sync::{Mutex, MutexGuard};

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

    /// Locks this `RwLock` with shared read access, blocking the current thread
    /// until it can be acquired.
    ///
    /// The calling thread will be blocked until there are no more writers which
    /// hold the lock. There may be other readers currently inside the lock when
    /// this method returns.
    ///
    /// Note that attempts to recursively acquire a read lock on a `RwLock` when
    /// the current thread already holds one may result in a deadlock.
    ///
    /// Returns an RAII guard which will release this thread's shared access
    /// once it is dropped.
    #[inline]
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        self.lock.read()
    }

    /// Attempts to acquire this `RwLock` with shared read access.
    ///
    /// If the access could not be granted at this time, then `None` is returned.
    /// Otherwise, an RAII guard is returned which will release the shared access
    /// when it is dropped.
    ///
    /// This function does not block.
    #[inline]
    pub fn try_read(&self) -> Option<RwLockReadGuard<'_, T>> {
        self.lock.try_read()
    }

    /// Attempts to acquire this `RwLock` with shared read access until a timeout
    /// is reached.
    ///
    /// If the access could not be granted before the timeout expires, then
    /// `None` is returned. Otherwise, an RAII guard is returned which will
    /// release the shared access when it is dropped.
    #[inline]
    pub fn try_read_for(&self, timeout: Duration) -> Option<RwLockReadGuard<'_, T>> {
        self.lock.try_read_for(timeout)
    }

    /// Attempts to lock this `RwLock` with exclusive write access.
    ///
    /// If the lock could not be acquired at this time, then `None` is returned.
    /// Otherwise, an RAII guard is returned which will release the lock when
    /// it is dropped.
    ///
    /// This function does not block.
    #[inline]
    pub fn try_write(&self) -> Option<RwLockWriteGuard<'_, T>> {
        self.lock.try_write()
    }

    /// Attempts to acquire this `RwLock` with exclusive write access until a
    /// timeout is reached.
    ///
    /// If the access could not be granted before the timeout expires, then
    /// `None` is returned. Otherwise, an RAII guard is returned which will
    /// release the exclusive access when it is dropped.
    #[inline]
    pub fn try_write_for(&self, timeout: Duration) -> Option<RwLockWriteGuard<'_, T>> {
        self.lock.try_write_for(timeout)
    }

    /// Locks this `RwLock` with exclusive write access, blocking the current
    /// thread until it can be acquired.
    ///
    /// This function will not return while other writers or other readers
    /// currently have access to the lock.
    ///
    /// Returns an RAII guard which will drop the write access of this `RwLock`
    /// when dropped.
    #[inline]
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        self.lock.write()
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
    /// Locks this `RwLock` with exclusive write access, blocking the current
    /// thread until it can be acquired.
    ///
    /// This function will not return while other writers or other readers
    /// currently have access to the lock.
    ///
    /// This will also release the queue so another potential writer will get access.    
    pub fn write(self) -> RwLockWriteGuard<'a, T> {
        self.lock.write()
    }
}
