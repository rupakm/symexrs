//! Symbolic Execution Engine for Rust
//!
//! This crate provides symbolic counterparts of common Rust types (e.g. `SymU64`)
//! that piggy-back on Rust's trait system to build symbolic constraints while
//! executing normal Rust code.
//!
//! Path exploration is replay-based: the engine re-executes the user closure with
//! different forced branch decisions. For undecided branches it follows the
//! current concrete (concolic) execution and records alternatives for scheduling.

pub mod engine;
pub mod error;
pub mod expressions;
pub mod manager;
pub mod runtime;
pub mod solver;
pub mod sym_int_macro;
pub mod symbolic_types;

#[cfg(test)]
mod test_deps;

use std::sync::{Arc, Mutex};

pub use engine::{
    ExplorationResult, ExplorationStrategy, ExploreConfig, ExploreResult, Explorer, RunBudget,
    RunOutcome, RunResult, WorkItem,
};
pub use error::{SymExError, SymExResult};
pub use expressions::{BinOp, ConstValue, SymExpr, UnOp};
pub use manager::{SymExManager, TypeInfo};
pub use solver::{Model, SatResult, SmtSolver, Z3Solver};
pub use symbolic_types::{SymBool, SymI32, SymI64, SymString, SymU32, SymU64, SymU8};

/// Version information
pub const VERSION: &str = "0.1.0";

/// Initialize a standalone manager (advanced use).
pub fn init() -> SymExResult<SymExManager> {
    let solver = Box::new(Z3Solver::new()?);
    Ok(SymExManager::new(solver))
}

/// Initialize the global (thread-local) runtime.
pub fn init_global() -> SymExResult<()> {
    let _ = runtime::get_or_create_default_runtime()?;
    Ok(())
}

/// Get the global manager (backed by the default runtime).
pub fn get_global_manager() -> SymExResult<Arc<Mutex<SymExManager>>> {
    runtime::global_manager()
}

/// Reset the global runtime and manager.
pub fn reset_global_manager() {
    runtime::reset_global_runtime();
}

/// Set a custom global manager (advanced/testing use).
pub fn set_global_manager(manager: Arc<Mutex<SymExManager>>) {
    runtime::set_global_manager(manager);
}

/// Explore paths with the given configuration.
pub fn explore(cfg: ExploreConfig, f: impl Fn() -> SymExResult<()>) -> SymExResult<ExploreResult> {
    engine::explore(cfg, f)
}

/// Convenience wrapper for `explore` with default configuration.
pub fn explore_default(f: impl Fn() -> SymExResult<()>) -> SymExResult<ExploreResult> {
    explore(ExploreConfig::default(), f)
}

/// Backwards-compatible explore wrapper that passes a manager.
pub fn explore_with_manager<F>(cfg: ExploreConfig, f: F) -> SymExResult<ExploreResult>
where
    F: Fn(&Arc<Mutex<SymExManager>>) -> SymExResult<()>,
{
    let mut ex = Explorer::new(cfg)?;
    let rt = ex.runtime();
    ex.explore(|| {
        let mgr = rt.manager();
        f(&mgr)
    })
}
