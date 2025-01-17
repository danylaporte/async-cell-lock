use super::LockData;
use crate::{Error, Result};
use std::{
    future::Future,
    sync::{
        atomic::{AtomicU64, Ordering::Relaxed},
        Arc,
    },
};
use tokio::{task::futures::TaskLocalFuture, task_local};

pub(crate) struct Task {
    pub await_lock_id: AtomicU64,
    pub name: String,
}

impl Task {
    pub fn clear_await_lock_id(&self) {
        self.await_lock_id.store(0, Relaxed);
    }

    pub fn await_lock_id(&self) -> u64 {
        self.await_lock_id.load(Relaxed)
    }

    pub fn set_await_lock_id(&self, lock_data: &LockData, op: &str) -> Result<()> {
        match self
            .await_lock_id
            .compare_exchange(0, lock_data.id(), Relaxed, Relaxed)
        {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::deadlock_detected(lock_data, op, &self.name)),
        }
    }
}

pub(crate) fn current() -> Result<Arc<Task>> {
    try_with(Arc::clone)
}

pub(crate) fn scope<F>(f: F, task_name: String) -> TaskLocalFuture<Arc<Task>, F>
where
    F: Future,
{
    TASK.scope(
        Arc::new(Task {
            await_lock_id: AtomicU64::new(0),
            name: task_name,
        }),
        f,
    )
}

pub(crate) fn try_with<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&Arc<Task>) -> R,
{
    TASK.try_with(f).map_err(Error::not_deadlock_check_future)
}

task_local! {
    static TASK: Arc<Task>;
}
