use super::{locks_held, task, LockAwaitGuard, LockData, Ops, Task};
use crate::Result;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub(crate) struct LockHeldGuard<'a> {
    #[cfg(feature = "telemetry")]
    gauge: metrics::Gauge,

    instant: Instant,
    lock_data: &'a LockData,

    #[cfg(feature = "telemetry")]
    op: Ops,

    task: Arc<Task>,
}

impl<'a> LockHeldGuard<'a> {
    pub fn new(guard: LockAwaitGuard<'a>) -> Result<Self> {
        Self::new_imp(guard.lock_data, guard.op, Arc::clone(&guard.task))
    }

    pub fn new_no_wait(lock_data: &'a LockData, op: Ops) -> Result<Self> {
        let task = task::current()?;

        Self::new_imp(lock_data, op, task)
    }

    #[cfg_attr(not(feature = "telemetry"), allow(unused_variables))]
    fn new_imp(lock_data: &'a LockData, op: Ops, task: Arc<Task>) -> Result<Self> {
        locks_held::add_lock(lock_data.id())?;
        lock_data.add_task(Arc::clone(&task));

        #[cfg(feature = "telemetry")]
        metrics::counter!("lock_held_counter", "name" => lock_data.name, "op" => op, "task" => task.name.clone()).increment(1);

        Ok(Self {
            instant: Instant::now(),
            lock_data,

            #[cfg(feature = "telemetry")]
            gauge: {
                let gauge = metrics::gauge!("lock_held_gauge", "name" => lock_data.name, "op" => op, "task" => task.name.clone());

                gauge.increment(1.0);
                gauge
            },

            #[cfg(feature = "telemetry")]
            op,

            task,
        })
    }

    #[cfg(feature = "telemetry")]
    fn drop_telemetry(&mut self) {
        let elapsed = self.instant.elapsed();
        let recommend_dur = self.op.recommend_dur();

        if elapsed > recommend_dur {
            let _ = tracing::warn_span!(
                "Lock held for too long",
                elasped_ms = elapsed.as_millis(),
                lock_name = self.lock_data.name,
                lock_op = self.op.as_str(),
                recommend_dur_ms = recommend_dur.as_millis(),
                task_name = &self.task.name,
            )
            .entered();
        }

        metrics::counter!("lock_held_for_too_long", "name" => self.lock_data.name, "op" => self.op, "task" => self.task.name.clone())
            .increment(elapsed.as_millis() as u64);

        metrics::counter!("lock_held_ms", "name" => self.lock_data.name, "op" => self.op, "task" => self.task.name.clone())
            .increment(elapsed.as_millis() as u64);

        metrics::counter!("lock_release_counter", "name" => self.lock_data.name, "op" => self.op, "task" => self.task.name.clone())
            .increment(1);

        self.gauge.decrement(1.0);
    }

    pub fn elapsed(&self) -> Duration {
        self.instant.elapsed()
    }
}

impl Drop for LockHeldGuard<'_> {
    fn drop(&mut self) {
        #[cfg(feature = "telemetry")]
        self.drop_telemetry();

        let _ = locks_held::remove_lock(self.lock_data.id());

        self.lock_data.remove_task(&self.task);
    }
}
