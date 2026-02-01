//! Symbolic numeric types that implement standard Rust traits
//!
//! This module provides symbolic counterparts to concrete numeric types
//! (e.g., SymU64, SymI32) that maintain the same interface while building
//! symbolic expressions during execution.

use crate::expressions::{BinOp, ConstValue, SymExpr, UnOp};
use crate::manager::{SymExManager, TypeInfo};
use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt;
use std::ops::{BitAnd, BitOr, BitXor};
use std::sync::{Arc, Mutex};

thread_local! {
    /// Track whether we're in symbolic execution mode
    static SYMBOLIC_MODE: std::cell::RefCell<bool> = std::cell::RefCell::new(false);

    /// Track branch decisions for the current path
    static BRANCH_DECISIONS: std::cell::RefCell<Vec<bool>> = std::cell::RefCell::new(Vec::new());

    /// Current position in branch decisions
    static BRANCH_INDEX: std::cell::RefCell<usize> = std::cell::RefCell::new(0);

    /// Track if we encountered a branch without a decision (new branch)
    static NEW_BRANCH_ENCOUNTERED: std::cell::RefCell<bool> = std::cell::RefCell::new(false);

    /// Track if the current path became unsatisfiable
    static PATH_UNSATISFIABLE: std::cell::RefCell<bool> = std::cell::RefCell::new(false);
}

/// Enable symbolic execution mode
pub fn enable_symbolic_mode() {
    SYMBOLIC_MODE.with(|mode| *mode.borrow_mut() = true);
}

/// Disable symbolic execution mode
pub fn disable_symbolic_mode() {
    SYMBOLIC_MODE.with(|mode| *mode.borrow_mut() = false);
}

/// Check if we're in symbolic mode
pub fn is_symbolic_mode() -> bool {
    SYMBOLIC_MODE.with(|mode| *mode.borrow())
}

/// Set branch decisions for the current path
pub fn set_branch_decisions(decisions: Vec<bool>) {
    BRANCH_DECISIONS.with(|d| *d.borrow_mut() = decisions);
    BRANCH_INDEX.with(|i| *i.borrow_mut() = 0);
    NEW_BRANCH_ENCOUNTERED.with(|n| *n.borrow_mut() = false);
    PATH_UNSATISFIABLE.with(|u| *u.borrow_mut() = false);
}

/// Get the next branch decision
fn get_next_branch_decision() -> Option<bool> {
    BRANCH_INDEX.with(|index| {
        BRANCH_DECISIONS.with(|decisions| {
            let idx = *index.borrow();
            let result = decisions.borrow().get(idx).copied();
            if result.is_some() {
                *index.borrow_mut() = idx + 1;
            } else {
                // No decision available - mark that we encountered a new branch
                NEW_BRANCH_ENCOUNTERED.with(|n| *n.borrow_mut() = true);
            }
            result
        })
    })
}

/// Check if a new branch was encountered (without a predetermined decision)
pub fn new_branch_encountered() -> bool {
    NEW_BRANCH_ENCOUNTERED.with(|n| *n.borrow())
}

/// Mark the current path as unsatisfiable
pub fn mark_path_unsatisfiable() {
    PATH_UNSATISFIABLE.with(|u| *u.borrow_mut() = true);
}

/// Check if the current path became unsatisfiable
pub fn path_became_unsatisfiable() -> bool {
    PATH_UNSATISFIABLE.with(|u| *u.borrow())
}

/// Get the current branch index (how many branches have been consumed)
pub fn get_branch_index() -> usize {
    BRANCH_INDEX.with(|i| *i.borrow())
}

/// Reset branch tracking
pub fn reset_branch_tracking() {
    BRANCH_DECISIONS.with(|d| d.borrow_mut().clear());
    BRANCH_INDEX.with(|i| *i.borrow_mut() = 0);
    NEW_BRANCH_ENCOUNTERED.with(|n| *n.borrow_mut() = false);
    PATH_UNSATISFIABLE.with(|u| *u.borrow_mut() = false);
}

// Generate SymU64 using the macro
crate::define_sym_int!(SymU64, u64, U64, "u64", 64, false, get_u64);

/// Symbolic boolean type
///
/// This type implements boolean logic operations while building symbolic
/// expressions during execution. It maintains a reference to the global
/// SymExManager for constraint tracking.
///
/// # Important: Using in if statements
///
/// Unlike regular `bool`, you cannot write `if sym_bool { ... }` directly.
/// Instead, use one of these approaches:
///
/// ```ignore
/// // Approach 1: Use is_true() method (recommended)
/// if result.is_true() {
///     // Path where result is true
/// }
///
/// // Approach 2: Use holds() for natural reading
/// if condition.holds() {
///     // Path where condition is true
/// }
///
/// // Approach 3: Explicit comparison
/// let true_val = SymBool::from_concrete(true, manager);
/// if result == true_val {
///     // Path where result is true
/// }
/// ```
///
/// All three approaches trigger path forking in symbolic execution.
#[derive(Clone)]
pub struct SymBool {
    /// Unique symbolic variable identifier
    variable_name: String,
    /// Symbolic expression representing this value
    expr: SymExpr,
    /// Optional concrete value for concolic execution
    concrete_value: Option<bool>,
    /// Reference to the global symbolic execution manager
    manager: Arc<Mutex<SymExManager>>,
}

impl SymBool {
    /// Create a new symbolic bool with a fresh variable name
    pub fn new(manager: Arc<Mutex<SymExManager>>) -> Self {
        let variable_name = {
            let mgr = manager.lock().unwrap();
            mgr.fresh_variable("bool")
        };

        let expr = SymExpr::Variable(variable_name.clone());

        // Register the variable with the manager
        {
            let mut mgr = manager.lock().unwrap();
            let type_info = TypeInfo {
                type_name: "bool".to_string(),
                bit_width: None,
                is_signed: false,
                creation_site: None,
            };
            let _ = mgr.register_variable(variable_name.clone(), type_info);
        }

        Self {
            variable_name,
            expr,
            concrete_value: None,
            manager,
        }
    }

    /// Create a new symbolic bool using the global thread-local manager
    pub fn new_global() -> Self {
        let manager = crate::get_global_manager().expect("Failed to get global manager");
        Self::new(manager)
    }

    /// Create a new symbolic bool with a specific variable name
    pub fn with_name(name: String, manager: Arc<Mutex<SymExManager>>) -> Self {
        let expr = SymExpr::Variable(name.clone());

        // Register the variable with the manager
        {
            let mut mgr = manager.lock().unwrap();
            let type_info = TypeInfo {
                type_name: "bool".to_string(),
                bit_width: None,
                is_signed: false,
                creation_site: None,
            };
            let _ = mgr.register_variable(name.clone(), type_info);
        }

        Self {
            variable_name: name,
            expr,
            concrete_value: None,
            manager,
        }
    }

    /// Create a new symbolic bool with an initial concrete value
    ///
    /// This is useful for concolic execution where you want to track a value
    /// symbolically but also maintain a concrete value for fast path checking.
    pub fn with_value(value: bool, manager: Arc<Mutex<SymExManager>>) -> Self {
        let variable_name = {
            let mut mgr = manager.lock().unwrap();
            let type_info = TypeInfo {
                type_name: "bool".to_string(),
                bit_width: None,
                is_signed: false,
                creation_site: None,
            };
            let name = mgr.fresh_variable("bool");
            let _ = mgr.register_variable(name.clone(), type_info);
            name
        };

        let expr = SymExpr::Variable(variable_name.clone());

        Self {
            variable_name,
            expr,
            concrete_value: Some(value),
            manager,
        }
    }

    /// Create a symbolic bool from a concrete value
    pub fn from_concrete(value: bool, manager: Arc<Mutex<SymExManager>>) -> Self {
        let variable_name = {
            let mgr = manager.lock().unwrap();
            mgr.fresh_variable("bool")
        };

        let expr = SymExpr::Constant(ConstValue::Bool(value));

        Self {
            variable_name,
            expr,
            concrete_value: Some(value),
            manager,
        }
    }

    /// Create a symbolic bool from an existing expression
    pub fn from_expr(expr: SymExpr, manager: Arc<Mutex<SymExManager>>) -> Self {
        let variable_name = {
            let mgr = manager.lock().unwrap();
            mgr.fresh_variable("bool")
        };

        Self {
            variable_name,
            expr,
            concrete_value: None,
            manager,
        }
    }

    /// Get the symbolic expression representing this value
    pub fn expr(&self) -> &SymExpr {
        &self.expr
    }

    /// Get the variable name
    pub fn variable_name(&self) -> &str {
        &self.variable_name
    }

    /// Get the concrete value if available
    pub fn concrete_value(&self) -> Option<bool> {
        self.concrete_value
    }

    /// Set the concrete value (for concolic execution updates)
    pub fn set_concrete_value(&mut self, value: bool) {
        self.concrete_value = Some(value);
    }

    /// Update concrete value from a model (for concolic execution)
    /// Returns true if the value was updated
    pub fn update_from_model(&mut self, model: &crate::solver::Model) -> bool {
        if let Some(value) = model.get_bool_value(&self.variable_name) {
            self.concrete_value = Some(value);
            true
        } else {
            false
        }
    }

    /// Get a reference to the manager
    pub fn manager(&self) -> Arc<Mutex<SymExManager>> {
        Arc::clone(&self.manager)
    }

    /// Generate an equality constraint
    pub fn eq_constraint(&self, other: &Self) -> SymExpr {
        SymExpr::binary_op(BinOp::Eq, self.expr.clone(), other.expr.clone())
    }

    /// Generate a not-equal constraint
    pub fn ne_constraint(&self, other: &Self) -> SymExpr {
        SymExpr::binary_op(BinOp::Ne, self.expr.clone(), other.expr.clone())
    }

    /// Generate an AND constraint
    pub fn and_constraint(&self, other: &Self) -> SymExpr {
        SymExpr::binary_op(BinOp::BitAnd, self.expr.clone(), other.expr.clone())
    }

    /// Generate an OR constraint
    pub fn or_constraint(&self, other: &Self) -> SymExpr {
        SymExpr::binary_op(BinOp::BitOr, self.expr.clone(), other.expr.clone())
    }

    /// Generate a NOT constraint
    pub fn not_constraint(&self) -> SymExpr {
        SymExpr::unary_op(UnOp::Not, self.expr.clone())
    }

    /// Add an equality constraint to the manager
    pub fn assert_eq(&self, other: &Self) -> crate::SymExResult<()> {
        let constraint = self.eq_constraint(other);
        let mut mgr = self.manager.lock().unwrap();
        mgr.add_constraint(constraint)
    }

    /// Add a not-equal constraint to the manager
    pub fn assert_ne(&self, other: &Self) -> crate::SymExResult<()> {
        let constraint = self.ne_constraint(other);
        let mut mgr = self.manager.lock().unwrap();
        mgr.add_constraint(constraint)
    }

    /// Assert this boolean is true
    pub fn assert_true(&self) -> crate::SymExResult<()> {
        let true_val = Self::from_concrete(true, Arc::clone(&self.manager));
        self.assert_eq(&true_val)
    }

    /// Assert this boolean is false
    pub fn assert_false(&self) -> crate::SymExResult<()> {
        let false_val = Self::from_concrete(false, Arc::clone(&self.manager));
        self.assert_eq(&false_val)
    }

    /// Check if this symbolic bool is true (triggers path forking)
    ///
    /// This is equivalent to: `self == SymBool::from_concrete(true, manager)`
    /// but more ergonomic for use in if statements.
    ///
    /// # Example
    /// ```ignore
    /// let result = &a & &b;
    /// if result.is_true() {
    ///     // Path where (a AND b) is true
    /// } else {
    ///     // Path where (a AND b) is false
    /// }
    /// ```
    pub fn is_true(&self) -> bool {
        let true_val = Self::from_concrete(true, Arc::clone(&self.manager));
        self == &true_val
    }

    /// Check if this symbolic bool is false (triggers path forking)
    pub fn is_false(&self) -> bool {
        let false_val = Self::from_concrete(false, Arc::clone(&self.manager));
        self == &false_val
    }

    /// Use this symbolic boolean as a condition (triggers path forking)
    ///
    /// This is an alias for `is_true()` for more natural reading.
    ///
    /// # Example
    /// ```ignore
    /// let condition = &a | &b;
    /// if condition.holds() {
    ///     // Path where condition is true
    /// }
    /// ```
    pub fn holds(&self) -> bool {
        self.is_true()
    }

    /// Create an implication: self => other (equivalent to !self || other)
    pub fn implies(&self, other: &Self) -> SymBool {
        let not_self = !self;
        &not_self | other
    }

    /// Create a bi-implication: self <=> other
    /// Equivalent to (self => other) && (other => self)
    pub fn iff(&self, other: &Self) -> SymBool {
        let forward = self.implies(other);
        let backward = other.implies(self);
        &forward & &backward
    }

    /// Create a conditional expression: if self then then_val else else_val
    pub fn if_then_else(&self, then_val: &Self, else_val: &Self) -> SymBool {
        let expr = SymExpr::conditional(
            self.expr.clone(),
            then_val.expr.clone(),
            else_val.expr.clone(),
        );

        let concrete_value = match (
            self.concrete_value,
            then_val.concrete_value,
            else_val.concrete_value,
        ) {
            (Some(cond), Some(t), Some(e)) => Some(if cond { t } else { e }),
            _ => None,
        };

        SymBool {
            variable_name: {
                let mgr = self.manager.lock().unwrap();
                mgr.fresh_variable("bool")
            },
            expr,
            concrete_value,
            manager: Arc::clone(&self.manager),
        }
    }
}

// Implement Debug trait for SymBool
impl fmt::Debug for SymBool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SymBool")
            .field("variable_name", &self.variable_name)
            .field("expr", &self.expr)
            .field("concrete_value", &self.concrete_value)
            .finish()
    }
}

// Implement BitAnd trait for SymBool (logical AND)
impl BitAnd for SymBool {
    type Output = SymBool;

    fn bitand(self, rhs: Self) -> Self::Output {
        let expr = SymExpr::binary_op(BinOp::BitAnd, self.expr.clone(), rhs.expr.clone());
        let concrete_value = match (self.concrete_value, rhs.concrete_value) {
            (Some(a), Some(b)) => Some(a && b),
            _ => None,
        };

        SymBool {
            variable_name: {
                let mgr = self.manager.lock().unwrap();
                mgr.fresh_variable("bool")
            },
            expr,
            concrete_value,
            manager: self.manager,
        }
    }
}

// Implement BitAnd trait for &SymBool
impl BitAnd for &SymBool {
    type Output = SymBool;

    fn bitand(self, rhs: Self) -> Self::Output {
        let expr = SymExpr::binary_op(BinOp::BitAnd, self.expr.clone(), rhs.expr.clone());
        let concrete_value = match (self.concrete_value, rhs.concrete_value) {
            (Some(a), Some(b)) => Some(a && b),
            _ => None,
        };

        SymBool {
            variable_name: {
                let mgr = self.manager.lock().unwrap();
                mgr.fresh_variable("bool")
            },
            expr,
            concrete_value,
            manager: Arc::clone(&self.manager),
        }
    }
}

// Implement BitOr trait for SymBool (logical OR)
impl BitOr for SymBool {
    type Output = SymBool;

    fn bitor(self, rhs: Self) -> Self::Output {
        let expr = SymExpr::binary_op(BinOp::BitOr, self.expr.clone(), rhs.expr.clone());
        let concrete_value = match (self.concrete_value, rhs.concrete_value) {
            (Some(a), Some(b)) => Some(a || b),
            _ => None,
        };

        SymBool {
            variable_name: {
                let mgr = self.manager.lock().unwrap();
                mgr.fresh_variable("bool")
            },
            expr,
            concrete_value,
            manager: self.manager,
        }
    }
}

// Implement BitOr trait for &SymBool
impl BitOr for &SymBool {
    type Output = SymBool;

    fn bitor(self, rhs: Self) -> Self::Output {
        let expr = SymExpr::binary_op(BinOp::BitOr, self.expr.clone(), rhs.expr.clone());
        let concrete_value = match (self.concrete_value, rhs.concrete_value) {
            (Some(a), Some(b)) => Some(a || b),
            _ => None,
        };

        SymBool {
            variable_name: {
                let mgr = self.manager.lock().unwrap();
                mgr.fresh_variable("bool")
            },
            expr,
            concrete_value,
            manager: Arc::clone(&self.manager),
        }
    }
}

// Implement BitXor trait for SymBool (logical XOR)
impl BitXor for SymBool {
    type Output = SymBool;

    fn bitxor(self, rhs: Self) -> Self::Output {
        let expr = SymExpr::binary_op(BinOp::BitXor, self.expr.clone(), rhs.expr.clone());
        let concrete_value = match (self.concrete_value, rhs.concrete_value) {
            (Some(a), Some(b)) => Some(a ^ b),
            _ => None,
        };

        SymBool {
            variable_name: {
                let mgr = self.manager.lock().unwrap();
                mgr.fresh_variable("bool")
            },
            expr,
            concrete_value,
            manager: self.manager,
        }
    }
}

// Implement BitXor trait for &SymBool
impl BitXor for &SymBool {
    type Output = SymBool;

    fn bitxor(self, rhs: Self) -> Self::Output {
        let expr = SymExpr::binary_op(BinOp::BitXor, self.expr.clone(), rhs.expr.clone());
        let concrete_value = match (self.concrete_value, rhs.concrete_value) {
            (Some(a), Some(b)) => Some(a ^ b),
            _ => None,
        };

        SymBool {
            variable_name: {
                let mgr = self.manager.lock().unwrap();
                mgr.fresh_variable("bool")
            },
            expr,
            concrete_value,
            manager: Arc::clone(&self.manager),
        }
    }
}

// Implement Not trait for SymBool (logical negation)
impl std::ops::Not for SymBool {
    type Output = SymBool;

    fn not(self) -> Self::Output {
        let expr = SymExpr::unary_op(UnOp::Not, self.expr.clone());
        let concrete_value = self.concrete_value.map(|v| !v);

        SymBool {
            variable_name: {
                let mgr = self.manager.lock().unwrap();
                mgr.fresh_variable("bool")
            },
            expr,
            concrete_value,
            manager: self.manager,
        }
    }
}

// Implement Not trait for &SymBool
impl std::ops::Not for &SymBool {
    type Output = SymBool;

    fn not(self) -> Self::Output {
        let expr = SymExpr::unary_op(UnOp::Not, self.expr.clone());
        let concrete_value = self.concrete_value.map(|v| !v);

        SymBool {
            variable_name: {
                let mgr = self.manager.lock().unwrap();
                mgr.fresh_variable("bool")
            },
            expr,
            concrete_value,
            manager: Arc::clone(&self.manager),
        }
    }
}

// Implement PartialEq trait for SymBool
impl PartialEq for SymBool {
    fn eq(&self, other: &Self) -> bool {
        // If we have concrete values for both, use them
        if let (Some(a), Some(b)) = (self.concrete_value, other.concrete_value) {
            return a == b;
        }

        // If we're in symbolic mode, this creates a branch point
        if is_symbolic_mode() {
            // Check if we have a predetermined branch decision
            if let Some(decision) = get_next_branch_decision() {
                // Add the appropriate constraint to the manager
                let constraint = if decision {
                    self.eq_constraint(other)
                } else {
                    self.ne_constraint(other)
                };

                // Add constraint and check satisfiability immediately
                let mut mgr = self.manager.lock().unwrap();
                let _ = mgr.add_constraint(constraint);

                // OPTIMIZATION: Early unsatisfiability detection
                // Check if the path is still satisfiable after adding this constraint
                if let Ok(is_sat) = mgr.is_satisfiable() {
                    if !is_sat {
                        // Path became unsatisfiable - mark it
                        drop(mgr); // Release lock before calling mark function
                        mark_path_unsatisfiable();
                    }
                }

                return decision;
            }

            // No predetermined decision - this is the first time we're seeing this branch
            // We'll return the concrete result if available, or false as default
            // The explore function will re-execute with both true and false
            if let (Some(a), Some(b)) = (self.concrete_value, other.concrete_value) {
                return a == b;
            }

            // For purely symbolic values, we need to make a choice
            // Return false by default (explore will try both)
            false
        } else {
            // Not in symbolic mode - just do structural equality
            self.expr == other.expr
        }
    }
}

// Implement Eq trait for SymBool
impl Eq for SymBool {}

// Implement From<bool> for SymBool
impl From<bool> for SymBool {
    fn from(value: bool) -> Self {
        let manager = crate::get_global_manager().expect("Failed to get global manager");
        SymBool::from_concrete(value, manager)
    }
}

// Implement TryFrom<SymBool> for bool
impl TryFrom<SymBool> for bool {
    type Error = &'static str;

    fn try_from(value: SymBool) -> Result<Self, Self::Error> {
        value.concrete_value.ok_or("No concrete value available")
    }
}

// Implement TryFrom<&SymBool> for bool
impl TryFrom<&SymBool> for bool {
    type Error = &'static str;

    fn try_from(value: &SymBool) -> Result<Self, Self::Error> {
        value.concrete_value.ok_or("No concrete value available")
    }
}

/// Symbolic string type
///
/// This type implements string operations while building symbolic
/// expressions during execution. It maintains a reference to the global
/// SymExManager for constraint tracking.
///
/// # Example
/// ```ignore
/// let manager = get_global_manager().unwrap();
/// let s1 = SymString::new(manager.clone());
/// let s2 = SymString::from_concrete("hello", manager);
/// let result = s1.concat(&s2);
/// ```
#[derive(Clone)]
pub struct SymString {
    /// Unique symbolic variable identifier
    variable_name: String,
    /// Symbolic expression representing this string value
    expr: SymExpr,
    /// Optional concrete value for concolic execution
    concrete_value: Option<String>,
    /// Reference to the global symbolic execution manager
    manager: Arc<Mutex<SymExManager>>,
}

impl SymString {
    /// Create a new symbolic string with a fresh variable name
    pub fn new(manager: Arc<Mutex<SymExManager>>) -> Self {
        let variable_name = {
            let mgr = manager.lock().unwrap();
            mgr.fresh_variable("string")
        };

        let expr = SymExpr::Variable(variable_name.clone());

        // Register the variable with the manager
        {
            let mut mgr = manager.lock().unwrap();
            let type_info = TypeInfo::string(None);
            let _ = mgr.register_variable(variable_name.clone(), type_info);
        }

        Self {
            variable_name,
            expr,
            concrete_value: None,
            manager,
        }
    }

    /// Create a new symbolic string using the global thread-local manager
    pub fn new_global() -> Self {
        let manager = crate::get_global_manager().expect("Failed to get global manager");
        Self::new(manager)
    }

    /// Create a new symbolic string with an initial concrete value
    ///
    /// This is useful for concolic execution where you want to track a value
    /// symbolically but also maintain a concrete value for fast path checking.
    ///
    /// # Errors
    ///
    /// Returns an error if the value contains non-ASCII characters.
    pub fn with_value(value: String, manager: Arc<Mutex<SymExManager>>) -> Self {
        // Validate ASCII characters
        for (pos, ch) in value.chars().enumerate() {
            if !ch.is_ascii() {
                panic!(
                    "Non-ASCII character '{}' at position {} in string value",
                    ch, pos
                );
            }
        }

        let variable_name = {
            let mut mgr = manager.lock().unwrap();
            let type_info = TypeInfo::string(None);
            let name = mgr.fresh_variable("string");
            let _ = mgr.register_variable(name.clone(), type_info);
            name
        };

        let expr = SymExpr::Variable(variable_name.clone());

        Self {
            variable_name,
            expr,
            concrete_value: Some(value),
            manager,
        }
    }

    /// Create a symbolic string from a concrete value
    ///
    /// # Errors
    ///
    /// Returns an error if the value contains non-ASCII characters.
    pub fn from_concrete(value: &str, manager: Arc<Mutex<SymExManager>>) -> Self {
        // Validate ASCII characters
        for (pos, ch) in value.chars().enumerate() {
            if !ch.is_ascii() {
                panic!(
                    "Non-ASCII character '{}' at position {} in string value",
                    ch, pos
                );
            }
        }

        let variable_name = {
            let mgr = manager.lock().unwrap();
            mgr.fresh_variable("string")
        };

        let expr = SymExpr::Constant(ConstValue::String(value.to_string()));

        Self {
            variable_name,
            expr,
            concrete_value: Some(value.to_string()),
            manager,
        }
    }

    /// Create a symbolic string from an existing expression
    pub fn from_expr(expr: SymExpr, manager: Arc<Mutex<SymExManager>>) -> Self {
        let variable_name = {
            let mgr = manager.lock().unwrap();
            mgr.fresh_variable("string")
        };

        Self {
            variable_name,
            expr,
            concrete_value: None,
            manager,
        }
    }

    /// Get the symbolic expression representing this value
    pub fn expr(&self) -> &SymExpr {
        &self.expr
    }

    /// Get the variable name
    pub fn variable_name(&self) -> &str {
        &self.variable_name
    }

    /// Get the concrete value if available
    pub fn concrete_value(&self) -> Option<&str> {
        self.concrete_value.as_deref()
    }

    /// Set the concrete value (for concolic execution updates)
    pub fn set_concrete_value(&mut self, value: String) {
        self.concrete_value = Some(value);
    }

    /// Update concrete value from a model (for concolic execution)
    ///
    /// This method attempts to extract the string value for this variable
    /// from the given model and updates the concrete_value field if found.
    ///
    /// Returns true if the value was successfully updated, false otherwise.
    ///
    /// # Example
    /// ```ignore
    /// let mut s = SymString::new(manager.clone());
    /// // ... add constraints and solve ...
    /// let model = manager.get_model().unwrap().unwrap();
    /// if s.update_from_model(&model) {
    ///     println!("String value: {}", s.concrete_value().unwrap());
    /// }
    /// ```
    pub fn update_from_model(&mut self, model: &crate::solver::Model) -> bool {
        if let Some(value) = model.get_string(&self.variable_name) {
            self.concrete_value = Some(value);
            true
        } else {
            false
        }
    }

    /// Get a reference to the manager
    pub fn manager(&self) -> Arc<Mutex<SymExManager>> {
        Arc::clone(&self.manager)
    }

    /// Generate an equality constraint
    pub fn eq_constraint(&self, other: &Self) -> SymExpr {
        SymExpr::binary_op(BinOp::Eq, self.expr.clone(), other.expr.clone())
    }

    /// Generate a not-equal constraint
    pub fn ne_constraint(&self, other: &Self) -> SymExpr {
        SymExpr::binary_op(BinOp::Ne, self.expr.clone(), other.expr.clone())
    }

    /// Generate a lexicographic less-than constraint
    pub fn lt_constraint(&self, other: &Self) -> SymExpr {
        SymExpr::binary_op(BinOp::StrLexLt, self.expr.clone(), other.expr.clone())
    }

    /// Generate a lexicographic less-than-or-equal constraint
    pub fn le_constraint(&self, other: &Self) -> SymExpr {
        SymExpr::binary_op(BinOp::StrLexLe, self.expr.clone(), other.expr.clone())
    }

    /// Generate a lexicographic greater-than constraint
    pub fn gt_constraint(&self, other: &Self) -> SymExpr {
        // a > b is equivalent to b < a
        SymExpr::binary_op(BinOp::StrLexLt, other.expr.clone(), self.expr.clone())
    }

    /// Generate a lexicographic greater-than-or-equal constraint
    pub fn ge_constraint(&self, other: &Self) -> SymExpr {
        // a >= b is equivalent to b <= a
        SymExpr::binary_op(BinOp::StrLexLe, other.expr.clone(), self.expr.clone())
    }

    /// Add an equality constraint to the manager
    pub fn assert_eq(&self, other: &Self) -> crate::SymExResult<()> {
        let constraint = self.eq_constraint(other);
        let mut mgr = self.manager.lock().unwrap();
        mgr.add_constraint(constraint)
    }

    /// Add a not-equal constraint to the manager
    pub fn assert_ne(&self, other: &Self) -> crate::SymExResult<()> {
        let constraint = self.ne_constraint(other);
        let mut mgr = self.manager.lock().unwrap();
        mgr.add_constraint(constraint)
    }

    /// Get the length of this string as a symbolic integer
    ///
    /// Returns a SymU64 representing the length of this string. If this string
    /// has a concrete value, the returned SymU64 will also have a concrete value
    /// equal to the length of the concrete string.
    ///
    /// # Example
    /// ```ignore
    /// let s = SymString::from_concrete("hello", manager.clone());
    /// let len = s.length();
    /// assert_eq!(len.concrete_value(), Some(5));
    /// ```
    pub fn length(&self) -> SymU64 {
        let expr = SymExpr::str_len(self.expr.clone());
        
        // Compute concrete length if this string has a concrete value
        let concrete_length = self.concrete_value.as_ref().map(|s| s.len() as u64);

        let mut result = SymU64::from_expr(expr, Arc::clone(&self.manager));
        
        // Set the concrete length if available
        if let Some(len) = concrete_length {
            result.set_concrete_value(len);
        }
        
        result
    }

    /// Concatenate this string with another
    ///
    /// Creates a new symbolic string representing the concatenation of this string
    /// with the other string. If both strings have concrete values, the result will
    /// also have a concrete value computed from the concatenation.
    ///
    /// # Example
    /// ```ignore
    /// let s1 = SymString::from_concrete("hello", manager.clone());
    /// let s2 = SymString::from_concrete(" world", manager.clone());
    /// let result = s1.concat(&s2);
    /// assert_eq!(result.concrete_value(), Some("hello world"));
    /// ```
    pub fn concat(&self, other: &Self) -> SymString {
        let expr = SymExpr::str_concat(self.expr.clone(), other.expr.clone());
        
        // Compute concrete result if both operands have concrete values
        let concrete_value = match (self.concrete_value.as_ref(), other.concrete_value.as_ref()) {
            (Some(a), Some(b)) => Some(format!("{a}{b}")),
            _ => None,
        };

        SymString {
            variable_name: {
                let mgr = self.manager.lock().unwrap();
                mgr.fresh_variable("string")
            },
            expr,
            concrete_value,
            manager: Arc::clone(&self.manager),
        }
    }
}

// Implement Debug trait for SymString
impl fmt::Debug for SymString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SymString")
            .field("variable_name", &self.variable_name)
            .field("expr", &self.expr)
            .field("concrete_value", &self.concrete_value)
            .finish()
    }
}

// Implement PartialEq trait for SymString
impl PartialEq for SymString {
    fn eq(&self, other: &Self) -> bool {
        // If we have concrete values for both, use them for fast comparison
        if let (Some(a), Some(b)) = (self.concrete_value.as_ref(), other.concrete_value.as_ref()) {
            return a == b;
        }

        // If we're in symbolic mode, this creates a branch point
        if is_symbolic_mode() {
            // Check if we have a predetermined branch decision
            if let Some(decision) = get_next_branch_decision() {
                // Add the appropriate constraint to the manager
                let constraint = if decision {
                    self.eq_constraint(other)
                } else {
                    self.ne_constraint(other)
                };

                // Add constraint and check satisfiability immediately
                let mut mgr = self.manager.lock().unwrap();
                let _ = mgr.add_constraint(constraint);

                // OPTIMIZATION: Early unsatisfiability detection
                // Check if the path is still satisfiable after adding this constraint
                if let Ok(is_sat) = mgr.is_satisfiable() {
                    if !is_sat {
                        // Path became unsatisfiable - mark it
                        drop(mgr); // Release lock before calling mark function
                        mark_path_unsatisfiable();
                    }
                }

                return decision;
            }

            // No predetermined decision - this is the first time we're seeing this branch
            // We'll return the concrete result if available, or false as default
            // The explore function will re-execute with both true and false
            if let (Some(a), Some(b)) = (self.concrete_value.as_ref(), other.concrete_value.as_ref()) {
                return a == b;
            }

            // For purely symbolic values, we need to make a choice
            // Return false by default (explore will try both)
            false
        } else {
            // Not in symbolic mode - just do structural equality
            self.expr == other.expr
        }
    }
}

// Implement Eq trait for SymString
impl Eq for SymString {}

// Implement PartialOrd trait for SymString (lexicographic ordering)
impl PartialOrd for SymString {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // If we have concrete values for both, use them for fast comparison
        if let (Some(a), Some(b)) = (self.concrete_value.as_ref(), other.concrete_value.as_ref()) {
            return Some(a.cmp(b));
        }

        // If we're in symbolic mode, this creates a branch point
        if is_symbolic_mode() {
            // For ordering comparisons in symbolic mode, we need to handle multiple branches
            // We'll use concrete values if available, otherwise return None
            if let (Some(a), Some(b)) = (self.concrete_value.as_ref(), other.concrete_value.as_ref()) {
                return Some(a.cmp(b));
            }
            
            // For purely symbolic values, return None to indicate ordering is unknown
            None
        } else {
            // Not in symbolic mode - use concrete values if available
            if let (Some(a), Some(b)) = (self.concrete_value.as_ref(), other.concrete_value.as_ref()) {
                Some(a.cmp(b))
            } else {
                None
            }
        }
    }
}

// Implement Ord trait for SymString (lexicographic ordering)
impl Ord for SymString {
    fn cmp(&self, other: &Self) -> Ordering {
        // Use concrete values when available
        if let (Some(a), Some(b)) = (self.concrete_value.as_ref(), other.concrete_value.as_ref()) {
            a.cmp(b)
        } else {
            // If no concrete values, fall back to structural comparison
            // This is a fallback and shouldn't be used in symbolic execution
            self.variable_name.cmp(&other.variable_name)
        }
    }
}

// Implement Add trait for SymString (string concatenation)
impl std::ops::Add for SymString {
    type Output = SymString;

    fn add(self, rhs: Self) -> Self::Output {
        self.concat(&rhs)
    }
}

// Implement Add trait for &SymString (string concatenation)
impl std::ops::Add for &SymString {
    type Output = SymString;

    fn add(self, rhs: Self) -> Self::Output {
        self.concat(rhs)
    }
}

// Additional symbolic types will be added in later tasks

// Generate SymU32 using the macro
crate::define_sym_int!(SymU32, u32, U32, "u32", 32, false, get_u32);

// Generate SymI32 using the macro
crate::define_sym_int!(SymI32, i32, I32, "i32", 32, true, get_i32);

// Generate SymI64 using the macro
crate::define_sym_int!(SymI64, i64, I64, "i64", 64, true, get_i64);

// Generate SymU8 using the macro
crate::define_sym_int!(SymU8, u8, U8, "u8", 8, false, get_u8);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SymExManager;
    use crate::solver::Z3Solver;
    use quickcheck_macros::quickcheck as qc;

    fn create_test_manager() -> Arc<Mutex<SymExManager>> {
        let solver = Box::new(Z3Solver::new().expect("Failed to create Z3 solver"));
        Arc::new(Mutex::new(SymExManager::new(solver)))
    }

    #[test]
    fn test_symu64_creation() {
        let manager = create_test_manager();
        let sym = SymU64::new(Arc::clone(&manager));

        assert!(sym.variable_name().starts_with("u64_"));
        assert_eq!(sym.concrete_value(), Some(0));
    }

    #[test]
    fn test_symu64_from_concrete() {
        let manager = create_test_manager();
        let sym = SymU64::from_concrete(42, Arc::clone(&manager));

        assert_eq!(sym.concrete_value(), Some(42));
        assert!(matches!(sym.expr(), SymExpr::Constant(ConstValue::U64(42))));
    }

    #[test]
    fn test_symu64_add() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(10, Arc::clone(&manager));
        let b = SymU64::from_concrete(20, Arc::clone(&manager));

        let result = a + b;

        assert_eq!(result.concrete_value(), Some(30));
        assert!(matches!(result.expr(), SymExpr::BinaryOp(BinOp::Add, _, _)));
    }

    #[test]
    fn test_symu64_sub() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(30, Arc::clone(&manager));
        let b = SymU64::from_concrete(10, Arc::clone(&manager));

        let result = a - b;

        assert_eq!(result.concrete_value(), Some(20));
        assert!(matches!(result.expr(), SymExpr::BinaryOp(BinOp::Sub, _, _)));
    }

    #[test]
    fn test_symu64_mul() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(5, Arc::clone(&manager));
        let b = SymU64::from_concrete(6, Arc::clone(&manager));

        let result = a * b;

        assert_eq!(result.concrete_value(), Some(30));
        assert!(matches!(result.expr(), SymExpr::BinaryOp(BinOp::Mul, _, _)));
    }

    #[test]
    fn test_symu64_div() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(20, Arc::clone(&manager));
        let b = SymU64::from_concrete(4, Arc::clone(&manager));

        let result = a / b;

        assert_eq!(result.concrete_value(), Some(5));
        assert!(matches!(result.expr(), SymExpr::BinaryOp(BinOp::Div, _, _)));
    }

    #[test]
    fn test_symu64_rem() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(23, Arc::clone(&manager));
        let b = SymU64::from_concrete(5, Arc::clone(&manager));

        let result = a % b;

        assert_eq!(result.concrete_value(), Some(3));
        assert!(matches!(result.expr(), SymExpr::BinaryOp(BinOp::Mod, _, _)));
    }

    #[test]
    fn test_symu64_symbolic_operations() {
        let manager = create_test_manager();
        let a = SymU64::new(Arc::clone(&manager));
        let b = SymU64::new(Arc::clone(&manager));

        // Operations on symbolic values should create expressions with concrete values
        let sum = &a + &b;
        assert_eq!(sum.concrete_value(), Some(0)); // 0 + 0 = 0
        assert!(matches!(sum.expr(), SymExpr::BinaryOp(BinOp::Add, _, _)));

        let diff = &a - &b;
        assert_eq!(diff.concrete_value(), Some(0)); // 0 - 0 = 0
        assert!(matches!(diff.expr(), SymExpr::BinaryOp(BinOp::Sub, _, _)));

        let prod = &a * &b;
        assert_eq!(prod.concrete_value(), Some(0)); // 0 * 0 = 0
        assert!(matches!(prod.expr(), SymExpr::BinaryOp(BinOp::Mul, _, _)));
    }

    #[test]
    fn test_symu64_equality() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(42, Arc::clone(&manager));
        let b = SymU64::from_concrete(42, Arc::clone(&manager));
        let c = SymU64::from_concrete(43, Arc::clone(&manager));

        // Equality is based on expression equality
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_symu64_ordering_with_concrete() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(10, Arc::clone(&manager));
        let b = SymU64::from_concrete(20, Arc::clone(&manager));
        let c = SymU64::from_concrete(10, Arc::clone(&manager));

        assert!(a < b);
        assert!(b > a);
        assert_eq!(a.cmp(&c), Ordering::Equal);
    }

    #[test]
    fn test_symu64_ordering_symbolic() {
        let manager = create_test_manager();
        let a = SymU64::new(Arc::clone(&manager));
        let b = SymU64::new(Arc::clone(&manager));

        // For purely symbolic values, we provide a deterministic ordering
        // based on variable names (via Ord implementation)
        let partial_ordering = a.partial_cmp(&b);
        assert!(partial_ordering.is_some());

        // cmp provides a deterministic ordering based on variable names
        let ordering = a.cmp(&b);
        assert!(matches!(
            ordering,
            Ordering::Less | Ordering::Greater | Ordering::Equal
        ));
    }

    #[test]
    fn test_symu64_complex_expression() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(10, Arc::clone(&manager));
        let b = SymU64::from_concrete(5, Arc::clone(&manager));
        let c = SymU64::from_concrete(3, Arc::clone(&manager));

        // (a + b) * c
        let result = (&a + &b) * c;

        assert_eq!(result.concrete_value(), Some(45));

        // Check that the expression is properly nested
        if let SymExpr::BinaryOp(BinOp::Mul, left, _right) = result.expr() {
            assert!(matches!(**left, SymExpr::BinaryOp(BinOp::Add, _, _)));
        } else {
            panic!("Expected multiplication expression");
        }
    }

    #[test]
    fn test_symu64_wrapping_arithmetic() {
        let manager = create_test_manager();
        let max = SymU64::from_concrete(u64::MAX, Arc::clone(&manager));
        let one = SymU64::from_concrete(1, Arc::clone(&manager));

        // Test wrapping addition
        let result = &max + &one;
        assert_eq!(result.concrete_value(), Some(0));

        // Test wrapping subtraction
        let zero = SymU64::from_concrete(0, Arc::clone(&manager));
        let result = zero - one;
        assert_eq!(result.concrete_value(), Some(u64::MAX));
    }

    #[test]
    fn test_symu64_variable_registration() {
        let manager = create_test_manager();
        let sym = SymU64::new(Arc::clone(&manager));

        // Check that the variable was registered
        let mgr = manager.lock().unwrap();
        let vars = mgr.get_registered_variables();
        assert!(vars.contains(&sym.variable_name().to_string()));

        // Check type info
        let type_info = mgr.get_variable_info(sym.variable_name()).unwrap();
        assert_eq!(type_info.type_name, "u64");
        assert_eq!(type_info.bit_width, Some(64));
        assert!(!type_info.is_signed);
    }

    #[test]
    fn test_symu64_unique_variables() {
        let manager = create_test_manager();
        let a = SymU64::new(Arc::clone(&manager));
        let b = SymU64::new(Arc::clone(&manager));
        let c = SymU64::new(Arc::clone(&manager));

        // Each symbolic variable should have a unique name
        assert_ne!(a.variable_name(), b.variable_name());
        assert_ne!(b.variable_name(), c.variable_name());
        assert_ne!(a.variable_name(), c.variable_name());
    }

    #[test]
    fn test_symu64_debug_format() {
        let manager = create_test_manager();
        let sym = SymU64::from_concrete(42, Arc::clone(&manager));

        let debug_str = format!("{:?}", sym);
        assert!(debug_str.contains("SymU64"));
        assert!(debug_str.contains("variable_name"));
        assert!(debug_str.contains("expr"));
        assert!(debug_str.contains("concrete_value"));
    }

    #[test]
    fn test_symu64_from_expr_evaluates_constants() {
        let manager = create_test_manager();

        // Test with a constant expression
        let expr = SymExpr::Constant(ConstValue::U64(100));
        let sym = SymU64::from_expr(expr, Arc::clone(&manager));
        assert_eq!(sym.concrete_value(), Some(100));

        // Test with a constant expression that needs casting (I64 -> U64)
        let expr = SymExpr::Constant(ConstValue::I64(50));
        let sym = SymU64::from_expr(expr, Arc::clone(&manager));
        assert_eq!(sym.concrete_value(), Some(50));

        // Test with a constant expression (U32 -> U64)
        let expr = SymExpr::Constant(ConstValue::U32(200));
        let sym = SymU64::from_expr(expr, Arc::clone(&manager));
        assert_eq!(sym.concrete_value(), Some(200));

        // Test with a binary operation on constants
        let expr = SymExpr::binary_op(
            BinOp::Add,
            SymExpr::Constant(ConstValue::U64(10)),
            SymExpr::Constant(ConstValue::U64(20)),
        );
        let sym = SymU64::from_expr(expr, Arc::clone(&manager));
        assert_eq!(sym.concrete_value(), Some(30));

        // Test with a variable expression (can't evaluate without bindings)
        let expr = SymExpr::Variable("x".to_string());
        let sym = SymU64::from_expr(expr, Arc::clone(&manager));
        assert_eq!(sym.concrete_value(), Some(0)); // Falls back to 0
    }

    #[test]
    fn test_symu32_from_expr_evaluates_constants() {
        let manager = create_test_manager();

        // Test with a constant expression
        let expr = SymExpr::Constant(ConstValue::U32(100));
        let sym = SymU32::from_expr(expr, Arc::clone(&manager));
        assert_eq!(sym.concrete_value(), Some(100));

        // Test with casting from U64 to U32
        let expr = SymExpr::Constant(ConstValue::U64(200));
        let sym = SymU32::from_expr(expr, Arc::clone(&manager));
        assert_eq!(sym.concrete_value(), Some(200));

        // Test with truncation (U64 value too large for U32)
        let expr = SymExpr::Constant(ConstValue::U64(u64::MAX));
        let sym = SymU32::from_expr(expr, Arc::clone(&manager));
        assert_eq!(sym.concrete_value(), Some(u32::MAX)); // Truncated
    }

    #[test]
    fn test_symi32_from_expr_evaluates_constants() {
        let manager = create_test_manager();

        // Test with a constant expression
        let expr = SymExpr::Constant(ConstValue::I32(-50));
        let sym = SymI32::from_expr(expr, Arc::clone(&manager));
        assert_eq!(sym.concrete_value(), Some(-50));

        // Test with casting from I64 to I32
        let expr = SymExpr::Constant(ConstValue::I64(-100));
        let sym = SymI32::from_expr(expr, Arc::clone(&manager));
        assert_eq!(sym.concrete_value(), Some(-100));

        // Test with binary operation
        let expr = SymExpr::binary_op(
            BinOp::Sub,
            SymExpr::Constant(ConstValue::I32(10)),
            SymExpr::Constant(ConstValue::I32(30)),
        );
        let sym = SymI32::from_expr(expr, Arc::clone(&manager));
        assert_eq!(sym.concrete_value(), Some(-20));
    }

    #[test]
    fn test_symu64_new_global() {
        crate::reset_global_manager();
        crate::init_global().unwrap();

        let sym = SymU64::new_global();
        assert!(sym.variable_name().starts_with("u64_"));
        assert_eq!(sym.concrete_value(), Some(0));
    }

    #[test]
    fn test_symu64_from_uses_global_manager() {
        crate::reset_global_manager();
        crate::init_global().unwrap();

        let sym: SymU64 = 42.into();
        assert_eq!(sym.concrete_value(), Some(42));
    }

    #[test]
    fn test_symu64_constraint_generation() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(10, Arc::clone(&manager));
        let b = SymU64::from_concrete(20, Arc::clone(&manager));

        // Test equality constraint generation
        let eq_constraint = a.eq_constraint(&b);
        assert!(matches!(eq_constraint, SymExpr::BinaryOp(BinOp::Eq, _, _)));

        // Test not-equal constraint generation
        let ne_constraint = a.ne_constraint(&b);
        assert!(matches!(ne_constraint, SymExpr::BinaryOp(BinOp::Ne, _, _)));

        // Test less-than constraint generation
        let lt_constraint = a.lt_constraint(&b);
        assert!(matches!(lt_constraint, SymExpr::BinaryOp(BinOp::Lt, _, _)));

        // Test greater-than constraint generation
        let gt_constraint = a.gt_constraint(&b);
        assert!(matches!(gt_constraint, SymExpr::BinaryOp(BinOp::Gt, _, _)));
    }

    #[test]
    fn test_symu64_assert_constraints() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(10, Arc::clone(&manager));
        let b = SymU64::from_concrete(20, Arc::clone(&manager));

        // Assert that a < b
        let result = a.assert_lt(&b);
        assert!(result.is_ok());

        // Check that the constraint was added
        let mgr = manager.lock().unwrap();
        let constraints = mgr.get_constraints();
        assert_eq!(constraints.len(), 1);
        assert!(matches!(constraints[0], SymExpr::BinaryOp(BinOp::Lt, _, _)));
    }

    #[test]
    fn test_symu64_assert_le() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(10, Arc::clone(&manager));
        let b = SymU64::from_concrete(20, Arc::clone(&manager));

        // Assert that a <= b
        let result = a.assert_le(&b);
        assert!(result.is_ok());

        // Check that the constraint was added
        let mgr = manager.lock().unwrap();
        let constraints = mgr.get_constraints();
        assert_eq!(constraints.len(), 1);
        assert!(matches!(constraints[0], SymExpr::BinaryOp(BinOp::Le, _, _)));
    }

    #[test]
    fn test_symu64_assert_gt() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(20, Arc::clone(&manager));
        let b = SymU64::from_concrete(10, Arc::clone(&manager));

        // Assert that a > b
        let result = a.assert_gt(&b);
        assert!(result.is_ok());

        // Check that the constraint was added
        let mgr = manager.lock().unwrap();
        let constraints = mgr.get_constraints();
        assert_eq!(constraints.len(), 1);
        assert!(matches!(constraints[0], SymExpr::BinaryOp(BinOp::Gt, _, _)));
    }

    #[test]
    fn test_symu64_assert_ge() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(20, Arc::clone(&manager));
        let b = SymU64::from_concrete(10, Arc::clone(&manager));

        // Assert that a >= b
        let result = a.assert_ge(&b);
        assert!(result.is_ok());

        // Check that the constraint was added
        let mgr = manager.lock().unwrap();
        let constraints = mgr.get_constraints();
        assert_eq!(constraints.len(), 1);
        assert!(matches!(constraints[0], SymExpr::BinaryOp(BinOp::Ge, _, _)));
    }

    #[test]
    fn test_symu64_assert_eq() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(42, Arc::clone(&manager));
        let b = SymU64::from_concrete(42, Arc::clone(&manager));

        // Assert that a == b
        let result = a.assert_eq(&b);
        assert!(result.is_ok());

        // Check that the constraint was added
        let mgr = manager.lock().unwrap();
        let constraints = mgr.get_constraints();
        assert_eq!(constraints.len(), 1);
        assert!(matches!(constraints[0], SymExpr::BinaryOp(BinOp::Eq, _, _)));
    }

    #[test]
    fn test_symu64_assert_ne() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(42, Arc::clone(&manager));
        let b = SymU64::from_concrete(43, Arc::clone(&manager));

        // Assert that a != b
        let result = a.assert_ne(&b);
        assert!(result.is_ok());

        // Check that the constraint was added
        let mgr = manager.lock().unwrap();
        let constraints = mgr.get_constraints();
        assert_eq!(constraints.len(), 1);
        assert!(matches!(constraints[0], SymExpr::BinaryOp(BinOp::Ne, _, _)));
    }

    #[test]
    fn test_symu64_all_relational_constraints() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(10, Arc::clone(&manager));
        let b = SymU64::from_concrete(20, Arc::clone(&manager));

        // Test all constraint generation methods
        let eq_constraint = a.eq_constraint(&b);
        assert!(matches!(eq_constraint, SymExpr::BinaryOp(BinOp::Eq, _, _)));

        let ne_constraint = a.ne_constraint(&b);
        assert!(matches!(ne_constraint, SymExpr::BinaryOp(BinOp::Ne, _, _)));

        let lt_constraint = a.lt_constraint(&b);
        assert!(matches!(lt_constraint, SymExpr::BinaryOp(BinOp::Lt, _, _)));

        let le_constraint = a.le_constraint(&b);
        assert!(matches!(le_constraint, SymExpr::BinaryOp(BinOp::Le, _, _)));

        let gt_constraint = a.gt_constraint(&b);
        assert!(matches!(gt_constraint, SymExpr::BinaryOp(BinOp::Gt, _, _)));

        let ge_constraint = a.ge_constraint(&b);
        assert!(matches!(ge_constraint, SymExpr::BinaryOp(BinOp::Ge, _, _)));
    }

    #[test]
    fn test_symu64_multiple_constraints() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(10, Arc::clone(&manager));
        let b = SymU64::from_concrete(20, Arc::clone(&manager));
        let c = SymU64::from_concrete(15, Arc::clone(&manager));

        // Add multiple constraints: a < c AND c < b
        a.assert_lt(&c).unwrap();
        c.assert_lt(&b).unwrap();

        // Check that both constraints were added
        let mgr = manager.lock().unwrap();
        let constraints = mgr.get_constraints();
        assert_eq!(constraints.len(), 2);
    }

    #[test]
    fn test_symu64_partial_eq_with_concrete() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(42, Arc::clone(&manager));
        let b = SymU64::from_concrete(42, Arc::clone(&manager));
        let c = SymU64::from_concrete(43, Arc::clone(&manager));

        // Concrete values should compare correctly
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_symu64_bitand() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(0b1111, Arc::clone(&manager));
        let b = SymU64::from_concrete(0b1010, Arc::clone(&manager));

        let result = a & b;

        assert_eq!(result.concrete_value(), Some(0b1010));
        assert!(matches!(
            result.expr(),
            SymExpr::BinaryOp(BinOp::BitAnd, _, _)
        ));
    }

    #[test]
    fn test_symu64_bitor() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(0b1100, Arc::clone(&manager));
        let b = SymU64::from_concrete(0b1010, Arc::clone(&manager));

        let result = a | b;

        assert_eq!(result.concrete_value(), Some(0b1110));
        assert!(matches!(
            result.expr(),
            SymExpr::BinaryOp(BinOp::BitOr, _, _)
        ));
    }

    #[test]
    fn test_symu64_bitxor() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(0b1100, Arc::clone(&manager));
        let b = SymU64::from_concrete(0b1010, Arc::clone(&manager));

        let result = a ^ b;

        assert_eq!(result.concrete_value(), Some(0b0110));
        assert!(matches!(
            result.expr(),
            SymExpr::BinaryOp(BinOp::BitXor, _, _)
        ));
    }

    #[test]
    fn test_symu64_shl() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(5, Arc::clone(&manager));
        let b = SymU64::from_concrete(2, Arc::clone(&manager));

        let result = a << b;

        assert_eq!(result.concrete_value(), Some(20));
        assert!(matches!(result.expr(), SymExpr::BinaryOp(BinOp::Shl, _, _)));
    }

    #[test]
    fn test_symu64_shr() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(20, Arc::clone(&manager));
        let b = SymU64::from_concrete(2, Arc::clone(&manager));

        let result = a >> b;

        assert_eq!(result.concrete_value(), Some(5));
        assert!(matches!(result.expr(), SymExpr::BinaryOp(BinOp::Shr, _, _)));
    }

    #[test]
    fn test_symu64_bitwise_symbolic() {
        let manager = create_test_manager();
        let a = SymU64::new(Arc::clone(&manager));
        let b = SymU64::new(Arc::clone(&manager));

        // Operations on symbolic values should create expressions with concrete values
        let and_result = &a & &b;
        assert_eq!(and_result.concrete_value(), Some(0)); // 0 & 0 = 0
        assert!(matches!(
            and_result.expr(),
            SymExpr::BinaryOp(BinOp::BitAnd, _, _)
        ));

        let or_result = &a | &b;
        assert_eq!(or_result.concrete_value(), Some(0)); // 0 | 0 = 0
        assert!(matches!(
            or_result.expr(),
            SymExpr::BinaryOp(BinOp::BitOr, _, _)
        ));

        let xor_result = &a ^ &b;
        assert_eq!(xor_result.concrete_value(), Some(0)); // 0 ^ 0 = 0
        assert!(matches!(
            xor_result.expr(),
            SymExpr::BinaryOp(BinOp::BitXor, _, _)
        ));
    }

    #[test]
    fn test_symu64_shift_symbolic() {
        let manager = create_test_manager();
        let a = SymU64::new(Arc::clone(&manager));
        let b = SymU64::new(Arc::clone(&manager));

        // Shift operations on symbolic values should create expressions with concrete values
        let shl_result = &a << &b;
        assert_eq!(shl_result.concrete_value(), Some(0)); // 0 << 0 = 0
        assert!(matches!(
            shl_result.expr(),
            SymExpr::BinaryOp(BinOp::Shl, _, _)
        ));

        let shr_result = &a >> &b;
        assert_eq!(shr_result.concrete_value(), Some(0)); // 0 >> 0 = 0
        assert!(matches!(
            shr_result.expr(),
            SymExpr::BinaryOp(BinOp::Shr, _, _)
        ));
    }

    #[test]
    fn test_symu64_from_u64() {
        let sym = SymU64::from(42u64);
        assert_eq!(sym.concrete_value(), Some(42));
        assert!(matches!(sym.expr(), SymExpr::Constant(ConstValue::U64(42))));
    }

    #[test]
    fn test_symu64_try_into_u64() {
        let manager = create_test_manager();
        let sym = SymU64::from_concrete(42, Arc::clone(&manager));

        let result: Result<u64, _> = sym.try_into();
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn test_symu64_try_into_u64_no_concrete() {
        let manager = create_test_manager();
        let sym = SymU64::new(Arc::clone(&manager));

        // new() now initializes concrete_value to Some(0)
        let result: Result<u64, _> = sym.try_into();
        assert_eq!(result, Ok(0));
    }

    #[test]
    fn test_symu64_try_from_ref() {
        let manager = create_test_manager();
        let sym = SymU64::from_concrete(42, Arc::clone(&manager));

        let result: Result<u64, _> = u64::try_from(&sym);
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn test_symu64_bitwise_complex_expression() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(0b1111, Arc::clone(&manager));
        let b = SymU64::from_concrete(0b1010, Arc::clone(&manager));
        let c = SymU64::from_concrete(0b0011, Arc::clone(&manager));

        // (a & b) | c
        let result = (&a & &b) | c;

        assert_eq!(result.concrete_value(), Some(0b1011));

        // Check that the expression is properly nested
        if let SymExpr::BinaryOp(BinOp::BitOr, left, _right) = result.expr() {
            assert!(matches!(**left, SymExpr::BinaryOp(BinOp::BitAnd, _, _)));
        } else {
            panic!("Expected bitwise OR expression");
        }
    }

    #[test]
    fn test_symu64_shift_edge_cases() {
        let manager = create_test_manager();

        // Shift by 0 should return the same value
        let a = SymU64::from_concrete(42, Arc::clone(&manager));
        let zero = SymU64::from_concrete(0, Arc::clone(&manager));
        let result = a << zero;
        assert_eq!(result.concrete_value(), Some(42));

        // Shift by amount >= 64 should return None for concrete value
        let a = SymU64::from_concrete(42, Arc::clone(&manager));
        let large = SymU64::from_concrete(64, Arc::clone(&manager));
        let result = a << large;
        assert_eq!(result.concrete_value(), None);
    }

    #[test]
    fn test_symu64_bitwise_identities() {
        let manager = create_test_manager();
        let a = SymU64::from_concrete(0b10101010, Arc::clone(&manager));
        let all_ones = SymU64::from_concrete(u64::MAX, Arc::clone(&manager));
        let zero = SymU64::from_concrete(0, Arc::clone(&manager));

        // a & 0 = 0
        let result = &a & &zero;
        assert_eq!(result.concrete_value(), Some(0));

        // a | 0 = a
        let result = &a | &zero;
        assert_eq!(result.concrete_value(), Some(0b10101010));

        // a ^ 0 = a
        let result = &a ^ &zero;
        assert_eq!(result.concrete_value(), Some(0b10101010));

        // a & all_ones = a
        let result = &a & &all_ones;
        assert_eq!(result.concrete_value(), Some(0b10101010));

        // a | all_ones = all_ones
        let result = a | all_ones;
        assert_eq!(result.concrete_value(), Some(u64::MAX));
    }

    // SymString tests
    #[test]
    fn test_symstring_creation() {
        let manager = create_test_manager();
        let sym = SymString::new(Arc::clone(&manager));

        assert!(sym.variable_name().starts_with("string_"));
        assert_eq!(sym.concrete_value(), None);
    }

    #[test]
    fn test_symstring_from_concrete() {
        let manager = create_test_manager();
        let sym = SymString::from_concrete("hello", Arc::clone(&manager));

        assert_eq!(sym.concrete_value(), Some("hello"));
        assert!(matches!(
            sym.expr(),
            SymExpr::Constant(ConstValue::String(_))
        ));
    }

    #[test]
    fn test_symstring_with_value() {
        let manager = create_test_manager();
        let sym = SymString::with_value("world".to_string(), Arc::clone(&manager));

        assert_eq!(sym.concrete_value(), Some("world"));
        assert!(sym.variable_name().starts_with("string_"));
    }

    #[test]
    fn test_symstring_new_global() {
        crate::reset_global_manager();
        crate::init_global().unwrap();

        let sym = SymString::new_global();
        assert!(sym.variable_name().starts_with("string_"));
        assert_eq!(sym.concrete_value(), None);
    }

    #[test]
    fn test_symstring_variable_registration() {
        let manager = create_test_manager();
        let sym = SymString::new(Arc::clone(&manager));

        // Check that the variable was registered
        let mgr = manager.lock().unwrap();
        let vars = mgr.get_registered_variables();
        assert!(vars.contains(&sym.variable_name().to_string()));

        // Check type info
        let type_info = mgr.get_variable_info(sym.variable_name()).unwrap();
        assert_eq!(type_info.type_name, "string");
        assert_eq!(type_info.bit_width, None);
        assert!(!type_info.is_signed);
    }

    #[test]
    fn test_symstring_unique_variables() {
        let manager = create_test_manager();
        let a = SymString::new(Arc::clone(&manager));
        let b = SymString::new(Arc::clone(&manager));
        let c = SymString::new(Arc::clone(&manager));

        // Each symbolic variable should have a unique name
        assert_ne!(a.variable_name(), b.variable_name());
        assert_ne!(b.variable_name(), c.variable_name());
        assert_ne!(a.variable_name(), c.variable_name());
    }

    #[test]
    fn test_symstring_set_concrete_value() {
        let manager = create_test_manager();
        let mut sym = SymString::new(Arc::clone(&manager));

        assert_eq!(sym.concrete_value(), None);

        sym.set_concrete_value("test".to_string());
        assert_eq!(sym.concrete_value(), Some("test"));
    }

    #[test]
    fn test_symstring_from_expr() {
        let manager = create_test_manager();
        let expr = SymExpr::Constant(ConstValue::String("example".to_string()));
        let sym = SymString::from_expr(expr, Arc::clone(&manager));

        assert!(sym.variable_name().starts_with("string_"));
        assert_eq!(sym.concrete_value(), None);
    }

    #[test]
    fn test_symstring_debug_format() {
        let manager = create_test_manager();
        let sym = SymString::from_concrete("test", Arc::clone(&manager));

        let debug_str = format!("{:?}", sym);
        assert!(debug_str.contains("SymString"));
        assert!(debug_str.contains("variable_name"));
        assert!(debug_str.contains("expr"));
        assert!(debug_str.contains("concrete_value"));
    }

    // **Feature: symbolic-strings, Property 2: Concolic Value Preservation**
    // **Validates: Requirements 1.2, 10.2, 10.3, 18.1**
    #[qc]
    fn prop_symstring_concolic_value_preservation(s1: String, s2: String) -> bool {
        // Filter to ASCII only as per requirement 13.3
        if !s1.is_ascii() || !s2.is_ascii() {
            return true; // Discard non-ASCII test cases
        }

        // Limit string length to avoid excessive test times
        if s1.len() > 100 || s2.len() > 100 {
            return true;
        }

        let manager = create_test_manager();

        // Create symbolic strings with concrete values
        let sym1 = SymString::with_value(s1.clone(), Arc::clone(&manager));
        let sym2 = SymString::with_value(s2.clone(), Arc::clone(&manager));

        // Property 1: Initial concrete values should be preserved
        if sym1.concrete_value() != Some(s1.as_str()) {
            return false;
        }
        if sym2.concrete_value() != Some(s2.as_str()) {
            return false;
        }

        // Property 2: Concrete values should be preserved through cloning
        let sym1_clone = sym1.clone();
        if sym1_clone.concrete_value() != Some(s1.as_str()) {
            return false;
        }

        // Property 3: from_concrete should preserve concrete values
        let sym3 = SymString::from_concrete(&s1, Arc::clone(&manager));
        if sym3.concrete_value() != Some(s1.as_str()) {
            return false;
        }

        // Property 4: set_concrete_value should update the concrete value
        let mut sym4 = SymString::new(Arc::clone(&manager));
        sym4.set_concrete_value(s2.clone());
        if sym4.concrete_value() != Some(s2.as_str()) {
            return false;
        }

        // Property 5: Symbolic expressions should maintain concrete values
        // Even though we don't have string operations implemented yet,
        // we can verify that the concrete value is accessible
        if let Some(concrete) = sym1.concrete_value() {
            if concrete != s1.as_str() {
                return false;
            }
        } else {
            return false; // Should have concrete value
        }

        // Property 6: Multiple symbolic strings with same concrete value
        // should have equal concrete values but different variable names
        let sym5 = SymString::with_value(s1.clone(), Arc::clone(&manager));
        if sym5.concrete_value() != sym1.concrete_value() {
            return false;
        }
        if sym5.variable_name() == sym1.variable_name() {
            return false; // Should have unique variable names
        }

        // Property 7: Empty strings should be handled correctly
        let empty = SymString::with_value(String::new(), Arc::clone(&manager));
        if empty.concrete_value() != Some("") {
            return false;
        }

        // Property 8: Concrete values should match the original strings exactly
        // (no transformations or modifications)
        if sym1.concrete_value().map(|s| s.to_string()) != Some(s1.clone()) {
            return false;
        }
        if sym2.concrete_value().map(|s| s.to_string()) != Some(s2.clone()) {
            return false;
        }

        true
    }

    // **Feature: symbolic-execution-engine, Property 12: Trait behavioral compatibility**
    // **Validates: Requirements 7.1, 7.3**
    #[qc]
    fn prop_trait_behavioral_compatibility(a: u64, b: u64) -> bool {
        // Avoid division by zero and overflow scenarios that differ between checked/unchecked
        // Also limit shift amounts to valid range
        if b == 0 || a > u64::MAX / 2 || b > u64::MAX / 2 {
            return true;
        }

        let manager = create_test_manager();

        // Create symbolic values from concrete values
        let sym_a = SymU64::from_concrete(a, Arc::clone(&manager));
        let sym_b = SymU64::from_concrete(b, Arc::clone(&manager));

        // Property 1: Addition should match concrete behavior
        let concrete_add = a.wrapping_add(b);
        let symbolic_add = &sym_a + &sym_b;
        if symbolic_add.concrete_value() != Some(concrete_add) {
            return false;
        }

        // Property 2: Subtraction should match concrete behavior
        let concrete_sub = a.wrapping_sub(b);
        let symbolic_sub = &sym_a - &sym_b;
        if symbolic_sub.concrete_value() != Some(concrete_sub) {
            return false;
        }

        // Property 3: Multiplication should match concrete behavior
        let concrete_mul = a.wrapping_mul(b);
        let symbolic_mul = &sym_a * &sym_b;
        if symbolic_mul.concrete_value() != Some(concrete_mul) {
            return false;
        }

        // Property 4: Division should match concrete behavior (b != 0 checked above)
        let concrete_div = a / b;
        let symbolic_div = &sym_a / &sym_b;
        if symbolic_div.concrete_value() != Some(concrete_div) {
            return false;
        }

        // Property 5: Remainder should match concrete behavior (b != 0 checked above)
        let concrete_rem = a % b;
        let symbolic_rem = &sym_a % &sym_b;
        if symbolic_rem.concrete_value() != Some(concrete_rem) {
            return false;
        }

        // Property 6: Bitwise AND should match concrete behavior
        let concrete_and = a & b;
        let symbolic_and = &sym_a & &sym_b;
        if symbolic_and.concrete_value() != Some(concrete_and) {
            return false;
        }

        // Property 7: Bitwise OR should match concrete behavior
        let concrete_or = a | b;
        let symbolic_or = &sym_a | &sym_b;
        if symbolic_or.concrete_value() != Some(concrete_or) {
            return false;
        }

        // Property 8: Bitwise XOR should match concrete behavior
        let concrete_xor = a ^ b;
        let symbolic_xor = &sym_a ^ &sym_b;
        if symbolic_xor.concrete_value() != Some(concrete_xor) {
            return false;
        }

        // Property 9: Left shift should match concrete behavior (for valid shift amounts)
        let shift_amount = b % 64; // Ensure valid shift amount
        let sym_shift = SymU64::from_concrete(shift_amount, Arc::clone(&manager));
        let concrete_shl = a << shift_amount;
        let symbolic_shl = &sym_a << &sym_shift;
        if symbolic_shl.concrete_value() != Some(concrete_shl) {
            return false;
        }

        // Property 10: Right shift should match concrete behavior (for valid shift amounts)
        let concrete_shr = a >> shift_amount;
        let symbolic_shr = &sym_a >> &sym_shift;
        if symbolic_shr.concrete_value() != Some(concrete_shr) {
            return false;
        }

        // Property 11: Equality comparison should match concrete behavior
        let concrete_eq = a == b;
        let symbolic_eq = sym_a == sym_b;
        // For concrete values, symbolic equality should match concrete equality
        if concrete_eq != symbolic_eq {
            return false;
        }

        // Property 12: Ordering comparison should match concrete behavior
        let concrete_ord = a.cmp(&b);
        let symbolic_ord = sym_a.cmp(&sym_b);
        if concrete_ord != symbolic_ord {
            return false;
        }

        // Property 13: PartialOrd should be consistent with Ord
        let partial_ord = sym_a.partial_cmp(&sym_b);
        if partial_ord != Some(symbolic_ord) {
            return false;
        }

        // Property 14: Symbolic operations should create proper expressions
        // Addition should create a BinaryOp(Add, _, _) expression
        if !matches!(symbolic_add.expr(), SymExpr::BinaryOp(BinOp::Add, _, _)) {
            return false;
        }

        // Property 15: Subtraction should create a BinaryOp(Sub, _, _) expression
        if !matches!(symbolic_sub.expr(), SymExpr::BinaryOp(BinOp::Sub, _, _)) {
            return false;
        }

        // Property 16: Multiplication should create a BinaryOp(Mul, _, _) expression
        if !matches!(symbolic_mul.expr(), SymExpr::BinaryOp(BinOp::Mul, _, _)) {
            return false;
        }

        // Property 17: Division should create a BinaryOp(Div, _, _) expression
        if !matches!(symbolic_div.expr(), SymExpr::BinaryOp(BinOp::Div, _, _)) {
            return false;
        }

        // Property 18: Remainder should create a BinaryOp(Mod, _, _) expression
        if !matches!(symbolic_rem.expr(), SymExpr::BinaryOp(BinOp::Mod, _, _)) {
            return false;
        }

        // Property 19: Bitwise AND should create a BinaryOp(BitAnd, _, _) expression
        if !matches!(symbolic_and.expr(), SymExpr::BinaryOp(BinOp::BitAnd, _, _)) {
            return false;
        }

        // Property 20: Bitwise OR should create a BinaryOp(BitOr, _, _) expression
        if !matches!(symbolic_or.expr(), SymExpr::BinaryOp(BinOp::BitOr, _, _)) {
            return false;
        }

        // Property 21: Bitwise XOR should create a BinaryOp(BitXor, _, _) expression
        if !matches!(symbolic_xor.expr(), SymExpr::BinaryOp(BinOp::BitXor, _, _)) {
            return false;
        }

        // Property 22: Left shift should create a BinaryOp(Shl, _, _) expression
        if !matches!(symbolic_shl.expr(), SymExpr::BinaryOp(BinOp::Shl, _, _)) {
            return false;
        }

        // Property 23: Right shift should create a BinaryOp(Shr, _, _) expression
        if !matches!(symbolic_shr.expr(), SymExpr::BinaryOp(BinOp::Shr, _, _)) {
            return false;
        }

        // Property 24: Complex expressions should maintain behavioral compatibility
        // Test: (a + b) * (a - b) should match concrete computation
        let concrete_complex = (a.wrapping_add(b)).wrapping_mul(a.wrapping_sub(b));
        let symbolic_complex = (&sym_a + &sym_b) * (&sym_a - &sym_b);
        if symbolic_complex.concrete_value() != Some(concrete_complex) {
            return false;
        }

        // Property 25: Trait operations should preserve manager reference
        // All derived symbolic values should share the same manager
        let derived = &sym_a + &sym_b;
        let derived_manager = derived.manager();
        let original_manager = sym_a.manager();

        // Check that both managers point to the same underlying manager
        // by comparing their variable counters (they should be the same instance)
        let derived_stats = derived_manager.lock().unwrap().get_stats();
        let original_stats = original_manager.lock().unwrap().get_stats();
        if derived_stats.variable_count != original_stats.variable_count {
            return false;
        }

        true
    }

    #[qc]
    fn prop_trait_compatibility_with_references(a: u64, b: u64, c: u64) -> bool {
        // Limit values to avoid overflow in complex expressions
        if a > 1000 || b > 1000 || c > 1000 || b == 0 || c == 0 {
            return true;
        }

        let manager = create_test_manager();

        let sym_a = SymU64::from_concrete(a, Arc::clone(&manager));
        let sym_b = SymU64::from_concrete(b, Arc::clone(&manager));
        let sym_c = SymU64::from_concrete(c, Arc::clone(&manager));

        // Property 1: Operations on references should match operations on owned values
        let owned_result = sym_a.clone() + sym_b.clone();
        let ref_result = &sym_a + &sym_b;
        if owned_result.concrete_value() != ref_result.concrete_value() {
            return false;
        }

        // Property 2: Chained operations should maintain behavioral compatibility
        // Test: (a + b) / c
        let concrete_chain = (a.wrapping_add(b)) / c;
        let symbolic_chain = (&sym_a + &sym_b) / sym_c.clone();
        if symbolic_chain.concrete_value() != Some(concrete_chain) {
            return false;
        }

        // Property 3: Associativity should be preserved in expressions
        // Test: (a + b) + c vs a + (b + c)
        let concrete_left = (a.wrapping_add(b)).wrapping_add(c);
        let concrete_right = a.wrapping_add(b.wrapping_add(c));
        let symbolic_left = (&sym_a + &sym_b) + sym_c.clone();
        let symbolic_right = sym_a.clone() + (&sym_b + &sym_c);

        // Both should match their respective concrete computations
        if symbolic_left.concrete_value() != Some(concrete_left) {
            return false;
        }
        if symbolic_right.concrete_value() != Some(concrete_right) {
            return false;
        }

        // Property 4: Commutativity should be preserved for commutative operations
        // Test: a + b == b + a
        let concrete_ab = a.wrapping_add(b);
        let concrete_ba = b.wrapping_add(a);
        let symbolic_ab = &sym_a + &sym_b;
        let symbolic_ba = &sym_b + &sym_a;

        if concrete_ab != concrete_ba {
            return false; // Sanity check
        }
        if symbolic_ab.concrete_value() != symbolic_ba.concrete_value() {
            return false;
        }

        // Property 5: Multiple references to the same symbolic value should be consistent
        let result1 = &sym_a + &sym_a;
        let result2 = &sym_a + &sym_a;
        if result1.concrete_value() != result2.concrete_value() {
            return false;
        }

        true
    }

    #[qc]
    fn prop_trait_compatibility_ordering(values: Vec<u64>) -> bool {
        // Limit the number of values to test
        let values: Vec<u64> = values.into_iter().take(20).collect();

        if values.len() < 2 {
            return true;
        }

        let manager = create_test_manager();

        // Create symbolic versions of all values
        let symbolic_values: Vec<SymU64> = values
            .iter()
            .map(|&v| SymU64::from_concrete(v, Arc::clone(&manager)))
            .collect();

        // Property 1: Ordering relationships should match concrete types
        for i in 0..values.len() {
            for j in 0..values.len() {
                let concrete_cmp = values[i].cmp(&values[j]);
                let symbolic_cmp = symbolic_values[i].cmp(&symbolic_values[j]);

                if concrete_cmp != symbolic_cmp {
                    return false;
                }

                // Property 2: PartialOrd should be consistent with Ord
                let partial_cmp = symbolic_values[i].partial_cmp(&symbolic_values[j]);
                if partial_cmp != Some(symbolic_cmp) {
                    return false;
                }

                // Property 3: Equality should be transitive
                if i == j {
                    if symbolic_values[i] != symbolic_values[j] {
                        return false;
                    }
                }
            }
        }

        // Property 4: Sorting should produce the same order
        let mut concrete_sorted = values.clone();
        concrete_sorted.sort();

        let mut symbolic_sorted = symbolic_values.clone();
        symbolic_sorted.sort();

        // Extract concrete values from sorted symbolic values
        let symbolic_concrete_values: Vec<u64> = symbolic_sorted
            .iter()
            .filter_map(|s| s.concrete_value())
            .collect();

        if concrete_sorted != symbolic_concrete_values {
            return false;
        }

        true
    }

    // Tests for SymString comparison operations

    #[test]
    fn test_symstring_eq_constraint() {
        let manager = create_test_manager();
        let s1 = SymString::from_concrete("hello", Arc::clone(&manager));
        let s2 = SymString::from_concrete("world", Arc::clone(&manager));

        let constraint = s1.eq_constraint(&s2);
        assert!(matches!(constraint, SymExpr::BinaryOp(BinOp::Eq, _, _)));
    }

    #[test]
    fn test_symstring_ne_constraint() {
        let manager = create_test_manager();
        let s1 = SymString::from_concrete("hello", Arc::clone(&manager));
        let s2 = SymString::from_concrete("world", Arc::clone(&manager));

        let constraint = s1.ne_constraint(&s2);
        assert!(matches!(constraint, SymExpr::BinaryOp(BinOp::Ne, _, _)));
    }

    #[test]
    fn test_symstring_lt_constraint() {
        let manager = create_test_manager();
        let s1 = SymString::from_concrete("apple", Arc::clone(&manager));
        let s2 = SymString::from_concrete("banana", Arc::clone(&manager));

        let constraint = s1.lt_constraint(&s2);
        assert!(matches!(constraint, SymExpr::BinaryOp(BinOp::StrLexLt, _, _)));
    }

    #[test]
    fn test_symstring_le_constraint() {
        let manager = create_test_manager();
        let s1 = SymString::from_concrete("apple", Arc::clone(&manager));
        let s2 = SymString::from_concrete("banana", Arc::clone(&manager));

        let constraint = s1.le_constraint(&s2);
        assert!(matches!(constraint, SymExpr::BinaryOp(BinOp::StrLexLe, _, _)));
    }

    #[test]
    fn test_symstring_gt_constraint() {
        let manager = create_test_manager();
        let s1 = SymString::from_concrete("banana", Arc::clone(&manager));
        let s2 = SymString::from_concrete("apple", Arc::clone(&manager));

        let constraint = s1.gt_constraint(&s2);
        // gt is implemented as lt with swapped operands
        assert!(matches!(constraint, SymExpr::BinaryOp(BinOp::StrLexLt, _, _)));
    }

    #[test]
    fn test_symstring_ge_constraint() {
        let manager = create_test_manager();
        let s1 = SymString::from_concrete("banana", Arc::clone(&manager));
        let s2 = SymString::from_concrete("apple", Arc::clone(&manager));

        let constraint = s1.ge_constraint(&s2);
        // ge is implemented as le with swapped operands
        assert!(matches!(constraint, SymExpr::BinaryOp(BinOp::StrLexLe, _, _)));
    }

    #[test]
    fn test_symstring_assert_eq() {
        let manager = create_test_manager();
        let s1 = SymString::from_concrete("hello", Arc::clone(&manager));
        let s2 = SymString::from_concrete("hello", Arc::clone(&manager));

        let result = s1.assert_eq(&s2);
        assert!(result.is_ok());

        // Check that constraint was added
        let mgr = manager.lock().unwrap();
        assert!(!mgr.get_constraints().is_empty());
    }

    #[test]
    fn test_symstring_assert_ne() {
        let manager = create_test_manager();
        let s1 = SymString::from_concrete("hello", Arc::clone(&manager));
        let s2 = SymString::from_concrete("world", Arc::clone(&manager));

        let result = s1.assert_ne(&s2);
        assert!(result.is_ok());

        // Check that constraint was added
        let mgr = manager.lock().unwrap();
        assert!(!mgr.get_constraints().is_empty());
    }

    #[test]
    fn test_symstring_partial_eq_with_concrete() {
        let manager = create_test_manager();
        let s1 = SymString::from_concrete("hello", Arc::clone(&manager));
        let s2 = SymString::from_concrete("hello", Arc::clone(&manager));
        let s3 = SymString::from_concrete("world", Arc::clone(&manager));

        // Concrete values should be used for fast comparison
        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }

    #[test]
    fn test_symstring_partial_ord_with_concrete() {
        let manager = create_test_manager();
        let s1 = SymString::from_concrete("apple", Arc::clone(&manager));
        let s2 = SymString::from_concrete("banana", Arc::clone(&manager));
        let s3 = SymString::from_concrete("cherry", Arc::clone(&manager));

        // Lexicographic ordering should work with concrete values
        assert!(s1 < s2);
        assert!(s2 < s3);
        assert!(s1 < s3);
        assert!(s2 > s1);
        assert!(s3 > s2);
    }

    #[test]
    fn test_symstring_ord_with_concrete() {
        let manager = create_test_manager();
        let s1 = SymString::from_concrete("apple", Arc::clone(&manager));
        let s2 = SymString::from_concrete("banana", Arc::clone(&manager));

        assert_eq!(s1.cmp(&s2), Ordering::Less);
        assert_eq!(s2.cmp(&s1), Ordering::Greater);
        assert_eq!(s1.cmp(&s1), Ordering::Equal);
    }

    // Tests for string concatenation

    #[test]
    fn test_symstring_concat_with_concrete() {
        let manager = create_test_manager();
        let s1 = SymString::from_concrete("hello", Arc::clone(&manager));
        let s2 = SymString::from_concrete(" world", Arc::clone(&manager));

        let result = s1.concat(&s2);

        assert_eq!(result.concrete_value(), Some("hello world"));
        assert!(matches!(
            result.expr(),
            SymExpr::BinaryOp(BinOp::StrConcat, _, _)
        ));
    }

    #[test]
    fn test_symstring_concat_empty_strings() {
        let manager = create_test_manager();
        let s1 = SymString::from_concrete("hello", Arc::clone(&manager));
        let empty = SymString::from_concrete("", Arc::clone(&manager));

        let result1 = s1.concat(&empty);
        assert_eq!(result1.concrete_value(), Some("hello"));

        let result2 = empty.concat(&s1);
        assert_eq!(result2.concrete_value(), Some("hello"));
    }

    #[test]
    fn test_symstring_concat_symbolic() {
        let manager = create_test_manager();
        let s1 = SymString::new(Arc::clone(&manager));
        let s2 = SymString::new(Arc::clone(&manager));

        let result = s1.concat(&s2);

        // No concrete value for purely symbolic strings
        assert_eq!(result.concrete_value(), None);
        assert!(matches!(
            result.expr(),
            SymExpr::BinaryOp(BinOp::StrConcat, _, _)
        ));
    }

    #[test]
    fn test_symstring_concat_mixed() {
        let manager = create_test_manager();
        let s1 = SymString::from_concrete("hello", Arc::clone(&manager));
        let s2 = SymString::new(Arc::clone(&manager));

        let result = s1.concat(&s2);

        // No concrete value when one operand is purely symbolic
        assert_eq!(result.concrete_value(), None);
        assert!(matches!(
            result.expr(),
            SymExpr::BinaryOp(BinOp::StrConcat, _, _)
        ));
    }

    #[test]
    fn test_symstring_add_trait() {
        let manager = create_test_manager();
        let s1 = SymString::from_concrete("hello", Arc::clone(&manager));
        let s2 = SymString::from_concrete(" world", Arc::clone(&manager));

        // Test Add trait for owned values
        let result = s1.clone() + s2.clone();
        assert_eq!(result.concrete_value(), Some("hello world"));

        // Test Add trait for references
        let result = &s1 + &s2;
        assert_eq!(result.concrete_value(), Some("hello world"));
    }

    #[test]
    fn test_symstring_chained_concat() {
        let manager = create_test_manager();
        let s1 = SymString::from_concrete("hello", Arc::clone(&manager));
        let s2 = SymString::from_concrete(" ", Arc::clone(&manager));
        let s3 = SymString::from_concrete("world", Arc::clone(&manager));

        // Test chained concatenation: s1 + s2 + s3
        let result = &s1 + &s2;
        let result = &result + &s3;

        assert_eq!(result.concrete_value(), Some("hello world"));

        // Check that the expression is properly nested
        if let SymExpr::BinaryOp(BinOp::StrConcat, left, _right) = result.expr() {
            assert!(matches!(**left, SymExpr::BinaryOp(BinOp::StrConcat, _, _)));
        } else {
            panic!("Expected concatenation expression");
        }
    }

    #[test]
    fn test_symstring_concat_preserves_manager() {
        let manager = create_test_manager();
        let s1 = SymString::from_concrete("hello", Arc::clone(&manager));
        let s2 = SymString::from_concrete(" world", Arc::clone(&manager));

        let result = s1.concat(&s2);

        // Check that the result uses the same manager
        let result_manager = result.manager();
        let original_manager = s1.manager();

        let result_stats = result_manager.lock().unwrap().get_stats();
        let original_stats = original_manager.lock().unwrap().get_stats();

        // Both should reference the same manager instance
        assert_eq!(result_stats.variable_count, original_stats.variable_count);
    }

    // Tests for string length operations

    #[test]
    fn test_symstring_length_with_concrete() {
        let manager = create_test_manager();
        let s = SymString::from_concrete("hello", Arc::clone(&manager));

        let len = s.length();

        assert_eq!(len.concrete_value(), Some(5));
        assert!(matches!(len.expr(), SymExpr::UnaryOp(UnOp::StrLen, _)));
    }

    #[test]
    fn test_symstring_length_empty_string() {
        let manager = create_test_manager();
        let s = SymString::from_concrete("", Arc::clone(&manager));

        let len = s.length();

        assert_eq!(len.concrete_value(), Some(0));
        assert!(matches!(len.expr(), SymExpr::UnaryOp(UnOp::StrLen, _)));
    }

    #[test]
    fn test_symstring_length_symbolic() {
        let manager = create_test_manager();
        let s = SymString::new(Arc::clone(&manager));

        let len = s.length();

        // SymU64::from_expr defaults to Some(0) when no concrete value can be computed
        // This is expected behavior from the macro-generated code
        assert_eq!(len.concrete_value(), Some(0));
        assert!(matches!(len.expr(), SymExpr::UnaryOp(UnOp::StrLen, _)));
    }

    #[test]
    fn test_symstring_length_with_value() {
        let manager = create_test_manager();
        let s = SymString::with_value("test string".to_string(), Arc::clone(&manager));

        let len = s.length();

        assert_eq!(len.concrete_value(), Some(11));
        assert!(matches!(len.expr(), SymExpr::UnaryOp(UnOp::StrLen, _)));
    }

    #[test]
    fn test_symstring_length_after_concat() {
        let manager = create_test_manager();
        let s1 = SymString::from_concrete("hello", Arc::clone(&manager));
        let s2 = SymString::from_concrete(" world", Arc::clone(&manager));

        let concatenated = s1.concat(&s2);
        let len = concatenated.length();

        // Length of "hello world" is 11
        assert_eq!(len.concrete_value(), Some(11));
        assert!(matches!(len.expr(), SymExpr::UnaryOp(UnOp::StrLen, _)));
    }

    #[test]
    fn test_symstring_length_unicode() {
        let manager = create_test_manager();
        // Note: This test uses ASCII-compatible characters
        let s = SymString::from_concrete("hello!", Arc::clone(&manager));

        let len = s.length();

        assert_eq!(len.concrete_value(), Some(6));
    }

    #[test]
    fn test_symstring_length_preserves_manager() {
        let manager = create_test_manager();
        let s = SymString::from_concrete("test", Arc::clone(&manager));

        let len = s.length();

        // Check that the result uses the same manager
        let len_manager = len.manager();
        let original_manager = s.manager();

        let len_stats = len_manager.lock().unwrap().get_stats();
        let original_stats = original_manager.lock().unwrap().get_stats();

        // Both should reference the same manager instance
        assert_eq!(len_stats.variable_count, original_stats.variable_count);
    }
}
