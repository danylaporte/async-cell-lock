use std::time::{Duration, Instant};

use tracing::warn_span;

pub(crate) struct LongLock(Instant, &'static str);

impl LongLock {
    pub fn new(op: &'static str) -> Self {
        LongLock(Instant::now(), op)
    }

    pub fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }
}

impl Drop for LongLock {
    fn drop(&mut self) {
        const LONG_LOCK: Duration = Duration::from_secs(30);

        let elapsed = self.0.elapsed();

        if elapsed > LONG_LOCK {
            let _ = warn_span!(
                "Locked for too long",
                elapsed = elapsed.as_secs(),
                op = self.1
            )
            .entered();
        }

        #[cfg(feature = "telemetry")]
        {
            metrics::counter!("queue_rw_lock_ms", elapsed.as_millis() as u64, "op" => self.1);
            metrics::decrement_gauge!("queue_rw_lock_active_gauge", 1.0, "op" => self.1);
        }
    }
}
