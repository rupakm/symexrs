//! Minimal single-threaded async runtime for symbolic execution.
//!
//! This module provides a controlled executor that:
//! - runs futures on a single thread
//! - exposes `spawn` and `yield_now`
//! - provides deterministic time and channels
//! - surfaces scheduling choices to the symex runtime (`Decision::Choice`)

pub mod channel;
mod executor;
pub mod time;

pub use executor::{Executor, JoinHandle, run, spawn, yield_now};
