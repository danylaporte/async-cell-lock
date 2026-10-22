// Crate-level docs are the README so its examples are compiled as doctests.
#![doc = include_str!("../README.md")]

#[cfg(feature = "actix_web_04")]
mod actix_web;

mod async_load_rw_lock;
mod async_once_cell;
mod deadlock;
mod error;
#[cfg(feature = "telemetry")]
pub mod monitors;
mod primitives;
mod queue_rw_lock;
pub mod sync;
mod utils;

// Lets `#[deadlock_check]` expand to `::async_cell_lock::...` inside this crate too.
extern crate self as async_cell_lock;

pub use async_cell_lock_macros::{deadlock_check, tokio_test};
pub use async_load_rw_lock::*;
pub use async_once_cell::*;
#[cfg(feature = "telemetry")]
pub use deadlock::warn_lock_held;
pub use deadlock::{spawn_with_deadlock_check, with_deadlock_check};
pub use error::Error;
pub use queue_rw_lock::*;
use utils::*;

#[cfg(feature = "actix_web_04")]
pub use actix_web::DeadlockDetector;

pub type Result<T> = std::result::Result<T, Error>;
