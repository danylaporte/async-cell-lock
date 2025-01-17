use crate::primitives::{locks_held, task};

pub async fn with_deadlock_check<F, R>(f: F, task_name: String) -> R
where
    F: std::future::Future<Output = R>,
{
    #[cfg(feature = "telemetry")]
    let _active = crate::monitors::ActiveGauge::new(
        metrics::gauge!("active_dl_chk_gauge", "task" => task_name.clone()),
    );

    #[cfg(feature = "telemetry")]
    metrics::counter!("started_dl_chk_counter", "task" => task_name.clone()).increment(1);

    #[cfg(feature = "telemetry")]
    let _on_complete = crate::monitors::CountOnEnd(
        metrics::counter!("completed_dl_chk_counter", "task" => task_name.clone()),
    );

    locks_held::scope(task::scope(f, task_name)).await
}

/// Log a "Lock held" warn in the trace if a lock is currently active.
/// This is useful to prevent a lock from being held while a call api.
#[cfg(feature = "telemetry")]
pub fn warn_lock_held() {
    if crate::primitives::locks_held::has_lock_held() {
        let _ = tracing::warn_span!("Lock held").entered();
    }
}
