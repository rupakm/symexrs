//! Symbolic Execution Engine for Rust
//! 
//! A symbolic execution tool that uses Rust traits to enable seamless switching
//! between concrete and symbolic execution. The system executes Rust code normally
//! while carrying symbolic expressions alongside concrete values.

pub mod error;
pub mod expressions;
pub mod manager;
pub mod solver;
pub mod symbolic_types;

#[cfg(test)]
mod test_deps;

// Re-export commonly used types for convenience
pub use error::{SymExError, SymExResult};
pub use expressions::{SymExpr, BinOp, UnOp, ConstValue};
pub use manager::{SymExManager, ExecutionState, TypeInfo};
pub use solver::{SmtSolver, Z3Solver, SatResult, Model};

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the symbolic execution engine with default configuration
pub fn init() -> SymExResult<SymExManager> {
    let solver = Box::new(Z3Solver::new()?);
    Ok(SymExManager::new(solver))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        let manager = init();
        assert!(manager.is_ok());
    }

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}