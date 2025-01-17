use super::Task;
use crate::{new_id, Error, Result};
use parking_lot::Mutex;
use std::sync::{
    atomic::{AtomicU64, Ordering::Relaxed},
    Arc,
};

pub struct LockData {
    locked_tasks: Mutex<Vec<Arc<Task>>>,
    lock_id: AtomicU64,

    #[cfg(feature = "telemetry")]
    pub name: &'static str,
}

impl LockData {
    #[cfg_attr(not(feature = "telemetry"), allow(unused_variables))]
    pub const fn new(name: &'static str) -> Self {
        Self {
            locked_tasks: Mutex::new(Vec::new()),
            lock_id: AtomicU64::new(0),

            #[cfg(feature = "telemetry")]
            name,
        }
    }

    pub fn add_task(&self, task: Arc<Task>) {
        self.locked_tasks.lock().push(task);
    }

    pub fn check_deadlock(&self, op: &str, locks_held: &[u64]) -> Result<()> {
        for t in self.locked_tasks.lock().iter() {
            let id = t.await_lock_id();

            if id > 0 && locks_held.contains(&id) {
                return Err(Error::deadlock_detected(self, op, &t.name));
            }
        }

        Ok(())
    }

    pub fn id(&self) -> u64 {
        let v = self.lock_id.load(Relaxed);

        if v == 0 {
            let v = new_id();

            match self.lock_id.compare_exchange(0, v, Relaxed, Relaxed) {
                Ok(_) => v,
                Err(v) => v,
            }
        } else {
            v
        }
    }

    pub fn remove_task(&self, task: &Arc<Task>) {
        let mut tasks = self.locked_tasks.lock();

        if let Some(idx) = tasks.iter().position(|t| Arc::ptr_eq(t, task)) {
            tasks.swap_remove(idx);
        } else {
            debug_assert!(false, "remove_task_not_found")
        }
    }
}
