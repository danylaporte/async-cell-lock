#[cfg(feature = "actix_web_04")]
mod actix_web;

mod async_load_rw_lock;
mod async_once_cell;
mod error;
mod queue_rw_lock;
mod tasks;

pub use async_load_rw_lock::*;
pub use async_once_cell::*;
pub use error::Error;
pub use queue_rw_lock::*;
pub use tasks::with_deadlock_check;

#[cfg(feature = "actix_web_04")]
pub use actix_web::DeadlockDetector;
