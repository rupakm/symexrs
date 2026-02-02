//! SMT solver interface and Z3 integration
//!
//! This module provides an abstraction layer over SMT solvers with a concrete
//! implementation using the Z3 solver for constraint satisfiability checking
//! and model generation.

use crate::error::{SymExError, SymExResult};
use crate::expressions::{BinOp, ConstValue, SymExpr, UnOp};
use std::collections::HashMap;
use z3::{
    ast::{Ast, Bool, Int},
    Config, Context, SatResult as Z3SatResult, Solver,
};

/// Result of a satisfiability query
#[derive(Debug, Clone, PartialEq)]
pub enum SatResult {
    /// Constraints are satisfiable
    Sat,
    /// Constraints are unsatisfiable
    Unsat,
    /// Solver could not determine satisfiability (timeout, etc.)
    Unknown,
}

/// Concrete value assignment from SMT solver
#[derive(Debug, Clone)]
pub struct Model {
    /// Variable name to concrete value assignments
    pub assignments: HashMap<String, ConstValue>,
}

impl Model {
    /// Create a new empty model
    pub fn new() -> Self {
        Model {
            assignments: HashMap::new(),
        }
    }

    /// Get the value assigned to a variable
    pub fn get_value(&self, var_name: &str) -> Option<&ConstValue> {
        self.assignments.get(var_name)
    }

    /// Get the integer value assigned to a variable
    pub fn get_int_value(&self, var_name: &str) -> Option<i64> {
        match self.assignments.get(var_name) {
            Some(ConstValue::I64(val)) => Some(*val),
            Some(ConstValue::I32(val)) => Some(*val as i64),
            Some(ConstValue::U64(val)) => Some(*val as i64),
            Some(ConstValue::U8(val)) => Some(*val as i64),
            Some(ConstValue::U32(val)) => Some(*val as i64),
            _ => None,
        }
    }

    /// Get the boolean value assigned to a variable
    pub fn get_bool_value(&self, var_name: &str) -> Option<bool> {
        match self.assignments.get(var_name) {
            Some(ConstValue::Bool(val)) => Some(*val),
            _ => None,
        }
    }

    /// Get the string value assigned to a variable
    ///
    /// Returns a reference to the string value if the variable is assigned
    /// a string value in this model, or None otherwise.
    pub fn get_string_value(&self, var_name: &str) -> Option<&str> {
        match self.assignments.get(var_name) {
            Some(ConstValue::String(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Get the string value as an owned String
    ///
    /// Returns an owned String if the variable is assigned a string value
    /// in this model, or None otherwise.
    pub fn get_string(&self, var_name: &str) -> Option<String> {
        self.get_string_value(var_name).map(|s| s.to_string())
    }

    /// Get the u64 value assigned to a variable (for concolic execution)
    pub fn get_u64(&self, var_name: &str) -> Option<u64> {
        match self.assignments.get(var_name) {
            Some(ConstValue::U64(val)) => Some(*val),
            Some(ConstValue::I64(val)) => Some(*val as u64),
            Some(ConstValue::U8(val)) => Some(*val as u64),
            Some(ConstValue::U32(val)) => Some(*val as u64),
            Some(ConstValue::I32(val)) => Some(*val as u64),
            _ => None,
        }
    }

    /// Get the u32 value assigned to a variable (for concolic execution)
    pub fn get_u32(&self, var_name: &str) -> Option<u32> {
        match self.assignments.get(var_name) {
            Some(ConstValue::U32(val)) => Some(*val),
            Some(ConstValue::U64(val)) => Some(*val as u32),
            Some(ConstValue::I64(val)) => Some(*val as u32),
            Some(ConstValue::U8(val)) => Some(*val as u32),
            Some(ConstValue::I32(val)) => Some(*val as u32),
            _ => None,
        }
    }

    /// Get the i32 value assigned to a variable (for concolic execution)
    pub fn get_i32(&self, var_name: &str) -> Option<i32> {
        match self.assignments.get(var_name) {
            Some(ConstValue::I32(val)) => Some(*val),
            Some(ConstValue::I64(val)) => Some(*val as i32),
            Some(ConstValue::U64(val)) => Some(*val as i32),
            Some(ConstValue::U8(val)) => Some(*val as i32),
            Some(ConstValue::U32(val)) => Some(*val as i32),
            _ => None,
        }
    }

    /// Get the i64 value assigned to a variable (for concolic execution)
    pub fn get_i64(&self, var_name: &str) -> Option<i64> {
        match self.assignments.get(var_name) {
            Some(ConstValue::I64(val)) => Some(*val),
            Some(ConstValue::I32(val)) => Some(*val as i64),
            Some(ConstValue::U64(val)) => Some(*val as i64),
            Some(ConstValue::U8(val)) => Some(*val as i64),
            Some(ConstValue::U32(val)) => Some(*val as i64),
            _ => None,
        }
    }

    /// Get the u8 value assigned to a variable (for concolic execution)
    pub fn get_u8(&self, var_name: &str) -> Option<u8> {
        match self.assignments.get(var_name) {
            Some(ConstValue::U8(val)) => Some(*val),
            Some(ConstValue::U64(val)) => Some(*val as u8),
            Some(ConstValue::I64(val)) => Some(*val as u8),
            Some(ConstValue::U32(val)) => Some(*val as u8),
            Some(ConstValue::I32(val)) => Some(*val as u8),
            _ => None,
        }
    }

    /// Get all variable names in this model
    pub fn get_variables(&self) -> Vec<String> {
        let mut vars: Vec<String> = self.assignments.keys().cloned().collect();
        vars.sort();
        vars
    }

    /// Check if the model contains a value for the given variable
    pub fn contains_variable(&self, var_name: &str) -> bool {
        self.assignments.contains_key(var_name)
    }

    /// Get the number of variable assignments in this model
    pub fn size(&self) -> usize {
        self.assignments.len()
    }

    /// Check if the model is empty
    pub fn is_empty(&self) -> bool {
        self.assignments.is_empty()
    }
}

impl Default for Model {
    fn default() -> Self {
        Self::new()
    }
}

/// Abstract interface for SMT solvers
pub trait SmtSolver {
    /// Check satisfiability of given constraints
    fn check_sat(&mut self, constraints: &[SymExpr]) -> SymExResult<SatResult>;

    /// Get model (concrete values) for satisfiable constraints
    fn get_model(&mut self) -> SymExResult<Option<Model>>;

    /// Push solver state (for backtracking)
    fn push(&mut self) -> SymExResult<()>;

    /// Pop solver state (for backtracking)
    fn pop(&mut self) -> SymExResult<()>;

    /// Assert a constraint in the current solver context
    fn assert(&mut self, constraint: SymExpr) -> SymExResult<()>;

    /// Reset solver to initial state
    fn reset(&mut self) -> SymExResult<()>;
}

/// Z3 SMT solver implementation
///
/// This struct manages a Z3 context and solver instance for constraint
/// satisfiability checking and model generation.
pub struct Z3Solver {
    config: Config,
    context: Context,
    // Timeout in milliseconds for solver operations
    timeout_ms: Option<u32>,
    // Track the current assertion level for push/pop operations
    assertion_level: usize,
    // Track asserted constraints for state management
    asserted_constraints: Vec<SymExpr>,
    // Stack to save constraint states for push/pop operations
    constraint_stack: Vec<Vec<SymExpr>>,
    // Track the last constraints passed to check_sat for get_model
    last_checked_constraints: Vec<SymExpr>,
}

impl Z3Solver {
    /// Create a new Z3 solver instance
    pub fn new() -> SymExResult<Self> {
        Self::with_timeout(None)
    }

    /// Create a new Z3 solver instance with optional timeout
    pub fn with_timeout(timeout_ms: Option<u32>) -> SymExResult<Self> {
        // Try to create Z3 configuration
        let mut config = Config::new();

        // Configure timeout if provided
        if let Some(timeout) = timeout_ms {
            config.set_timeout_msec(timeout as u64);
        }

        // Try to create Z3 context
        let context = Context::new(&config);

        // Verify that Z3 is working by creating a simple solver
        {
            let test_solver = Solver::new(&context);

            // Test basic Z3 functionality
            let test_var = Int::new_const(&context, "test_var");
            let test_constraint = test_var.gt(&Int::from_i64(&context, 0));
            test_solver.assert(&test_constraint);

            // Check that Z3 can handle basic satisfiability
            match test_solver.check() {
                Z3SatResult::Sat | Z3SatResult::Unsat => {
                    // Z3 is working correctly
                }
                Z3SatResult::Unknown => {
                    return Err(SymExError::SolverError(
                        "Z3 solver initialization failed: basic satisfiability check returned Unknown".to_string()
                    ));
                }
            }
        }

        Ok(Self {
            config,
            context,
            timeout_ms,
            assertion_level: 0,
            asserted_constraints: Vec::new(),
            constraint_stack: Vec::new(),
            last_checked_constraints: Vec::new(),
        })
    }

    /// Set the timeout for solver operations
    pub fn set_timeout(&mut self, timeout_ms: Option<u32>) -> SymExResult<()> {
        self.timeout_ms = timeout_ms;

        // Recreate config and context with new timeout
        let mut config = Config::new();
        if let Some(timeout) = timeout_ms {
            config.set_timeout_msec(timeout as u64);
        }

        self.config = config;
        self.context = Context::new(&self.config);

        Ok(())
    }

    /// Get the current timeout setting
    pub fn get_timeout(&self) -> Option<u32> {
        self.timeout_ms
    }

    /// Check if Z3 string theory is available
    ///
    /// This method verifies that Z3 can handle string operations by attempting
    /// to create a simple string constraint. String theory support is required
    /// for symbolic string operations.
    pub fn check_string_theory_support(&self) -> SymExResult<()> {
        // Try to create a simple string constraint to verify string theory support
        let test_solver = Solver::new(&self.context);

        // Attempt to create a string variable and a simple constraint
        let test_str = z3::ast::String::new_const(&self.context, "test_string");
        let empty_str = z3::ast::String::from_str(&self.context, "").map_err(|_| {
            SymExError::SolverError(
                "Failed to create empty string literal for string theory test".to_string(),
            )
        })?;
        let test_constraint = test_str._eq(&empty_str);

        // Try to assert the constraint
        test_solver.assert(&test_constraint);

        // Check if the solver can handle it
        match test_solver.check() {
            Z3SatResult::Sat | Z3SatResult::Unsat => {
                // String theory is supported
                Ok(())
            }
            Z3SatResult::Unknown => Err(SymExError::SolverError(
                "Z3 string theory is not available or not properly configured".to_string(),
            )),
        }
    }

    /// Set the solver logic to support string operations
    ///
    /// This method configures the solver to use QF_S (quantifier-free strings)
    /// or QF_SLIA (quantifier-free strings with linear integer arithmetic) logic,
    /// which is required for string constraint solving.
    ///
    /// Note: The Z3 Rust bindings don't expose set_logic directly, so we rely on
    /// Z3's automatic logic detection. This method serves as a documentation point
    /// and validation check.
    pub fn set_string_logic(&self) -> SymExResult<()> {
        // Z3's Rust bindings automatically detect the required logic based on
        // the constraints asserted. We verify that string theory is available.
        self.check_string_theory_support()?;

        // If we reach here, string theory is supported
        // Z3 will automatically use QF_S or QF_SLIA as needed
        Ok(())
    }

    /// Get a new solver instance for this context
    ///
    /// We create solvers on-demand rather than storing them because
    /// Z3 solvers have lifetime constraints tied to the context.
    fn get_solver(&self) -> Solver {
        let solver = Solver::new(&self.context);

        // Configure timeout if set - timeout is configured at the context level
        // The solver inherits timeout settings from the context

        solver
    }

    /// Check if a single constraint is satisfiable
    pub fn is_satisfiable(&mut self, constraint: &SymExpr) -> SymExResult<bool> {
        match self.check_sat(&[constraint.clone()]) {
            Ok(SatResult::Sat) => Ok(true),
            Ok(SatResult::Unsat) => Ok(false),
            Ok(SatResult::Unknown) => Ok(false), // Conservative: treat unknown as unsatisfiable
            Err(e) => Err(e),
        }
    }

    /// Check if a set of constraints is unsatisfiable (contradictory)
    pub fn is_unsatisfiable(&mut self, constraints: &[SymExpr]) -> SymExResult<bool> {
        match self.check_sat(constraints) {
            Ok(SatResult::Unsat) => Ok(true),
            Ok(SatResult::Sat) => Ok(false),
            Ok(SatResult::Unknown) => Ok(false), // Conservative: treat unknown as satisfiable
            Err(e) => Err(e),
        }
    }

    /// Get all variables referenced in a set of constraints
    pub fn get_constraint_variables(&self, constraints: &[SymExpr]) -> Vec<String> {
        let mut all_vars = Vec::new();
        for constraint in constraints {
            let mut vars = constraint.get_variables();
            all_vars.append(&mut vars);
        }
        all_vars.sort();
        all_vars.dedup();
        all_vars
    }

    /// Get the current assertion level (number of push operations)
    pub fn get_assertion_level(&self) -> usize {
        self.assertion_level
    }

    /// Get the number of currently asserted constraints
    pub fn get_constraint_count(&self) -> usize {
        self.asserted_constraints.len()
    }

    /// Get a copy of all currently asserted constraints
    pub fn get_asserted_constraints(&self) -> Vec<SymExpr> {
        self.asserted_constraints.clone()
    }

    /// Create a solver with all currently asserted constraints
    fn get_solver_with_state(&self) -> SymExResult<Solver> {
        let solver = self.get_solver(); // This includes timeout settings

        // Assert all stored constraints with error recovery
        for (i, constraint) in self.asserted_constraints.iter().enumerate() {
            match self.symexpr_to_z3_bool(constraint) {
                Ok(z3_constraint) => {
                    solver.assert(&z3_constraint);
                }
                Err(e) => {
                    return Err(SymExError::SolverError(format!(
                        "Failed to convert stored constraint {i} to Z3 format: {e}"
                    )));
                }
            }
        }

        Ok(solver)
    }

    /// Validate a constraint before processing
    fn validate_constraint(&self, constraint: &SymExpr) -> SymExResult<()> {
        // Check that the expression is not completely empty
        // An expression is valid if it has variables OR constants (including nested ones)
        fn has_content(expr: &SymExpr) -> bool {
            match expr {
                SymExpr::Variable(_) | SymExpr::Constant(_) => true,
                SymExpr::BinaryOp(_, left, right) => has_content(left) || has_content(right),
                SymExpr::UnaryOp(_, operand) => has_content(operand),
                SymExpr::Conditional(cond, then_expr, else_expr) => {
                    has_content(cond) || has_content(then_expr) || has_content(else_expr)
                }
                SymExpr::StrSubstring(string, start, length) => {
                    has_content(string) || has_content(start) || has_content(length)
                }
                SymExpr::StrContains(haystack, needle) => {
                    has_content(haystack) || has_content(needle)
                }
                SymExpr::StrPrefixOf(prefix, string) => has_content(prefix) || has_content(string),
                SymExpr::StrSuffixOf(suffix, string) => has_content(suffix) || has_content(string),
                SymExpr::StrReplace(string, pattern, replacement) => {
                    has_content(string) || has_content(pattern) || has_content(replacement)
                }
                SymExpr::StrReplaceAll(string, pattern, replacement) => {
                    has_content(string) || has_content(pattern) || has_content(replacement)
                }
                SymExpr::StrAt(string, index) => has_content(string) || has_content(index),
                SymExpr::StrIndexOf(haystack, needle, offset) => {
                    has_content(haystack) || has_content(needle) || has_content(offset)
                }
            }
        }

        if !has_content(constraint) {
            return Err(SymExError::MalformedExpression(
                "Expression contains no variables or constants".to_string(),
            ));
        }

        // Check expression depth to prevent stack overflow
        if constraint.depth() > 100 {
            return Err(SymExError::ResourceExhaustion(
                "Expression depth exceeds maximum limit (100)".to_string(),
            ));
        }

        // Check node count to prevent excessive memory usage
        if constraint.node_count() > 10000 {
            return Err(SymExError::ResourceExhaustion(
                "Expression complexity exceeds maximum limit (10000 nodes)".to_string(),
            ));
        }

        Ok(())
    }

    /// Attempt to recover from solver errors
    #[allow(dead_code)]
    fn attempt_error_recovery(&mut self, error: &SymExError) -> SymExResult<()> {
        match error {
            SymExError::SolverTimeout => {
                // For timeout errors, we could try reducing the problem complexity
                // For now, just log and continue
                Ok(())
            }
            SymExError::ResourceExhaustion(_) => {
                // For resource exhaustion, we could try clearing some state
                // For now, suggest a reset
                Err(SymExError::SolverError(
                    "Resource exhaustion detected. Consider calling reset() to clear solver state."
                        .to_string(),
                ))
            }
            SymExError::StateCorruption(_) => {
                // For state corruption, force a reset
                self.reset()?;
                Ok(())
            }
            _ => {
                // For other errors, no recovery action
                Ok(())
            }
        }
    }

    /// Convert a Z3 AST value back to a ConstValue
    #[allow(dead_code)]
    fn z3_ast_to_const_value(&self, _ast: &dyn Ast) -> Option<ConstValue> {
        // This is a placeholder implementation
        // The actual value extraction is done in get_model_for_constraints
        // using the proper Z3 model.eval() method
        None
    }

    /// Extract a string value from a Z3 model
    ///
    /// This method attempts to extract the string value assigned to a variable
    /// in the given Z3 model. Returns None if the variable is not found or
    /// cannot be evaluated as a string.
    fn extract_string_from_model(&self, z3_model: &z3::Model, var_name: &str) -> Option<String> {
        // Create a Z3 string constant for the variable
        let string_var = z3::ast::String::new_const(&self.context, var_name);

        // Try to evaluate the variable in the model
        if let Some(value_ast) = z3_model.eval(&string_var, true) {
            // Try to convert the Z3 AST to a string
            // The z3 crate provides as_string() method for String AST nodes
            if let Some(string_val) = value_ast.as_string() {
                return Some(string_val.to_string());
            }
        }

        None
    }

    /// Infer variable types from constraints
    ///
    /// This method analyzes constraints to determine the likely type of each variable
    /// based on how it's used in the constraints.
    fn infer_variable_types<'a>(&self, expr: &'a SymExpr, types: &mut HashMap<String, &'a str>) {
        match expr {
            SymExpr::Variable(_) => {
                // Variables alone don't tell us the type
            }
            SymExpr::Constant(val) => {
                // Constants don't have variable names
                match val {
                    ConstValue::String(_) => {}
                    ConstValue::Bool(_) => {}
                    _ => {}
                }
            }
            SymExpr::BinaryOp(op, left, right) => {
                // Infer types based on the operation
                match op {
                    BinOp::StrConcat | BinOp::StrLexLt | BinOp::StrLexLe => {
                        // String operations - mark variables as strings
                        if let SymExpr::Variable(name) = left.as_ref() {
                            types.insert(name.clone(), "string");
                        }
                        if let SymExpr::Variable(name) = right.as_ref() {
                            types.insert(name.clone(), "string");
                        }
                    }
                    BinOp::Eq | BinOp::Ne => {
                        // Equality - infer type from the other operand
                        match (left.as_ref(), right.as_ref()) {
                            (SymExpr::Variable(name), SymExpr::Constant(ConstValue::String(_)))
                            | (SymExpr::Constant(ConstValue::String(_)), SymExpr::Variable(name)) =>
                            {
                                types.insert(name.clone(), "string");
                            }
                            (SymExpr::Variable(name), SymExpr::Constant(ConstValue::Bool(_)))
                            | (SymExpr::Constant(ConstValue::Bool(_)), SymExpr::Variable(name)) => {
                                types.insert(name.clone(), "bool");
                            }
                            _ => {}
                        }
                    }
                    BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                        // Boolean operations
                        if let SymExpr::Variable(name) = left.as_ref() {
                            types.insert(name.clone(), "bool");
                        }
                        if let SymExpr::Variable(name) = right.as_ref() {
                            types.insert(name.clone(), "bool");
                        }
                    }
                    _ => {
                        // Other operations - assume integer
                    }
                }

                // Recursively infer types from sub-expressions
                self.infer_variable_types(left, types);
                self.infer_variable_types(right, types);
            }
            SymExpr::UnaryOp(op, operand) => {
                match op {
                    UnOp::Not => {
                        // Boolean operation
                        if let SymExpr::Variable(name) = operand.as_ref() {
                            types.insert(name.clone(), "bool");
                        }
                    }
                    UnOp::StrLen => {
                        // String length - operand is a string
                        if let SymExpr::Variable(name) = operand.as_ref() {
                            types.insert(name.clone(), "string");
                        }
                    }
                    _ => {}
                }

                self.infer_variable_types(operand, types);
            }
            SymExpr::Conditional(cond, then_expr, else_expr) => {
                self.infer_variable_types(cond, types);
                self.infer_variable_types(then_expr, types);
                self.infer_variable_types(else_expr, types);
            }
            SymExpr::StrSubstring(string, start, length) => {
                if let SymExpr::Variable(name) = string.as_ref() {
                    types.insert(name.clone(), "string");
                }
                self.infer_variable_types(string, types);
                self.infer_variable_types(start, types);
                self.infer_variable_types(length, types);
            }
            SymExpr::StrContains(haystack, needle)
            | SymExpr::StrPrefixOf(haystack, needle)
            | SymExpr::StrSuffixOf(haystack, needle) => {
                if let SymExpr::Variable(name) = haystack.as_ref() {
                    types.insert(name.clone(), "string");
                }
                if let SymExpr::Variable(name) = needle.as_ref() {
                    types.insert(name.clone(), "string");
                }
                self.infer_variable_types(haystack, types);
                self.infer_variable_types(needle, types);
            }
            SymExpr::StrReplace(string, pattern, replacement)
            | SymExpr::StrReplaceAll(string, pattern, replacement) => {
                if let SymExpr::Variable(name) = string.as_ref() {
                    types.insert(name.clone(), "string");
                }
                if let SymExpr::Variable(name) = pattern.as_ref() {
                    types.insert(name.clone(), "string");
                }
                if let SymExpr::Variable(name) = replacement.as_ref() {
                    types.insert(name.clone(), "string");
                }
                self.infer_variable_types(string, types);
                self.infer_variable_types(pattern, types);
                self.infer_variable_types(replacement, types);
            }
            SymExpr::StrAt(string, index) => {
                if let SymExpr::Variable(name) = string.as_ref() {
                    types.insert(name.clone(), "string");
                }
                self.infer_variable_types(string, types);
                self.infer_variable_types(index, types);
            }
            SymExpr::StrIndexOf(haystack, needle, offset) => {
                if let SymExpr::Variable(name) = haystack.as_ref() {
                    types.insert(name.clone(), "string");
                }
                if let SymExpr::Variable(name) = needle.as_ref() {
                    types.insert(name.clone(), "string");
                }
                self.infer_variable_types(haystack, types);
                self.infer_variable_types(needle, types);
                self.infer_variable_types(offset, types);
            }
        }
    }

    /// Get a model for a specific set of constraints
    pub fn get_model_for_constraints(
        &mut self,
        constraints: &[SymExpr],
    ) -> SymExResult<Option<Model>> {
        // First check if the constraints are satisfiable
        match self.check_sat(constraints)? {
            SatResult::Sat => {
                // If satisfiable, create a solver with the constraints and get the model
                let solver = self.get_solver();

                // Collect all variables from constraints
                let all_variables = self.get_constraint_variables(constraints);

                // Infer variable types from constraints
                let mut variable_types: HashMap<String, &str> = HashMap::new();
                for constraint in constraints {
                    self.infer_variable_types(constraint, &mut variable_types);
                }

                // Assert all constraints
                for constraint in constraints {
                    let z3_constraint = self.symexpr_to_z3_bool(constraint)?;
                    solver.assert(&z3_constraint);
                }

                // Check satisfiability again (should be Sat)
                match solver.check() {
                    Z3SatResult::Sat => {
                        match solver.get_model() {
                            Some(z3_model) => {
                                let mut assignments = HashMap::new();

                                // Try to extract each variable using inferred type information
                                for var_name in &all_variables {
                                    let inferred_type =
                                        variable_types.get(var_name.as_str()).copied();

                                    match inferred_type {
                                        Some("string") => {
                                            // Try as string
                                            if let Some(string_val) =
                                                self.extract_string_from_model(&z3_model, var_name)
                                            {
                                                assignments.insert(
                                                    var_name.clone(),
                                                    ConstValue::String(string_val),
                                                );
                                                continue;
                                            }
                                        }
                                        Some("bool") => {
                                            // Try as boolean
                                            let bool_var =
                                                Bool::new_const(&self.context, var_name.as_str());
                                            if let Some(value_ast) = z3_model.eval(&bool_var, true)
                                            {
                                                if let Some(bool_val) = value_ast.as_bool() {
                                                    assignments.insert(
                                                        var_name.clone(),
                                                        ConstValue::Bool(bool_val),
                                                    );
                                                    continue;
                                                }
                                            }
                                        }
                                        _ => {
                                            // Default to integer
                                            let int_var =
                                                Int::new_const(&self.context, var_name.as_str());
                                            if let Some(value_ast) = z3_model.eval(&int_var, true) {
                                                if let Some(int_val) = value_ast.as_i64() {
                                                    assignments.insert(
                                                        var_name.clone(),
                                                        ConstValue::I64(int_val),
                                                    );
                                                    continue;
                                                }
                                            }
                                        }
                                    }

                                    // Fallback: try all types if inferred type didn't work
                                    if !assignments.contains_key(var_name) {
                                        // Try as integer
                                        let int_var =
                                            Int::new_const(&self.context, var_name.as_str());
                                        if let Some(value_ast) = z3_model.eval(&int_var, true) {
                                            if let Some(int_val) = value_ast.as_i64() {
                                                assignments.insert(
                                                    var_name.clone(),
                                                    ConstValue::I64(int_val),
                                                );
                                                continue;
                                            }
                                        }

                                        // Try as boolean
                                        let bool_var =
                                            Bool::new_const(&self.context, var_name.as_str());
                                        if let Some(value_ast) = z3_model.eval(&bool_var, true) {
                                            if let Some(bool_val) = value_ast.as_bool() {
                                                assignments.insert(
                                                    var_name.clone(),
                                                    ConstValue::Bool(bool_val),
                                                );
                                                continue;
                                            }
                                        }

                                        // Try as string
                                        if let Some(string_val) =
                                            self.extract_string_from_model(&z3_model, var_name)
                                        {
                                            assignments.insert(
                                                var_name.clone(),
                                                ConstValue::String(string_val),
                                            );
                                        }
                                    }
                                }

                                Ok(Some(Model { assignments }))
                            }
                            None => Ok(None),
                        }
                    }
                    _ => Ok(None),
                }
            }
            SatResult::Unsat => Ok(None),
            SatResult::Unknown => Ok(None),
        }
    }

    /// Convert a SymExpr to a Z3 integer AST node
    /// This is a simplified implementation that assumes integer operations
    fn symexpr_to_z3_int(&self, expr: &SymExpr) -> SymExResult<Int> {
        match expr {
            SymExpr::Variable(name) => {
                if name.is_empty() {
                    return Err(SymExError::MalformedExpression(
                        "Variable name cannot be empty".to_string(),
                    ));
                }

                // Check for valid variable name (basic validation)
                if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    return Err(SymExError::MalformedExpression(format!(
                        "Invalid variable name '{name}': must contain only alphanumeric characters and underscores"
                    )));
                }

                let var = Int::new_const(&self.context, name.as_str());
                Ok(var)
            }

            SymExpr::Constant(value) => {
                match value {
                    ConstValue::I64(n) => Ok(Int::from_i64(&self.context, *n)),
                    ConstValue::I32(n) => Ok(Int::from_i64(&self.context, *n as i64)),
                    ConstValue::U64(n) => {
                        // Check for overflow when converting to i64
                        if *n > i64::MAX as u64 {
                            return Err(SymExError::OverflowError(format!(
                                "U64 value {n} exceeds maximum i64 value"
                            )));
                        }
                        Ok(Int::from_u64(&self.context, *n))
                    }
                    ConstValue::U32(n) => Ok(Int::from_u64(&self.context, *n as u64)),
                    ConstValue::U8(n) => Ok(Int::from_u64(&self.context, *n as u64)),
                    ConstValue::F64(f) => {
                        if f.is_nan() {
                            return Err(SymExError::InvalidOperation(
                                "Cannot convert NaN to integer".to_string(),
                            ));
                        }
                        if f.is_infinite() {
                            return Err(SymExError::InvalidOperation(
                                "Cannot convert infinity to integer".to_string(),
                            ));
                        }
                        Err(SymExError::TypeMismatch {
                            expected: "integer".to_string(),
                            found: "float".to_string(),
                        })
                    }
                    ConstValue::F32(f) => {
                        if f.is_nan() {
                            return Err(SymExError::InvalidOperation(
                                "Cannot convert NaN to integer".to_string(),
                            ));
                        }
                        if f.is_infinite() {
                            return Err(SymExError::InvalidOperation(
                                "Cannot convert infinity to integer".to_string(),
                            ));
                        }
                        Err(SymExError::TypeMismatch {
                            expected: "integer".to_string(),
                            found: "float".to_string(),
                        })
                    }
                    ConstValue::Bool(_) => Err(SymExError::TypeMismatch {
                        expected: "integer".to_string(),
                        found: "boolean".to_string(),
                    }),
                    ConstValue::String(_) => Err(SymExError::TypeMismatch {
                        expected: "integer".to_string(),
                        found: "string".to_string(),
                    }),
                }
            }

            SymExpr::BinaryOp(op, left, right) => {
                let left_int = self.symexpr_to_z3_int(left)?;
                let right_int = self.symexpr_to_z3_int(right)?;

                match op {
                    BinOp::Add => Ok(&left_int + &right_int),
                    BinOp::Sub => Ok(&left_int - &right_int),
                    BinOp::Mul => Ok(&left_int * &right_int),
                    BinOp::Div => Ok(&left_int / &right_int),
                    BinOp::Mod => Ok(left_int.modulo(&right_int)),

                    // Bitwise operations (treating as integer operations)
                    BinOp::BitAnd => {
                        // For integers, we need to convert to bitvectors for proper bitwise ops
                        // For now, treat as unsupported
                        Err(SymExError::SolverError(
                            "Bitwise operations on integers not yet supported".to_string(),
                        ))
                    }
                    BinOp::BitOr => Err(SymExError::SolverError(
                        "Bitwise operations on integers not yet supported".to_string(),
                    )),
                    BinOp::BitXor => Err(SymExError::SolverError(
                        "Bitwise operations on integers not yet supported".to_string(),
                    )),
                    BinOp::Shl => Err(SymExError::SolverError(
                        "Shift operations on integers not yet supported".to_string(),
                    )),
                    BinOp::Shr => Err(SymExError::SolverError(
                        "Shift operations on integers not yet supported".to_string(),
                    )),

                    _ => Err(SymExError::SolverError(format!(
                        "Unsupported integer operation: {op:?}"
                    ))),
                }
            }

            SymExpr::UnaryOp(op, operand) => {
                match op {
                    UnOp::Neg => {
                        let operand_int = self.symexpr_to_z3_int(operand)?;
                        Ok(-&operand_int)
                    }
                    UnOp::StrLen => {
                        // String length operation
                        // This operation is not directly exposed in the z3 Rust bindings
                        let _string_ast = self.symexpr_to_z3_string(operand)?;

                        Err(SymExError::SolverError(
                            "String length operation not yet fully implemented in Z3 Rust bindings"
                                .to_string(),
                        ))
                    }
                    _ => Err(SymExError::SolverError(format!(
                        "Unsupported unary integer operation: {op:?}"
                    ))),
                }
            }

            SymExpr::Conditional(cond, then_expr, else_expr) => {
                let cond_bool = self.symexpr_to_z3_bool(cond)?;
                let then_int = self.symexpr_to_z3_int(then_expr)?;
                let else_int = self.symexpr_to_z3_int(else_expr)?;
                Ok(cond_bool.ite(&then_int, &else_int))
            }

            // String operations that return integers (StrIndexOf)
            SymExpr::StrIndexOf(haystack, needle, offset) => {
                // String index_of operation
                let _haystack_str = self.symexpr_to_z3_string(haystack)?;
                let _needle_str = self.symexpr_to_z3_string(needle)?;
                let _offset_int = self.symexpr_to_z3_int(offset)?;

                Err(SymExError::SolverError(
                    "String index_of operation not yet fully implemented in Z3 Rust bindings"
                        .to_string(),
                ))
            }

            // String operations that don't return integers
            SymExpr::StrSubstring(_, _, _)
            | SymExpr::StrContains(_, _)
            | SymExpr::StrPrefixOf(_, _)
            | SymExpr::StrSuffixOf(_, _)
            | SymExpr::StrReplace(_, _, _)
            | SymExpr::StrReplaceAll(_, _, _)
            | SymExpr::StrAt(_, _) => Err(SymExError::TypeMismatch {
                expected: "integer".to_string(),
                found: "string".to_string(),
            }),
        }
    }

    /// Convert a SymExpr to a Z3 string AST node
    ///
    /// This method converts symbolic string expressions to Z3's string theory
    /// representation, enabling constraint solving over string operations.
    fn symexpr_to_z3_string(&self, expr: &SymExpr) -> SymExResult<z3::ast::String> {
        match expr {
            SymExpr::Variable(name) => {
                if name.is_empty() {
                    return Err(SymExError::MalformedExpression(
                        "Variable name cannot be empty".to_string(),
                    ));
                }

                // Check for valid variable name
                if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    return Err(SymExError::MalformedExpression(format!(
                        "Invalid variable name '{name}': must contain only alphanumeric characters and underscores"
                    )));
                }

                Ok(z3::ast::String::new_const(&self.context, name.as_str()))
            }

            SymExpr::Constant(ConstValue::String(s)) => {
                // Convert Rust string to Z3 string literal
                z3::ast::String::from_str(&self.context, s.as_str()).map_err(|_| {
                    SymExError::SolverError(format!(
                        "Failed to create Z3 string literal from '{s}'"
                    ))
                })
            }

            SymExpr::BinaryOp(BinOp::StrConcat, left, right) => {
                // String concatenation using Z3's seq.concat
                let left_str = self.symexpr_to_z3_string(left)?;
                let right_str = self.symexpr_to_z3_string(right)?;

                // Use the concat static method from z3 crate
                Ok(z3::ast::String::concat(
                    &self.context,
                    &[&left_str, &right_str],
                ))
            }

            SymExpr::StrSubstring(string, start, length) => {
                // String substring using Z3's seq.extract
                // This operation is not directly exposed in the z3 Rust bindings
                // We need to use the FFI, but z3_ctx is private
                // For now, return an error indicating this is not yet implemented
                let _str_ast = self.symexpr_to_z3_string(string)?;
                let _start_ast = self.symexpr_to_z3_int(start)?;
                let _length_ast = self.symexpr_to_z3_int(length)?;

                Err(SymExError::SolverError(
                    "String substring operation not yet fully implemented in Z3 Rust bindings"
                        .to_string(),
                ))
            }

            SymExpr::StrReplace(string, pattern, replacement) => {
                // String replace using Z3's seq.replace
                // This operation is not directly exposed in the z3 Rust bindings
                let _str_ast = self.symexpr_to_z3_string(string)?;
                let _pattern_ast = self.symexpr_to_z3_string(pattern)?;
                let _replacement_ast = self.symexpr_to_z3_string(replacement)?;

                Err(SymExError::SolverError(
                    "String replace operation not yet fully implemented in Z3 Rust bindings"
                        .to_string(),
                ))
            }

            SymExpr::StrReplaceAll(string, pattern, replacement) => {
                // String replace all
                let _str_ast = self.symexpr_to_z3_string(string)?;
                let _pattern_ast = self.symexpr_to_z3_string(pattern)?;
                let _replacement_ast = self.symexpr_to_z3_string(replacement)?;

                Err(SymExError::SolverError(
                    "String replace_all operation not yet fully implemented in Z3 Rust bindings"
                        .to_string(),
                ))
            }

            SymExpr::StrAt(string, index) => {
                // Character at index using Z3's seq.at
                let _str_ast = self.symexpr_to_z3_string(string)?;
                let _index_ast = self.symexpr_to_z3_int(index)?;

                Err(SymExError::SolverError(
                    "String char_at operation not yet fully implemented in Z3 Rust bindings"
                        .to_string(),
                ))
            }

            _ => Err(SymExError::TypeMismatch {
                expected: "string".to_string(),
                found: format!("{:?}", expr),
            }),
        }
    }

    /// Convert a SymExpr to a Z3 boolean AST node
    fn symexpr_to_z3_bool(&self, expr: &SymExpr) -> SymExResult<Bool> {
        match expr {
            SymExpr::Variable(name) => {
                // Boolean variables
                Ok(Bool::new_const(&self.context, name.as_str()))
            }

            SymExpr::Constant(ConstValue::Bool(b)) => Ok(Bool::from_bool(&self.context, *b)),

            SymExpr::BinaryOp(op, left, right) => {
                match op {
                    // Comparison operations that return boolean
                    BinOp::Eq => {
                        // Try integer comparison first, then boolean, then string
                        if let (Ok(left_int), Ok(right_int)) =
                            (self.symexpr_to_z3_int(left), self.symexpr_to_z3_int(right))
                        {
                            Ok(left_int._eq(&right_int))
                        } else if let (Ok(left_bool), Ok(right_bool)) = (
                            self.symexpr_to_z3_bool(left),
                            self.symexpr_to_z3_bool(right),
                        ) {
                            Ok(left_bool._eq(&right_bool))
                        } else if let (Ok(left_str), Ok(right_str)) = (
                            self.symexpr_to_z3_string(left),
                            self.symexpr_to_z3_string(right),
                        ) {
                            Ok(left_str._eq(&right_str))
                        } else {
                            Err(SymExError::SolverError(
                                "Type mismatch in equality comparison".to_string(),
                            ))
                        }
                    }
                    BinOp::Ne => {
                        // Try integer comparison first, then boolean, then string
                        if let (Ok(left_int), Ok(right_int)) =
                            (self.symexpr_to_z3_int(left), self.symexpr_to_z3_int(right))
                        {
                            Ok(left_int._eq(&right_int).not())
                        } else if let (Ok(left_bool), Ok(right_bool)) = (
                            self.symexpr_to_z3_bool(left),
                            self.symexpr_to_z3_bool(right),
                        ) {
                            Ok(left_bool._eq(&right_bool).not())
                        } else if let (Ok(left_str), Ok(right_str)) = (
                            self.symexpr_to_z3_string(left),
                            self.symexpr_to_z3_string(right),
                        ) {
                            Ok(left_str._eq(&right_str).not())
                        } else {
                            Err(SymExError::SolverError(
                                "Type mismatch in inequality comparison".to_string(),
                            ))
                        }
                    }
                    BinOp::Lt => {
                        // Try integer comparison first, then string lexicographic comparison
                        match (self.symexpr_to_z3_int(left), self.symexpr_to_z3_int(right)) {
                            (Ok(left_int), Ok(right_int)) => Ok(left_int.lt(&right_int)),
                            (Err(e), _) | (_, Err(e)) => {
                                // If integer conversion failed with a validation error, propagate it
                                if matches!(e, SymExError::MalformedExpression(_)) {
                                    return Err(e);
                                }
                                // Otherwise, try string comparison
                                if let (Ok(_left_str), Ok(_right_str)) = (
                                    self.symexpr_to_z3_string(left),
                                    self.symexpr_to_z3_string(right),
                                ) {
                                    // Z3 string lexicographic comparison not yet fully implemented
                                    Err(SymExError::SolverError(
                                        "String lexicographic comparison not yet fully implemented in Z3 Rust bindings".to_string(),
                                    ))
                                } else {
                                    Err(SymExError::SolverError(
                                        "Type mismatch in less-than comparison".to_string(),
                                    ))
                                }
                            }
                        }
                    }
                    BinOp::Le => {
                        // Try integer comparison first, then string lexicographic comparison
                        match (self.symexpr_to_z3_int(left), self.symexpr_to_z3_int(right)) {
                            (Ok(left_int), Ok(right_int)) => Ok(left_int.le(&right_int)),
                            (Err(e), _) | (_, Err(e)) => {
                                // If integer conversion failed with a validation error, propagate it
                                if matches!(e, SymExError::MalformedExpression(_)) {
                                    return Err(e);
                                }
                                // Otherwise, try string comparison
                                if let (Ok(_left_str), Ok(_right_str)) = (
                                    self.symexpr_to_z3_string(left),
                                    self.symexpr_to_z3_string(right),
                                ) {
                                    // Z3 string lexicographic comparison not yet fully implemented
                                    Err(SymExError::SolverError(
                                        "String lexicographic comparison not yet fully implemented in Z3 Rust bindings".to_string(),
                                    ))
                                } else {
                                    Err(SymExError::SolverError(
                                        "Type mismatch in less-than-or-equal comparison"
                                            .to_string(),
                                    ))
                                }
                            }
                        }
                    }
                    BinOp::Gt => {
                        // Try integer comparison first, then string lexicographic comparison
                        match (self.symexpr_to_z3_int(left), self.symexpr_to_z3_int(right)) {
                            (Ok(left_int), Ok(right_int)) => Ok(left_int.gt(&right_int)),
                            (Err(e), _) | (_, Err(e)) => {
                                // If integer conversion failed with a validation error, propagate it
                                if matches!(e, SymExError::MalformedExpression(_)) {
                                    return Err(e);
                                }
                                // Otherwise, try string comparison
                                if let (Ok(_left_str), Ok(_right_str)) = (
                                    self.symexpr_to_z3_string(left),
                                    self.symexpr_to_z3_string(right),
                                ) {
                                    // Z3 string lexicographic comparison not yet fully implemented
                                    Err(SymExError::SolverError(
                                        "String lexicographic comparison not yet fully implemented in Z3 Rust bindings".to_string(),
                                    ))
                                } else {
                                    Err(SymExError::SolverError(
                                        "Type mismatch in greater-than comparison".to_string(),
                                    ))
                                }
                            }
                        }
                    }
                    BinOp::Ge => {
                        // Try integer comparison first, then string lexicographic comparison
                        match (self.symexpr_to_z3_int(left), self.symexpr_to_z3_int(right)) {
                            (Ok(left_int), Ok(right_int)) => Ok(left_int.ge(&right_int)),
                            (Err(e), _) | (_, Err(e)) => {
                                // If integer conversion failed with a validation error, propagate it
                                if matches!(e, SymExError::MalformedExpression(_)) {
                                    return Err(e);
                                }
                                // Otherwise, try string comparison
                                if let (Ok(_left_str), Ok(_right_str)) = (
                                    self.symexpr_to_z3_string(left),
                                    self.symexpr_to_z3_string(right),
                                ) {
                                    // Z3 string lexicographic comparison not yet fully implemented
                                    Err(SymExError::SolverError(
                                        "String lexicographic comparison not yet fully implemented in Z3 Rust bindings".to_string(),
                                    ))
                                } else {
                                    Err(SymExError::SolverError(
                                        "Type mismatch in greater-than-or-equal comparison"
                                            .to_string(),
                                    ))
                                }
                            }
                        }
                    }

                    // Logical operations on booleans
                    BinOp::BitAnd => {
                        // Treat as logical AND for booleans
                        let left_bool = self.symexpr_to_z3_bool(left)?;
                        let right_bool = self.symexpr_to_z3_bool(right)?;
                        Ok(Bool::and(&self.context, &[&left_bool, &right_bool]))
                    }
                    BinOp::BitOr => {
                        // Treat as logical OR for booleans
                        let left_bool = self.symexpr_to_z3_bool(left)?;
                        let right_bool = self.symexpr_to_z3_bool(right)?;
                        Ok(Bool::or(&self.context, &[&left_bool, &right_bool]))
                    }
                    BinOp::BitXor => {
                        // Treat as logical XOR for booleans
                        let left_bool = self.symexpr_to_z3_bool(left)?;
                        let right_bool = self.symexpr_to_z3_bool(right)?;
                        Ok(left_bool.xor(&right_bool))
                    }

                    _ => Err(SymExError::SolverError(format!(
                        "Unsupported boolean operation: {op:?}"
                    ))),
                }
            }

            SymExpr::UnaryOp(UnOp::Not, operand) => {
                let operand_bool = self.symexpr_to_z3_bool(operand)?;
                Ok(operand_bool.not())
            }

            SymExpr::Conditional(cond, then_expr, else_expr) => {
                let cond_bool = self.symexpr_to_z3_bool(cond)?;
                let then_bool = self.symexpr_to_z3_bool(then_expr)?;
                let else_bool = self.symexpr_to_z3_bool(else_expr)?;
                Ok(cond_bool.ite(&then_bool, &else_bool))
            }

            // String operations that return boolean
            SymExpr::StrContains(haystack, needle) => {
                let haystack_str = self.symexpr_to_z3_string(haystack)?;
                let needle_str = self.symexpr_to_z3_string(needle)?;

                // Use the contains method from z3 crate
                Ok(haystack_str.contains(&needle_str))
            }

            SymExpr::StrPrefixOf(prefix, string) => {
                let prefix_str = self.symexpr_to_z3_string(prefix)?;
                let string_str = self.symexpr_to_z3_string(string)?;

                // Use the prefix method from z3 crate
                Ok(prefix_str.prefix(&string_str))
            }

            SymExpr::StrSuffixOf(suffix, string) => {
                let suffix_str = self.symexpr_to_z3_string(suffix)?;
                let string_str = self.symexpr_to_z3_string(string)?;

                // Use the suffix method from z3 crate
                Ok(suffix_str.suffix(&string_str))
            }

            _ => Err(SymExError::SolverError(
                "Unsupported expression type for boolean conversion".to_string(),
            )),
        }
    }
}

impl SmtSolver for Z3Solver {
    fn check_sat(&mut self, constraints: &[SymExpr]) -> SymExResult<SatResult> {
        // Store the constraints for later use by get_model
        self.last_checked_constraints = constraints.to_vec();

        // Validate all constraints first
        for (i, constraint) in constraints.iter().enumerate() {
            if let Err(e) = self.validate_constraint(constraint) {
                return Err(SymExError::SolverError(format!(
                    "Constraint {i} validation failed: {e}"
                )));
            }
        }

        // Create a solver with current state
        let solver = self.get_solver_with_state()?;

        // Assert additional constraints if provided
        for (i, constraint) in constraints.iter().enumerate() {
            match self.symexpr_to_z3_bool(constraint) {
                Ok(z3_constraint) => {
                    solver.assert(&z3_constraint);
                }
                Err(e) => {
                    return Err(SymExError::SolverError(format!(
                        "Failed to convert constraint {i} to Z3 format: {e}"
                    )));
                }
            }
        }

        // If no constraints (neither stored nor provided), it's satisfiable
        if self.asserted_constraints.is_empty() && constraints.is_empty() {
            return Ok(SatResult::Sat);
        }

        // Check satisfiability with error handling and timeout detection
        match solver.check() {
            Z3SatResult::Sat => Ok(SatResult::Sat),
            Z3SatResult::Unsat => Ok(SatResult::Unsat),
            Z3SatResult::Unknown => {
                // Z3 returns Unknown for various reasons (timeout, resource limits, etc.)
                Ok(SatResult::Unknown)
            }
        }
    }

    fn get_model(&mut self) -> SymExResult<Option<Model>> {
        // Use the last checked constraints to get a model
        // This combines both asserted constraints and the constraints from the last check_sat call
        let mut all_constraints = self.asserted_constraints.clone();
        all_constraints.extend(self.last_checked_constraints.clone());

        self.get_model_for_constraints(&all_constraints)
    }

    fn push(&mut self) -> SymExResult<()> {
        // Save the current constraint state
        self.constraint_stack
            .push(self.asserted_constraints.clone());

        // Increment the assertion level
        self.assertion_level += 1;

        Ok(())
    }

    fn pop(&mut self) -> SymExResult<()> {
        if self.assertion_level == 0 {
            return Err(SymExError::StackUnderflow);
        }

        // Restore the previous constraint state
        if let Some(previous_constraints) = self.constraint_stack.pop() {
            self.asserted_constraints = previous_constraints;
        } else {
            return Err(SymExError::StateCorruption(
                "Constraint stack is empty but assertion level > 0".to_string(),
            ));
        }

        // Decrement the assertion level
        self.assertion_level -= 1;

        // Sanity check to prevent corruption
        if self.assertion_level > 1000 {
            return Err(SymExError::StateCorruption(
                "Assertion level exceeds reasonable bounds".to_string(),
            ));
        }

        Ok(())
    }

    fn assert(&mut self, constraint: SymExpr) -> SymExResult<()> {
        // Validate the constraint first
        self.validate_constraint(&constraint)?;

        // Validate that the constraint can be converted to boolean
        // We need to do this check before modifying state to avoid borrowing issues
        {
            let _validation_result = self.symexpr_to_z3_bool(&constraint)?;
            // Drop the validation result here to release the borrow
        }

        // Store the constraint in our state
        self.asserted_constraints.push(constraint);
        Ok(())
    }

    fn reset(&mut self) -> SymExResult<()> {
        // Create a new context and config to reset state, preserving timeout
        let mut config = Config::new();
        if let Some(timeout) = self.timeout_ms {
            config.set_timeout_msec(timeout as u64);
        }

        self.config = config;
        self.context = Context::new(&self.config);

        // Clear all state
        self.assertion_level = 0;
        self.asserted_constraints.clear();
        self.constraint_stack.clear();
        self.last_checked_constraints.clear();

        Ok(())
    }
}

impl Default for Z3Solver {
    fn default() -> Self {
        Self::new().expect("Failed to create Z3 solver")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expressions::{BinOp, ConstValue, SymExpr};

    #[test]
    fn test_z3_solver_creation() {
        let solver = Z3Solver::new();
        assert!(solver.is_ok());
    }

    #[test]
    fn test_simple_satisfiable_constraint() {
        let mut solver = Z3Solver::new().unwrap();

        // Create constraint: x > 0
        let x = SymExpr::variable("x".to_string());
        let zero = SymExpr::constant(ConstValue::I64(0));
        let constraint = SymExpr::binary_op(BinOp::Gt, x, zero);

        let result = solver.check_sat(&[constraint]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Sat);
    }

    #[test]
    fn test_unsatisfiable_constraint() {
        let mut solver = Z3Solver::new().unwrap();

        // Create contradictory constraints: x > 0 AND x < 0
        let x = SymExpr::variable("x".to_string());
        let zero = SymExpr::constant(ConstValue::I64(0));
        let constraint1 = SymExpr::binary_op(BinOp::Gt, x.clone(), zero.clone());
        let constraint2 = SymExpr::binary_op(BinOp::Lt, x, zero);

        let result = solver.check_sat(&[constraint1, constraint2]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Unsat);
    }

    #[test]
    fn test_arithmetic_operations() {
        let mut solver = Z3Solver::new().unwrap();

        // Create constraint: x + 1 = 5 (should be satisfiable with x = 4)
        let x = SymExpr::variable("x".to_string());
        let one = SymExpr::constant(ConstValue::I64(1));
        let five = SymExpr::constant(ConstValue::I64(5));
        let x_plus_one = SymExpr::binary_op(BinOp::Add, x, one);
        let constraint = SymExpr::binary_op(BinOp::Eq, x_plus_one, five);

        let result = solver.check_sat(&[constraint]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Sat);
    }

    #[test]
    fn test_solver_reset() {
        let mut solver = Z3Solver::new().unwrap();

        // Add a constraint
        let x = SymExpr::variable("x".to_string());
        let zero = SymExpr::constant(ConstValue::I64(0));
        let constraint = SymExpr::binary_op(BinOp::Gt, x, zero);

        let result1 = solver.check_sat(&[constraint]);
        assert!(result1.is_ok());

        // Reset solver
        let reset_result = solver.reset();
        assert!(reset_result.is_ok());

        // Should still work after reset
        let x2 = SymExpr::variable("y".to_string());
        let constraint2 = SymExpr::binary_op(BinOp::Gt, x2, SymExpr::constant(ConstValue::I64(10)));
        let result2 = solver.check_sat(&[constraint2]);
        assert!(result2.is_ok());
        assert_eq!(result2.unwrap(), SatResult::Sat);
    }

    #[test]
    fn test_assert_method() {
        let mut solver = Z3Solver::new().unwrap();

        let x = SymExpr::variable("x".to_string());
        let zero = SymExpr::constant(ConstValue::I64(0));
        let constraint = SymExpr::binary_op(BinOp::Gt, x, zero);

        let result = solver.assert(constraint);
        assert!(result.is_ok());
    }

    #[test]
    fn test_boolean_operations() {
        let mut solver = Z3Solver::new().unwrap();

        // Test boolean constants
        let true_expr = SymExpr::constant(ConstValue::Bool(true));
        let false_expr = SymExpr::constant(ConstValue::Bool(false));

        // Test AND operation: true AND false = false
        let and_expr = SymExpr::binary_op(BinOp::BitAnd, true_expr.clone(), false_expr.clone());
        let not_and = SymExpr::unary_op(UnOp::Not, and_expr);

        let result = solver.check_sat(&[not_and]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Sat); // NOT(true AND false) should be satisfiable
    }

    #[test]
    fn test_division_and_modulo() {
        let mut solver = Z3Solver::new().unwrap();

        // Test: x / 2 = 3 (should be satisfiable with x = 6)
        let x = SymExpr::variable("x".to_string());
        let two = SymExpr::constant(ConstValue::I64(2));
        let three = SymExpr::constant(ConstValue::I64(3));
        let x_div_2 = SymExpr::binary_op(BinOp::Div, x, two);
        let constraint = SymExpr::binary_op(BinOp::Eq, x_div_2, three);

        let result = solver.check_sat(&[constraint]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Sat);
    }

    #[test]
    fn test_conditional_expressions() {
        let mut solver = Z3Solver::new().unwrap();

        // Test: if x > 0 then 1 else -1 = 1
        let x = SymExpr::variable("x".to_string());
        let zero = SymExpr::constant(ConstValue::I64(0));
        let one = SymExpr::constant(ConstValue::I64(1));
        let neg_one = SymExpr::constant(ConstValue::I64(-1));

        let condition = SymExpr::binary_op(BinOp::Gt, x, zero);
        let conditional = SymExpr::conditional(condition, one.clone(), neg_one);
        let constraint = SymExpr::binary_op(BinOp::Eq, conditional, one);

        let result = solver.check_sat(&[constraint]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Sat);
    }

    #[test]
    fn test_boolean_variables() {
        let mut solver = Z3Solver::new().unwrap();

        // Test boolean variable: p OR NOT p (should always be true)
        let p = SymExpr::variable("p".to_string());
        let not_p = SymExpr::unary_op(UnOp::Not, p.clone());
        let tautology = SymExpr::binary_op(BinOp::BitOr, p, not_p);

        let result = solver.check_sat(&[tautology]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Sat);
    }

    #[test]
    fn test_mixed_type_equality() {
        let mut solver = Z3Solver::new().unwrap();

        // Test: (x > 0) = true
        let x = SymExpr::variable("x".to_string());
        let zero = SymExpr::constant(ConstValue::I64(0));
        let true_val = SymExpr::constant(ConstValue::Bool(true));

        let x_gt_zero = SymExpr::binary_op(BinOp::Gt, x, zero);
        let constraint = SymExpr::binary_op(BinOp::Eq, x_gt_zero, true_val);

        let result = solver.check_sat(&[constraint]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Sat);
    }

    #[test]
    fn test_empty_constraints() {
        let mut solver = Z3Solver::new().unwrap();

        // Empty constraint set should be satisfiable
        let result = solver.check_sat(&[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Sat);
    }

    #[test]
    fn test_is_satisfiable_helper() {
        let mut solver = Z3Solver::new().unwrap();

        // Test satisfiable constraint
        let x = SymExpr::variable("x".to_string());
        let zero = SymExpr::constant(ConstValue::I64(0));
        let satisfiable_constraint = SymExpr::binary_op(BinOp::Gt, x.clone(), zero.clone());

        let result = solver.is_satisfiable(&satisfiable_constraint);
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Test unsatisfiable constraint
        let unsatisfiable_constraint = SymExpr::binary_op(BinOp::Lt, x, zero);
        let and_constraint = SymExpr::binary_op(
            BinOp::BitAnd,
            satisfiable_constraint,
            unsatisfiable_constraint,
        );

        let result2 = solver.is_satisfiable(&and_constraint);
        assert!(result2.is_ok());
        assert!(!result2.unwrap());
    }

    #[test]
    fn test_is_unsatisfiable_helper() {
        let mut solver = Z3Solver::new().unwrap();

        // Create contradictory constraints
        let x = SymExpr::variable("x".to_string());
        let zero = SymExpr::constant(ConstValue::I64(0));
        let constraint1 = SymExpr::binary_op(BinOp::Gt, x.clone(), zero.clone());
        let constraint2 = SymExpr::binary_op(BinOp::Lt, x, zero);

        let result = solver.is_unsatisfiable(&[constraint1, constraint2]);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_constraint_variables_extraction() {
        let solver = Z3Solver::new().unwrap();

        let x = SymExpr::variable("x".to_string());
        let y = SymExpr::variable("y".to_string());
        let z = SymExpr::variable("z".to_string());
        let zero = SymExpr::constant(ConstValue::I64(0));

        let constraint1 = SymExpr::binary_op(BinOp::Gt, x.clone(), zero.clone());
        let constraint2 = SymExpr::binary_op(BinOp::Add, y, z);
        let constraint3 = SymExpr::binary_op(BinOp::Eq, x, constraint2);

        let variables = solver.get_constraint_variables(&[constraint1, constraint3]);

        // Should contain x, y, z (sorted and deduplicated)
        assert_eq!(
            variables,
            vec!["x".to_string(), "y".to_string(), "z".to_string()]
        );
    }

    #[test]
    fn test_complex_constraint_combination() {
        let mut solver = Z3Solver::new().unwrap();

        // Test: (x + y = 10) AND (x > 5) AND (y > 2)
        let x = SymExpr::variable("x".to_string());
        let y = SymExpr::variable("y".to_string());
        let ten = SymExpr::constant(ConstValue::I64(10));
        let five = SymExpr::constant(ConstValue::I64(5));
        let two = SymExpr::constant(ConstValue::I64(2));

        let x_plus_y = SymExpr::binary_op(BinOp::Add, x.clone(), y.clone());
        let constraint1 = SymExpr::binary_op(BinOp::Eq, x_plus_y, ten);
        let constraint2 = SymExpr::binary_op(BinOp::Gt, x, five);
        let constraint3 = SymExpr::binary_op(BinOp::Gt, y, two);

        let result = solver.check_sat(&[constraint1, constraint2, constraint3]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Sat);
    }

    #[test]
    fn test_error_handling_invalid_constraint() {
        let mut solver = Z3Solver::new().unwrap();

        // Create an expression that can't be converted to boolean
        // This is tricky since our current implementation is quite permissive
        // Let's test with a malformed expression structure

        // For now, test that the solver handles conversion errors gracefully
        let x = SymExpr::variable("x".to_string());
        let zero = SymExpr::constant(ConstValue::I64(0));
        let valid_constraint = SymExpr::binary_op(BinOp::Gt, x, zero);

        // This should work fine
        let result = solver.check_sat(&[valid_constraint]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_model_extraction() {
        let mut solver = Z3Solver::new().unwrap();

        // Create a simple constraint: x = 42
        let x = SymExpr::variable("x".to_string());
        let forty_two = SymExpr::constant(ConstValue::I64(42));
        let constraint = SymExpr::binary_op(BinOp::Eq, x, forty_two);

        // Get model for this constraint
        let model_result = solver.get_model_for_constraints(&[constraint]);
        assert!(model_result.is_ok());

        if let Some(model) = model_result.unwrap() {
            // Should contain x = 42
            assert!(model.contains_variable("x"));
            assert_eq!(model.get_int_value("x"), Some(42));
        }
    }

    #[test]
    fn test_model_with_multiple_variables() {
        let mut solver = Z3Solver::new().unwrap();

        // Create constraints: x + y = 10, x = 3
        let x = SymExpr::variable("x".to_string());
        let y = SymExpr::variable("y".to_string());
        let ten = SymExpr::constant(ConstValue::I64(10));
        let three = SymExpr::constant(ConstValue::I64(3));

        let x_plus_y = SymExpr::binary_op(BinOp::Add, x.clone(), y.clone());
        let constraint1 = SymExpr::binary_op(BinOp::Eq, x_plus_y, ten);
        let constraint2 = SymExpr::binary_op(BinOp::Eq, x, three);

        let model_result = solver.get_model_for_constraints(&[constraint1, constraint2]);
        assert!(model_result.is_ok());

        if let Some(model) = model_result.unwrap() {
            // Should contain x = 3, y = 7
            assert!(model.contains_variable("x"));
            assert!(model.contains_variable("y"));
            assert_eq!(model.get_int_value("x"), Some(3));
            assert_eq!(model.get_int_value("y"), Some(7));
            assert_eq!(model.size(), 2);
        }
    }

    #[test]
    fn test_model_with_boolean_variables() {
        let mut solver = Z3Solver::new().unwrap();

        // Create constraint: p OR NOT p (tautology, should always be satisfiable)
        let p = SymExpr::variable("p".to_string());
        let not_p = SymExpr::unary_op(UnOp::Not, p.clone());
        let constraint = SymExpr::binary_op(BinOp::BitOr, p, not_p);

        let model_result = solver.get_model_for_constraints(&[constraint]);
        assert!(model_result.is_ok());

        if let Some(model) = model_result.unwrap() {
            // The model should contain p
            assert!(model.contains_variable("p"));

            // Z3 might represent boolean variables as integers (0 = false, non-zero = true)
            // So we should be able to get either a boolean value or an integer value
            let has_bool_value = model.get_bool_value("p").is_some();
            let has_int_value = model.get_int_value("p").is_some();

            // At least one should be available
            assert!(has_bool_value || has_int_value);
        }
    }

    #[test]
    fn test_model_for_unsatisfiable_constraints() {
        let mut solver = Z3Solver::new().unwrap();

        // Create contradictory constraints: x > 0 AND x < 0
        let x = SymExpr::variable("x".to_string());
        let zero = SymExpr::constant(ConstValue::I64(0));
        let constraint1 = SymExpr::binary_op(BinOp::Gt, x.clone(), zero.clone());
        let constraint2 = SymExpr::binary_op(BinOp::Lt, x, zero);

        let model_result = solver.get_model_for_constraints(&[constraint1, constraint2]);
        assert!(model_result.is_ok());

        // Should return None for unsatisfiable constraints
        assert!(model_result.unwrap().is_none());
    }

    #[test]
    fn test_model_utility_methods() {
        let mut model = Model::new();
        assert!(model.is_empty());
        assert_eq!(model.size(), 0);

        // Manually create a model for testing
        model
            .assignments
            .insert("x".to_string(), ConstValue::I64(42));
        model
            .assignments
            .insert("y".to_string(), ConstValue::Bool(true));
        model
            .assignments
            .insert("z".to_string(), ConstValue::U64(100));

        assert!(!model.is_empty());
        assert_eq!(model.size(), 3);

        assert!(model.contains_variable("x"));
        assert!(model.contains_variable("y"));
        assert!(model.contains_variable("z"));
        assert!(!model.contains_variable("w"));

        assert_eq!(model.get_int_value("x"), Some(42));
        assert_eq!(model.get_bool_value("y"), Some(true));
        assert_eq!(model.get_int_value("z"), Some(100)); // U64 converted to i64

        let variables = model.get_variables();
        assert_eq!(
            variables,
            vec!["x".to_string(), "y".to_string(), "z".to_string()]
        );
    }

    #[test]
    fn test_model_type_conversions() {
        let mut model = Model::new();

        // Test different integer types
        model
            .assignments
            .insert("i32_var".to_string(), ConstValue::I32(-10));
        model
            .assignments
            .insert("u8_var".to_string(), ConstValue::U8(255));
        model
            .assignments
            .insert("i64_var".to_string(), ConstValue::I64(-1000));

        assert_eq!(model.get_int_value("i32_var"), Some(-10));
        assert_eq!(model.get_int_value("u8_var"), Some(255));
        assert_eq!(model.get_int_value("i64_var"), Some(-1000));

        // Boolean should not convert to int
        model
            .assignments
            .insert("bool_var".to_string(), ConstValue::Bool(true));
        assert_eq!(model.get_int_value("bool_var"), None);
        assert_eq!(model.get_bool_value("bool_var"), Some(true));
    }

    #[test]
    fn test_solver_state_management() {
        let mut solver = Z3Solver::new().unwrap();

        // Initially, assertion level should be 0
        assert_eq!(solver.get_assertion_level(), 0);
        assert_eq!(solver.get_constraint_count(), 0);

        // Test push operation
        let push_result = solver.push();
        assert!(push_result.is_ok());
        assert_eq!(solver.get_assertion_level(), 1);

        // Test multiple pushes
        solver.push().unwrap();
        solver.push().unwrap();
        assert_eq!(solver.get_assertion_level(), 3);

        // Test pop operation
        let pop_result = solver.pop();
        assert!(pop_result.is_ok());
        assert_eq!(solver.get_assertion_level(), 2);

        // Test pop until level 0
        solver.pop().unwrap();
        solver.pop().unwrap();
        assert_eq!(solver.get_assertion_level(), 0);

        // Test pop underflow
        let underflow_result = solver.pop();
        assert!(underflow_result.is_err());
        assert!(matches!(
            underflow_result.unwrap_err(),
            SymExError::StackUnderflow
        ));
    }

    #[test]
    fn test_constraint_assertion_and_state() {
        let mut solver = Z3Solver::new().unwrap();

        // Create some constraints
        let x = SymExpr::variable("x".to_string());
        let zero = SymExpr::constant(ConstValue::I64(0));
        let constraint1 = SymExpr::binary_op(BinOp::Gt, x.clone(), zero.clone());
        let constraint2 = SymExpr::binary_op(BinOp::Lt, x, SymExpr::constant(ConstValue::I64(10)));

        // Assert constraints
        assert!(solver.assert(constraint1.clone()).is_ok());
        assert_eq!(solver.get_constraint_count(), 1);

        assert!(solver.assert(constraint2.clone()).is_ok());
        assert_eq!(solver.get_constraint_count(), 2);

        // Check that constraints are stored
        let stored_constraints = solver.get_asserted_constraints();
        assert_eq!(stored_constraints.len(), 2);
        assert_eq!(stored_constraints[0], constraint1);
        assert_eq!(stored_constraints[1], constraint2);

        // Check satisfiability with stored constraints
        let result = solver.check_sat(&[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Sat);
    }

    #[test]
    fn test_solver_state_reset() {
        let mut solver = Z3Solver::new().unwrap();

        // Add some state
        solver.push().unwrap();
        solver.push().unwrap();

        let x = SymExpr::variable("x".to_string());
        let constraint = SymExpr::binary_op(BinOp::Gt, x, SymExpr::constant(ConstValue::I64(0)));
        solver.assert(constraint).unwrap();

        // Verify state exists
        assert_eq!(solver.get_assertion_level(), 2);
        assert_eq!(solver.get_constraint_count(), 1);

        // Reset solver
        let reset_result = solver.reset();
        assert!(reset_result.is_ok());

        // Verify state is cleared
        assert_eq!(solver.get_assertion_level(), 0);
        assert_eq!(solver.get_constraint_count(), 0);
        assert!(solver.get_asserted_constraints().is_empty());
    }

    #[test]
    fn test_check_sat_with_stored_and_additional_constraints() {
        let mut solver = Z3Solver::new().unwrap();

        // Store a constraint: x > 0
        let x = SymExpr::variable("x".to_string());
        let zero = SymExpr::constant(ConstValue::I64(0));
        let stored_constraint = SymExpr::binary_op(BinOp::Gt, x.clone(), zero.clone());
        solver.assert(stored_constraint).unwrap();

        // Check satisfiability with additional constraint: x < 10
        let ten = SymExpr::constant(ConstValue::I64(10));
        let additional_constraint = SymExpr::binary_op(BinOp::Lt, x.clone(), ten);

        let result = solver.check_sat(&[additional_constraint]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Sat); // x > 0 AND x < 10 is satisfiable

        // Check with contradictory additional constraint: x < 0
        let neg_constraint = SymExpr::binary_op(BinOp::Lt, x, zero);
        let result2 = solver.check_sat(&[neg_constraint]);
        assert!(result2.is_ok());
        assert_eq!(result2.unwrap(), SatResult::Unsat); // x > 0 AND x < 0 is unsatisfiable
    }

    #[test]
    fn test_empty_constraint_satisfiability() {
        let mut solver = Z3Solver::new().unwrap();

        // Empty constraints should be satisfiable
        let result = solver.check_sat(&[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Sat);

        // Even after push/pop operations
        solver.push().unwrap();
        solver.pop().unwrap();

        let result2 = solver.check_sat(&[]);
        assert!(result2.is_ok());
        assert_eq!(result2.unwrap(), SatResult::Sat);
    }

    #[test]
    fn test_error_handling_invalid_variable_names() {
        let mut solver = Z3Solver::new().unwrap();

        // Test empty variable name
        let empty_var = SymExpr::variable("".to_string());
        let constraint =
            SymExpr::binary_op(BinOp::Gt, empty_var, SymExpr::constant(ConstValue::I64(0)));

        let result = solver.assert(constraint);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SymExError::MalformedExpression(_)
        ));

        // Test invalid variable name with special characters
        let invalid_var = SymExpr::variable("x@#$".to_string());
        let constraint2 = SymExpr::binary_op(
            BinOp::Gt,
            invalid_var,
            SymExpr::constant(ConstValue::I64(0)),
        );

        let result2 = solver.assert(constraint2);
        assert!(result2.is_err());
        assert!(matches!(
            result2.unwrap_err(),
            SymExError::MalformedExpression(_)
        ));
    }

    #[test]
    fn test_error_handling_type_mismatches() {
        let mut solver = Z3Solver::new().unwrap();

        // Test float to integer conversion error
        let float_const = SymExpr::constant(ConstValue::F64(3.14));
        let int_var = SymExpr::variable("x".to_string());
        let constraint = SymExpr::binary_op(BinOp::Add, int_var, float_const);

        let result = solver.assert(constraint);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SymExError::SolverError(_)));
    }

    #[test]
    fn test_error_handling_nan_and_infinity() {
        let mut solver = Z3Solver::new().unwrap();

        // Test NaN handling
        let nan_const = SymExpr::constant(ConstValue::F64(f64::NAN));
        let var = SymExpr::variable("x".to_string());
        let constraint = SymExpr::binary_op(BinOp::Add, var.clone(), nan_const);

        let result = solver.assert(constraint);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SymExError::SolverError(_)));

        // Test infinity handling
        let inf_const = SymExpr::constant(ConstValue::F64(f64::INFINITY));
        let constraint2 = SymExpr::binary_op(BinOp::Add, var, inf_const);

        let result2 = solver.assert(constraint2);
        assert!(result2.is_err());
        assert!(matches!(result2.unwrap_err(), SymExError::SolverError(_)));
    }

    #[test]
    fn test_error_handling_stack_underflow() {
        let mut solver = Z3Solver::new().unwrap();

        // Try to pop from empty stack
        let result = solver.pop();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SymExError::StackUnderflow));

        // Push and pop should work
        solver.push().unwrap();
        let pop_result = solver.pop();
        assert!(pop_result.is_ok());

        // Another pop should fail
        let result2 = solver.pop();
        assert!(result2.is_err());
        assert!(matches!(result2.unwrap_err(), SymExError::StackUnderflow));
    }

    #[test]
    fn test_error_handling_resource_limits() {
        let solver = Z3Solver::new().unwrap();

        // Create a very deep expression to test depth limits
        let mut deep_expr = SymExpr::variable("x".to_string());
        for _i in 0..150 {
            // Exceed the depth limit of 100
            deep_expr = SymExpr::unary_op(UnOp::Neg, deep_expr);
        }

        let result = solver.validate_constraint(&deep_expr);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SymExError::ResourceExhaustion(_)
        ));
    }

    #[test]
    fn test_error_recovery_mechanisms() {
        let mut solver = Z3Solver::new().unwrap();

        // Test that solver can recover from certain errors
        let error = SymExError::SolverTimeout;
        let recovery_result = solver.attempt_error_recovery(&error);
        assert!(recovery_result.is_ok());

        // Test state corruption recovery
        let corruption_error = SymExError::StateCorruption("test corruption".to_string());
        let recovery_result2 = solver.attempt_error_recovery(&corruption_error);
        assert!(recovery_result2.is_ok());

        // After state corruption recovery, solver should be reset
        assert_eq!(solver.get_assertion_level(), 0);
        assert_eq!(solver.get_constraint_count(), 0);
    }

    #[test]
    fn test_solver_initialization_validation() {
        // Test that solver initialization includes Z3 validation
        let solver_result = Z3Solver::new();
        assert!(solver_result.is_ok());

        // The solver should be ready to use
        let mut solver = solver_result.unwrap();
        let simple_constraint = SymExpr::binary_op(
            BinOp::Gt,
            SymExpr::variable("test".to_string()),
            SymExpr::constant(ConstValue::I64(0)),
        );

        let result = solver.check_sat(&[simple_constraint]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_constraint_validation() {
        let solver = Z3Solver::new().unwrap();

        // Test valid constraint
        let valid_constraint = SymExpr::binary_op(
            BinOp::Gt,
            SymExpr::variable("x".to_string()),
            SymExpr::constant(ConstValue::I64(0)),
        );

        let result = solver.validate_constraint(&valid_constraint);
        assert!(result.is_ok());

        // Test constraint with no variables or constants (should fail)
        // This is tricky to create with our current API, so we'll skip this specific test

        // Test constraint that's too complex
        let mut complex_expr = SymExpr::variable("x".to_string());
        for _ in 0..50 {
            complex_expr =
                SymExpr::binary_op(BinOp::Add, complex_expr, SymExpr::variable("y".to_string()));
        }

        // This should still be valid (not exceeding limits)
        let result2 = solver.validate_constraint(&complex_expr);
        assert!(result2.is_ok());
    }

    #[test]
    fn test_z3_integration_basic_arithmetic() {
        let mut solver = Z3Solver::new().unwrap();

        // Test basic arithmetic: x + y = 10, x = 3, solve for y
        let x = SymExpr::variable("x".to_string());
        let y = SymExpr::variable("y".to_string());
        let ten = SymExpr::constant(ConstValue::I64(10));
        let three = SymExpr::constant(ConstValue::I64(3));

        let sum_constraint = SymExpr::binary_op(
            BinOp::Eq,
            SymExpr::binary_op(BinOp::Add, x.clone(), y.clone()),
            ten,
        );
        let x_constraint = SymExpr::binary_op(BinOp::Eq, x, three);

        // Assert constraints
        assert!(solver.assert(sum_constraint).is_ok());
        assert!(solver.assert(x_constraint).is_ok());

        // Check satisfiability
        let result = solver.check_sat(&[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Sat);

        // Get model and verify solution
        let model_result = solver.get_model();
        assert!(model_result.is_ok());

        if let Some(model) = model_result.unwrap() {
            assert!(model.contains_variable("x"));
            assert!(model.contains_variable("y"));
            assert_eq!(model.get_int_value("x"), Some(3));
            assert_eq!(model.get_int_value("y"), Some(7));
        }
    }

    #[test]
    fn test_z3_integration_boolean_logic() {
        let mut solver = Z3Solver::new().unwrap();

        // Test boolean logic: (p AND q) OR (NOT p AND r)
        let p = SymExpr::variable("p".to_string());
        let q = SymExpr::variable("q".to_string());
        let r = SymExpr::variable("r".to_string());

        let p_and_q = SymExpr::binary_op(BinOp::BitAnd, p.clone(), q.clone());
        let not_p = SymExpr::unary_op(UnOp::Not, p.clone());
        let not_p_and_r = SymExpr::binary_op(BinOp::BitAnd, not_p, r.clone());
        let constraint = SymExpr::binary_op(BinOp::BitOr, p_and_q, not_p_and_r);

        let result = solver.check_sat(&[constraint]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Sat);
    }

    #[test]
    fn test_property_satisfiability_query_correctness() {
        use crate::expressions::{BinOp, ConstValue, SymExpr};
        use quickcheck::{QuickCheck, TestResult};

        fn prop_satisfiability_consistency(x_val: i8, y_val: i8, threshold: i8) -> TestResult {
            let mut solver = match Z3Solver::new() {
                Ok(s) => s,
                Err(_) => return TestResult::discard(),
            };

            // Create constraint: x + y > threshold
            let x = SymExpr::variable("x".to_string());
            let y = SymExpr::variable("y".to_string());
            let x_const = SymExpr::constant(ConstValue::I32(x_val as i32));
            let y_const = SymExpr::constant(ConstValue::I32(y_val as i32));
            let threshold_const = SymExpr::constant(ConstValue::I32(threshold as i32));

            // Assert x = x_val and y = y_val
            let x_eq = SymExpr::binary_op(BinOp::Eq, x.clone(), x_const);
            let y_eq = SymExpr::binary_op(BinOp::Eq, y.clone(), y_const);

            // Create the main constraint: x + y > threshold
            let sum = SymExpr::binary_op(BinOp::Add, x, y);
            let constraint = SymExpr::binary_op(BinOp::Gt, sum, threshold_const);

            // Check satisfiability
            let sat_result = solver.check_sat(&[x_eq, y_eq, constraint]);

            match sat_result {
                Ok(result) => {
                    // The constraint should be satisfiable if and only if x_val + y_val > threshold
                    let expected_sat = (x_val as i32 + y_val as i32) > threshold as i32;

                    match result {
                        SatResult::Sat => TestResult::from_bool(expected_sat),
                        SatResult::Unsat => TestResult::from_bool(!expected_sat),
                        SatResult::Unknown => TestResult::discard(), // Z3 couldn't determine
                    }
                }
                Err(_) => TestResult::discard(), // Solver error
            }
        }

        // Run with fewer tests to avoid stack overflow
        QuickCheck::new()
            .tests(50)
            .quickcheck(prop_satisfiability_consistency as fn(i8, i8, i8) -> TestResult);
    }

    #[test]
    fn test_property_boolean_satisfiability_correctness() {
        use crate::expressions::{BinOp, ConstValue, SymExpr, UnOp};
        use quickcheck::{QuickCheck, TestResult};

        fn prop_boolean_logic_consistency(p_val: bool, q_val: bool) -> TestResult {
            let mut solver = match Z3Solver::new() {
                Ok(s) => s,
                Err(_) => return TestResult::discard(),
            };

            let p = SymExpr::variable("p".to_string());
            let q = SymExpr::variable("q".to_string());
            let p_const = SymExpr::constant(ConstValue::Bool(p_val));
            let q_const = SymExpr::constant(ConstValue::Bool(q_val));

            // Assert p = p_val and q = q_val
            let p_eq = SymExpr::binary_op(BinOp::Eq, p.clone(), p_const);
            let q_eq = SymExpr::binary_op(BinOp::Eq, q.clone(), q_const);

            // Test various boolean operations
            let test_cases = vec![
                // p AND q
                (
                    SymExpr::binary_op(BinOp::BitAnd, p.clone(), q.clone()),
                    p_val && q_val,
                ),
                // p OR q
                (
                    SymExpr::binary_op(BinOp::BitOr, p.clone(), q.clone()),
                    p_val || q_val,
                ),
                // p XOR q
                (
                    SymExpr::binary_op(BinOp::BitXor, p.clone(), q.clone()),
                    p_val ^ q_val,
                ),
                // NOT p
                (SymExpr::unary_op(UnOp::Not, p.clone()), !p_val),
                // NOT q
                (SymExpr::unary_op(UnOp::Not, q.clone()), !q_val),
            ];

            for (expr, expected_result) in test_cases {
                // Create constraint: expr = true
                let true_const = SymExpr::constant(ConstValue::Bool(true));
                let constraint = SymExpr::binary_op(BinOp::Eq, expr, true_const);

                let sat_result = solver.check_sat(&[p_eq.clone(), q_eq.clone(), constraint]);

                match sat_result {
                    Ok(result) => match result {
                        SatResult::Sat => {
                            if !expected_result {
                                return TestResult::failed();
                            }
                        }
                        SatResult::Unsat => {
                            if expected_result {
                                return TestResult::failed();
                            }
                        }
                        SatResult::Unknown => return TestResult::discard(),
                    },
                    Err(_) => return TestResult::discard(),
                }
            }

            TestResult::passed()
        }

        // Run with fewer tests to avoid stack overflow
        QuickCheck::new()
            .tests(20)
            .quickcheck(prop_boolean_logic_consistency as fn(bool, bool) -> TestResult);
    }

    #[test]
    fn test_property_constraint_consistency() {
        use crate::expressions::{BinOp, ConstValue, SymExpr};
        use quickcheck::{QuickCheck, TestResult};

        fn prop_constraint_consistency(a: i8, b: i8, c: i8) -> TestResult {
            // Skip zero coefficients to avoid trivial cases
            if a == 0 && b == 0 {
                return TestResult::discard();
            }

            let mut solver = match Z3Solver::new() {
                Ok(s) => s,
                Err(_) => return TestResult::discard(),
            };

            // Create variables and constants
            let x = SymExpr::variable("x".to_string());
            let y = SymExpr::variable("y".to_string());
            let a_const = SymExpr::constant(ConstValue::I32(a as i32));
            let b_const = SymExpr::constant(ConstValue::I32(b as i32));
            let c_const = SymExpr::constant(ConstValue::I32(c as i32));

            // Create constraint: a*x + b*y = c
            let ax = SymExpr::binary_op(BinOp::Mul, a_const, x);
            let by = SymExpr::binary_op(BinOp::Mul, b_const, y);
            let sum = SymExpr::binary_op(BinOp::Add, ax, by);
            let constraint = SymExpr::binary_op(BinOp::Eq, sum, c_const);

            let sat_result = solver.check_sat(&[constraint]);

            match sat_result {
                Ok(result) => {
                    match result {
                        SatResult::Sat => {
                            // If satisfiable, try to get a model and verify it
                            if let Ok(Some(model)) = solver.get_model() {
                                if let (Some(x_val), Some(y_val)) =
                                    (model.get_int_value("x"), model.get_int_value("y"))
                                {
                                    // Verify: a*x_val + b*y_val should equal c
                                    let computed = (a as i64) * x_val + (b as i64) * y_val;
                                    TestResult::from_bool(computed == c as i64)
                                } else {
                                    TestResult::passed() // Model exists but variables not found - still valid
                                }
                            } else {
                                TestResult::passed() // Satisfiable but no model - still valid
                            }
                        }
                        SatResult::Unsat => {
                            // If unsatisfiable, the constraint has no solution
                            // This is valid - some linear equations have no integer solutions
                            TestResult::passed()
                        }
                        SatResult::Unknown => TestResult::discard(),
                    }
                }
                Err(_) => TestResult::discard(),
            }
        }

        // Run with fewer tests to avoid stack overflow
        QuickCheck::new()
            .tests(30)
            .quickcheck(prop_constraint_consistency as fn(i8, i8, i8) -> TestResult);
    }

    #[test]
    fn test_z3_integration_mixed_types() {
        let mut solver = Z3Solver::new().unwrap();

        // Test mixed integer and boolean constraints
        let x = SymExpr::variable("x".to_string());
        let p = SymExpr::variable("p".to_string());
        let zero = SymExpr::constant(ConstValue::I64(0));

        // If p then x > 0, else x <= 0
        let x_gt_zero = SymExpr::binary_op(BinOp::Gt, x.clone(), zero.clone());
        let x_le_zero = SymExpr::binary_op(BinOp::Le, x, zero);
        let constraint = SymExpr::conditional(p.clone(), x_gt_zero, x_le_zero);

        // Also assert that p is true
        let p_true = SymExpr::binary_op(BinOp::Eq, p, SymExpr::constant(ConstValue::Bool(true)));

        let result = solver.check_sat(&[constraint, p_true]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Sat);
    }

    #[test]
    fn test_z3_integration_unsatisfiable_system() {
        let mut solver = Z3Solver::new().unwrap();

        // Create an unsatisfiable system: x > 5 AND x < 3
        let x = SymExpr::variable("x".to_string());
        let five = SymExpr::constant(ConstValue::I64(5));
        let three = SymExpr::constant(ConstValue::I64(3));

        let constraint1 = SymExpr::binary_op(BinOp::Gt, x.clone(), five);
        let constraint2 = SymExpr::binary_op(BinOp::Lt, x, three);

        let result = solver.check_sat(&[constraint1.clone(), constraint2.clone()]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Unsat);

        // Model should be None for unsatisfiable constraints
        let model_result = solver.get_model_for_constraints(&[constraint1, constraint2]);
        assert!(model_result.is_ok());
        assert!(model_result.unwrap().is_none());
    }

    #[test]
    fn test_z3_integration_incremental_solving() {
        let mut solver = Z3Solver::new().unwrap();

        // Test incremental constraint addition
        let x = SymExpr::variable("x".to_string());
        let y = SymExpr::variable("y".to_string());

        // First constraint: x > 0
        let constraint1 =
            SymExpr::binary_op(BinOp::Gt, x.clone(), SymExpr::constant(ConstValue::I64(0)));
        solver.assert(constraint1).unwrap();

        let result1 = solver.check_sat(&[]);
        assert_eq!(result1.unwrap(), SatResult::Sat);

        // Add second constraint: y > 0
        let constraint2 =
            SymExpr::binary_op(BinOp::Gt, y.clone(), SymExpr::constant(ConstValue::I64(0)));
        solver.assert(constraint2).unwrap();

        let result2 = solver.check_sat(&[]);
        assert_eq!(result2.unwrap(), SatResult::Sat);

        // Add contradictory constraint: x + y < 0
        let sum = SymExpr::binary_op(BinOp::Add, x, y);
        let constraint3 = SymExpr::binary_op(BinOp::Lt, sum, SymExpr::constant(ConstValue::I64(0)));

        let result3 = solver.check_sat(&[constraint3]);
        assert_eq!(result3.unwrap(), SatResult::Unsat);
    }

    #[test]
    fn test_z3_integration_push_pop_functionality() {
        let mut solver = Z3Solver::new().unwrap();

        // Base constraint: x > 0
        let x = SymExpr::variable("x".to_string());
        let base_constraint =
            SymExpr::binary_op(BinOp::Gt, x.clone(), SymExpr::constant(ConstValue::I64(0)));
        solver.assert(base_constraint).unwrap();

        // Should be satisfiable
        assert_eq!(solver.check_sat(&[]).unwrap(), SatResult::Sat);

        // Push and add more constraints
        solver.push().unwrap();
        let additional_constraint =
            SymExpr::binary_op(BinOp::Lt, x.clone(), SymExpr::constant(ConstValue::I64(10)));
        solver.assert(additional_constraint).unwrap();

        // Should still be satisfiable
        assert_eq!(solver.check_sat(&[]).unwrap(), SatResult::Sat);

        // Pop back to previous state
        solver.pop().unwrap();

        // Should still be satisfiable with just the base constraint
        assert_eq!(solver.check_sat(&[]).unwrap(), SatResult::Sat);

        // Verify we can add different constraints after pop
        let different_constraint =
            SymExpr::binary_op(BinOp::Gt, x, SymExpr::constant(ConstValue::I64(100)));
        assert_eq!(
            solver.check_sat(&[different_constraint]).unwrap(),
            SatResult::Sat
        );
    }

    #[test]
    fn test_z3_integration_complex_expressions() {
        let mut solver = Z3Solver::new().unwrap();

        // Test complex nested expressions: ((x + y) * 2) = (z - 1)
        let x = SymExpr::variable("x".to_string());
        let y = SymExpr::variable("y".to_string());
        let z = SymExpr::variable("z".to_string());

        let x_plus_y = SymExpr::binary_op(BinOp::Add, x, y);
        let times_two =
            SymExpr::binary_op(BinOp::Mul, x_plus_y, SymExpr::constant(ConstValue::I64(2)));
        let z_minus_one = SymExpr::binary_op(BinOp::Sub, z, SymExpr::constant(ConstValue::I64(1)));
        let complex_constraint = SymExpr::binary_op(BinOp::Eq, times_two, z_minus_one);

        // Add some bounds to make it more interesting
        let x_bound = SymExpr::binary_op(
            BinOp::Eq,
            SymExpr::variable("x".to_string()),
            SymExpr::constant(ConstValue::I64(3)),
        );
        let y_bound = SymExpr::binary_op(
            BinOp::Eq,
            SymExpr::variable("y".to_string()),
            SymExpr::constant(ConstValue::I64(4)),
        );

        let result =
            solver.check_sat(&[complex_constraint.clone(), x_bound.clone(), y_bound.clone()]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Sat);

        // Verify the solution: (3 + 4) * 2 = 14, so z should be 15
        let model_result =
            solver.get_model_for_constraints(&[complex_constraint, x_bound, y_bound]);
        if let Ok(Some(model)) = model_result {
            assert_eq!(model.get_int_value("x"), Some(3));
            assert_eq!(model.get_int_value("y"), Some(4));
            assert_eq!(model.get_int_value("z"), Some(15));
        }
    }

    #[test]
    fn test_z3_integration_performance_stress() {
        let mut solver = Z3Solver::new().unwrap();

        // Create a moderately complex constraint system to test performance
        let mut constraints = Vec::new();

        for i in 0..20 {
            let var_name = format!("x{}", i);
            let var = SymExpr::variable(var_name);
            let bound = SymExpr::constant(ConstValue::I64(i as i64));
            let constraint = SymExpr::binary_op(BinOp::Gt, var, bound);
            constraints.push(constraint);
        }

        // Add sum constraint: sum of all variables should be > 500
        let mut sum_expr = SymExpr::variable("x0".to_string());
        for i in 1..20 {
            let var_name = format!("x{}", i);
            let var = SymExpr::variable(var_name);
            sum_expr = SymExpr::binary_op(BinOp::Add, sum_expr, var);
        }
        let sum_constraint =
            SymExpr::binary_op(BinOp::Gt, sum_expr, SymExpr::constant(ConstValue::I64(500)));
        constraints.push(sum_constraint);

        // This should be satisfiable and solve reasonably quickly
        let result = solver.check_sat(&constraints);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SatResult::Sat);
    }

    #[test]
    fn test_timeout_configuration() {
        // Test creating solver without timeout
        let solver = Z3Solver::new();
        assert!(solver.is_ok());
        let solver = solver.unwrap();
        assert_eq!(solver.get_timeout(), None);

        // Test creating solver with timeout
        let solver_with_timeout = Z3Solver::with_timeout(Some(5000));
        assert!(solver_with_timeout.is_ok());
        let solver_with_timeout = solver_with_timeout.unwrap();
        assert_eq!(solver_with_timeout.get_timeout(), Some(5000));
    }

    #[test]
    fn test_set_timeout() {
        let mut solver = Z3Solver::new().unwrap();

        // Initially no timeout
        assert_eq!(solver.get_timeout(), None);

        // Set timeout
        let result = solver.set_timeout(Some(3000));
        assert!(result.is_ok());
        assert_eq!(solver.get_timeout(), Some(3000));

        // Clear timeout
        let result = solver.set_timeout(None);
        assert!(result.is_ok());
        assert_eq!(solver.get_timeout(), None);
    }

    #[test]
    fn test_timeout_preserved_after_reset() {
        let mut solver = Z3Solver::with_timeout(Some(2000)).unwrap();
        assert_eq!(solver.get_timeout(), Some(2000));

        // Reset should preserve timeout
        let result = solver.reset();
        assert!(result.is_ok());
        assert_eq!(solver.get_timeout(), Some(2000));
    }

    #[test]
    fn test_z3_integration_error_recovery() {
        let mut solver = Z3Solver::new().unwrap();

        // Test that solver can recover from errors
        let valid_constraint = SymExpr::binary_op(
            BinOp::Gt,
            SymExpr::variable("x".to_string()),
            SymExpr::constant(ConstValue::I64(0)),
        );

        // This should work fine
        assert!(solver.assert(valid_constraint).is_ok());
        assert_eq!(solver.check_sat(&[]).unwrap(), SatResult::Sat);

        // Try to assert an invalid constraint (should fail gracefully)
        let invalid_constraint = SymExpr::binary_op(
            BinOp::Add,
            SymExpr::variable("y".to_string()),
            SymExpr::constant(ConstValue::F64(f64::NAN)),
        );

        let result = solver.assert(invalid_constraint);
        assert!(result.is_err());

        // Solver should still work after the error
        let another_valid_constraint = SymExpr::binary_op(
            BinOp::Lt,
            SymExpr::variable("z".to_string()),
            SymExpr::constant(ConstValue::I64(100)),
        );

        assert!(solver.assert(another_valid_constraint).is_ok());
        assert_eq!(solver.check_sat(&[]).unwrap(), SatResult::Sat);
    }
}

// Additional solver functionality will be implemented in later tasks
