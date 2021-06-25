use std::{
    error,
    fmt::{self, Formatter},
};

#[derive(Clone, Copy, PartialEq)]
pub enum Error {
    DeadlockDetected,
    NotDeadlockCheckFuture,
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeadlockDetected => f.write_str("Deadlock detected."),
            Self::NotDeadlockCheckFuture => {
                f.write_str("Must run inside a with_deadlock_check future.")
            }
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self, f)
    }
}

impl error::Error for Error {}
