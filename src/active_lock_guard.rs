use crate::WaitLockGuard;
use std::time::{Duration, Instant};

pub(crate) struct ActiveLockGuard {
    #[cfg(feature = "telemetry")]
    gauge: metrics::Gauge,

    instant: Instant,

    #[cfg(feature = "telemetry")]
    op: &'static str,
}

impl ActiveLockGuard {
    #[cfg(feature = "telemetry")]
    pub fn new(guard: WaitLockGuard) -> Self {
        let op = guard.op;

        drop(guard);

        let gauge = metrics::gauge!("queue_rw_lock_active_gauge", "op" => op);
        gauge.increment(1.0);

        Self {
            gauge,
            instant: Instant::now(),
            op,
        }
    }

    #[cfg(not(feature = "telemetry"))]
    pub fn new(_: WaitLockGuard) -> Self {
        Self {
            instant: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.instant.elapsed()
    }
}

#[cfg(feature = "telemetry")]
impl Drop for ActiveLockGuard {
    fn drop(&mut self) {
        const LONG_LOCK: Duration = Duration::from_secs(30);

        let elapsed = self.instant.elapsed();

        if elapsed > LONG_LOCK {
            let _ = tracing::warn_span!(
                "Lock kept for too long",
                elapsed = elapsed.as_secs(),
                op = self.op
            )
            .entered();
        }

        metrics::counter!("queue_rw_lock_ms", "op" => self.op)
            .increment(elapsed.as_millis() as u64);

        self.gauge.decrement(1.0);
    }
}

#[cfg(not(feature = "telemetry"))]
impl Drop for ActiveLockGuard {
    fn drop(&mut self) {}
}
