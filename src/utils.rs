use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

static ID: AtomicU64 = AtomicU64::new(1);

pub(crate) fn new_id() -> u64 {
    let id = ID.fetch_add(1, Relaxed);

    debug_assert!(id > 0);

    id
}

pub(crate) fn is_async() -> bool {
    tokio::runtime::Handle::try_current().is_ok()
}
