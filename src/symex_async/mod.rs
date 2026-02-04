//! Minimal single-threaded async runtime for symbolic execution.
//!
//! This module provides a controlled executor that:
//! - runs futures on a single thread
//! - exposes `spawn` and `yield_now`
//! - provides deterministic time and channels
//! - surfaces scheduling choices to the symex runtime (`Decision::Choice`)

mod executor;
pub mod time;
pub mod channel;

pub use executor::{run, spawn, yield_now, Executor, JoinHandle};
