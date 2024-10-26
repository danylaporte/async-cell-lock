#[cfg(feature = "actix_web_04")]
mod actix_web;

mod active_lock_guard;
mod async_load_rw_lock;
mod async_once_cell;
mod deadlock;
mod error;
mod queue_rw_lock;
mod wait_lock_guard;

use active_lock_guard::ActiveLockGuard;
pub use async_load_rw_lock::*;
pub use async_once_cell::*;
#[cfg(feature = "telemetry")]
pub use deadlock::warn_lock_held;
pub use deadlock::{lock_held, with_deadlock_check};
pub use error::Error;
pub use queue_rw_lock::*;
use wait_lock_guard::WaitLockGuard;

#[cfg(feature = "actix_web_04")]
pub use actix_web::DeadlockDetector;
