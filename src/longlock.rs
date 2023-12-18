use std::time::{Duration, Instant};

use tracing::warn_span;

pub(crate) struct LongLock(Instant);

impl LongLock {
    pub fn new() -> Self {
        LongLock(Instant::now())
    }

    pub fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }
}

impl Drop for LongLock {
    fn drop(&mut self) {
        const LONG_LOCK: Duration = Duration::from_secs(30);

        let elasped = self.0.elapsed();

        if elasped > LONG_LOCK {
            let _ = warn_span!("Locked for too long", elapsed = elasped.as_secs()).entered();
        }
    }
}
