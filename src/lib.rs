//! Symbolic Execution Engine for Rust
//!
//! This crate provides symbolic counterparts of common Rust types (e.g. `SymU64`)
//! that piggy-back on Rust's trait system to build symbolic constraints while
//! executing normal Rust code.
//!
//! Path exploration is replay-based: the engine re-executes the user closure with
//! different forced branch decisions. For undecided branches it follows the
//! current concrete (concolic) execution and records alternatives for scheduling.

pub mod decision;
pub mod engine;
pub mod error;
pub mod expressions;
pub mod manager;
pub mod runtime;
pub mod scheduler;
pub mod symex_async;
pub mod solver;
pub mod sym_int_macro;
pub mod symbolic_types;

#[cfg(test)]
mod test_deps;

use std::sync::{Arc, Mutex};

pub use decision::Decision;
pub use engine::{
    BugCase, ExplorationResult, ExplorationStrategy, ExploreConfig, ExploreResult, Explorer,
    RunBudget, RunOutcome, RunResult, WorkItem,
};
pub use error::{SymExError, SymExResult};
pub use expressions::{BinOp, ConstValue, SymExpr, UnOp};
pub use manager::{SymExManager, TypeInfo};
pub use scheduler::{Scheduler, SchedulerKind};
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

/// Explore paths using a custom scheduler implementation.
pub fn explore_with_scheduler(
    cfg: ExploreConfig,
    scheduler: Box<dyn Scheduler>,
    f: impl Fn() -> SymExResult<()>,
) -> SymExResult<ExploreResult> {
    engine::explore_with_scheduler(cfg, scheduler, f)
}

/// Replay a specific `WorkItem` deterministically.
pub fn replay(
    work: WorkItem,
    cfg: ExploreConfig,
    f: impl Fn() -> SymExResult<()>,
) -> SymExResult<RunResult> {
    engine::replay(work, cfg, f)
}

/// Convenience wrapper for `explore` with default configuration.
pub fn explore_default(f: impl Fn() -> SymExResult<()>) -> SymExResult<ExploreResult> {
    explore(ExploreConfig::default(), f)
}

pub fn explore_async<F, Fut>(cfg: ExploreConfig, f: F) -> SymExResult<ExploreResult>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = SymExResult<()>> + 'static,
{
    explore(cfg, move || {
        let fut = f();
        crate::symex_async::run(fut)
    })
}

pub fn explore_async_default<F, Fut>(f: F) -> SymExResult<ExploreResult>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = SymExResult<()>> + 'static,
{
    explore_async(ExploreConfig::default(), f)
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

/// Print the current symbolic execution state to stdout.
///
/// This includes:
/// - Symbolic variables and their types
/// - Path constraints
/// - Concrete input values
/// - Decision and branch traces
///
/// This is useful for debugging symbolic execution and understanding
/// what constraints have been accumulated.
pub fn print_state() -> SymExResult<()> {
    // First try to get current runtime without nested borrows
    let rt_opt = runtime::with_current_runtime(|rt| rt.cloned());
    
    if let Some(rt) = rt_opt {
        rt.print_state();
    } else {
        // Try to get the default runtime
        match runtime::get_or_create_default_runtime() {
            Ok(rt) => rt.print_state(),
            Err(_) => println!("No runtime available"),
        }
    }
    Ok(())
}

/// Get the current symbolic execution state as a formatted string.
///
/// This includes:
/// - Symbolic variables and their types
/// - Path constraints
/// - Concrete input values
/// - Decision and branch traces
///
/// Returns the formatted state, or an error message if no runtime is available.
pub fn format_state() -> String {
    // First try to get current runtime without nested borrows
    let rt_opt = runtime::with_current_runtime(|rt| rt.cloned());
    
    if let Some(rt) = rt_opt {
        rt.format_state()
    } else {
        // Try to get the default runtime
        match runtime::get_or_create_default_runtime() {
            Ok(rt) => rt.format_state(),
            Err(_) => "No runtime available\n".to_string(),
        }
    }
}

/// Print only the symbolic variables and path constraints (without runtime details).
///
/// This is a lighter-weight version of `print_state()` that focuses on the
/// symbolic state managed by the constraint manager.
pub fn print_symbolic_state() -> SymExResult<()> {
    let mgr = get_global_manager()?;
    let mgr = mgr.lock().unwrap();
    mgr.print_state();
    Ok(())
}

/// Get only the symbolic variables and path constraints as a formatted string.
///
/// This is a lighter-weight version of `format_state()` that focuses on the
/// symbolic state managed by the constraint manager.
pub fn format_symbolic_state() -> SymExResult<String> {
    let mgr = get_global_manager()?;
    let mgr = mgr.lock().unwrap();
    Ok(mgr.format_state())
}
