use super::{locks_held, task, LockData, Task};
use crate::Result;
use std::sync::Arc;

pub(crate) struct LockAwaitGuard<'a> {
    #[cfg(feature = "telemetry")]
    gauge: metrics::Gauge,

    #[cfg(feature = "telemetry")]
    instant: std::time::Instant,

    pub lock_data: &'a LockData,
    pub op: &'static str,
    pub task: Arc<Task>,
}

impl<'a> LockAwaitGuard<'a> {
    pub fn new(lock_data: &'a LockData, op: &'static str) -> Result<Self> {
        locks_held::check_deadlock(lock_data, op)?;

        let task = task::current()?;

        task.set_await_lock_id(lock_data, op)?;

        #[cfg(feature = "telemetry")]
        metrics::counter!("lock_await_counter", "name" => lock_data.name, "op" => op).increment(1);

        Ok(Self {
            #[cfg(feature = "telemetry")]
            gauge: {
                let gauge =
                    metrics::gauge!("lock_await_gauge", "name" => lock_data.name, "op" => op);

                gauge.increment(1.0);
                gauge
            },

            #[cfg(feature = "telemetry")]
            instant: std::time::Instant::now(),

            lock_data,
            op,
            task,
        })
    }

    #[cfg(feature = "telemetry")]
    fn drop_telemetry(&mut self) {
        const LONG_WAIT: std::time::Duration = std::time::Duration::from_millis(500);

        let elapsed = self.instant.elapsed();

        if elapsed > LONG_WAIT {
            tracing::warn!(
                elapsed_ms = elapsed.as_millis(),
                name = self.lock_data.name,
                op = self.op,
                "Lock wait for too long",
            );
        }

        metrics::counter!("lock_await_ms", "name" => self.lock_data.name, "op" => self.op)
            .increment(elapsed.as_millis() as u64);

        self.gauge.decrement(1.0);
    }
}

impl Drop for LockAwaitGuard<'_> {
    fn drop(&mut self) {
        #[cfg(feature = "telemetry")]
        self.drop_telemetry();

        self.task.clear_await_lock_id();
    }
}
