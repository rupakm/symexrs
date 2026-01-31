//! Symbolic Execution Engine for Rust
//!
//! A symbolic execution tool that uses Rust traits to enable seamless switching
//! between concrete and symbolic execution. The system executes Rust code normally
//! while carrying symbolic expressions alongside concrete values.

pub mod error;
pub mod expressions;
pub mod manager;
pub mod solver;
pub mod sym_int_macro;
pub mod symbolic_types;

#[cfg(test)]
mod test_deps;

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

// Custom panic payload for unsatisfiable path detection
#[derive(Debug)]
pub(crate) struct UnsatPanic;

// Re-export commonly used types for convenience
pub use error::{SymExError, SymExResult};
pub use expressions::{BinOp, ConstValue, SymExpr, UnOp};
pub use manager::{ExecutionState, SymExManager, TypeInfo};
pub use solver::{Model, SatResult, SmtSolver, Z3Solver};
pub use symbolic_types::{SymBool, SymI32, SymI64, SymU8, SymU32, SymU64};

/// Version information
pub const VERSION: &str = "0.1.0";

/// Strategy for exploring execution paths
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplorationStrategy {
    /// Depth-first search: explore paths deeply before backtracking
    DepthFirst,
    /// Breadth-first search: explore all paths at same depth before going deeper
    BreadthFirst,
}

/// Result of path exploration
#[derive(Debug, Clone)]
pub struct ExplorationResult {
    /// Total number of paths explored
    pub paths_explored: usize,
    /// Number of satisfiable paths found
    pub satisfiable_paths: usize,
    /// Number of unsatisfiable paths found
    pub unsatisfiable_paths: usize,
    /// Maximum depth reached during exploration
    pub max_depth_reached: usize,
    /// Maximum number of variables in any path
    pub max_variable_count: usize,
    /// Maximum number of constraints in any path
    pub max_constraint_count: usize,
}

impl ExplorationResult {
    /// Calculate path coverage percentage (satisfiable paths / total paths)
    pub fn coverage_percentage(&self) -> f64 {
        if self.paths_explored == 0 {
            0.0
        } else {
            (self.satisfiable_paths as f64 / self.paths_explored as f64) * 100.0
        }
    }

    /// Get a summary string of the exploration results
    pub fn summary(&self) -> String {
        format!(
            "Explored {} paths ({} satisfiable, {} unsatisfiable)\n\
             Max depth: {}, Max variables: {}, Max constraints: {}, Coverage: {:.2}%",
            self.paths_explored,
            self.satisfiable_paths,
            self.unsatisfiable_paths,
            self.max_depth_reached,
            self.max_variable_count,
            self.max_constraint_count,
            self.coverage_percentage()
        )
    }
}

/// Configuration for symbolic execution
#[derive(Debug, Clone)]
pub struct ExploreConfig {
    /// Maximum depth for path exploration
    pub max_depth: usize,
    /// Maximum number of paths to explore
    pub max_paths: Option<usize>,
    /// Exploration strategy (depth-first or breadth-first)
    pub strategy: ExplorationStrategy,
    /// Maximum number of visits to a state before considering it a loop
    pub max_state_visits: usize,
    /// Maximum size of the backtracking stack before garbage collection
    pub max_stack_size: usize,
    /// Enable state compression
    pub enable_compression: bool,
    /// Timeout in seconds (None for no timeout)
    pub timeout_seconds: Option<u64>,
}

impl Default for ExploreConfig {
    fn default() -> Self {
        Self {
            max_depth: 100,
            max_paths: None,
            strategy: ExplorationStrategy::DepthFirst,
            max_state_visits: 100,
            max_stack_size: 1000,
            enable_compression: true,
            timeout_seconds: None,
        }
    }
}

impl ExploreConfig {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum depth for path exploration
    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Set the maximum number of paths to explore
    pub fn with_max_paths(mut self, max_paths: usize) -> Self {
        self.max_paths = Some(max_paths);
        self
    }

    /// Set the exploration strategy
    pub fn with_strategy(mut self, strategy: ExplorationStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Set the maximum state visits for loop detection
    pub fn with_max_state_visits(mut self, max_state_visits: usize) -> Self {
        self.max_state_visits = max_state_visits;
        self
    }

    /// Enable or disable state compression
    pub fn with_compression(mut self, enable: bool) -> Self {
        self.enable_compression = enable;
        self
    }

    /// Set a timeout for exploration
    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = Some(seconds);
        self
    }
}

/// Result of symbolic exploration
#[derive(Debug, Clone)]
pub struct ExploreResult {
    /// Exploration statistics
    pub exploration_result: ExplorationResult,
    /// Final manager state (for inspection)
    pub manager_stats: ManagerStats,
    /// Whether exploration completed successfully
    pub completed: bool,
    /// Error message if exploration failed
    pub error: Option<String>,
}

/// Statistics about the symbolic execution manager
#[derive(Debug, Clone)]
pub struct ManagerStats {
    /// Number of variables created
    pub variable_count: usize,
    /// Number of constraints accumulated
    pub constraint_count: usize,
    /// Number of paths explored
    pub paths_explored: usize,
    /// Cache hit rate
    pub cache_hit_rate: f64,
}

/// Main entry point for symbolic execution
///
/// This function sets up a symbolic execution environment, executes the provided
/// function symbolically, and returns the exploration results.
///
/// The function automatically explores all paths by re-executing the closure
/// with different branch decisions for symbolic comparisons.
///
/// # Arguments
///
/// * `config` - Configuration for symbolic execution
/// * `f` - Function to execute symbolically. It receives a reference to the manager
///         and should perform symbolic operations.
///
/// # Example
///
/// ```
/// use rust_project::{explore, ExploreConfig, SymU64};
/// use std::sync::Arc;
///
/// let config = ExploreConfig::default();
/// let result = explore(config, |manager| {
///     // Create symbolic variables
///     let x = SymU64::new(Arc::clone(manager));
///     let zero = SymU64::from_concrete(0, Arc::clone(manager));
///     
///     // This comparison will fork execution!
///     if x == zero {
///         println!("x is zero");
///     } else {
///         println!("x is not zero");
///     }
///     
///     Ok(())
/// });
///
/// // Will explore both paths (x == 0 and x != 0)
/// assert!(result.is_ok());
/// ```
pub fn explore<F>(config: ExploreConfig, f: F) -> SymExResult<ExploreResult>
where
    F: Fn(&Arc<Mutex<SymExManager>>) -> SymExResult<()>,
{
    use crate::symbolic_types::{
        disable_symbolic_mode, enable_symbolic_mode, new_branch_encountered, reset_branch_tracking,
        set_branch_decisions,
    };

    // Enable symbolic mode
    enable_symbolic_mode();

    // We'll use a work queue to explore all paths
    let mut work_queue: Vec<Vec<bool>> = vec![vec![]]; // Start with empty branch decisions
    let mut all_paths = Vec::new();
    let mut total_paths_explored = 0;
    let mut satisfiable_paths = 0;
    let mut unsatisfiable_paths = 0;
    let mut max_variable_count = 0;
    let mut max_constraint_count = 0;
    let mut _early_terminations = 0; // Track early unsatisfiability detections

    while let Some(branch_decisions) = work_queue.pop() {
        if config
            .max_paths
            .map_or(false, |max| total_paths_explored >= max)
        {
            break;
        }

        if branch_decisions.len() > config.max_depth {
            continue;
        }

        // Create a fresh manager for this path
        let solver = Box::new(Z3Solver::new()?);
        let mut manager = SymExManager::with_max_visits(solver, config.max_state_visits);
        manager.set_max_stack_size(config.max_stack_size);
        manager.set_compression_enabled(config.enable_compression);
        let manager_arc = Arc::new(Mutex::new(manager));

        // Set as the global manager for this thread
        set_global_manager(Arc::clone(&manager_arc));

        // Set the branch decisions for this path
        set_branch_decisions(branch_decisions.clone());

        // Execute the user's function with panic catching for early UNSAT detection
        let execution_result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&manager_arc)));

        // Handle the execution result
        match execution_result {
            Ok(Ok(())) => {
                // Function executed successfully - continue to check satisfiability
            }
            Ok(Err(_)) => {
                // User function returned an error - skip this path
                continue;
            }
            Err(panic_payload) => {
                // A panic occurred - check if it's our UnsatPanic
                if panic_payload.downcast_ref::<UnsatPanic>().is_some() {
                    // This is an early UNSAT detection - count it and skip
                    total_paths_explored += 1;
                    unsatisfiable_paths += 1;
                    _early_terminations += 1;

                    // Collect statistics for reporting
                    let (variable_count, constraint_count) = {
                        let mgr = manager_arc.lock().unwrap();
                        let stats = mgr.get_stats();
                        (stats.variable_count, stats.constraint_count)
                    };

                    max_variable_count = max_variable_count.max(variable_count);
                    max_constraint_count = max_constraint_count.max(constraint_count);

                    all_paths.push((branch_decisions.clone(), constraint_count, false));
                    continue; // Skip to next path
                } else {
                    // This is a real panic from user code - propagate it
                    std::panic::resume_unwind(panic_payload);
                }
            }
        }

        // Check if we encountered a new branch
        let encountered_new_branch = new_branch_encountered();

        // Only count this path if we didn't encounter a new branch
        // (if we did, we'll re-execute with both branch outcomes)
        if !encountered_new_branch {
            total_paths_explored += 1;

            // Check if this path is satisfiable
            // (may be redundant if early detection caught it, but handles other cases)
            let is_sat = {
                let mut mgr = manager_arc.lock().unwrap();
                mgr.is_satisfiable().unwrap_or(false)
            };

            if is_sat {
                satisfiable_paths += 1;
            } else {
                unsatisfiable_paths += 1;
            }

            // Collect statistics from this path
            let (variable_count, constraint_count) = {
                let mgr = manager_arc.lock().unwrap();
                let stats = mgr.get_stats();
                (stats.variable_count, stats.constraint_count)
            };

            // Track maximum values across all paths
            max_variable_count = max_variable_count.max(variable_count);
            max_constraint_count = max_constraint_count.max(constraint_count);

            // Collect path information
            all_paths.push((branch_decisions.clone(), constraint_count, is_sat));
        }

        // If we encountered a new branch, fork for both outcomes
        if encountered_new_branch {
            // Fork for both outcomes
            let mut true_branch = branch_decisions.clone();
            true_branch.push(true);
            work_queue.push(true_branch);

            let mut false_branch = branch_decisions.clone();
            false_branch.push(false);
            work_queue.push(false_branch);
        }
    }

    // Disable symbolic mode
    disable_symbolic_mode();
    reset_branch_tracking();

    // Create final result
    let exploration_result = ExplorationResult {
        paths_explored: total_paths_explored,
        satisfiable_paths,
        unsatisfiable_paths,
        max_depth_reached: all_paths.iter().map(|(d, _, _)| d.len()).max().unwrap_or(0),
        max_variable_count,
        max_constraint_count,
    };

    let manager_stats = ManagerStats {
        variable_count: max_variable_count,
        constraint_count: max_constraint_count,
        paths_explored: total_paths_explored,
        cache_hit_rate: 0.0,
    };

    Ok(ExploreResult {
        exploration_result,
        manager_stats,
        completed: true,
        error: None,
    })
}

/// Simplified explore function that uses default configuration
///
/// This is a convenience wrapper around `explore` that uses default settings.
pub fn explore_default<F>(f: F) -> SymExResult<ExploreResult>
where
    F: Fn(&Arc<Mutex<SymExManager>>) -> SymExResult<()>,
{
    explore(ExploreConfig::default(), f)
}

// Thread-local storage for the global symbolic execution manager
thread_local! {
    static GLOBAL_MANAGER: RefCell<Option<Arc<Mutex<SymExManager>>>> = const { RefCell::new(None) };
}

/// Initialize the symbolic execution engine with default configuration
pub fn init() -> SymExResult<SymExManager> {
    let solver = Box::new(Z3Solver::new()?);
    Ok(SymExManager::new(solver))
}

/// Initialize the global symbolic execution manager for the current thread
///
/// This creates a thread-local manager that can be accessed by symbolic types
/// without explicitly passing a manager reference.
pub fn init_global() -> SymExResult<()> {
    let solver = Box::new(Z3Solver::new()?);
    let manager = Arc::new(Mutex::new(SymExManager::new(solver)));
    GLOBAL_MANAGER.with(|global| {
        *global.borrow_mut() = Some(manager);
    });
    Ok(())
}

/// Get a reference to the global symbolic execution manager
///
/// This will automatically initialize the manager if it hasn't been initialized yet.
/// Each thread has its own manager instance.
pub fn get_global_manager() -> SymExResult<Arc<Mutex<SymExManager>>> {
    GLOBAL_MANAGER.with(|global| {
        let mut manager_ref = global.borrow_mut();
        if manager_ref.is_none() {
            // Auto-initialize if not already initialized
            let solver = Box::new(Z3Solver::new()?);
            *manager_ref = Some(Arc::new(Mutex::new(SymExManager::new(solver))));
        }
        Ok(Arc::clone(manager_ref.as_ref().unwrap()))
    })
}

/// Reset the global symbolic execution manager
///
/// This clears the thread-local manager, forcing a new one to be created
/// on the next access.
pub fn reset_global_manager() {
    GLOBAL_MANAGER.with(|global| {
        *global.borrow_mut() = None;
    });
}

/// Set a custom global manager (useful for testing)
///
/// This allows tests to inject a specific manager configuration.
pub fn set_global_manager(manager: Arc<Mutex<SymExManager>>) {
    GLOBAL_MANAGER.with(|global| {
        *global.borrow_mut() = Some(manager);
    });
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
    fn test_global_manager() {
        reset_global_manager();

        // First access should auto-initialize
        let manager1 = get_global_manager();
        assert!(manager1.is_ok());

        // Second access should return the same manager
        let manager2 = get_global_manager();
        assert!(manager2.is_ok());

        // They should be the same Arc
        let m1 = manager1.unwrap();
        let m2 = manager2.unwrap();
        assert!(Arc::ptr_eq(&m1, &m2));
    }

    #[test]
    fn test_init_global() {
        reset_global_manager();

        let result = init_global();
        assert!(result.is_ok());

        let manager = get_global_manager();
        assert!(manager.is_ok());
    }

    #[test]
    fn test_reset_global_manager() {
        reset_global_manager();

        let manager1 = get_global_manager().unwrap();
        let id1 = {
            let mgr = manager1.lock().unwrap();
            mgr.fresh_variable("test")
        };

        reset_global_manager();

        let manager2 = get_global_manager().unwrap();
        let id2 = {
            let mgr = manager2.lock().unwrap();
            mgr.fresh_variable("test")
        };

        // After reset, we should get a new manager with fresh IDs
        assert_eq!(id1, "test_0");
        assert_eq!(id2, "test_0"); // New manager starts from 0 again
    }

    #[test]
    fn test_explore_basic() {
        let config = ExploreConfig::default();

        let result = explore(config, |manager| {
            // Create symbolic variables
            let x = SymU64::new(Arc::clone(manager));
            let y = SymU64::new(Arc::clone(manager));

            // Add some constraints
            let zero = SymU64::from_concrete(0, Arc::clone(manager));
            x.assert_gt(&zero)?;
            y.assert_gt(&zero)?;

            Ok(())
        });

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.completed);
        assert!(result.error.is_none());
        assert_eq!(result.manager_stats.variable_count, 2);
        assert_eq!(result.manager_stats.constraint_count, 2);
    }

    #[test]
    fn test_explore_with_custom_config() {
        let config = ExploreConfig::new()
            .with_max_depth(50)
            .with_strategy(ExplorationStrategy::BreadthFirst)
            .with_compression(false);

        let result = explore(config, |manager| {
            let x = SymU64::new(Arc::clone(manager));
            let ten = SymU64::from_concrete(10, Arc::clone(manager));
            x.assert_lt(&ten)?;
            Ok(())
        });

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.completed);
        assert_eq!(result.manager_stats.variable_count, 1);
    }

    #[test]
    fn test_explore_default() {
        let result = explore_default(|manager| {
            let x = SymU64::new(Arc::clone(manager));
            let y = SymU64::new(Arc::clone(manager));

            // Create a simple constraint
            x.assert_eq(&y)?;

            Ok(())
        });

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.completed);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_explore_with_arithmetic() {
        let result = explore_default(|manager| {
            let a = SymU64::from_concrete(10, Arc::clone(manager));
            let b = SymU64::from_concrete(20, Arc::clone(manager));
            let c = SymU64::from_concrete(30, Arc::clone(manager));

            // Perform operations
            let sum = &a + &b;

            // Add constraint: sum == c
            sum.assert_eq(&c)?;

            Ok(())
        });

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.completed);
        assert!(result.exploration_result.satisfiable_paths > 0);
    }

    #[test]
    fn test_explore_with_contradictory_constraints() {
        let result = explore_default(|manager| {
            let x = SymU64::new(Arc::clone(manager));
            let zero = SymU64::from_concrete(0, Arc::clone(manager));

            // Add contradictory constraints: x > 0 AND x < 0
            x.assert_gt(&zero)?;
            x.assert_lt(&zero)?;

            Ok(())
        });

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.completed);
        // Should have unsatisfiable paths
        assert!(result.exploration_result.unsatisfiable_paths > 0);
    }

    #[test]
    fn test_explore_with_multiple_variables() {
        let result = explore_default(|manager| {
            // Create multiple symbolic variables
            let x = SymU64::new(Arc::clone(manager));
            let y = SymU64::new(Arc::clone(manager));
            let z = SymU64::new(Arc::clone(manager));

            let ten = SymU64::from_concrete(10, Arc::clone(manager));
            let twenty = SymU64::from_concrete(20, Arc::clone(manager));

            // Add constraints: x < 10, y > 10, z == 20
            x.assert_lt(&ten)?;
            y.assert_gt(&ten)?;
            z.assert_eq(&twenty)?;

            Ok(())
        });

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.completed);
        assert_eq!(result.manager_stats.variable_count, 3);
        assert_eq!(result.manager_stats.constraint_count, 3);
    }

    #[test]
    fn test_explore_config_builder() {
        let config = ExploreConfig::new()
            .with_max_depth(200)
            .with_max_paths(1000)
            .with_strategy(ExplorationStrategy::DepthFirst)
            .with_max_state_visits(50)
            .with_compression(true)
            .with_timeout(60);

        assert_eq!(config.max_depth, 200);
        assert_eq!(config.max_paths, Some(1000));
        assert_eq!(config.strategy, ExplorationStrategy::DepthFirst);
        assert_eq!(config.max_state_visits, 50);
        assert!(config.enable_compression);
        assert_eq!(config.timeout_seconds, Some(60));
    }

    #[test]
    fn test_explore_uses_global_manager() {
        reset_global_manager();

        let result = explore_default(|_manager| {
            // Use the global manager through SymU64::new_global()
            let x = SymU64::new_global();
            let y = SymU64::new_global();

            let ten = SymU64::from_concrete(10, Arc::clone(&get_global_manager().unwrap()));
            x.assert_gt(&ten)?;
            y.assert_lt(&ten)?;

            Ok(())
        });

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.completed);
    }

    #[test]
    fn test_explore_result_statistics() {
        let result = explore_default(|manager| {
            let x = SymU64::new(Arc::clone(manager));
            let y = SymU64::new(Arc::clone(manager));

            let five = SymU64::from_concrete(5, Arc::clone(manager));
            let ten = SymU64::from_concrete(10, Arc::clone(manager));

            // x > 5 AND y < 10
            x.assert_gt(&five)?;
            y.assert_lt(&ten)?;

            Ok(())
        });

        assert!(result.is_ok());
        let result = result.unwrap();

        // Check that statistics are populated
        assert!(result.completed);
        assert!(result.manager_stats.variable_count > 0);
        assert!(result.manager_stats.constraint_count > 0);
        assert!(result.exploration_result.paths_explored > 0);
    }

    #[test]
    fn test_explore_path_forking() {
        let result = explore_default(|manager| {
            let x = SymU64::new(Arc::clone(manager));
            let zero = SymU64::from_concrete(0, Arc::clone(manager));

            // This comparison should fork execution into two paths
            if x == zero {
                // Path 1: x == 0
                // Just add the constraint, don't do anything else
            } else {
                // Path 2: x != 0
                // Just add the constraint, don't do anything else
            }

            Ok(())
        });

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.completed);

        // Should explore both paths (x == 0 and x != 0)
        assert_eq!(result.exploration_result.paths_explored, 2);
        assert_eq!(result.exploration_result.satisfiable_paths, 2);
    }

    #[test]
    fn test_explore_multiple_branches() {
        let result = explore_default(|manager| {
            let x = SymU64::new(Arc::clone(manager));
            let y = SymU64::new(Arc::clone(manager));
            let zero = SymU64::from_concrete(0, Arc::clone(manager));

            // First branch: x == 0 or x != 0
            if x == zero {
                x.assert_eq(&zero)?;
            } else {
                x.assert_ne(&zero)?;
            }

            // Second branch: y == 0 or y != 0
            if y == zero {
                y.assert_eq(&zero)?;
            } else {
                y.assert_ne(&zero)?;
            }

            Ok(())
        });

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.completed);

        // Should explore 4 paths: (x==0,y==0), (x==0,y!=0), (x!=0,y==0), (x!=0,y!=0)
        assert_eq!(result.exploration_result.paths_explored, 4);
    }

    #[test]
    fn test_explore_early_unsatisfiability_detection() {
        let result = explore_default(|manager| {
            let x = SymU64::new(Arc::clone(manager));
            let zero = SymU64::from_concrete(0, Arc::clone(manager));
            let one = SymU64::from_concrete(1, Arc::clone(manager));

            // Branch on x == 0
            if x == zero {
                // This path will become unsatisfiable immediately
                // because we already have x == 0 from the branch
                x.assert_eq(&one)?; // Contradiction: x == 0 AND x == 1

                // These operations should NOT be executed due to early termination
                let y = SymU64::new(Arc::clone(manager));
                let z = SymU64::new(Arc::clone(manager));
                y.assert_gt(&z)?;
            } else {
                // This path is satisfiable
                x.assert_ne(&zero)?;
            }

            Ok(())
        });

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.completed);

        // Should explore 2 paths
        assert_eq!(result.exploration_result.paths_explored, 2);
        // One satisfiable (x != 0), one unsatisfiable (x == 0 AND x == 1)
        assert_eq!(result.exploration_result.satisfiable_paths, 1);
        assert_eq!(result.exploration_result.unsatisfiable_paths, 1);
    }

    #[test]
    fn test_explore_multiple_contradictions() {
        let result = explore_default(|manager| {
            let x = SymU64::new(Arc::clone(manager));
            let zero = SymU64::from_concrete(0, Arc::clone(manager));
            let one = SymU64::from_concrete(1, Arc::clone(manager));
            let two = SymU64::from_concrete(2, Arc::clone(manager));

            if x == zero {
                // Contradiction
                x.assert_eq(&one)?;
            } else if x == one {
                // Contradiction
                x.assert_eq(&two)?;
            } else {
                // Satisfiable
                x.assert_ne(&zero)?;
                x.assert_ne(&one)?;
            }

            Ok(())
        });

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.completed);

        // Should have more unsatisfiable than satisfiable paths
        assert!(result.exploration_result.unsatisfiable_paths >= 2);
    }
}
