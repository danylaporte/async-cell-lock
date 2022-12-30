use crate::Error;
use parking_lot::Mutex;
use std::{
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Access {
    Read,
    Queue,
    Write,
}

/// A deadlock detector.
pub(crate) struct DLDetector(Mutex<Vec<Task>>);

impl DLDetector {
    pub fn new() -> Self {
        Self(Mutex::new(Vec::new()))
    }

    pub fn check(&self, id: u64, access: Access) -> Result<(), Error> {
        self.0
            .lock()
            .iter()
            .find(|task| task.id == id)
            .filter(|task| task.is_deadlock(access))
            .map_or(Ok(()), |_| Err(Error::DeadlockDetected))
    }

    pub fn register<T>(&self, id: u64, access: Access, val: T) -> Result<DLGuard<'_, T>, Error> {
        let mut guard = self.0.lock();

        match guard.iter_mut().find(|task| task.id == id) {
            Some(task) => {
                if task.is_deadlock(access) {
                    return Err(Error::DeadlockDetected);
                }
                task.count += 1;
            }
            None => {
                guard.push(Task {
                    access,
                    id,
                    count: 1,
                });
            }
        }

        Ok(DLGuard {
            detector: self,
            id,
            val,
        })
    }

    fn remove(&self, id: u64) {
        let mut guard = self.0.lock();

        let (index, task) = guard
            .iter_mut()
            .enumerate()
            .find(|(_, task)| task.id == id)
            .expect("Task not found.");

        task.count -= 1;

        if task.count == 0 {
            guard.remove(index);
        }
    }
}

pub(crate) struct DLGuard<'a, T> {
    detector: &'a DLDetector,
    id: u64,
    val: T,
}

impl<'a, T> Deref for DLGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.val
    }
}

impl<'a, T> DerefMut for DLGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.val
    }
}

impl<'a, T> Drop for DLGuard<'a, T> {
    fn drop(&mut self) {
        self.detector.remove(self.id)
    }
}

struct Task {
    access: Access,
    count: u64,
    id: u64,
}

impl Task {
    fn is_deadlock(&self, req: Access) -> bool {
        self.access != Access::Read || req != Access::Read
    }
}

/// # NOTE
/// Use only inside a tokio task.
pub(crate) fn task_id() -> Result<u64, Error> {
    LOCKS
        .try_with(|id| *id)
        .map_err(|_| Error::NotDeadlockCheckFuture)
}

pub async fn with_deadlock_check<F, R>(f: F) -> R
where
    F: std::future::Future<Output = R>,
{
    if LOCKS.try_with(|_| ()).is_err() {
        LOCKS.scope(COUNTER.fetch_add(1, Relaxed), f).await
    } else {
        f.await
    }
}

tokio::task_local! {
    static LOCKS: u64;
}

static COUNTER: AtomicU64 = AtomicU64::new(0);
