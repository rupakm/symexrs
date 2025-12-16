//! Global symbolic execution context management
//! 
//! This module provides the SymExManager which coordinates symbolic state,
//! path constraints, and SMT solver interface across the entire program execution.

use crate::expressions::SymExpr;
use crate::solver::SmtSolver;
use crate::error::{SymExResult, SymExError};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Global context manager for symbolic execution
pub struct SymExManager {
    /// Counter for generating unique variable names
    variable_counter: AtomicUsize,
    /// Current path constraints accumulated during execution
    path_constraints: Vec<SymExpr>,
    /// SMT solver interface for constraint satisfiability
    solver: Box<dyn SmtSolver>,
    /// Backtracking stack for path exploration
    backtrack_stack: Vec<ExecutionState>,
    /// Registry of symbolic variables and their metadata
    variable_registry: HashMap<String, TypeInfo>,
}

/// Represents a point in symbolic execution for backtracking
#[derive(Debug, Clone)]
pub struct ExecutionState {
    /// Path constraints at this execution point
    pub path_constraints: Vec<SymExpr>,
    /// Variable bindings at this execution point
    pub variable_bindings: HashMap<String, SymExpr>,
    /// Solver state identifier for restoration
    pub solver_state_id: usize,
    /// Branch condition that led to this state
    pub branch_condition: Option<SymExpr>,
}

/// Metadata about symbolic variables
#[derive(Debug, Clone)]
pub struct TypeInfo {
    /// Type name (e.g., "u64", "i32", "bool")
    pub type_name: String,
    /// Bit width for numeric types
    pub bit_width: Option<usize>,
    /// Whether the type is signed
    pub is_signed: bool,
    /// Optional creation site for debugging
    pub creation_site: Option<String>,
}

impl SymExManager {
    /// Create a new SymExManager with the given solver
    pub fn new(solver: Box<dyn SmtSolver>) -> Self {
        Self {
            variable_counter: AtomicUsize::new(0),
            path_constraints: Vec::new(),
            solver,
            backtrack_stack: Vec::new(),
            variable_registry: HashMap::new(),
        }
    }

    /// Generate a unique symbolic variable name
    pub fn fresh_variable(&self, type_name: &str) -> String {
        let id = self.variable_counter.fetch_add(1, Ordering::SeqCst);
        format!("{}_{}", type_name, id)
    }

    /// Register a new symbolic variable with metadata
    pub fn register_variable(&mut self, name: String, type_info: TypeInfo) -> SymExResult<()> {
        if self.variable_registry.contains_key(&name) {
            return Err(SymExError::DuplicateVariable(name));
        }
        self.variable_registry.insert(name, type_info);
        Ok(())
    }

    /// Add a constraint to the current path
    pub fn add_constraint(&mut self, constraint: SymExpr) -> SymExResult<()> {
        self.path_constraints.push(constraint);
        Ok(())
    }

    /// Get current path constraints
    pub fn get_constraints(&self) -> &[SymExpr] {
        &self.path_constraints
    }

    /// Save current execution state to backtracking stack
    pub fn push_state(&mut self, branch_condition: Option<SymExpr>) -> SymExResult<()> {
        let state = ExecutionState {
            path_constraints: self.path_constraints.clone(),
            variable_bindings: HashMap::new(), // Will be populated in later tasks
            solver_state_id: 0, // Will be managed by solver in later tasks
            branch_condition,
        };
        self.backtrack_stack.push(state);
        Ok(())
    }

    /// Restore execution state from backtracking stack
    pub fn pop_state(&mut self) -> SymExResult<ExecutionState> {
        self.backtrack_stack
            .pop()
            .ok_or(SymExError::StackUnderflow)
    }

    /// Check if backtracking stack is empty
    pub fn is_stack_empty(&self) -> bool {
        self.backtrack_stack.is_empty()
    }
}

// Additional manager functionality will be implemented in later tasks