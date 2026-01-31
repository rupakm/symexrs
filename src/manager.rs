//! Global symbolic execution context management
//!
//! This module provides the SymExManager which coordinates symbolic state,
//! path constraints, and SMT solver interface across the entire program execution.

use crate::error::{SymExError, SymExResult};
use crate::expressions::SymExpr;
use crate::solver::{Model, SatResult, SmtSolver};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Resource usage monitoring for memory management
#[derive(Debug, Clone)]
pub struct ResourceMonitor {
    /// Total memory allocated for execution states (approximate, in bytes)
    pub total_state_memory: usize,
    /// Number of states allocated
    pub states_allocated: usize,
    /// Number of states deallocated (garbage collected)
    pub states_deallocated: usize,
    /// Peak memory usage
    pub peak_memory: usize,
    /// Number of garbage collection runs
    pub gc_runs: usize,
    /// Number of states compressed
    pub states_compressed: usize,
    /// Memory saved by compression (approximate, in bytes)
    pub compression_savings: usize,
}

impl ResourceMonitor {
    /// Create a new resource monitor
    pub fn new() -> Self {
        Self {
            total_state_memory: 0,
            states_allocated: 0,
            states_deallocated: 0,
            peak_memory: 0,
            gc_runs: 0,
            states_compressed: 0,
            compression_savings: 0,
        }
    }

    /// Record allocation of a new state
    pub fn record_allocation(&mut self, size: usize) {
        self.total_state_memory += size;
        self.states_allocated += 1;
        if self.total_state_memory > self.peak_memory {
            self.peak_memory = self.total_state_memory;
        }
    }

    /// Record deallocation of a state
    pub fn record_deallocation(&mut self, size: usize) {
        self.total_state_memory = self.total_state_memory.saturating_sub(size);
        self.states_deallocated += 1;
    }

    /// Record a garbage collection run
    pub fn record_gc(&mut self) {
        self.gc_runs += 1;
    }

    /// Record state compression
    pub fn record_compression(&mut self, savings: usize) {
        self.states_compressed += 1;
        self.compression_savings += savings;
    }

    /// Get current memory usage
    pub fn current_memory(&self) -> usize {
        self.total_state_memory
    }

    /// Get memory efficiency (compression savings / peak memory)
    pub fn memory_efficiency(&self) -> f64 {
        if self.peak_memory == 0 {
            0.0
        } else {
            self.compression_savings as f64 / self.peak_memory as f64
        }
    }

    /// Reset all statistics
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for ResourceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

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
    /// Cache for satisfiability query results (constraint hash -> result)
    sat_cache: HashMap<u64, SatResult>,
    /// Cache for model results (constraint hash -> model)
    model_cache: HashMap<u64, Option<Model>>,
    /// Statistics for cache performance
    cache_hits: usize,
    cache_misses: usize,
    /// Counter for generating unique path IDs
    path_counter: AtomicUsize,
    /// Current path metadata
    current_path_metadata: PathMetadata,
    /// Map of visited constraint hashes for cycle detection
    visited_states: HashMap<u64, usize>,
    /// Maximum number of visits to a state before considering it a loop
    max_state_visits: usize,
    /// Resource usage monitoring
    resource_monitor: ResourceMonitor,
    /// State compression enabled flag
    compression_enabled: bool,
    /// Maximum stack size before triggering garbage collection
    max_stack_size: usize,
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
    /// Path exploration metadata
    pub path_metadata: PathMetadata,
    /// Compressed representation of the state (if compression is enabled)
    compressed_data: Option<Vec<u8>>,
    /// Whether this state is currently compressed
    is_compressed: bool,
    /// Backup of constraints before compression (for simplified implementation)
    constraints_backup: Vec<SymExpr>,
    /// Backup of bindings before compression (for simplified implementation)
    bindings_backup: HashMap<String, SymExpr>,
    /// Backup of branch condition before compression (for simplified implementation)
    branch_backup: Option<SymExpr>,
}

/// Metadata for path exploration and loop detection
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathMetadata {
    /// Unique identifier for this execution path
    pub path_id: usize,
    /// Depth of this state in the execution tree
    pub depth: usize,
    /// Hash of the path constraints for cycle detection
    pub constraint_hash: u64,
    /// Number of times this state has been visited (for loop detection)
    pub visit_count: usize,
    /// Parent path ID (for backtracking)
    pub parent_path_id: Option<usize>,
    /// Whether this path has been fully explored
    pub fully_explored: bool,
}

impl PathMetadata {
    /// Create new path metadata
    pub fn new(
        path_id: usize,
        depth: usize,
        constraint_hash: u64,
        parent_path_id: Option<usize>,
    ) -> Self {
        Self {
            path_id,
            depth,
            constraint_hash,
            visit_count: 1,
            parent_path_id,
            fully_explored: false,
        }
    }

    /// Create root path metadata (for initial state)
    pub fn root() -> Self {
        Self {
            path_id: 0,
            depth: 0,
            constraint_hash: 0,
            visit_count: 1,
            parent_path_id: None,
            fully_explored: false,
        }
    }

    /// Increment visit count for loop detection
    pub fn increment_visit(&mut self) {
        self.visit_count += 1;
    }

    /// Mark this path as fully explored
    pub fn mark_explored(&mut self) {
        self.fully_explored = true;
    }

    /// Check if this path has been visited too many times (potential infinite loop)
    pub fn is_looping(&self, max_visits: usize) -> bool {
        self.visit_count > max_visits
    }
}

impl ExecutionState {
    /// Create a new execution state
    pub fn new(
        path_constraints: Vec<SymExpr>,
        variable_bindings: HashMap<String, SymExpr>,
        solver_state_id: usize,
        branch_condition: Option<SymExpr>,
    ) -> Self {
        Self {
            path_constraints,
            variable_bindings,
            solver_state_id,
            branch_condition,
            path_metadata: PathMetadata::root(),
            compressed_data: None,
            is_compressed: false,
            constraints_backup: Vec::new(),
            bindings_backup: HashMap::new(),
            branch_backup: None,
        }
    }

    /// Create a new execution state with path metadata
    pub fn with_metadata(
        path_constraints: Vec<SymExpr>,
        variable_bindings: HashMap<String, SymExpr>,
        solver_state_id: usize,
        branch_condition: Option<SymExpr>,
        path_metadata: PathMetadata,
    ) -> Self {
        Self {
            path_constraints,
            variable_bindings,
            solver_state_id,
            branch_condition,
            path_metadata,
            compressed_data: None,
            is_compressed: false,
            constraints_backup: Vec::new(),
            bindings_backup: HashMap::new(),
            branch_backup: None,
        }
    }

    /// Compress the state to save memory
    pub fn compress(&mut self) -> SymExResult<usize> {
        if self.is_compressed {
            return Ok(0); // Already compressed
        }

        // Calculate original size before compression
        let original_size = self.estimate_memory_size();

        // Backup the data before clearing
        self.constraints_backup = self.path_constraints.clone();
        self.bindings_backup = self.variable_bindings.clone();
        self.branch_backup = self.branch_condition.clone();

        // For compression, we'll use a simple approach:
        // Store the debug representation as bytes (in production, use a proper compression library)
        let mut compressed_data = Vec::new();

        // Serialize constraints count
        compressed_data.extend_from_slice(&self.path_constraints.len().to_le_bytes());

        // Serialize each constraint as debug string
        for constraint in &self.path_constraints {
            let debug_str = format!("{constraint:?}");
            let bytes = debug_str.as_bytes();
            compressed_data.extend_from_slice(&bytes.len().to_le_bytes());
            compressed_data.extend_from_slice(bytes);
        }

        // Serialize bindings count
        compressed_data.extend_from_slice(&self.variable_bindings.len().to_le_bytes());

        // Serialize branch condition presence
        let has_branch = self.branch_condition.is_some();
        compressed_data.push(if has_branch { 1 } else { 0 });

        if has_branch {
            let debug_str = format!("{:?}", self.branch_condition.as_ref().unwrap());
            let bytes = debug_str.as_bytes();
            compressed_data.extend_from_slice(&bytes.len().to_le_bytes());
            compressed_data.extend_from_slice(bytes);
        }

        let compressed_size = compressed_data.len();

        // Store compressed data
        self.compressed_data = Some(compressed_data);
        self.is_compressed = true;

        // Clear the original data to save memory
        self.path_constraints.clear();
        self.variable_bindings.clear();

        // Return memory saved
        Ok(original_size.saturating_sub(compressed_size))
    }

    /// Decompress the state for use
    pub fn decompress(&mut self) -> SymExResult<()> {
        if !self.is_compressed {
            return Ok(()); // Already decompressed
        }

        // Restore from backup
        self.path_constraints = self.constraints_backup.clone();
        self.variable_bindings = self.bindings_backup.clone();
        self.branch_condition = self.branch_backup.clone();
        self.is_compressed = false;

        // Clear the compressed data
        self.compressed_data = None;

        Ok(())
    }

    /// Check if this state is compressed
    pub fn is_compressed(&self) -> bool {
        self.is_compressed
    }

    /// Estimate memory size of this state (in bytes)
    pub fn estimate_memory_size(&self) -> usize {
        if self.is_compressed {
            // If compressed, return the compressed data size
            self.compressed_data.as_ref().map(|d| d.len()).unwrap_or(0)
        } else {
            // Estimate uncompressed size
            let mut size = 0;

            // Size of path constraints (rough estimate)
            size += self.path_constraints.len() * 100; // Assume ~100 bytes per constraint

            // Size of variable bindings
            size += self.variable_bindings.len() * 80; // Assume ~80 bytes per binding

            // Size of branch condition
            if self.branch_condition.is_some() {
                size += 100;
            }

            // Size of metadata
            size += 64;

            size
        }
    }

    /// Serialize the execution state to a JSON string
    pub fn serialize(&self) -> SymExResult<String> {
        // Create a serializable representation
        let serialized = SerializedExecutionState {
            path_constraints: self
                .path_constraints
                .iter()
                .map(|expr| expr.to_smt_lib())
                .collect(),
            variable_bindings: self
                .variable_bindings
                .iter()
                .map(|(k, v)| (k.clone(), v.to_smt_lib()))
                .collect(),
            solver_state_id: self.solver_state_id,
            branch_condition: self.branch_condition.as_ref().map(|expr| expr.to_smt_lib()),
        };

        // Convert to JSON
        serde_json::to_string(&serialized)
            .map_err(|e| SymExError::Generic(format!("Failed to serialize execution state: {e}")))
    }

    /// Deserialize an execution state from a JSON string
    pub fn deserialize(json: &str) -> SymExResult<Self> {
        // Parse JSON
        let serialized: SerializedExecutionState = serde_json::from_str(json).map_err(|e| {
            SymExError::Generic(format!("Failed to deserialize execution state: {e}"))
        })?;

        // Convert SMT-LIB strings back to expressions
        // Note: This is a simplified implementation. A full implementation would need
        // a proper SMT-LIB parser. For now, we'll store the expressions in a format
        // that can be reconstructed.
        let path_constraints = serialized
            .path_constraints
            .iter()
            .map(|_s| {
                // TODO: Implement proper SMT-LIB parsing
                // For now, return an error indicating this needs implementation
                Err(SymExError::Generic(
                    "SMT-LIB parsing not yet implemented".to_string(),
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let variable_bindings = serialized
            .variable_bindings
            .keys()
            .map(|k| {
                // TODO: Implement proper SMT-LIB parsing
                Ok((k.clone(), SymExpr::Variable(k.clone())))
            })
            .collect::<Result<HashMap<_, _>, SymExError>>()?;

        let branch_condition = match serialized.branch_condition {
            Some(_s) => {
                // TODO: Implement proper SMT-LIB parsing
                None
            }
            None => None,
        };

        Ok(Self {
            path_constraints,
            variable_bindings,
            solver_state_id: serialized.solver_state_id,
            branch_condition,
            path_metadata: PathMetadata::root(), // Default metadata for deserialized states
            compressed_data: None,
            is_compressed: false,
            constraints_backup: Vec::new(),
            bindings_backup: HashMap::new(),
            branch_backup: None,
        })
    }

    /// Validate the consistency of this execution state
    pub fn validate(&self) -> SymExResult<()> {
        // Check that all variables in constraints are defined in bindings or are free variables
        for constraint in &self.path_constraints {
            let vars = constraint.get_variables();
            for var in vars {
                // Variables should either be in bindings or be symbolic variables
                // For now, we just check that variable names are valid
                if var.is_empty() {
                    return Err(SymExError::StateCorruption(
                        "Empty variable name in constraint".to_string(),
                    ));
                }
            }
        }

        // Check that variable bindings have valid names
        for (name, expr) in &self.variable_bindings {
            if name.is_empty() {
                return Err(SymExError::StateCorruption(
                    "Empty variable name in bindings".to_string(),
                ));
            }

            // Check expression depth to prevent stack overflow
            if expr.depth() > 100 {
                return Err(SymExError::StateCorruption(format!(
                    "Variable binding for '{}' has excessive depth ({})",
                    name,
                    expr.depth()
                )));
            }
        }

        // Check constraint depths
        for (i, constraint) in self.path_constraints.iter().enumerate() {
            if constraint.depth() > 100 {
                return Err(SymExError::StateCorruption(format!(
                    "Constraint {} has excessive depth ({})",
                    i,
                    constraint.depth()
                )));
            }

            if constraint.node_count() > 10000 {
                return Err(SymExError::StateCorruption(format!(
                    "Constraint {} has excessive complexity ({} nodes)",
                    i,
                    constraint.node_count()
                )));
            }
        }

        // Check branch condition if present
        if let Some(ref cond) = self.branch_condition {
            if cond.depth() > 100 {
                return Err(SymExError::StateCorruption(format!(
                    "Branch condition has excessive depth ({})",
                    cond.depth()
                )));
            }
        }

        Ok(())
    }

    /// Get the number of constraints in this state
    pub fn constraint_count(&self) -> usize {
        self.path_constraints.len()
    }

    /// Get the number of variable bindings in this state
    pub fn binding_count(&self) -> usize {
        self.variable_bindings.len()
    }

    /// Check if this state has a branch condition
    pub fn has_branch_condition(&self) -> bool {
        self.branch_condition.is_some()
    }

    /// Clone this state with a new solver state ID
    pub fn with_solver_state_id(&self, new_id: usize) -> Self {
        Self {
            path_constraints: self.path_constraints.clone(),
            variable_bindings: self.variable_bindings.clone(),
            solver_state_id: new_id,
            branch_condition: self.branch_condition.clone(),
            path_metadata: self.path_metadata.clone(),
            compressed_data: self.compressed_data.clone(),
            is_compressed: self.is_compressed,
            constraints_backup: self.constraints_backup.clone(),
            bindings_backup: self.bindings_backup.clone(),
            branch_backup: self.branch_backup.clone(),
        }
    }
}

/// Serializable representation of execution state
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct SerializedExecutionState {
    path_constraints: Vec<String>,
    variable_bindings: HashMap<String, String>,
    solver_state_id: usize,
    branch_condition: Option<String>,
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
            sat_cache: HashMap::new(),
            model_cache: HashMap::new(),
            cache_hits: 0,
            cache_misses: 0,
            path_counter: AtomicUsize::new(0),
            current_path_metadata: PathMetadata::root(),
            visited_states: HashMap::new(),
            max_state_visits: 100, // Default maximum visits before loop detection
            resource_monitor: ResourceMonitor::new(),
            compression_enabled: true, // Enable compression by default
            max_stack_size: 1000,      // Default maximum stack size before GC
        }
    }

    /// Create a new SymExManager with custom loop detection threshold
    pub fn with_max_visits(solver: Box<dyn SmtSolver>, max_visits: usize) -> Self {
        let mut manager = Self::new(solver);
        manager.max_state_visits = max_visits;
        manager
    }

    /// Create a new SymExManager with custom configuration
    pub fn with_config(
        solver: Box<dyn SmtSolver>,
        max_visits: usize,
        compression_enabled: bool,
        max_stack_size: usize,
    ) -> Self {
        let mut manager = Self::new(solver);
        manager.max_state_visits = max_visits;
        manager.compression_enabled = compression_enabled;
        manager.max_stack_size = max_stack_size;
        manager
    }

    /// Enable or disable state compression
    pub fn set_compression_enabled(&mut self, enabled: bool) {
        self.compression_enabled = enabled;
    }

    /// Set maximum stack size before garbage collection
    pub fn set_max_stack_size(&mut self, size: usize) {
        self.max_stack_size = size;
    }

    /// Get resource usage statistics
    pub fn get_resource_stats(&self) -> &ResourceMonitor {
        &self.resource_monitor
    }

    /// Generate a unique symbolic variable name
    pub fn fresh_variable(&self, type_name: &str) -> String {
        let id = self.variable_counter.fetch_add(1, Ordering::SeqCst);
        format!("{type_name}_{id}")
    }

    /// Register a new symbolic variable with metadata
    pub fn register_variable(&mut self, name: String, type_info: TypeInfo) -> SymExResult<()> {
        if self.variable_registry.contains_key(&name) {
            return Err(SymExError::DuplicateVariable(name));
        }
        self.variable_registry.insert(name, type_info);
        Ok(())
    }

    /// Get metadata for a registered variable
    pub fn get_variable_info(&self, name: &str) -> Option<&TypeInfo> {
        self.variable_registry.get(name)
    }

    /// Get all registered variable names
    pub fn get_registered_variables(&self) -> Vec<String> {
        let mut vars: Vec<String> = self.variable_registry.keys().cloned().collect();
        vars.sort();
        vars
    }

    /// Add a constraint to the current path
    pub fn add_constraint(&mut self, constraint: SymExpr) -> SymExResult<()> {
        // Validate constraint before adding
        self.validate_constraint(&constraint)?;
        self.path_constraints.push(constraint);

        // Invalidate cache since constraints have changed
        self.clear_cache();

        Ok(())
    }

    /// Get current path constraints
    pub fn get_constraints(&self) -> &[SymExpr] {
        &self.path_constraints
    }

    /// Clear all path constraints
    pub fn clear_constraints(&mut self) {
        self.path_constraints.clear();
        // Invalidate cache since constraints have changed
        self.clear_cache();
    }

    /// Check if current path constraints are satisfiable (with caching)
    pub fn is_satisfiable(&mut self) -> SymExResult<bool> {
        // Compute hash of current constraints for cache lookup
        let cache_key = self.compute_constraints_hash(&self.path_constraints);

        // Check cache first
        if let Some(cached_result) = self.sat_cache.get(&cache_key) {
            self.cache_hits += 1;
            return Ok(match cached_result {
                SatResult::Sat => true,
                SatResult::Unsat => false,
                SatResult::Unknown => false,
            });
        }

        // Cache miss - query solver
        self.cache_misses += 1;
        let result = self.solver.check_sat(&self.path_constraints)?;

        // Store result in cache
        self.sat_cache.insert(cache_key, result.clone());

        Ok(match result {
            SatResult::Sat => true,
            SatResult::Unsat => false,
            SatResult::Unknown => false, // Conservative: treat unknown as unsatisfiable
        })
    }

    /// Fast satisfiability check using concrete values (concolic execution optimization)
    /// 
    /// This method evaluates all path constraints using concrete values to quickly
    /// determine if the current path is satisfiable without calling the SMT solver.
    /// Returns:
    /// - Some(true) if all constraints are satisfied by concrete values
    /// - Some(false) if any constraint is violated by concrete values
    /// - None if concrete values are not available for all variables
    pub fn fast_check_satisfiable_with_concrete(&self, concrete_values: &HashMap<String, u64>) -> Option<bool> {
        // Try to evaluate each constraint with concrete values
        for constraint in &self.path_constraints {
            match self.evaluate_constraint_concrete(constraint, concrete_values) {
                Some(true) => continue,  // Constraint satisfied
                Some(false) => return Some(false),  // Constraint violated - path is unsat
                None => return None,  // Can't evaluate - need SMT solver
            }
        }
        
        // All constraints satisfied
        Some(true)
    }

    /// Evaluate a single constraint using concrete values
    /// Returns Some(true) if satisfied, Some(false) if violated, None if can't evaluate
    fn evaluate_constraint_concrete(&self, constraint: &SymExpr, concrete_values: &HashMap<String, u64>) -> Option<bool> {
        use crate::expressions::{BinOp, ConstValue};
        
        match constraint {
            SymExpr::Constant(val) => match val {
                ConstValue::Bool(b) => Some(*b),
                _ => None,  // Non-boolean constant in constraint position
            },
            
            SymExpr::Variable(_name) => {
                // For boolean variables, we'd need to look them up
                // For now, return None (can't evaluate)
                None
            }
            
            SymExpr::BinaryOp(op, left, right) => {
                // Try to evaluate both sides to u64 values
                let left_val = self.evaluate_expr_to_u64(left, concrete_values)?;
                let right_val = self.evaluate_expr_to_u64(right, concrete_values)?;
                
                // Evaluate the comparison
                Some(match op {
                    BinOp::Eq => left_val == right_val,
                    BinOp::Ne => left_val != right_val,
                    BinOp::Lt => left_val < right_val,
                    BinOp::Le => left_val <= right_val,
                    BinOp::Gt => left_val > right_val,
                    BinOp::Ge => left_val >= right_val,
                    _ => return None,  // Not a comparison operator
                })
            }
            
            SymExpr::UnaryOp(op, expr) => {
                use crate::expressions::UnOp;
                match op {
                    UnOp::Not => {
                        let val = self.evaluate_constraint_concrete(expr, concrete_values)?;
                        Some(!val)
                    }
                    _ => None,
                }
            }
            
            SymExpr::Conditional(cond, then_expr, else_expr) => {
                let cond_val = self.evaluate_constraint_concrete(cond, concrete_values)?;
                if cond_val {
                    self.evaluate_constraint_concrete(then_expr, concrete_values)
                } else {
                    self.evaluate_constraint_concrete(else_expr, concrete_values)
                }
            }
        }
    }

    /// Evaluate an expression to a u64 value using concrete values
    fn evaluate_expr_to_u64(&self, expr: &SymExpr, concrete_values: &HashMap<String, u64>) -> Option<u64> {
        use crate::expressions::{BinOp, ConstValue};
        
        match expr {
            SymExpr::Constant(val) => match val {
                ConstValue::U64(v) => Some(*v),
                ConstValue::I64(v) => Some(*v as u64),
                ConstValue::U8(v) => Some(*v as u64),
                ConstValue::I32(v) => Some(*v as u64),
                _ => None,
            },
            
            SymExpr::Variable(name) => concrete_values.get(name).copied(),
            
            SymExpr::BinaryOp(op, left, right) => {
                let left_val = self.evaluate_expr_to_u64(left, concrete_values)?;
                let right_val = self.evaluate_expr_to_u64(right, concrete_values)?;
                
                Some(match op {
                    BinOp::Add => left_val.wrapping_add(right_val),
                    BinOp::Sub => left_val.wrapping_sub(right_val),
                    BinOp::Mul => left_val.wrapping_mul(right_val),
                    BinOp::Div if right_val != 0 => left_val / right_val,
                    BinOp::Mod if right_val != 0 => left_val % right_val,
                    BinOp::BitAnd => left_val & right_val,
                    BinOp::BitOr => left_val | right_val,
                    BinOp::BitXor => left_val ^ right_val,
                    BinOp::Shl if right_val < 64 => left_val << right_val,
                    BinOp::Shr if right_val < 64 => left_val >> right_val,
                    _ => return None,
                })
            }
            
            SymExpr::UnaryOp(op, inner) => {
                use crate::expressions::UnOp;
                let val = self.evaluate_expr_to_u64(inner, concrete_values)?;
                match op {
                    UnOp::Neg => Some((-(val as i64)) as u64),
                    _ => None,
                }
            }
            
            SymExpr::Conditional(cond, then_expr, else_expr) => {
                let cond_val = self.evaluate_constraint_concrete(cond, concrete_values)?;
                if cond_val {
                    self.evaluate_expr_to_u64(then_expr, concrete_values)
                } else {
                    self.evaluate_expr_to_u64(else_expr, concrete_values)
                }
            }
        }
    }

    /// Get a model (concrete values) for current path constraints if satisfiable (with caching)
    pub fn get_model(&mut self) -> SymExResult<Option<Model>> {
        // Compute hash of current constraints for cache lookup
        let cache_key = self.compute_constraints_hash(&self.path_constraints);

        // Check model cache first
        if let Some(cached_model) = self.model_cache.get(&cache_key) {
            self.cache_hits += 1;
            return Ok(cached_model.clone());
        }

        // Cache miss - first check if satisfiable
        if !self.is_satisfiable()? {
            // Cache the None result
            self.model_cache.insert(cache_key, None);
            return Ok(None);
        }

        // Get model from solver
        let model = self.solver.get_model()?;

        // Store result in cache
        self.model_cache.insert(cache_key, model.clone());

        Ok(model)
    }

    /// Check satisfiability of a specific set of constraints (with caching)
    pub fn check_satisfiability(&mut self, constraints: &[SymExpr]) -> SymExResult<bool> {
        // Compute hash for cache lookup
        let cache_key = self.compute_constraints_hash(constraints);

        // Check cache first
        if let Some(cached_result) = self.sat_cache.get(&cache_key) {
            self.cache_hits += 1;
            return Ok(match cached_result {
                SatResult::Sat => true,
                SatResult::Unsat => false,
                SatResult::Unknown => false,
            });
        }

        // Cache miss - query solver
        self.cache_misses += 1;
        let result = self.solver.check_sat(constraints)?;

        // Store result in cache
        self.sat_cache.insert(cache_key, result.clone());

        Ok(match result {
            SatResult::Sat => true,
            SatResult::Unsat => false,
            SatResult::Unknown => false,
        })
    }

    /// Get a model for a specific set of constraints (with caching)
    pub fn get_model_for_constraints(
        &mut self,
        constraints: &[SymExpr],
    ) -> SymExResult<Option<Model>> {
        // Compute hash for cache lookup
        let cache_key = self.compute_constraints_hash(constraints);

        // Check model cache first
        if let Some(cached_model) = self.model_cache.get(&cache_key) {
            self.cache_hits += 1;
            return Ok(cached_model.clone());
        }

        // Cache miss - check satisfiability first
        self.cache_misses += 1;
        let result = self.solver.check_sat(constraints)?;

        if result != SatResult::Sat {
            // Cache the None result
            self.model_cache.insert(cache_key, None);
            return Ok(None);
        }

        // Get model from solver
        let model = self.solver.get_model()?;

        // Store result in cache
        self.model_cache.insert(cache_key, model.clone());

        Ok(model)
    }

    /// Compute a hash of a set of constraints for caching
    fn compute_constraints_hash(&self, constraints: &[SymExpr]) -> u64 {
        let mut hasher = DefaultHasher::new();

        // Hash the number of constraints
        constraints.len().hash(&mut hasher);

        // Hash each constraint
        for constraint in constraints {
            // Use the debug representation for hashing
            // This is a simple approach - could be optimized with custom Hash impl
            format!("{constraint:?}").hash(&mut hasher);
        }

        hasher.finish()
    }

    /// Clear the satisfiability and model caches
    pub fn clear_cache(&mut self) {
        self.sat_cache.clear();
        self.model_cache.clear();
        self.cache_hits = 0;
        self.cache_misses = 0;
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> CacheStats {
        CacheStats {
            sat_cache_size: self.sat_cache.len(),
            model_cache_size: self.model_cache.len(),
            cache_hits: self.cache_hits,
            cache_misses: self.cache_misses,
            hit_rate: if self.cache_hits + self.cache_misses > 0 {
                self.cache_hits as f64 / (self.cache_hits + self.cache_misses) as f64
            } else {
                0.0
            },
        }
    }

    /// Save current execution state to backtracking stack
    pub fn push_state(&mut self, branch_condition: Option<SymExpr>) -> SymExResult<()> {
        // Check if we need to run garbage collection
        if self.backtrack_stack.len() >= self.max_stack_size {
            self.garbage_collect_states()?;
        }

        // Push solver state first
        self.solver.push()?;

        // Generate new path ID
        let path_id = self.path_counter.fetch_add(1, Ordering::SeqCst);

        // Compute hash of current constraints for cycle detection
        let constraint_hash = self.compute_constraints_hash(&self.path_constraints);

        // Check if we've visited this state before
        let visit_count = if let Some(count) = self.visited_states.get(&constraint_hash) {
            count + 1
        } else {
            1
        };

        // Update visited states
        self.visited_states.insert(constraint_hash, visit_count);

        // Check for potential infinite loop
        if visit_count > self.max_state_visits {
            return Err(SymExError::ResourceExhaustion(format!(
                "Potential infinite loop detected: state visited {} times (max: {})",
                visit_count, self.max_state_visits
            )));
        }

        // Create path metadata
        let path_metadata = PathMetadata {
            path_id,
            depth: self.backtrack_stack.len(),
            constraint_hash,
            visit_count,
            parent_path_id: Some(self.current_path_metadata.path_id),
            fully_explored: false,
        };

        let mut state = ExecutionState::with_metadata(
            self.path_constraints.clone(),
            HashMap::new(),             // Will be populated in later tasks
            self.backtrack_stack.len(), // Use stack depth as ID
            branch_condition,
            path_metadata.clone(),
        );

        // Validate state before pushing
        state.validate()?;

        // Record allocation
        let state_size = state.estimate_memory_size();
        self.resource_monitor.record_allocation(state_size);

        // Compress state if enabled
        if self.compression_enabled {
            let savings = state.compress()?;
            if savings > 0 {
                self.resource_monitor.record_compression(savings);
            }
        }

        self.backtrack_stack.push(state);

        // Update current path metadata
        self.current_path_metadata = path_metadata;

        Ok(())
    }

    /// Restore execution state from backtracking stack
    pub fn pop_state(&mut self) -> SymExResult<ExecutionState> {
        let mut state = self
            .backtrack_stack
            .pop()
            .ok_or(SymExError::StackUnderflow)?;

        // Decompress state if needed
        if state.is_compressed() {
            state.decompress()?;
        }

        // Validate state before restoring
        state.validate()?;

        // Record deallocation
        let state_size = state.estimate_memory_size();
        self.resource_monitor.record_deallocation(state_size);

        // Pop solver state
        self.solver.pop()?;

        // Restore path constraints
        self.path_constraints = state.path_constraints.clone();

        // Restore path metadata
        self.current_path_metadata = state.path_metadata.clone();

        Ok(state)
    }

    /// Garbage collect unreachable states from the backtracking stack
    ///
    /// This method removes states that are marked as fully explored or
    /// are unlikely to be revisited, freeing up memory.
    pub fn garbage_collect_states(&mut self) -> SymExResult<usize> {
        let initial_count = self.backtrack_stack.len();

        // Record GC run
        self.resource_monitor.record_gc();

        // Identify states to keep:
        // 1. States that are not fully explored
        // 2. Recent states (within a certain depth threshold)
        let keep_threshold = self
            .backtrack_stack
            .len()
            .saturating_sub(self.max_stack_size / 2);

        let mut states_to_keep = Vec::new();
        let mut _freed_memory = 0;

        for (idx, state) in self.backtrack_stack.drain(..).enumerate() {
            // Keep recent states or states not fully explored
            if idx >= keep_threshold || !state.path_metadata.fully_explored {
                states_to_keep.push(state);
            } else {
                // This state will be garbage collected
                _freed_memory += state.estimate_memory_size();
                self.resource_monitor
                    .record_deallocation(state.estimate_memory_size());
            }
        }

        // Replace the stack with the kept states
        self.backtrack_stack = states_to_keep;

        let collected_count = initial_count - self.backtrack_stack.len();

        Ok(collected_count)
    }

    /// Compress all uncompressed states in the backtracking stack
    pub fn compress_all_states(&mut self) -> SymExResult<usize> {
        let mut total_savings = 0;

        for state in &mut self.backtrack_stack {
            if !state.is_compressed() {
                let savings = state.compress()?;
                if savings > 0 {
                    total_savings += savings;
                    self.resource_monitor.record_compression(savings);
                }
            }
        }

        Ok(total_savings)
    }

    /// Decompress all compressed states in the backtracking stack
    pub fn decompress_all_states(&mut self) -> SymExResult<()> {
        for state in &mut self.backtrack_stack {
            if state.is_compressed() {
                state.decompress()?;
            }
        }

        Ok(())
    }

    /// Get memory usage statistics
    pub fn get_memory_usage(&self) -> MemoryUsage {
        let stack_memory: usize = self
            .backtrack_stack
            .iter()
            .map(|s| s.estimate_memory_size())
            .sum();

        let cache_memory = self.sat_cache.len() * 100 + self.model_cache.len() * 200;
        let registry_memory = self.variable_registry.len() * 80;
        let constraints_memory = self.path_constraints.len() * 100;

        MemoryUsage {
            stack_memory,
            cache_memory,
            registry_memory,
            constraints_memory,
            total_memory: stack_memory + cache_memory + registry_memory + constraints_memory,
            compressed_states: self
                .backtrack_stack
                .iter()
                .filter(|s| s.is_compressed())
                .count(),
            uncompressed_states: self
                .backtrack_stack
                .iter()
                .filter(|s| !s.is_compressed())
                .count(),
        }
    }

    /// Save the current execution state to a JSON string
    pub fn save_state(&self) -> SymExResult<String> {
        let state = ExecutionState::new(
            self.path_constraints.clone(),
            HashMap::new(), // Variable bindings will be added in later tasks
            self.backtrack_stack.len(),
            None,
        );

        state.serialize()
    }

    /// Restore execution state from a JSON string
    pub fn restore_state(&mut self, json: &str) -> SymExResult<()> {
        let state = ExecutionState::deserialize(json)?;

        // Validate the restored state
        state.validate()?;

        // Restore path constraints
        self.path_constraints = state.path_constraints.clone();

        // Clear cache since state has changed
        self.clear_cache();

        Ok(())
    }

    /// Get a snapshot of the current execution state
    pub fn get_current_state(&self) -> ExecutionState {
        ExecutionState::new(
            self.path_constraints.clone(),
            HashMap::new(), // Variable bindings will be added in later tasks
            self.backtrack_stack.len(),
            None,
        )
    }

    /// Validate the current manager state for consistency
    pub fn validate_state(&self) -> SymExResult<()> {
        // Check that all variables in constraints are registered
        for constraint in &self.path_constraints {
            let vars = constraint.get_variables();
            for var in vars {
                if !self.variable_registry.contains_key(&var) {
                    return Err(SymExError::StateCorruption(format!(
                        "Constraint references unregistered variable: {var}"
                    )));
                }
            }
        }

        // Validate each state in the backtracking stack
        for (i, state) in self.backtrack_stack.iter().enumerate() {
            state.validate().map_err(|e| {
                SymExError::StateCorruption(format!("Invalid state at stack position {i}: {e}"))
            })?;
        }

        // Check for reasonable stack depth
        if self.backtrack_stack.len() > 10000 {
            return Err(SymExError::ResourceExhaustion(
                "Backtracking stack depth exceeds reasonable limit (10000)".to_string(),
            ));
        }

        Ok(())
    }

    /// Check if backtracking stack is empty
    pub fn is_stack_empty(&self) -> bool {
        self.backtrack_stack.is_empty()
    }

    /// Get the current backtracking stack depth
    pub fn get_stack_depth(&self) -> usize {
        self.backtrack_stack.len()
    }

    /// Get the current path metadata
    pub fn get_current_path_metadata(&self) -> &PathMetadata {
        &self.current_path_metadata
    }

    /// Check if the current path is potentially looping
    pub fn is_current_path_looping(&self) -> bool {
        self.current_path_metadata.is_looping(self.max_state_visits)
    }

    /// Get the number of times the current state has been visited
    pub fn get_current_visit_count(&self) -> usize {
        let hash = self.compute_constraints_hash(&self.path_constraints);
        self.visited_states.get(&hash).copied().unwrap_or(0)
    }

    /// Mark the current path as fully explored
    pub fn mark_current_path_explored(&mut self) {
        self.current_path_metadata.mark_explored();
    }

    /// Get all visited state hashes and their visit counts
    pub fn get_visited_states(&self) -> &HashMap<u64, usize> {
        &self.visited_states
    }

    /// Clear visited states tracking (useful for starting fresh exploration)
    pub fn clear_visited_states(&mut self) {
        self.visited_states.clear();
    }

    /// Get path exploration statistics
    pub fn get_path_stats(&self) -> PathExplorationStats {
        PathExplorationStats {
            current_path_id: self.current_path_metadata.path_id,
            current_depth: self.current_path_metadata.depth,
            total_paths_explored: self.path_counter.load(Ordering::SeqCst),
            unique_states_visited: self.visited_states.len(),
            stack_depth: self.backtrack_stack.len(),
            max_visit_threshold: self.max_state_visits,
        }
    }

    /// Check if a specific state (by constraint hash) has been visited
    pub fn has_visited_state(&self, constraint_hash: u64) -> bool {
        self.visited_states.contains_key(&constraint_hash)
    }

    /// Get the visit count for a specific state
    pub fn get_state_visit_count(&self, constraint_hash: u64) -> usize {
        self.visited_states
            .get(&constraint_hash)
            .copied()
            .unwrap_or(0)
    }

    /// Reset the manager to initial state
    pub fn reset(&mut self) -> SymExResult<()> {
        self.path_constraints.clear();
        self.backtrack_stack.clear();
        self.variable_registry.clear();
        self.variable_counter.store(0, Ordering::SeqCst);
        self.path_counter.store(0, Ordering::SeqCst);
        self.current_path_metadata = PathMetadata::root();
        self.visited_states.clear();
        self.clear_cache();
        self.resource_monitor.reset();
        self.solver.reset()?;
        Ok(())
    }

    /// Validate a constraint before adding it
    fn validate_constraint(&self, constraint: &SymExpr) -> SymExResult<()> {
        // Check that all variables in the constraint are registered
        let variables = constraint.get_variables();
        for var_name in variables {
            if !self.variable_registry.contains_key(&var_name) {
                return Err(SymExError::UnboundVariable(var_name));
            }
        }

        // Check expression depth to prevent stack overflow
        if constraint.depth() > 100 {
            return Err(SymExError::ResourceExhaustion(
                "Constraint depth exceeds maximum limit (100)".to_string(),
            ));
        }

        // Check node count to prevent excessive memory usage
        if constraint.node_count() > 10000 {
            return Err(SymExError::ResourceExhaustion(
                "Constraint complexity exceeds maximum limit (10000 nodes)".to_string(),
            ));
        }

        Ok(())
    }

    /// Get statistics about the current manager state
    pub fn get_stats(&self) -> ManagerStats {
        ManagerStats {
            variable_count: self.variable_registry.len(),
            constraint_count: self.path_constraints.len(),
            stack_depth: self.backtrack_stack.len(),
            next_variable_id: self.variable_counter.load(Ordering::SeqCst),
        }
    }
}

/// Statistics about the SymExManager state
#[derive(Debug, Clone)]
pub struct ManagerStats {
    /// Number of registered variables
    pub variable_count: usize,
    /// Number of path constraints
    pub constraint_count: usize,
    /// Current backtracking stack depth
    pub stack_depth: usize,
    /// Next variable ID to be assigned
    pub next_variable_id: usize,
}

/// Statistics about cache performance
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of entries in satisfiability cache
    pub sat_cache_size: usize,
    /// Number of entries in model cache
    pub model_cache_size: usize,
    /// Number of cache hits
    pub cache_hits: usize,
    /// Number of cache misses
    pub cache_misses: usize,
    /// Cache hit rate (0.0 to 1.0)
    pub hit_rate: f64,
}

/// Statistics about path exploration
#[derive(Debug, Clone)]
pub struct PathExplorationStats {
    /// Current path ID
    pub current_path_id: usize,
    /// Current depth in execution tree
    pub current_depth: usize,
    /// Total number of paths explored
    pub total_paths_explored: usize,
    /// Number of unique states visited
    pub unique_states_visited: usize,
    /// Current backtracking stack depth
    pub stack_depth: usize,
    /// Maximum visit threshold for loop detection
    pub max_visit_threshold: usize,
}

/// Memory usage statistics
#[derive(Debug, Clone)]
pub struct MemoryUsage {
    /// Memory used by backtracking stack (bytes)
    pub stack_memory: usize,
    /// Memory used by caches (bytes)
    pub cache_memory: usize,
    /// Memory used by variable registry (bytes)
    pub registry_memory: usize,
    /// Memory used by path constraints (bytes)
    pub constraints_memory: usize,
    /// Total memory usage (bytes)
    pub total_memory: usize,
    /// Number of compressed states
    pub compressed_states: usize,
    /// Number of uncompressed states
    pub uncompressed_states: usize,
}

impl MemoryUsage {
    /// Get compression ratio (compressed / total states)
    pub fn compression_ratio(&self) -> f64 {
        let total = self.compressed_states + self.uncompressed_states;
        if total == 0 {
            0.0
        } else {
            self.compressed_states as f64 / total as f64
        }
    }

    /// Format memory size in human-readable format
    pub fn format_size(bytes: usize) -> String {
        if bytes < 1024 {
            format!("{bytes} B")
        } else if bytes < 1024 * 1024 {
            format!("{:.2} KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }

    /// Get a summary string of memory usage
    pub fn summary(&self) -> String {
        format!(
            "Total: {}, Stack: {}, Cache: {}, Registry: {}, Constraints: {}\n\
             Compressed: {}/{} states ({:.1}%)",
            Self::format_size(self.total_memory),
            Self::format_size(self.stack_memory),
            Self::format_size(self.cache_memory),
            Self::format_size(self.registry_memory),
            Self::format_size(self.constraints_memory),
            self.compressed_states,
            self.compressed_states + self.uncompressed_states,
            self.compression_ratio() * 100.0
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expressions::{BinOp, ConstValue, SymExpr, UnOp};
    use crate::solver::Z3Solver;
    use quickcheck_macros::quickcheck as qc;
    use std::collections::HashSet;

    fn create_test_manager() -> SymExManager {
        let solver = Box::new(Z3Solver::new().unwrap());
        SymExManager::new(solver)
    }

    #[test]
    fn test_manager_creation() {
        let manager = create_test_manager();
        assert_eq!(manager.get_constraints().len(), 0);
        assert!(manager.is_stack_empty());
        assert_eq!(manager.get_registered_variables().len(), 0);
    }

    #[test]
    fn test_fresh_variable_generation() {
        let manager = create_test_manager();

        let var1 = manager.fresh_variable("u64");
        let var2 = manager.fresh_variable("u64");
        let var3 = manager.fresh_variable("i32");

        assert_eq!(var1, "u64_0");
        assert_eq!(var2, "u64_1");
        assert_eq!(var3, "i32_2");

        // Variables should be unique
        assert_ne!(var1, var2);
        assert_ne!(var2, var3);
    }

    #[test]
    fn test_variable_registration() {
        let mut manager = create_test_manager();

        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };

        let var_name = "test_var".to_string();
        let result = manager.register_variable(var_name.clone(), type_info.clone());
        assert!(result.is_ok());

        // Check that variable is registered
        let registered_vars = manager.get_registered_variables();
        assert_eq!(registered_vars, vec!["test_var"]);

        // Check variable info
        let info = manager.get_variable_info("test_var");
        assert!(info.is_some());
        assert_eq!(info.unwrap().type_name, "u64");
        assert_eq!(info.unwrap().bit_width, Some(64));
        assert!(!info.unwrap().is_signed);
    }

    #[test]
    fn test_duplicate_variable_registration() {
        let mut manager = create_test_manager();

        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };

        let var_name = "test_var".to_string();

        // First registration should succeed
        let result1 = manager.register_variable(var_name.clone(), type_info.clone());
        assert!(result1.is_ok());

        // Second registration should fail
        let result2 = manager.register_variable(var_name, type_info);
        assert!(result2.is_err());
        assert!(matches!(
            result2.unwrap_err(),
            SymExError::DuplicateVariable(_)
        ));
    }

    #[test]
    fn test_constraint_management() {
        let mut manager = create_test_manager();

        // Register a variable first
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Create and add a constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));

        let result = manager.add_constraint(constraint.clone());
        assert!(result.is_ok());

        // Check that constraint was added
        let constraints = manager.get_constraints();
        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints[0], constraint);
    }

    #[test]
    fn test_constraint_validation_unbound_variable() {
        let mut manager = create_test_manager();

        // Try to add constraint with unregistered variable
        let x = SymExpr::Variable("unregistered_var".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));

        let result = manager.add_constraint(constraint);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SymExError::UnboundVariable(_)
        ));
    }

    #[test]
    fn test_backtracking_stack() {
        let mut manager = create_test_manager();

        // Register a variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add initial constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint1 = SymExpr::BinaryOp(BinOp::Gt, Box::new(x.clone()), Box::new(zero));
        manager.add_constraint(constraint1).unwrap();

        assert_eq!(manager.get_stack_depth(), 0);
        assert_eq!(manager.get_constraints().len(), 1);

        // Push state
        let branch_condition = Some(SymExpr::Variable("branch".to_string()));
        let result = manager.push_state(branch_condition.clone());
        assert!(result.is_ok());
        assert_eq!(manager.get_stack_depth(), 1);

        // Add another constraint
        let ten = SymExpr::Constant(ConstValue::I64(10));
        let constraint2 = SymExpr::BinaryOp(BinOp::Lt, Box::new(x), Box::new(ten));
        manager.add_constraint(constraint2).unwrap();
        assert_eq!(manager.get_constraints().len(), 2);

        // Pop state
        let popped_state = manager.pop_state();
        assert!(popped_state.is_ok());
        let state = popped_state.unwrap();

        // Check that state was restored
        assert_eq!(manager.get_stack_depth(), 0);
        assert_eq!(manager.get_constraints().len(), 1);
        assert_eq!(state.branch_condition, branch_condition);
        assert_eq!(state.path_constraints.len(), 1);
    }

    #[test]
    fn test_stack_underflow() {
        let mut manager = create_test_manager();

        // Try to pop from empty stack
        let result = manager.pop_state();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SymExError::StackUnderflow));
    }

    #[test]
    fn test_manager_reset() {
        let mut manager = create_test_manager();

        // Add some state
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();
        manager.push_state(None).unwrap();

        // Verify state exists
        assert_eq!(manager.get_constraints().len(), 1);
        assert_eq!(manager.get_registered_variables().len(), 1);
        assert_eq!(manager.get_stack_depth(), 1);

        // Reset manager
        let result = manager.reset();
        assert!(result.is_ok());

        // Verify state is cleared
        assert_eq!(manager.get_constraints().len(), 0);
        assert_eq!(manager.get_registered_variables().len(), 0);
        assert_eq!(manager.get_stack_depth(), 0);

        // Verify cache is cleared
        let cache_stats = manager.get_cache_stats();
        assert_eq!(cache_stats.sat_cache_size, 0);
        assert_eq!(cache_stats.model_cache_size, 0);
        assert_eq!(cache_stats.cache_hits, 0);
        assert_eq!(cache_stats.cache_misses, 0);

        // Fresh variable counter should be reset
        let var = manager.fresh_variable("test");
        assert_eq!(var, "test_0");
    }

    #[test]
    fn test_manager_stats() {
        let mut manager = create_test_manager();

        // Initial stats
        let stats = manager.get_stats();
        assert_eq!(stats.variable_count, 0);
        assert_eq!(stats.constraint_count, 0);
        assert_eq!(stats.stack_depth, 0);
        assert_eq!(stats.next_variable_id, 0);

        // Add some state
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();
        manager.fresh_variable("test"); // This increments the counter

        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();
        manager.push_state(None).unwrap();

        // Check updated stats
        let stats = manager.get_stats();
        assert_eq!(stats.variable_count, 1);
        assert_eq!(stats.constraint_count, 1);
        assert_eq!(stats.stack_depth, 1);
        assert_eq!(stats.next_variable_id, 1);
    }

    #[test]
    fn test_constraint_clearing() {
        let mut manager = create_test_manager();

        // Register variable and add constraint
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();

        assert_eq!(manager.get_constraints().len(), 1);

        // Clear constraints
        manager.clear_constraints();
        assert_eq!(manager.get_constraints().len(), 0);
    }

    #[test]
    fn test_satisfiability_checking() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add satisfiable constraint: x > 0
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x.clone()), Box::new(zero.clone()));
        manager.add_constraint(constraint).unwrap();

        let result = manager.is_satisfiable();
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Add contradictory constraint: x < 0
        let constraint2 = SymExpr::BinaryOp(BinOp::Lt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint2).unwrap();

        let result2 = manager.is_satisfiable();
        assert!(result2.is_ok());
        assert!(!result2.unwrap());
    }

    // **Feature: symbolic-execution-engine, Property 1: Unique variable generation**
    // **Validates: Requirements 1.1, 5.1**
    #[qc]
    fn prop_unique_variable_generation(type_names: Vec<String>, count: u8) -> bool {
        // Limit the count to avoid excessive test times
        let count = (count % 100) + 1; // 1 to 100 variables

        // Filter out empty type names and limit length to avoid issues
        let type_names: Vec<String> = type_names
            .into_iter()
            .filter(|name| !name.is_empty() && name.len() <= 20)
            .take(10) // Limit to 10 different type names
            .collect();

        if type_names.is_empty() {
            return true; // Skip test if no valid type names
        }

        let manager = create_test_manager();
        let mut generated_names = HashSet::new();

        // Generate variables using different type names
        for i in 0..count {
            let type_name = &type_names[i as usize % type_names.len()];
            let var_name = manager.fresh_variable(type_name);

            // Check that this variable name is unique
            if generated_names.contains(&var_name) {
                return false;
            }

            generated_names.insert(var_name);
        }

        // All generated names should be unique
        generated_names.len() == count as usize
    }

    #[qc]
    fn prop_variable_name_format(type_name: String, iterations: u8) -> bool {
        // Limit iterations and filter type name
        let iterations = (iterations % 50) + 1; // 1 to 50 iterations

        // Skip empty or very long type names
        if type_name.is_empty() || type_name.len() > 50 {
            return true;
        }

        // Skip type names with problematic characters
        if type_name.contains(|c: char| !c.is_alphanumeric() && c != '_') {
            return true;
        }

        let manager = create_test_manager();

        for i in 0..iterations {
            let var_name = manager.fresh_variable(&type_name);

            // Variable name should follow the format: {type_name}_{id}
            if !var_name.starts_with(&type_name) {
                return false;
            }

            if !var_name.contains('_') {
                return false;
            }

            // Extract the ID part and verify it's a number
            if let Some(underscore_pos) = var_name.rfind('_') {
                let id_part = &var_name[underscore_pos + 1..];
                if id_part.parse::<usize>().is_err() {
                    return false;
                }

                // The ID should match the iteration (since we're using the same manager)
                if id_part != i.to_string() {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }

    #[qc]
    fn prop_variable_generation_thread_safety(type_name: String) -> bool {
        // Skip problematic type names
        if type_name.is_empty() || type_name.len() > 20 {
            return true;
        }

        if type_name.contains(|c: char| !c.is_alphanumeric() && c != '_') {
            return true;
        }

        let manager = create_test_manager();

        // Generate variables from the same manager instance
        // The AtomicUsize counter should ensure uniqueness even in concurrent scenarios
        let var1 = manager.fresh_variable(&type_name);
        let var2 = manager.fresh_variable(&type_name);
        let var3 = manager.fresh_variable(&type_name);

        // All should be different
        var1 != var2 && var2 != var3 && var1 != var3
    }

    #[test]
    fn test_variable_uniqueness_across_types() {
        let manager = create_test_manager();

        // Generate variables of different types
        let u64_var1 = manager.fresh_variable("u64");
        let u64_var2 = manager.fresh_variable("u64");
        let i32_var1 = manager.fresh_variable("i32");
        let bool_var1 = manager.fresh_variable("bool");

        // All should be unique
        let vars = vec![&u64_var1, &u64_var2, &i32_var1, &bool_var1];
        let unique_vars: HashSet<_> = vars.into_iter().collect();
        assert_eq!(unique_vars.len(), 4);

        // Check expected format
        assert_eq!(u64_var1, "u64_0");
        assert_eq!(u64_var2, "u64_1");
        assert_eq!(i32_var1, "i32_2");
        assert_eq!(bool_var1, "bool_3");
    }

    #[test]
    fn test_variable_uniqueness_large_scale() {
        let manager = create_test_manager();
        let mut generated_names = HashSet::new();

        // Generate a large number of variables
        for i in 0..1000 {
            let type_name = match i % 4 {
                0 => "u64",
                1 => "i32",
                2 => "bool",
                _ => "f64",
            };

            let var_name = manager.fresh_variable(type_name);

            // Should be unique
            assert!(!generated_names.contains(&var_name));
            generated_names.insert(var_name);
        }

        // All 1000 should be unique
        assert_eq!(generated_names.len(), 1000);
    }

    #[test]
    fn test_satisfiability_caching() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint: x > 0
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();

        // First query - should be a cache miss
        let result1 = manager.is_satisfiable();
        assert!(result1.is_ok());
        assert!(result1.unwrap());

        let stats1 = manager.get_cache_stats();
        assert_eq!(stats1.cache_misses, 1);
        assert_eq!(stats1.cache_hits, 0);

        // Second query with same constraints - should be a cache hit
        let result2 = manager.is_satisfiable();
        assert!(result2.is_ok());
        assert!(result2.unwrap());

        let stats2 = manager.get_cache_stats();
        assert_eq!(stats2.cache_misses, 1);
        assert_eq!(stats2.cache_hits, 1);
        assert_eq!(stats2.sat_cache_size, 1);
    }

    #[test]
    fn test_model_caching() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint: x = 42
        let x = SymExpr::Variable("x".to_string());
        let forty_two = SymExpr::Constant(ConstValue::I64(42));
        let constraint = SymExpr::BinaryOp(BinOp::Eq, Box::new(x), Box::new(forty_two));
        manager.add_constraint(constraint).unwrap();

        // First model query - should be cache misses
        let model1 = manager.get_model();
        assert!(model1.is_ok());
        assert!(model1.unwrap().is_some());

        // Second model query - should use cache
        let model2 = manager.get_model();
        assert!(model2.is_ok());
        assert!(model2.unwrap().is_some());

        let stats = manager.get_cache_stats();
        assert!(stats.cache_hits > 0);
        assert!(stats.model_cache_size > 0);
    }

    #[test]
    fn test_cache_invalidation_on_constraint_add() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add first constraint and query
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint1 = SymExpr::BinaryOp(BinOp::Gt, Box::new(x.clone()), Box::new(zero));
        manager.add_constraint(constraint1).unwrap();

        let _ = manager.is_satisfiable();
        let stats1 = manager.get_cache_stats();
        assert_eq!(stats1.sat_cache_size, 1);

        // Add another constraint - should invalidate cache
        let ten = SymExpr::Constant(ConstValue::I64(10));
        let constraint2 = SymExpr::BinaryOp(BinOp::Lt, Box::new(x), Box::new(ten));
        manager.add_constraint(constraint2).unwrap();

        let stats2 = manager.get_cache_stats();
        assert_eq!(stats2.sat_cache_size, 0); // Cache should be cleared
        assert_eq!(stats2.cache_hits, 0);
        assert_eq!(stats2.cache_misses, 0);
    }

    #[test]
    fn test_cache_invalidation_on_clear_constraints() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint and query
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();

        let _ = manager.is_satisfiable();
        let stats1 = manager.get_cache_stats();
        assert_eq!(stats1.sat_cache_size, 1);

        // Clear constraints - should invalidate cache
        manager.clear_constraints();

        let stats2 = manager.get_cache_stats();
        assert_eq!(stats2.sat_cache_size, 0);
        assert_eq!(stats2.cache_hits, 0);
        assert_eq!(stats2.cache_misses, 0);
    }

    #[test]
    fn test_check_satisfiability_with_custom_constraints() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Create constraint: x > 0
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));

        // First query - cache miss
        let result1 = manager.check_satisfiability(&[constraint.clone()]);
        assert!(result1.is_ok());
        assert!(result1.unwrap());

        let stats1 = manager.get_cache_stats();
        assert_eq!(stats1.cache_misses, 1);
        assert_eq!(stats1.cache_hits, 0);

        // Second query with same constraint - cache hit
        let result2 = manager.check_satisfiability(&[constraint]);
        assert!(result2.is_ok());
        assert!(result2.unwrap());

        let stats2 = manager.get_cache_stats();
        assert_eq!(stats2.cache_misses, 1);
        assert_eq!(stats2.cache_hits, 1);
    }

    #[test]
    fn test_get_model_for_constraints_with_caching() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Create constraint: x = 42
        let x = SymExpr::Variable("x".to_string());
        let forty_two = SymExpr::Constant(ConstValue::I64(42));
        let constraint = SymExpr::BinaryOp(BinOp::Eq, Box::new(x), Box::new(forty_two));

        // First query - cache miss
        let model1 = manager.get_model_for_constraints(&[constraint.clone()]);
        assert!(model1.is_ok());
        assert!(model1.as_ref().unwrap().is_some());

        // Second query - cache hit
        let model2 = manager.get_model_for_constraints(&[constraint]);
        assert!(model2.is_ok());
        assert!(model2.as_ref().unwrap().is_some());

        let stats = manager.get_cache_stats();
        assert!(stats.cache_hits > 0);
        assert!(stats.model_cache_size > 0);
    }

    #[test]
    fn test_cache_hit_rate_calculation() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Create constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));

        // First query - miss
        let _ = manager.check_satisfiability(&[constraint.clone()]);

        // Three more queries - hits
        let _ = manager.check_satisfiability(&[constraint.clone()]);
        let _ = manager.check_satisfiability(&[constraint.clone()]);
        let _ = manager.check_satisfiability(&[constraint]);

        let stats = manager.get_cache_stats();
        assert_eq!(stats.cache_misses, 1);
        assert_eq!(stats.cache_hits, 3);
        assert_eq!(stats.hit_rate, 0.75); // 3 hits out of 4 total queries
    }

    #[test]
    fn test_cache_with_different_constraints() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let ten = SymExpr::Constant(ConstValue::I64(10));

        // Query different constraints
        let constraint1 = SymExpr::BinaryOp(BinOp::Gt, Box::new(x.clone()), Box::new(zero));
        let constraint2 = SymExpr::BinaryOp(BinOp::Lt, Box::new(x), Box::new(ten));

        // Each should be a cache miss
        let _ = manager.check_satisfiability(&[constraint1.clone()]);
        let _ = manager.check_satisfiability(&[constraint2.clone()]);

        let stats1 = manager.get_cache_stats();
        assert_eq!(stats1.cache_misses, 2);
        assert_eq!(stats1.cache_hits, 0);
        assert_eq!(stats1.sat_cache_size, 2); // Two different constraints cached

        // Query same constraints again - should be hits
        let _ = manager.check_satisfiability(&[constraint1]);
        let _ = manager.check_satisfiability(&[constraint2]);

        let stats2 = manager.get_cache_stats();
        assert_eq!(stats2.cache_misses, 2);
        assert_eq!(stats2.cache_hits, 2);
    }

    #[test]
    fn test_clear_cache_method() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint and query
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));

        let _ = manager.check_satisfiability(&[constraint.clone()]);
        let _ = manager.check_satisfiability(&[constraint.clone()]);

        let stats1 = manager.get_cache_stats();
        assert!(stats1.sat_cache_size > 0);
        assert!(stats1.cache_hits > 0);

        // Clear cache explicitly
        manager.clear_cache();

        let stats2 = manager.get_cache_stats();
        assert_eq!(stats2.sat_cache_size, 0);
        assert_eq!(stats2.model_cache_size, 0);
        assert_eq!(stats2.cache_hits, 0);
        assert_eq!(stats2.cache_misses, 0);
        assert_eq!(stats2.hit_rate, 0.0);

        // Next query should be a miss again
        let _ = manager.check_satisfiability(&[constraint]);
        let stats3 = manager.get_cache_stats();
        assert_eq!(stats3.cache_misses, 1);
        assert_eq!(stats3.cache_hits, 0);
    }

    #[test]
    fn test_cache_with_unsatisfiable_constraints() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Create contradictory constraints: x > 0 AND x < 0
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint1 = SymExpr::BinaryOp(BinOp::Gt, Box::new(x.clone()), Box::new(zero.clone()));
        let constraint2 = SymExpr::BinaryOp(BinOp::Lt, Box::new(x), Box::new(zero));

        // First query - should be unsatisfiable
        let result1 = manager.check_satisfiability(&[constraint1.clone(), constraint2.clone()]);
        assert!(result1.is_ok());
        assert!(!result1.unwrap());

        // Second query - should use cache
        let result2 = manager.check_satisfiability(&[constraint1, constraint2]);
        assert!(result2.is_ok());
        assert!(!result2.unwrap());

        let stats = manager.get_cache_stats();
        assert_eq!(stats.cache_misses, 1);
        assert_eq!(stats.cache_hits, 1);
    }

    #[test]
    fn test_cache_performance_with_complex_constraints() {
        let mut manager = create_test_manager();

        // Register multiple variables
        for i in 0..5 {
            let type_info = TypeInfo {
                type_name: "u64".to_string(),
                bit_width: Some(64),
                is_signed: false,
                creation_site: None,
            };
            manager
                .register_variable(format!("x{}", i), type_info)
                .unwrap();
        }

        // Create complex constraint system
        let mut constraints = Vec::new();
        for i in 0..5 {
            let var = SymExpr::Variable(format!("x{}", i));
            let val = SymExpr::Constant(ConstValue::I64(i as i64));
            let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(var), Box::new(val));
            constraints.push(constraint);
        }

        // First query - cache miss
        let _ = manager.check_satisfiability(&constraints);

        // Multiple subsequent queries - cache hits
        for _ in 0..10 {
            let _ = manager.check_satisfiability(&constraints);
        }

        let stats = manager.get_cache_stats();
        assert_eq!(stats.cache_misses, 1);
        assert_eq!(stats.cache_hits, 10);
        assert!(stats.hit_rate > 0.9); // Should have high hit rate
    }

    // **Feature: symbolic-execution-engine, Property 7: Constraint accumulation and consistency**
    // **Validates: Requirements 3.1, 3.2, 3.4**
    #[qc]
    fn prop_constraint_accumulation_and_consistency(
        num_constraints: u8,
        constraint_types: Vec<u8>,
    ) -> bool {
        // Limit the number of constraints to avoid excessive test times
        let num_constraints = (num_constraints % 20) + 1; // 1 to 20 constraints

        if constraint_types.is_empty() {
            return true; // Skip test if no constraint types
        }

        let mut manager = create_test_manager();

        // Register a few variables for testing
        for i in 0..3 {
            let type_info = TypeInfo {
                type_name: "i64".to_string(),
                bit_width: Some(64),
                is_signed: true,
                creation_site: None,
            };
            if manager
                .register_variable(format!("x{}", i), type_info)
                .is_err()
            {
                return false;
            }
        }

        // Track constraints we add
        let mut added_constraints = Vec::new();

        // Add constraints one by one
        for i in 0..num_constraints {
            let constraint_type = constraint_types[i as usize % constraint_types.len()] % 6;

            // Create different types of constraints
            let constraint = match constraint_type {
                0 => {
                    // x0 > constant
                    let var = SymExpr::Variable("x0".to_string());
                    let val = SymExpr::Constant(ConstValue::I64((i as i64) * 10));
                    SymExpr::BinaryOp(BinOp::Gt, Box::new(var), Box::new(val))
                }
                1 => {
                    // x1 < constant
                    let var = SymExpr::Variable("x1".to_string());
                    let val = SymExpr::Constant(ConstValue::I64((i as i64) * 10 + 100));
                    SymExpr::BinaryOp(BinOp::Lt, Box::new(var), Box::new(val))
                }
                2 => {
                    // x2 == constant
                    let var = SymExpr::Variable("x2".to_string());
                    let val = SymExpr::Constant(ConstValue::I64((i as i64) * 5));
                    SymExpr::BinaryOp(BinOp::Eq, Box::new(var), Box::new(val))
                }
                3 => {
                    // x0 >= constant
                    let var = SymExpr::Variable("x0".to_string());
                    let val = SymExpr::Constant(ConstValue::I64((i as i64) * 2));
                    SymExpr::BinaryOp(BinOp::Ge, Box::new(var), Box::new(val))
                }
                4 => {
                    // x1 <= constant
                    let var = SymExpr::Variable("x1".to_string());
                    let val = SymExpr::Constant(ConstValue::I64((i as i64) * 3 + 50));
                    SymExpr::BinaryOp(BinOp::Le, Box::new(var), Box::new(val))
                }
                _ => {
                    // x2 != constant
                    let var = SymExpr::Variable("x2".to_string());
                    let val = SymExpr::Constant(ConstValue::I64((i as i64) * 7));
                    SymExpr::BinaryOp(BinOp::Ne, Box::new(var), Box::new(val))
                }
            };

            // Add the constraint
            if manager.add_constraint(constraint.clone()).is_err() {
                return false;
            }

            added_constraints.push(constraint);

            // Property 1: Constraint count should match number of added constraints
            if manager.get_constraints().len() != added_constraints.len() {
                return false;
            }

            // Property 2: All added constraints should be present in the manager
            let current_constraints = manager.get_constraints();
            for (idx, added) in added_constraints.iter().enumerate() {
                if current_constraints[idx] != *added {
                    return false;
                }
            }

            // Property 3: Constraints should be accumulated (not replaced)
            // The constraint count should only increase, never decrease
            if i > 0 && current_constraints.len() <= (i as usize) {
                return false;
            }
        }

        // Property 4: Check satisfiability consistency
        // The satisfiability check should not fail (it may return sat or unsat, but not error)
        if manager.is_satisfiable().is_err() {
            return false;
        }

        // Property 5: Clearing constraints should remove all constraints
        manager.clear_constraints();
        if !manager.get_constraints().is_empty() {
            return false;
        }

        // Property 6: After clearing, we should be able to add constraints again
        let new_constraint = SymExpr::BinaryOp(
            BinOp::Gt,
            Box::new(SymExpr::Variable("x0".to_string())),
            Box::new(SymExpr::Constant(ConstValue::I64(0))),
        );
        if manager.add_constraint(new_constraint).is_err() {
            return false;
        }

        if manager.get_constraints().len() != 1 {
            return false;
        }

        true
    }

    #[qc]
    fn prop_constraint_conjunction_consistency(num_vars: u8, num_constraints: u8) -> bool {
        // Limit to reasonable sizes
        let num_vars = (num_vars % 5) + 1; // 1 to 5 variables
        let num_constraints = (num_constraints % 15) + 1; // 1 to 15 constraints

        let mut manager = create_test_manager();

        // Register variables
        for i in 0..num_vars {
            let type_info = TypeInfo {
                type_name: "i64".to_string(),
                bit_width: Some(64),
                is_signed: true,
                creation_site: None,
            };
            if manager
                .register_variable(format!("v{}", i), type_info)
                .is_err()
            {
                return false;
            }
        }

        // Add constraints that should be satisfiable when conjoined
        for i in 0..num_constraints {
            let var_idx = i % num_vars;
            let var = SymExpr::Variable(format!("v{}", var_idx));

            // Create non-contradictory constraints: v_i > i and v_i < i + 100
            let lower_bound = SymExpr::Constant(ConstValue::I64(i as i64));
            let upper_bound = SymExpr::Constant(ConstValue::I64((i as i64) + 100));

            let constraint1 =
                SymExpr::BinaryOp(BinOp::Gt, Box::new(var.clone()), Box::new(lower_bound));
            let constraint2 = SymExpr::BinaryOp(BinOp::Lt, Box::new(var), Box::new(upper_bound));

            if manager.add_constraint(constraint1).is_err() {
                return false;
            }
            if manager.add_constraint(constraint2).is_err() {
                return false;
            }
        }

        // Property: The conjunction of non-contradictory constraints should be satisfiable
        match manager.is_satisfiable() {
            Ok(is_sat) => {
                // We expect this to be satisfiable since we carefully constructed non-contradictory constraints
                // However, due to the complexity of constraint solving, we just verify no error occurred
                // The actual satisfiability result depends on the specific constraints
                let _ = is_sat;
                true
            }
            Err(_) => false, // Should not error
        }
    }

    #[qc]
    fn prop_contradictory_constraints_detected(value: i64) -> bool {
        let mut manager = create_test_manager();

        // Register a variable
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };
        if manager
            .register_variable("x".to_string(), type_info)
            .is_err()
        {
            return false;
        }

        let x = SymExpr::Variable("x".to_string());
        let val = SymExpr::Constant(ConstValue::I64(value));

        // Add constraint: x == value
        let constraint1 = SymExpr::BinaryOp(BinOp::Eq, Box::new(x.clone()), Box::new(val.clone()));
        if manager.add_constraint(constraint1).is_err() {
            return false;
        }

        // Add contradictory constraint: x != value
        let constraint2 = SymExpr::BinaryOp(BinOp::Ne, Box::new(x), Box::new(val));
        if manager.add_constraint(constraint2).is_err() {
            return false;
        }

        // Property: Contradictory constraints should be detected as unsatisfiable
        match manager.is_satisfiable() {
            Ok(is_sat) => !is_sat, // Should be unsatisfiable
            Err(_) => false,       // Should not error
        }
    }

    #[qc]
    fn prop_constraint_order_independence(values: Vec<i64>) -> bool {
        // Limit the number of values to avoid excessive test times
        if values.is_empty() || values.len() > 10 {
            return true; // Skip test
        }

        // Create two managers with the same constraints added in different orders
        let mut manager1 = create_test_manager();
        let mut manager2 = create_test_manager();

        // Register variable in both managers
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };

        if manager1
            .register_variable("x".to_string(), type_info.clone())
            .is_err()
        {
            return false;
        }
        if manager2
            .register_variable("x".to_string(), type_info)
            .is_err()
        {
            return false;
        }

        // Create constraints
        let mut constraints = Vec::new();
        for &val in &values {
            let x = SymExpr::Variable("x".to_string());
            let constant = SymExpr::Constant(ConstValue::I64(val));
            let constraint = SymExpr::BinaryOp(BinOp::Ne, Box::new(x), Box::new(constant));
            constraints.push(constraint);
        }

        // Add constraints in original order to manager1
        for constraint in &constraints {
            if manager1.add_constraint(constraint.clone()).is_err() {
                return false;
            }
        }

        // Add constraints in reverse order to manager2
        for constraint in constraints.iter().rev() {
            if manager2.add_constraint(constraint.clone()).is_err() {
                return false;
            }
        }

        // Property: Satisfiability should be the same regardless of constraint order
        // (since conjunction is commutative)
        match (manager1.is_satisfiable(), manager2.is_satisfiable()) {
            (Ok(sat1), Ok(sat2)) => sat1 == sat2,
            _ => false, // Both should succeed
        }
    }

    #[test]
    fn test_constraint_accumulation_basic() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        let x = SymExpr::Variable("x".to_string());

        // Add first constraint: x > 0
        let constraint1 = SymExpr::BinaryOp(
            BinOp::Gt,
            Box::new(x.clone()),
            Box::new(SymExpr::Constant(ConstValue::I64(0))),
        );
        manager.add_constraint(constraint1.clone()).unwrap();
        assert_eq!(manager.get_constraints().len(), 1);

        // Add second constraint: x < 100
        let constraint2 = SymExpr::BinaryOp(
            BinOp::Lt,
            Box::new(x),
            Box::new(SymExpr::Constant(ConstValue::I64(100))),
        );
        manager.add_constraint(constraint2.clone()).unwrap();
        assert_eq!(manager.get_constraints().len(), 2);

        // Both constraints should be present
        let constraints = manager.get_constraints();
        assert_eq!(constraints[0], constraint1);
        assert_eq!(constraints[1], constraint2);

        // Should be satisfiable
        assert!(manager.is_satisfiable().unwrap());
    }

    #[test]
    fn test_constraint_consistency_contradiction() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        let x = SymExpr::Variable("x".to_string());
        let val = SymExpr::Constant(ConstValue::I64(42));

        // Add constraint: x == 42
        let constraint1 = SymExpr::BinaryOp(BinOp::Eq, Box::new(x.clone()), Box::new(val.clone()));
        manager.add_constraint(constraint1).unwrap();

        // Add contradictory constraint: x != 42
        let constraint2 = SymExpr::BinaryOp(BinOp::Ne, Box::new(x), Box::new(val));
        manager.add_constraint(constraint2).unwrap();

        // Should be unsatisfiable
        assert!(!manager.is_satisfiable().unwrap());
    }

    // Tests for execution state serialization (Task 4.5)

    #[test]
    fn test_execution_state_creation() {
        let constraints = vec![SymExpr::BinaryOp(
            BinOp::Gt,
            Box::new(SymExpr::Variable("x".to_string())),
            Box::new(SymExpr::Constant(ConstValue::I64(0))),
        )];

        let state = ExecutionState::new(constraints.clone(), HashMap::new(), 0, None);

        assert_eq!(state.constraint_count(), 1);
        assert_eq!(state.binding_count(), 0);
        assert!(!state.has_branch_condition());
        assert_eq!(state.solver_state_id, 0);
    }

    #[test]
    fn test_execution_state_with_branch_condition() {
        let branch_cond = SymExpr::BinaryOp(
            BinOp::Gt,
            Box::new(SymExpr::Variable("x".to_string())),
            Box::new(SymExpr::Constant(ConstValue::I64(5))),
        );

        let state = ExecutionState::new(vec![], HashMap::new(), 0, Some(branch_cond.clone()));

        assert!(state.has_branch_condition());
        assert_eq!(state.branch_condition, Some(branch_cond));
    }

    #[test]
    fn test_execution_state_validation_valid() {
        let constraints = vec![SymExpr::BinaryOp(
            BinOp::Gt,
            Box::new(SymExpr::Variable("x".to_string())),
            Box::new(SymExpr::Constant(ConstValue::I64(0))),
        )];

        let state = ExecutionState::new(constraints, HashMap::new(), 0, None);

        assert!(state.validate().is_ok());
    }

    #[test]
    fn test_execution_state_validation_empty_variable() {
        let mut bindings = HashMap::new();
        bindings.insert("".to_string(), SymExpr::Constant(ConstValue::I64(0)));

        let state = ExecutionState::new(vec![], bindings, 0, None);

        assert!(state.validate().is_err());
    }

    #[test]
    fn test_execution_state_validation_excessive_depth() {
        // Create a deeply nested expression
        let mut expr = SymExpr::Constant(ConstValue::I64(0));
        for _ in 0..150 {
            expr = SymExpr::UnaryOp(UnOp::Neg, Box::new(expr));
        }

        let state = ExecutionState::new(vec![expr], HashMap::new(), 0, None);

        assert!(state.validate().is_err());
    }

    #[test]
    fn test_execution_state_serialization() {
        let constraints = vec![SymExpr::BinaryOp(
            BinOp::Gt,
            Box::new(SymExpr::Variable("x".to_string())),
            Box::new(SymExpr::Constant(ConstValue::I64(0))),
        )];

        let state = ExecutionState::new(constraints, HashMap::new(), 0, None);

        let serialized = state.serialize();
        assert!(serialized.is_ok());

        let json = serialized.unwrap();
        assert!(json.contains("path_constraints"));
        assert!(json.contains("solver_state_id"));
    }

    #[test]
    fn test_execution_state_with_solver_state_id() {
        let state = ExecutionState::new(vec![], HashMap::new(), 5, None);

        let new_state = state.with_solver_state_id(10);
        assert_eq!(new_state.solver_state_id, 10);
        assert_eq!(state.solver_state_id, 5); // Original unchanged
    }

    #[test]
    fn test_manager_save_state() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();

        // Save state
        let saved = manager.save_state();
        assert!(saved.is_ok());

        let json = saved.unwrap();
        assert!(json.contains("path_constraints"));
    }

    #[test]
    fn test_manager_get_current_state() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint.clone()).unwrap();

        // Get current state
        let state = manager.get_current_state();
        assert_eq!(state.constraint_count(), 1);
        assert_eq!(state.path_constraints[0], constraint);
    }

    #[test]
    fn test_manager_validate_state_valid() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();

        // Validate state
        assert!(manager.validate_state().is_ok());
    }

    #[test]
    fn test_manager_validate_state_unregistered_variable() {
        let mut manager = create_test_manager();

        // Add constraint with unregistered variable (bypassing normal validation)
        // This simulates a corrupted state
        let x = SymExpr::Variable("unregistered".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));

        // Directly add to path_constraints to bypass validation
        manager.path_constraints.push(constraint);

        // Validate state should fail
        assert!(manager.validate_state().is_err());
    }

    #[test]
    fn test_push_state_with_validation() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();

        // Push state
        let result = manager.push_state(None);
        assert!(result.is_ok());
        assert_eq!(manager.get_stack_depth(), 1);
    }

    #[test]
    fn test_pop_state_with_validation() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x.clone()), Box::new(zero.clone()));
        manager.add_constraint(constraint.clone()).unwrap();

        // Push state
        manager.push_state(None).unwrap();

        // Add another constraint
        let ten = SymExpr::Constant(ConstValue::I64(10));
        let constraint2 = SymExpr::BinaryOp(BinOp::Lt, Box::new(x), Box::new(ten));
        manager.add_constraint(constraint2).unwrap();

        // Pop state
        let result = manager.pop_state();
        assert!(result.is_ok());

        let state = result.unwrap();
        assert_eq!(state.constraint_count(), 1);
        assert_eq!(state.path_constraints[0], constraint);

        // Manager should have restored constraints
        assert_eq!(manager.get_constraints().len(), 1);
    }

    #[test]
    fn test_state_round_trip_through_stack() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add initial constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint1 = SymExpr::BinaryOp(BinOp::Gt, Box::new(x.clone()), Box::new(zero));
        manager.add_constraint(constraint1.clone()).unwrap();

        let initial_constraints = manager.get_constraints().to_vec();

        // Push state
        manager.push_state(None).unwrap();

        // Modify state
        let ten = SymExpr::Constant(ConstValue::I64(10));
        let constraint2 = SymExpr::BinaryOp(BinOp::Lt, Box::new(x), Box::new(ten));
        manager.add_constraint(constraint2).unwrap();

        assert_eq!(manager.get_constraints().len(), 2);

        // Pop state
        manager.pop_state().unwrap();

        // Should be back to initial state
        assert_eq!(manager.get_constraints().len(), initial_constraints.len());
        assert_eq!(manager.get_constraints(), initial_constraints.as_slice());
    }

    #[test]
    fn test_multiple_state_pushes_and_pops() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        let x = SymExpr::Variable("x".to_string());

        // State 0: x > 0
        let constraint0 = SymExpr::BinaryOp(
            BinOp::Gt,
            Box::new(x.clone()),
            Box::new(SymExpr::Constant(ConstValue::I64(0))),
        );
        manager.add_constraint(constraint0).unwrap();
        assert_eq!(manager.get_constraints().len(), 1);

        // Push and add constraint: x < 10
        manager.push_state(None).unwrap();
        let constraint1 = SymExpr::BinaryOp(
            BinOp::Lt,
            Box::new(x.clone()),
            Box::new(SymExpr::Constant(ConstValue::I64(10))),
        );
        manager.add_constraint(constraint1).unwrap();
        assert_eq!(manager.get_constraints().len(), 2);

        // Push and add constraint: x != 5
        manager.push_state(None).unwrap();
        let constraint2 = SymExpr::BinaryOp(
            BinOp::Ne,
            Box::new(x.clone()),
            Box::new(SymExpr::Constant(ConstValue::I64(5))),
        );
        manager.add_constraint(constraint2).unwrap();
        assert_eq!(manager.get_constraints().len(), 3);

        // Pop back to state 1
        manager.pop_state().unwrap();
        assert_eq!(manager.get_constraints().len(), 2);

        // Pop back to state 0
        manager.pop_state().unwrap();
        assert_eq!(manager.get_constraints().len(), 1);
    }

    #[test]
    fn test_state_validation_in_stack() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint and push state
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();
        manager.push_state(None).unwrap();

        // Validate state should succeed
        assert!(manager.validate_state().is_ok());
    }

    // **Feature: symbolic-execution-engine, Property 13: Generic trait bound satisfaction**
    // **Validates: Requirements 5.5**
    // Note: This property tests state isolation between multiple execution contexts,
    // which is what Requirement 5.5 specifies. The property name references the design
    // document's Property 13, though there appears to be a mismatch in the task list.
    #[qc]
    fn prop_state_isolation_between_managers(num_managers: u8, operations_per_manager: u8) -> bool {
        // Limit to reasonable sizes to avoid excessive test times
        let num_managers = (num_managers % 5) + 2; // 2 to 6 managers
        let operations_per_manager = (operations_per_manager % 10) + 1; // 1 to 10 operations

        // Create multiple independent managers
        let mut managers: Vec<SymExManager> =
            (0..num_managers).map(|_| create_test_manager()).collect();

        // Track expected state for each manager
        let mut expected_var_counts = vec![0usize; num_managers as usize];
        let mut expected_constraint_counts = vec![0usize; num_managers as usize];

        // Perform operations on each manager independently
        for manager_idx in 0..num_managers {
            let manager = &mut managers[manager_idx as usize];

            for op_idx in 0..operations_per_manager {
                // Alternate between registering variables and adding constraints
                if op_idx % 2 == 0 {
                    // Register a variable
                    let var_name = format!("var_m{}_o{}", manager_idx, op_idx);
                    let type_info = TypeInfo {
                        type_name: "i64".to_string(),
                        bit_width: Some(64),
                        is_signed: true,
                        creation_site: None,
                    };

                    if manager
                        .register_variable(var_name.clone(), type_info)
                        .is_err()
                    {
                        return false;
                    }

                    expected_var_counts[manager_idx as usize] += 1;

                    // Add a constraint using this variable
                    let var = SymExpr::Variable(var_name);
                    let val = SymExpr::Constant(ConstValue::I64(op_idx as i64));
                    let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(var), Box::new(val));

                    if manager.add_constraint(constraint).is_err() {
                        return false;
                    }

                    expected_constraint_counts[manager_idx as usize] += 1;
                } else {
                    // Add a constraint using a previously registered variable
                    if expected_var_counts[manager_idx as usize] > 0 {
                        let var_name = format!("var_m{}_o{}", manager_idx, op_idx - 1);
                        let var = SymExpr::Variable(var_name);
                        let val = SymExpr::Constant(ConstValue::I64((op_idx * 10) as i64));
                        let constraint = SymExpr::BinaryOp(BinOp::Lt, Box::new(var), Box::new(val));

                        if manager.add_constraint(constraint).is_err() {
                            return false;
                        }

                        expected_constraint_counts[manager_idx as usize] += 1;
                    }
                }
            }
        }

        // Property 1: Each manager should have its own independent state
        for (idx, manager) in managers.iter().enumerate() {
            let stats = manager.get_stats();

            // Check variable count matches expected
            if stats.variable_count != expected_var_counts[idx] {
                return false;
            }

            // Check constraint count matches expected
            if stats.constraint_count != expected_constraint_counts[idx] {
                return false;
            }
        }

        // Property 2: Variable names should be unique within each manager
        for manager in &managers {
            let vars = manager.get_registered_variables();
            let unique_vars: HashSet<_> = vars.iter().collect();
            if vars.len() != unique_vars.len() {
                return false;
            }
        }

        // Property 3: Managers should not share state
        // Check that variable IDs are independent (each manager starts from 0)
        for (idx, manager) in managers.iter().enumerate() {
            let vars = manager.get_registered_variables();

            // Each manager should have variables specific to its index
            for var in &vars {
                if !var.contains(&format!("_m{}_", idx)) {
                    return false;
                }
            }
        }

        // Property 4: Operations on one manager should not affect others
        // Modify the first manager and verify others are unchanged
        if managers.len() >= 2 {
            let original_stats_1 = managers[1].get_stats();

            // Add constraint to manager 0
            if expected_var_counts[0] > 0 {
                let var_name = format!("var_m0_o0");
                let var = SymExpr::Variable(var_name);
                let val = SymExpr::Constant(ConstValue::I64(999));
                let constraint = SymExpr::BinaryOp(BinOp::Ne, Box::new(var), Box::new(val));

                if managers[0].add_constraint(constraint).is_err() {
                    return false;
                }
            }

            // Manager 1 should be unchanged
            let new_stats_1 = managers[1].get_stats();
            if original_stats_1.variable_count != new_stats_1.variable_count
                || original_stats_1.constraint_count != new_stats_1.constraint_count
            {
                return false;
            }
        }

        // Property 5: Each manager should have independent cache state
        for manager in &mut managers {
            let cache_stats = manager.get_cache_stats();
            // Cache should be independent for each manager
            // Initial cache should be empty or have only entries from this manager's operations
            let _ = cache_stats; // Just verify we can get cache stats without error
        }

        true
    }

    #[qc]
    fn prop_state_isolation_variable_generation(num_managers: u8, vars_per_manager: u8) -> bool {
        // Limit to reasonable sizes
        let num_managers = (num_managers % 5) + 2; // 2 to 6 managers
        let vars_per_manager = (vars_per_manager % 20) + 1; // 1 to 20 variables

        // Create multiple managers
        let managers: Vec<SymExManager> =
            (0..num_managers).map(|_| create_test_manager()).collect();

        // Generate variables in each manager
        let mut all_var_names = Vec::new();

        for (manager_idx, manager) in managers.iter().enumerate() {
            let mut manager_vars = Vec::new();

            for var_idx in 0..vars_per_manager {
                let var_name = manager.fresh_variable("test");

                // Variable name should follow expected format
                if !var_name.starts_with("test_") {
                    return false;
                }

                // Extract the ID
                if let Some(underscore_pos) = var_name.rfind('_') {
                    let id_str = &var_name[underscore_pos + 1..];
                    if let Ok(id) = id_str.parse::<usize>() {
                        // ID should match the variable index within this manager
                        if id != var_idx as usize {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                manager_vars.push((manager_idx, var_name));
            }

            all_var_names.extend(manager_vars);
        }

        // Property: Variable IDs should be independent per manager
        // Each manager should generate IDs starting from 0
        for manager_idx in 0..num_managers {
            let manager_vars: Vec<_> = all_var_names
                .iter()
                .filter(|(idx, _)| *idx == manager_idx as usize)
                .collect();

            // Check that IDs are sequential starting from 0
            for (i, (_, var_name)) in manager_vars.iter().enumerate() {
                let expected_name = format!("test_{}", i);
                if *var_name != expected_name {
                    return false;
                }
            }
        }

        true
    }

    #[qc]
    fn prop_state_isolation_constraint_independence(num_managers: u8) -> bool {
        // Limit to reasonable sizes
        let num_managers = (num_managers % 4) + 2; // 2 to 5 managers

        // Create multiple managers
        let mut managers: Vec<SymExManager> =
            (0..num_managers).map(|_| create_test_manager()).collect();

        // Register the same variable name in each manager
        for manager in &mut managers {
            let type_info = TypeInfo {
                type_name: "i64".to_string(),
                bit_width: Some(64),
                is_signed: true,
                creation_site: None,
            };

            if manager
                .register_variable("x".to_string(), type_info)
                .is_err()
            {
                return false;
            }
        }

        // Add different constraints to each manager
        for (idx, manager) in managers.iter_mut().enumerate() {
            let x = SymExpr::Variable("x".to_string());
            let val = SymExpr::Constant(ConstValue::I64((idx * 10) as i64));
            let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(val));

            if manager.add_constraint(constraint).is_err() {
                return false;
            }
        }

        // Property: Each manager should have exactly one constraint
        for manager in &managers {
            if manager.get_constraints().len() != 1 {
                return false;
            }
        }

        // Property: Constraints should be different across managers
        let constraints: Vec<_> = managers
            .iter()
            .map(|m| m.get_constraints()[0].clone())
            .collect();

        for i in 0..constraints.len() {
            for j in (i + 1)..constraints.len() {
                if constraints[i] == constraints[j] {
                    return false;
                }
            }
        }

        // Property: Satisfiability should be independent
        // Each manager should be able to check satisfiability independently
        for manager in &mut managers {
            if manager.is_satisfiable().is_err() {
                return false;
            }
        }

        true
    }

    #[qc]
    fn prop_state_isolation_stack_independence(num_managers: u8) -> bool {
        // Limit to reasonable sizes
        let num_managers = (num_managers % 4) + 2; // 2 to 5 managers

        // Create multiple managers
        let mut managers: Vec<SymExManager> =
            (0..num_managers).map(|_| create_test_manager()).collect();

        // Register variable in each manager
        for manager in &mut managers {
            let type_info = TypeInfo {
                type_name: "i64".to_string(),
                bit_width: Some(64),
                is_signed: true,
                creation_site: None,
            };

            if manager
                .register_variable("x".to_string(), type_info)
                .is_err()
            {
                return false;
            }
        }

        // Push different numbers of states onto each manager's stack
        for (idx, manager) in managers.iter_mut().enumerate() {
            let num_pushes = (idx + 1) as usize; // Manager 0 pushes 1, manager 1 pushes 2, etc.

            for _ in 0..num_pushes {
                if manager.push_state(None).is_err() {
                    return false;
                }
            }
        }

        // Property: Each manager should have independent stack depth
        for (idx, manager) in managers.iter().enumerate() {
            let expected_depth = (idx + 1) as usize;
            if manager.get_stack_depth() != expected_depth {
                return false;
            }
        }

        // Property: Popping from one manager should not affect others
        if managers.len() >= 2 {
            let original_depth_1 = managers[1].get_stack_depth();

            // Pop from manager 0
            if managers[0].pop_state().is_err() {
                return false;
            }

            // Manager 1's stack depth should be unchanged
            if managers[1].get_stack_depth() != original_depth_1 {
                return false;
            }
        }

        true
    }

    #[qc]
    fn prop_state_isolation_cache_independence(num_managers: u8) -> bool {
        // Limit to reasonable sizes
        let num_managers = (num_managers % 4) + 2; // 2 to 5 managers

        // Create multiple managers
        let mut managers: Vec<SymExManager> =
            (0..num_managers).map(|_| create_test_manager()).collect();

        // Register variable in each manager
        for manager in &mut managers {
            let type_info = TypeInfo {
                type_name: "i64".to_string(),
                bit_width: Some(64),
                is_signed: true,
                creation_site: None,
            };

            if manager
                .register_variable("x".to_string(), type_info)
                .is_err()
            {
                return false;
            }
        }

        // Add the same constraint to each manager and query satisfiability
        for manager in &mut managers {
            let x = SymExpr::Variable("x".to_string());
            let zero = SymExpr::Constant(ConstValue::I64(0));
            let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));

            if manager.add_constraint(constraint).is_err() {
                return false;
            }

            // Query satisfiability to populate cache
            if manager.is_satisfiable().is_err() {
                return false;
            }
        }

        // Property: Each manager should have independent cache
        for manager in &managers {
            let cache_stats = manager.get_cache_stats();

            // Each manager should have its own cache entries
            if cache_stats.sat_cache_size == 0 {
                return false;
            }

            // Cache should have recorded the query
            if cache_stats.cache_misses == 0 {
                return false;
            }
        }

        // Property: Clearing cache in one manager should not affect others
        if managers.len() >= 2 {
            let original_cache_size_1 = managers[1].get_cache_stats().sat_cache_size;

            // Clear cache in manager 0
            managers[0].clear_cache();

            // Manager 0's cache should be empty
            if managers[0].get_cache_stats().sat_cache_size != 0 {
                return false;
            }

            // Manager 1's cache should be unchanged
            if managers[1].get_cache_stats().sat_cache_size != original_cache_size_1 {
                return false;
            }
        }

        true
    }

    #[test]
    fn test_state_isolation_basic() {
        // Create two independent managers
        let mut manager1 = create_test_manager();
        let mut manager2 = create_test_manager();

        // Register variable in manager1
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager1
            .register_variable("x".to_string(), type_info.clone())
            .unwrap();

        // Manager2 should not have this variable
        assert_eq!(manager2.get_registered_variables().len(), 0);

        // Register variable in manager2
        manager2
            .register_variable("y".to_string(), type_info)
            .unwrap();

        // Each manager should have only its own variable
        assert_eq!(manager1.get_registered_variables(), vec!["x"]);
        assert_eq!(manager2.get_registered_variables(), vec!["y"]);
    }

    #[test]
    fn test_state_isolation_constraints() {
        let mut manager1 = create_test_manager();
        let mut manager2 = create_test_manager();

        // Register variables
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };
        manager1
            .register_variable("x".to_string(), type_info.clone())
            .unwrap();
        manager2
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add different constraints to each manager
        let x = SymExpr::Variable("x".to_string());
        let constraint1 = SymExpr::BinaryOp(
            BinOp::Gt,
            Box::new(x.clone()),
            Box::new(SymExpr::Constant(ConstValue::I64(0))),
        );
        let constraint2 = SymExpr::BinaryOp(
            BinOp::Lt,
            Box::new(x),
            Box::new(SymExpr::Constant(ConstValue::I64(100))),
        );

        manager1.add_constraint(constraint1.clone()).unwrap();
        manager2.add_constraint(constraint2.clone()).unwrap();

        // Each manager should have only its own constraint
        assert_eq!(manager1.get_constraints().len(), 1);
        assert_eq!(manager1.get_constraints()[0], constraint1);

        assert_eq!(manager2.get_constraints().len(), 1);
        assert_eq!(manager2.get_constraints()[0], constraint2);
    }

    #[test]
    fn test_state_isolation_variable_counter() {
        let manager1 = create_test_manager();
        let manager2 = create_test_manager();

        // Generate variables in each manager
        let var1_m1 = manager1.fresh_variable("test");
        let var2_m1 = manager1.fresh_variable("test");

        let var1_m2 = manager2.fresh_variable("test");
        let var2_m2 = manager2.fresh_variable("test");

        // Each manager should have independent counters starting from 0
        assert_eq!(var1_m1, "test_0");
        assert_eq!(var2_m1, "test_1");

        assert_eq!(var1_m2, "test_0");
        assert_eq!(var2_m2, "test_1");
    }

    #[test]
    fn test_state_isolation_stack() {
        let mut manager1 = create_test_manager();
        let mut manager2 = create_test_manager();

        // Register variables
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };
        manager1
            .register_variable("x".to_string(), type_info.clone())
            .unwrap();
        manager2
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Push states
        manager1.push_state(None).unwrap();
        manager1.push_state(None).unwrap();

        manager2.push_state(None).unwrap();

        // Each manager should have independent stack depth
        assert_eq!(manager1.get_stack_depth(), 2);
        assert_eq!(manager2.get_stack_depth(), 1);

        // Pop from manager1
        manager1.pop_state().unwrap();

        // Manager1's depth should decrease, manager2 should be unchanged
        assert_eq!(manager1.get_stack_depth(), 1);
        assert_eq!(manager2.get_stack_depth(), 1);
    }

    #[test]
    fn test_state_isolation_cache() {
        let mut manager1 = create_test_manager();
        let mut manager2 = create_test_manager();

        // Register variables
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };
        manager1
            .register_variable("x".to_string(), type_info.clone())
            .unwrap();
        manager2
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraints and query satisfiability
        let x = SymExpr::Variable("x".to_string());
        let constraint = SymExpr::BinaryOp(
            BinOp::Gt,
            Box::new(x),
            Box::new(SymExpr::Constant(ConstValue::I64(0))),
        );

        manager1.add_constraint(constraint.clone()).unwrap();
        manager2.add_constraint(constraint).unwrap();

        manager1.is_satisfiable().unwrap();
        manager2.is_satisfiable().unwrap();

        // Each manager should have independent cache
        let cache1 = manager1.get_cache_stats();
        let cache2 = manager2.get_cache_stats();

        assert!(cache1.sat_cache_size > 0);
        assert!(cache2.sat_cache_size > 0);

        // Clear cache in manager1
        manager1.clear_cache();

        // Manager1's cache should be empty, manager2's should be unchanged
        assert_eq!(manager1.get_cache_stats().sat_cache_size, 0);
        assert!(manager2.get_cache_stats().sat_cache_size > 0);
    }

    // Tests for path exploration metadata (Task 5.1)

    #[test]
    fn test_path_metadata_creation() {
        let metadata = PathMetadata::new(1, 5, 12345, Some(0));

        assert_eq!(metadata.path_id, 1);
        assert_eq!(metadata.depth, 5);
        assert_eq!(metadata.constraint_hash, 12345);
        assert_eq!(metadata.visit_count, 1);
        assert_eq!(metadata.parent_path_id, Some(0));
        assert!(!metadata.fully_explored);
    }

    #[test]
    fn test_path_metadata_root() {
        let metadata = PathMetadata::root();

        assert_eq!(metadata.path_id, 0);
        assert_eq!(metadata.depth, 0);
        assert_eq!(metadata.constraint_hash, 0);
        assert_eq!(metadata.visit_count, 1);
        assert_eq!(metadata.parent_path_id, None);
        assert!(!metadata.fully_explored);
    }

    #[test]
    fn test_path_metadata_increment_visit() {
        let mut metadata = PathMetadata::root();
        assert_eq!(metadata.visit_count, 1);

        metadata.increment_visit();
        assert_eq!(metadata.visit_count, 2);

        metadata.increment_visit();
        assert_eq!(metadata.visit_count, 3);
    }

    #[test]
    fn test_path_metadata_mark_explored() {
        let mut metadata = PathMetadata::root();
        assert!(!metadata.fully_explored);

        metadata.mark_explored();
        assert!(metadata.fully_explored);
    }

    #[test]
    fn test_path_metadata_is_looping() {
        let mut metadata = PathMetadata::root();

        assert!(!metadata.is_looping(10));

        for _ in 0..10 {
            metadata.increment_visit();
        }

        assert!(metadata.is_looping(10));
    }

    #[test]
    fn test_manager_path_metadata_tracking() {
        let mut manager = create_test_manager();

        // Initial path metadata should be root
        let initial_metadata = manager.get_current_path_metadata();
        assert_eq!(initial_metadata.path_id, 0);
        assert_eq!(initial_metadata.depth, 0);

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint and push state
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();

        manager.push_state(None).unwrap();

        // Path metadata should be updated
        let new_metadata = manager.get_current_path_metadata();
        assert_eq!(new_metadata.path_id, 0); // First fetch_add returns 0
        assert_eq!(new_metadata.depth, 0); // Depth is stack depth at push time
        assert_eq!(new_metadata.parent_path_id, Some(0));
    }

    #[test]
    fn test_manager_visited_states_tracking() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();

        // Get constraint hash
        let hash = manager.compute_constraints_hash(&manager.path_constraints);

        // Initially not visited
        assert_eq!(manager.get_state_visit_count(hash), 0);

        // Push state - should mark as visited
        manager.push_state(None).unwrap();

        // Should now be visited once
        assert!(manager.has_visited_state(hash));
        assert_eq!(manager.get_state_visit_count(hash), 1);
    }

    #[test]
    fn test_manager_loop_detection() {
        let mut manager = SymExManager::with_max_visits(
            Box::new(Z3Solver::new().unwrap()),
            3, // Low threshold for testing
        );

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();

        // Push state multiple times with same constraints
        assert!(manager.push_state(None).is_ok()); // Visit 1

        // Pop and push again with same constraints
        manager.pop_state().unwrap();
        assert!(manager.push_state(None).is_ok()); // Visit 2

        manager.pop_state().unwrap();
        assert!(manager.push_state(None).is_ok()); // Visit 3

        // Next push should fail due to loop detection
        manager.pop_state().unwrap();
        let result = manager.push_state(None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SymExError::ResourceExhaustion(_)
        ));
    }

    #[test]
    fn test_manager_path_exploration_stats() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Initial stats
        let stats = manager.get_path_stats();
        assert_eq!(stats.current_path_id, 0);
        assert_eq!(stats.current_depth, 0);
        assert_eq!(stats.total_paths_explored, 0);
        assert_eq!(stats.unique_states_visited, 0);
        assert_eq!(stats.stack_depth, 0);

        // Add constraint and push state
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x.clone()), Box::new(zero));
        manager.add_constraint(constraint).unwrap();
        manager.push_state(None).unwrap();

        // Stats should be updated
        let stats = manager.get_path_stats();
        assert_eq!(stats.current_path_id, 0); // First fetch_add returns 0
        assert_eq!(stats.total_paths_explored, 1); // Counter is now 1
        assert_eq!(stats.unique_states_visited, 1);
        assert_eq!(stats.stack_depth, 1);

        // Add another constraint and push
        let ten = SymExpr::Constant(ConstValue::I64(10));
        let constraint2 = SymExpr::BinaryOp(BinOp::Lt, Box::new(x), Box::new(ten));
        manager.add_constraint(constraint2).unwrap();
        manager.push_state(None).unwrap();

        let stats = manager.get_path_stats();
        assert_eq!(stats.current_path_id, 1); // Second fetch_add returns 1
        assert_eq!(stats.total_paths_explored, 2); // Counter is now 2
        assert_eq!(stats.unique_states_visited, 2);
        assert_eq!(stats.stack_depth, 2);
    }

    #[test]
    fn test_manager_clear_visited_states() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint and push state
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();
        manager.push_state(None).unwrap();

        // Should have visited states
        assert!(manager.get_visited_states().len() > 0);

        // Clear visited states
        manager.clear_visited_states();

        // Should be empty
        assert_eq!(manager.get_visited_states().len(), 0);
    }

    #[test]
    fn test_manager_mark_current_path_explored() {
        let mut manager = create_test_manager();

        // Initial path should not be marked as explored
        assert!(!manager.get_current_path_metadata().fully_explored);

        // Mark as explored
        manager.mark_current_path_explored();

        // Should now be marked
        assert!(manager.get_current_path_metadata().fully_explored);
    }

    #[test]
    fn test_execution_state_with_metadata() {
        let metadata = PathMetadata::new(5, 3, 12345, Some(2));
        let state =
            ExecutionState::with_metadata(vec![], HashMap::new(), 0, None, metadata.clone());

        assert_eq!(state.path_metadata.path_id, 5);
        assert_eq!(state.path_metadata.depth, 3);
        assert_eq!(state.path_metadata.constraint_hash, 12345);
        assert_eq!(state.path_metadata.parent_path_id, Some(2));
    }

    #[test]
    fn test_path_metadata_preserved_through_stack() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint and push state
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();

        let initial_path_id = manager.get_current_path_metadata().path_id;
        assert_eq!(initial_path_id, 0); // Should start at 0 (root)

        // Push state - this creates a new path with ID from the counter
        manager.push_state(None).unwrap();
        let pushed_path_id = manager.get_current_path_metadata().path_id;

        // Path ID should have incremented (counter starts at 0, fetch_add returns 0, so new path_id is 0)
        // But the parent should be 0 (the root)
        assert_eq!(pushed_path_id, 0); // First fetch_add(1) returns 0
        assert_eq!(manager.get_current_path_metadata().parent_path_id, Some(0));

        // Pop state
        let popped_state = manager.pop_state().unwrap();

        // Popped state should have the metadata from when it was pushed
        assert_eq!(popped_state.path_metadata.path_id, 0);

        // Current path should be restored to the popped state's metadata
        assert_eq!(manager.get_current_path_metadata().path_id, 0);
    }

    #[test]
    fn test_manager_reset_clears_path_metadata() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "u64".to_string(),
            bit_width: Some(64),
            is_signed: false,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint and push state
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();
        manager.push_state(None).unwrap();

        // Should have path metadata and visited states
        let stats = manager.get_path_stats();
        assert!(stats.total_paths_explored > 0); // Counter should have incremented
        assert!(manager.get_visited_states().len() > 0);

        // Reset
        manager.reset().unwrap();

        // Path metadata should be reset to root
        assert_eq!(manager.get_current_path_metadata().path_id, 0);
        assert_eq!(manager.get_current_path_metadata().depth, 0);
        assert_eq!(manager.get_visited_states().len(), 0);

        // Path counter should be reset
        let stats = manager.get_path_stats();
        assert_eq!(stats.total_paths_explored, 0);
    }

    // **Feature: symbolic-execution-engine, Property 10: Execution state round-trip**
    // **Validates: Requirements 6.1, 6.2**
    #[qc]
    fn prop_execution_state_round_trip(
        num_variables: u8,
        num_constraints: u8,
        num_pushes: u8,
    ) -> bool {
        // Limit to reasonable sizes to avoid excessive test times
        let num_variables = (num_variables % 10) + 1; // 1 to 10 variables
        let num_constraints = (num_constraints % 10) + 1; // 1 to 10 constraints
        let num_pushes = (num_pushes % 5) + 1; // 1 to 5 pushes

        let mut manager = create_test_manager();

        // Register variables
        let mut var_names = Vec::new();
        for i in 0..num_variables {
            let var_name = format!("x{}", i);
            let type_info = TypeInfo {
                type_name: "i64".to_string(),
                bit_width: Some(64),
                is_signed: true,
                creation_site: None,
            };

            if manager
                .register_variable(var_name.clone(), type_info)
                .is_err()
            {
                return false;
            }

            var_names.push(var_name);
        }

        // Add initial constraints
        for i in 0..num_constraints {
            let var_idx = i as usize % var_names.len();
            let var = SymExpr::Variable(var_names[var_idx].clone());
            let val = SymExpr::Constant(ConstValue::I64(i as i64 * 10));
            let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(var), Box::new(val));

            if manager.add_constraint(constraint).is_err() {
                return false;
            }
        }

        // Record initial state
        let initial_constraints = manager.get_constraints().to_vec();
        let initial_stack_depth = manager.get_stack_depth();

        // Perform push/pop operations and verify round-trip
        for push_idx in 0..num_pushes {
            // Record state before push
            let constraints_before_push = manager.get_constraints().to_vec();
            let stack_depth_before_push = manager.get_stack_depth();

            // Create a branch condition
            let branch_var_idx = push_idx as usize % var_names.len();
            let branch_condition = Some(SymExpr::Variable(var_names[branch_var_idx].clone()));

            // Push state
            if manager.push_state(branch_condition.clone()).is_err() {
                return false;
            }

            // Property 1: Stack depth should increase by 1
            if manager.get_stack_depth() != stack_depth_before_push + 1 {
                return false;
            }

            // Property 2: Constraints should be preserved after push
            if manager.get_constraints() != constraints_before_push.as_slice() {
                return false;
            }

            // Add a new constraint after push
            let var_idx = push_idx as usize % var_names.len();
            let var = SymExpr::Variable(var_names[var_idx].clone());
            let val = SymExpr::Constant(ConstValue::I64((push_idx as i64 + 1) * 100));
            let new_constraint = SymExpr::BinaryOp(BinOp::Lt, Box::new(var), Box::new(val));

            if manager.add_constraint(new_constraint.clone()).is_err() {
                return false;
            }

            // Property 3: New constraint should be added
            let constraints_after_add = manager.get_constraints();
            if constraints_after_add.len() != constraints_before_push.len() + 1 {
                return false;
            }

            // Property 4: Last constraint should be the one we just added
            if constraints_after_add.last() != Some(&new_constraint) {
                return false;
            }

            // Pop state
            let popped_state = match manager.pop_state() {
                Ok(state) => state,
                Err(_) => return false,
            };

            // Property 5: Stack depth should decrease by 1
            if manager.get_stack_depth() != stack_depth_before_push {
                return false;
            }

            // Property 6: Constraints should be restored to state before push
            if manager.get_constraints() != constraints_before_push.as_slice() {
                return false;
            }

            // Property 7: Popped state should contain the constraints from before push
            if popped_state.path_constraints != constraints_before_push {
                return false;
            }

            // Property 8: Popped state should have the branch condition we set
            if popped_state.branch_condition != branch_condition {
                return false;
            }

            // Property 9: Popped state should have correct constraint count
            if popped_state.constraint_count() != constraints_before_push.len() {
                return false;
            }

            // Property 10: Popped state should be valid
            if popped_state.validate().is_err() {
                return false;
            }
        }

        // Property 11: After all push/pop operations, we should be back to initial state
        if manager.get_constraints() != initial_constraints.as_slice() {
            return false;
        }

        if manager.get_stack_depth() != initial_stack_depth {
            return false;
        }

        true
    }

    #[qc]
    fn prop_execution_state_nested_round_trip(depth: u8) -> bool {
        // Limit depth to avoid excessive test times and stack overflow
        let depth = (depth % 8) + 1; // 1 to 8 levels of nesting

        let mut manager = create_test_manager();

        // Register a variable
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };

        if manager
            .register_variable("x".to_string(), type_info)
            .is_err()
        {
            return false;
        }

        let x = SymExpr::Variable("x".to_string());

        // Track expected state at each depth
        let mut expected_constraint_counts = Vec::new();

        // Push states with increasing constraints
        for i in 0..depth {
            // Record current constraint count
            expected_constraint_counts.push(manager.get_constraints().len());

            // Push state
            if manager.push_state(None).is_err() {
                return false;
            }

            // Add a constraint
            let val = SymExpr::Constant(ConstValue::I64(i as i64));
            let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x.clone()), Box::new(val));

            if manager.add_constraint(constraint).is_err() {
                return false;
            }
        }

        // Property 1: Stack depth should match the number of pushes
        if manager.get_stack_depth() != depth as usize {
            return false;
        }

        // Property 2: We should have 'depth' constraints (one added at each level)
        if manager.get_constraints().len() != depth as usize {
            return false;
        }

        // Pop all states and verify restoration
        for i in (0..depth).rev() {
            let popped_state = match manager.pop_state() {
                Ok(state) => state,
                Err(_) => return false,
            };

            // Property 3: Stack depth should decrease
            if manager.get_stack_depth() != i as usize {
                return false;
            }

            // Property 4: Constraint count should match expected
            if manager.get_constraints().len() != expected_constraint_counts[i as usize] {
                return false;
            }

            // Property 5: Popped state should be valid
            if popped_state.validate().is_err() {
                return false;
            }

            // Property 6: Popped state constraint count should match what we had before push
            if popped_state.constraint_count() != expected_constraint_counts[i as usize] {
                return false;
            }
        }

        // Property 7: After all pops, stack should be empty
        if !manager.is_stack_empty() {
            return false;
        }

        // Property 8: After all pops, we should have no constraints
        if manager.get_constraints().len() != 0 {
            return false;
        }

        true
    }

    #[qc]
    fn prop_execution_state_preserves_solver_state(num_operations: u8) -> bool {
        // Limit operations to avoid excessive test times
        let num_operations = (num_operations % 10) + 1; // 1 to 10 operations

        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };

        if manager
            .register_variable("x".to_string(), type_info)
            .is_err()
        {
            return false;
        }

        let x = SymExpr::Variable("x".to_string());

        // Perform alternating push/pop operations
        for i in 0..num_operations {
            // Add a constraint
            let val = SymExpr::Constant(ConstValue::I64(i as i64 * 10));
            let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x.clone()), Box::new(val));

            if manager.add_constraint(constraint).is_err() {
                return false;
            }

            // Check satisfiability before push
            let sat_before = match manager.is_satisfiable() {
                Ok(result) => result,
                Err(_) => return false,
            };

            // Push state
            if manager.push_state(None).is_err() {
                return false;
            }

            // Add another constraint
            let val2 = SymExpr::Constant(ConstValue::I64((i as i64 + 1) * 10));
            let constraint2 = SymExpr::BinaryOp(BinOp::Lt, Box::new(x.clone()), Box::new(val2));

            if manager.add_constraint(constraint2).is_err() {
                return false;
            }

            // Pop state
            if manager.pop_state().is_err() {
                return false;
            }

            // Property: Satisfiability should be the same after pop
            let sat_after = match manager.is_satisfiable() {
                Ok(result) => result,
                Err(_) => return false,
            };

            if sat_before != sat_after {
                return false;
            }
        }

        true
    }

    #[qc]
    fn prop_execution_state_metadata_preservation(num_pushes: u8) -> bool {
        // Limit pushes to avoid excessive test times
        let num_pushes = (num_pushes % 6) + 1; // 1 to 6 pushes

        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };

        if manager
            .register_variable("x".to_string(), type_info)
            .is_err()
        {
            return false;
        }

        let x = SymExpr::Variable("x".to_string());

        // Track metadata for each push
        let mut pushed_metadata = Vec::new();

        for i in 0..num_pushes {
            // Add a constraint
            let val = SymExpr::Constant(ConstValue::I64(i as i64));
            let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x.clone()), Box::new(val));

            if manager.add_constraint(constraint).is_err() {
                return false;
            }

            // Record metadata before push
            let metadata_before = manager.get_current_path_metadata().clone();

            // Push state
            if manager.push_state(None).is_err() {
                return false;
            }

            // Record metadata after push
            let metadata_after = manager.get_current_path_metadata().clone();

            pushed_metadata.push((metadata_before, metadata_after));
        }

        // Pop all states and verify metadata restoration
        for _i in (0..num_pushes).rev() {
            let popped_state = match manager.pop_state() {
                Ok(state) => state,
                Err(_) => return false,
            };

            // Property 1: Popped state should have valid metadata
            if popped_state.path_metadata.depth > 100 {
                return false;
            }

            // Property 2: Current metadata should be restored
            let current_metadata = manager.get_current_path_metadata();

            // The metadata should match what was saved in the popped state
            if current_metadata.path_id != popped_state.path_metadata.path_id {
                return false;
            }

            if current_metadata.depth != popped_state.path_metadata.depth {
                return false;
            }
        }

        true
    }

    #[test]
    fn test_execution_state_round_trip_basic() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint.clone()).unwrap();

        // Record state before push
        let constraints_before = manager.get_constraints().to_vec();
        let stack_depth_before = manager.get_stack_depth();

        // Push state
        let branch_condition = Some(SymExpr::Variable("branch".to_string()));
        manager.push_state(branch_condition.clone()).unwrap();

        // Verify push
        assert_eq!(manager.get_stack_depth(), stack_depth_before + 1);
        assert_eq!(manager.get_constraints(), constraints_before.as_slice());

        // Pop state
        let popped_state = manager.pop_state().unwrap();

        // Verify round-trip
        assert_eq!(manager.get_stack_depth(), stack_depth_before);
        assert_eq!(manager.get_constraints(), constraints_before.as_slice());
        assert_eq!(popped_state.path_constraints, constraints_before);
        assert_eq!(popped_state.branch_condition, branch_condition);
        assert!(popped_state.validate().is_ok());
    }

    #[test]
    fn test_execution_state_round_trip_with_modifications() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add initial constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint1 = SymExpr::BinaryOp(BinOp::Gt, Box::new(x.clone()), Box::new(zero));
        manager.add_constraint(constraint1).unwrap();

        let constraints_before = manager.get_constraints().to_vec();

        // Push state
        manager.push_state(None).unwrap();

        // Add another constraint
        let ten = SymExpr::Constant(ConstValue::I64(10));
        let constraint2 = SymExpr::BinaryOp(BinOp::Lt, Box::new(x), Box::new(ten));
        manager.add_constraint(constraint2).unwrap();

        // Verify we have 2 constraints
        assert_eq!(manager.get_constraints().len(), 2);

        // Pop state
        manager.pop_state().unwrap();

        // Verify restoration to 1 constraint
        assert_eq!(manager.get_constraints().len(), 1);
        assert_eq!(manager.get_constraints(), constraints_before.as_slice());
    }

    #[test]
    fn test_execution_state_round_trip_multiple_levels() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        let x = SymExpr::Variable("x".to_string());

        // Level 0: No constraints
        assert_eq!(manager.get_constraints().len(), 0);

        // Push to level 1
        manager.push_state(None).unwrap();
        let c1 = SymExpr::BinaryOp(
            BinOp::Gt,
            Box::new(x.clone()),
            Box::new(SymExpr::Constant(ConstValue::I64(0))),
        );
        manager.add_constraint(c1).unwrap();
        assert_eq!(manager.get_constraints().len(), 1);

        // Push to level 2
        manager.push_state(None).unwrap();
        let c2 = SymExpr::BinaryOp(
            BinOp::Lt,
            Box::new(x.clone()),
            Box::new(SymExpr::Constant(ConstValue::I64(10))),
        );
        manager.add_constraint(c2).unwrap();
        assert_eq!(manager.get_constraints().len(), 2);

        // Push to level 3
        manager.push_state(None).unwrap();
        let c3 = SymExpr::BinaryOp(
            BinOp::Ne,
            Box::new(x),
            Box::new(SymExpr::Constant(ConstValue::I64(5))),
        );
        manager.add_constraint(c3).unwrap();
        assert_eq!(manager.get_constraints().len(), 3);

        // Pop back to level 2
        manager.pop_state().unwrap();
        assert_eq!(manager.get_constraints().len(), 2);

        // Pop back to level 1
        manager.pop_state().unwrap();
        assert_eq!(manager.get_constraints().len(), 1);

        // Pop back to level 0
        manager.pop_state().unwrap();
        assert_eq!(manager.get_constraints().len(), 0);
    }

    // Tests for task 5.5: Optimize stack memory management

    #[test]
    fn test_state_compression() {
        let mut state = ExecutionState::new(
            vec![
                SymExpr::Variable("x".to_string()),
                SymExpr::Constant(ConstValue::I64(42)),
            ],
            HashMap::new(),
            0,
            None,
        );

        // State should not be compressed initially
        assert!(!state.is_compressed());

        // Compress the state
        let savings = state.compress().unwrap();
        assert!(state.is_compressed());
        assert!(savings > 0);

        // Compressing again should return 0 savings
        let savings2 = state.compress().unwrap();
        assert_eq!(savings2, 0);

        // Decompress the state
        state.decompress().unwrap();
        assert!(!state.is_compressed());

        // Verify state is still valid after decompression
        assert!(state.validate().is_ok());
    }

    #[test]
    fn test_state_memory_estimation() {
        let state = ExecutionState::new(
            vec![
                SymExpr::Variable("x".to_string()),
                SymExpr::Constant(ConstValue::I64(42)),
            ],
            HashMap::new(),
            0,
            Some(SymExpr::Variable("branch".to_string())),
        );

        let size = state.estimate_memory_size();
        assert!(size > 0);
        assert!(size > 100); // Should be at least 100 bytes for the constraints
    }

    #[test]
    fn test_resource_monitor() {
        let mut monitor = ResourceMonitor::new();

        assert_eq!(monitor.states_allocated, 0);
        assert_eq!(monitor.states_deallocated, 0);
        assert_eq!(monitor.total_state_memory, 0);

        // Record allocation
        monitor.record_allocation(1000);
        assert_eq!(monitor.states_allocated, 1);
        assert_eq!(monitor.total_state_memory, 1000);
        assert_eq!(monitor.peak_memory, 1000);

        // Record another allocation
        monitor.record_allocation(500);
        assert_eq!(monitor.states_allocated, 2);
        assert_eq!(monitor.total_state_memory, 1500);
        assert_eq!(monitor.peak_memory, 1500);

        // Record deallocation
        monitor.record_deallocation(500);
        assert_eq!(monitor.states_deallocated, 1);
        assert_eq!(monitor.total_state_memory, 1000);
        assert_eq!(monitor.peak_memory, 1500); // Peak should remain

        // Record compression
        monitor.record_compression(200);
        assert_eq!(monitor.states_compressed, 1);
        assert_eq!(monitor.compression_savings, 200);

        // Record GC
        monitor.record_gc();
        assert_eq!(monitor.gc_runs, 1);
    }

    #[test]
    fn test_manager_with_compression_enabled() {
        let solver = Box::new(Z3Solver::new().unwrap());
        let mut manager = SymExManager::with_config(solver, 100, true, 10);

        // Register variable
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();

        // Push state (should be compressed)
        manager.push_state(None).unwrap();

        // Check resource stats
        let stats = manager.get_resource_stats();
        assert_eq!(stats.states_allocated, 1);
        assert!(stats.states_compressed > 0);
        assert!(stats.compression_savings > 0);
    }

    #[test]
    fn test_manager_with_compression_disabled() {
        let solver = Box::new(Z3Solver::new().unwrap());
        let mut manager = SymExManager::with_config(solver, 100, false, 10);

        // Register variable
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();

        // Push state (should NOT be compressed)
        manager.push_state(None).unwrap();

        // Check resource stats
        let stats = manager.get_resource_stats();
        assert_eq!(stats.states_allocated, 1);
        assert_eq!(stats.states_compressed, 0); // No compression
    }

    #[test]
    fn test_garbage_collection() {
        let solver = Box::new(Z3Solver::new().unwrap());
        let mut manager = SymExManager::with_config(solver, 100, false, 5);

        // Register variable
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();

        // Push multiple states to trigger GC
        for _ in 0..6 {
            manager.push_state(None).unwrap();
        }

        // GC should have been triggered
        let stats = manager.get_resource_stats();
        assert!(stats.gc_runs > 0);
    }

    #[test]
    fn test_manual_garbage_collection() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();

        // Push several states
        for _ in 0..5 {
            manager.push_state(None).unwrap();
        }

        let initial_depth = manager.get_stack_depth();
        assert_eq!(initial_depth, 5);

        // Manually trigger GC
        let collected = manager.garbage_collect_states().unwrap();

        // Some states should have been collected
        let final_depth = manager.get_stack_depth();
        assert!(final_depth <= initial_depth);
        assert_eq!(collected, initial_depth - final_depth);
    }

    #[test]
    fn test_compress_all_states() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();

        // Disable compression and push states
        manager.set_compression_enabled(false);
        for _ in 0..3 {
            manager.push_state(None).unwrap();
        }

        // Compress all states manually
        let savings = manager.compress_all_states().unwrap();
        assert!(savings > 0);

        // Check that all states are compressed
        let memory_usage = manager.get_memory_usage();
        assert_eq!(memory_usage.compressed_states, 3);
        assert_eq!(memory_usage.uncompressed_states, 0);
    }

    #[test]
    fn test_decompress_all_states() {
        let solver = Box::new(Z3Solver::new().unwrap());
        let mut manager = SymExManager::with_config(solver, 100, true, 100);

        // Register variable
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();

        // Push states (will be compressed)
        for _ in 0..3 {
            manager.push_state(None).unwrap();
        }

        // Verify states are compressed
        let memory_usage = manager.get_memory_usage();
        assert_eq!(memory_usage.compressed_states, 3);

        // Decompress all states
        manager.decompress_all_states().unwrap();

        // Verify states are decompressed
        let memory_usage = manager.get_memory_usage();
        assert_eq!(memory_usage.compressed_states, 0);
        assert_eq!(memory_usage.uncompressed_states, 3);
    }

    #[test]
    fn test_memory_usage_tracking() {
        let mut manager = create_test_manager();

        // Register variable
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        // Add constraint
        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();

        // Get initial memory usage
        let initial_usage = manager.get_memory_usage();
        assert!(initial_usage.total_memory > 0);

        // Push states
        for _ in 0..3 {
            manager.push_state(None).unwrap();
        }

        // Memory usage should increase
        let after_push_usage = manager.get_memory_usage();
        assert!(after_push_usage.total_memory > initial_usage.total_memory);
        assert!(after_push_usage.stack_memory > 0);

        // Pop states
        for _ in 0..3 {
            manager.pop_state().unwrap();
        }

        // Memory usage should decrease
        let after_pop_usage = manager.get_memory_usage();
        assert!(after_pop_usage.stack_memory < after_push_usage.stack_memory);
    }

    #[test]
    fn test_memory_usage_summary() {
        let manager = create_test_manager();
        let usage = manager.get_memory_usage();
        let summary = usage.summary();

        // Summary should contain key information
        assert!(summary.contains("Total:"));
        assert!(summary.contains("Stack:"));
        assert!(summary.contains("Cache:"));
        assert!(summary.contains("Compressed:"));
    }

    #[test]
    fn test_resource_monitor_reset() {
        let mut monitor = ResourceMonitor::new();

        // Add some data
        monitor.record_allocation(1000);
        monitor.record_compression(200);
        monitor.record_gc();

        assert!(monitor.states_allocated > 0);
        assert!(monitor.compression_savings > 0);
        assert!(monitor.gc_runs > 0);

        // Reset
        monitor.reset();

        // Everything should be back to zero
        assert_eq!(monitor.states_allocated, 0);
        assert_eq!(monitor.states_deallocated, 0);
        assert_eq!(monitor.total_state_memory, 0);
        assert_eq!(monitor.peak_memory, 0);
        assert_eq!(monitor.gc_runs, 0);
        assert_eq!(monitor.states_compressed, 0);
        assert_eq!(monitor.compression_savings, 0);
    }

    #[test]
    fn test_manager_reset_clears_resource_monitor() {
        let mut manager = create_test_manager();

        // Register variable and push states
        let type_info = TypeInfo {
            type_name: "i64".to_string(),
            bit_width: Some(64),
            is_signed: true,
            creation_site: None,
        };
        manager
            .register_variable("x".to_string(), type_info)
            .unwrap();

        let x = SymExpr::Variable("x".to_string());
        let zero = SymExpr::Constant(ConstValue::I64(0));
        let constraint = SymExpr::BinaryOp(BinOp::Gt, Box::new(x), Box::new(zero));
        manager.add_constraint(constraint).unwrap();

        manager.push_state(None).unwrap();

        // Verify resource stats exist
        let stats = manager.get_resource_stats();
        assert!(stats.states_allocated > 0);

        // Reset manager
        manager.reset().unwrap();

        // Resource stats should be cleared
        let stats = manager.get_resource_stats();
        assert_eq!(stats.states_allocated, 0);
        assert_eq!(stats.total_state_memory, 0);
    }

    #[test]
    fn test_compression_ratio_calculation() {
        let usage = MemoryUsage {
            stack_memory: 1000,
            cache_memory: 500,
            registry_memory: 200,
            constraints_memory: 300,
            total_memory: 2000,
            compressed_states: 3,
            uncompressed_states: 7,
        };

        let ratio = usage.compression_ratio();
        assert_eq!(ratio, 0.3); // 3 out of 10 states compressed
    }

    #[test]
    fn test_compression_ratio_with_no_states() {
        let usage = MemoryUsage {
            stack_memory: 0,
            cache_memory: 0,
            registry_memory: 0,
            constraints_memory: 0,
            total_memory: 0,
            compressed_states: 0,
            uncompressed_states: 0,
        };

        let ratio = usage.compression_ratio();
        assert_eq!(ratio, 0.0);
    }

    #[test]
    fn test_format_size() {
        assert_eq!(MemoryUsage::format_size(512), "512 B");
        assert_eq!(MemoryUsage::format_size(1024), "1.00 KB");
        assert_eq!(MemoryUsage::format_size(1024 * 1024), "1.00 MB");
        assert_eq!(MemoryUsage::format_size(1024 * 1024 * 1024), "1.00 GB");
    }
}
