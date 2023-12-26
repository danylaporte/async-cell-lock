#[cfg(feature = "telemetry")]
use std::time::{Duration, Instant};

pub(crate) struct WaitLockGuard {
    #[cfg(feature = "telemetry")]
    gauge: metrics::Gauge,

    #[cfg(feature = "telemetry")]
    instant: Instant,

    #[cfg(feature = "telemetry")]
    pub op: &'static str,
}

impl WaitLockGuard {
    #[cfg(feature = "telemetry")]
    pub fn new(op: &'static str) -> Self {
        let gauge = metrics::gauge!("queue_rw_lock_waiting_gauge", "op" => op);

        gauge.increment(1.0);

        Self {
            gauge,
            instant: Instant::now(),
            op,
        }
    }

    #[cfg(not(feature = "telemetry"))]
    pub fn new(_: &'static str) -> Self {
        Self {}
    }
}

#[cfg(feature = "telemetry")]
impl Drop for WaitLockGuard {
    fn drop(&mut self) {
        const LONG_WAIT: Duration = Duration::from_millis(500);

        let elapsed = self.instant.elapsed();

        if elapsed > LONG_WAIT {
            tracing::warn!(
                elapsed = elapsed.as_millis(),
                op = self.op,
                "Lock wait for too long",
            );
        }

        metrics::counter!("queue_rw_lock_waiting_ms", "op" => self.op)
            .increment(elapsed.as_millis() as u64);

        self.gauge.decrement(1.0);
    }
}
