#[cfg(feature = "telemetry")]
use std::time::Duration;

#[derive(Clone, Copy)]
pub(crate) enum Ops {
    Read,
    Write,
    Queue,
}

#[cfg(feature = "telemetry")]
impl Ops {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queue => "queue",
            Self::Write => "write",
            Self::Read => "read",
        }
    }

    pub fn recommend_dur(self) -> Duration {
        const QUEUE_DUR: Duration = Duration::from_secs(2);
        const READ_DUR: Duration = Duration::from_secs(30);
        const WRITE_DUR: Duration = Duration::from_secs(1);

        match self {
            Ops::Queue => QUEUE_DUR,
            Ops::Read => READ_DUR,
            Ops::Write => WRITE_DUR,
        }
    }
}

#[cfg(feature = "telemetry")]
impl From<Ops> for metrics::SharedString {
    fn from(op: Ops) -> Self {
        op.as_str().into()
    }
}
