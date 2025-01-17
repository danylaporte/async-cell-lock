use super::{locks_held, task, LockAwaitGuard, LockData, Task};
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
    op: &'static str,

    task: Arc<Task>,
}

impl<'a> LockHeldGuard<'a> {
    pub fn new(guard: LockAwaitGuard<'a>) -> Result<Self> {
        Self::new_imp(guard.lock_data, guard.op, Arc::clone(&guard.task))
    }

    pub fn new_no_wait(lock_data: &'a LockData, op: &'static str) -> Result<Self> {
        let task = task::current()?;

        Self::new_imp(lock_data, op, task)
    }

    #[cfg_attr(not(feature = "telemetry"), allow(unused_variables))]
    fn new_imp(lock_data: &'a LockData, op: &'static str, task: Arc<Task>) -> Result<Self> {
        locks_held::add_lock(lock_data.id())?;
        lock_data.add_task(Arc::clone(&task));

        #[cfg(feature = "telemetry")]
        metrics::counter!("lock_held_counter", "name" => lock_data.name, "op" => op).increment(1);

        Ok(Self {
            instant: Instant::now(),
            lock_data,
            task,

            #[cfg(feature = "telemetry")]
            gauge: {
                let gauge =
                    metrics::gauge!("lock_held_gauge", "name" => lock_data.name, "op" => op);

                gauge.increment(1.0);
                gauge
            },

            #[cfg(feature = "telemetry")]
            op,
        })
    }

    #[cfg(feature = "telemetry")]
    fn drop_telemetry(&mut self) {
        const LONG_LOCK: Duration = Duration::from_secs(30);

        let elapsed = self.instant.elapsed();

        if elapsed > LONG_LOCK {
            let _ = tracing::warn_span!(
                "Lock held for too long",
                elapsed_secs = elapsed.as_secs(),
                name = self.lock_data.name,
                op = self.op
            )
            .entered();
        }

        metrics::counter!("lock_held_ms", "name" => self.lock_data.name, "op" => self.op)
            .increment(elapsed.as_millis() as u64);

        metrics::counter!("lock_release_counter", "name" => self.lock_data.name, "op" => self.op)
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
