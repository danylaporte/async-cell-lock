use metrics::{Counter, Gauge};

/// Increment [Gauge] on create and decrement on drop.
pub struct ActiveGauge(Gauge);

impl ActiveGauge {
    pub fn new(gauge: Gauge) -> Self {
        gauge.increment(1);
        Self(gauge)
    }
}

impl Drop for ActiveGauge {
    fn drop(&mut self) {
        self.0.decrement(1);
    }
}

pub struct CountOnEnd(pub Counter);

impl Drop for CountOnEnd {
    fn drop(&mut self) {
        self.0.increment(1);
    }
}
