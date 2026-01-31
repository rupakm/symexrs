//! Macro for generating symbolic integer types
//!
//! This module provides macros that generate symbolic integer types
//! (SymU64, SymU32, SymI32, SymI64, SymU8) with all necessary trait implementations.

/// Macro to generate a symbolic integer type with all trait implementations
///
/// # Parameters
/// - `$sym_type`: The symbolic type name (e.g., `SymU64`)
/// - `$concrete_type`: The concrete Rust type (e.g., `u64`)
/// - `$const_variant`: The ConstValue variant (e.g., `U64`)
/// - `$type_name`: String name for the type (e.g., `"u64"`)
/// - `$bit_width`: Bit width of the type (e.g., `64`)
/// - `$is_signed`: Whether the type is signed (e.g., `false`)
/// - `$model_getter`: The Model method to get values (e.g., `get_u64`)
#[macro_export]
macro_rules! define_sym_int {
    (
        $sym_type:ident,
        $concrete_type:ty,
        $const_variant:ident,
        $type_name:expr,
        $bit_width:expr,
        $is_signed:expr,
        $model_getter:ident
    ) => {
        /// Symbolic integer type that implements standard Rust traits
        ///
        /// This type implements the same traits as its concrete counterpart while building
        /// symbolic expressions during execution. It maintains a reference to the global
        /// SymExManager for constraint tracking.
        #[derive(Clone)]
        pub struct $sym_type {
            /// Unique symbolic variable identifier
            variable_name: String,
            /// Symbolic expression representing this value
            expr: $crate::expressions::SymExpr,
            /// Optional concrete value for concolic execution
            concrete_value: Option<$concrete_type>,
            /// Reference to the global symbolic execution manager
            manager: std::sync::Arc<std::sync::Mutex<$crate::manager::SymExManager>>,
        }

        impl $sym_type {
            /// Create a new symbolic value with a fresh variable name
            pub fn new(
                manager: std::sync::Arc<std::sync::Mutex<$crate::manager::SymExManager>>,
            ) -> Self {
                let variable_name = {
                    let mgr = manager.lock().unwrap();
                    mgr.fresh_variable($type_name)
                };
                let expr = $crate::expressions::SymExpr::Variable(variable_name.clone());

                // Register the variable with the manager
                {
                    let mut mgr = manager.lock().unwrap();
                    let type_info = $crate::manager::TypeInfo {
                        type_name: $type_name.to_string(),
                        bit_width: Some($bit_width),
                        is_signed: $is_signed,
                        creation_site: None,
                    };
                    let _ = mgr.register_variable(variable_name.clone(), type_info);
                }

                Self {
                    variable_name,
                    expr,
                    concrete_value: Some(0 as $concrete_type),
                    manager,
                }
            }

            /// Create a new symbolic value using the global thread-local manager
            pub fn new_global() -> Self {
                let manager = $crate::get_global_manager().expect("Failed to get global manager");
                Self::new(manager)
            }

            /// Create a new symbolic value with a specific variable name
            pub fn with_name(
                name: String,
                manager: std::sync::Arc<std::sync::Mutex<$crate::manager::SymExManager>>,
            ) -> Self {
                let expr = $crate::expressions::SymExpr::Variable(name.clone());

                // Register the variable with the manager
                {
                    let mut mgr = manager.lock().unwrap();
                    let type_info = $crate::manager::TypeInfo {
                        type_name: $type_name.to_string(),
                        bit_width: Some($bit_width),
                        is_signed: $is_signed,
                        creation_site: None,
                    };
                    let _ = mgr.register_variable(name.clone(), type_info);
                }

                Self {
                    variable_name: name,
                    expr,
                    concrete_value: Some(0 as $concrete_type),
                    manager,
                }
            }

            /// Create a new symbolic value with an initial concrete value
            ///
            /// This is useful for concolic execution where you want to track a value
            /// symbolically but also maintain a concrete value for fast path checking.
            pub fn with_value(
                value: $concrete_type,
                manager: std::sync::Arc<std::sync::Mutex<$crate::manager::SymExManager>>,
            ) -> Self {
                let variable_name = {
                    let mut mgr = manager.lock().unwrap();
                    let type_info = $crate::manager::TypeInfo {
                        type_name: $type_name.to_string(),
                        bit_width: Some($bit_width),
                        is_signed: $is_signed,
                        creation_site: None,
                    };
                    let name = mgr.fresh_variable($type_name);
                    let _ = mgr.register_variable(name.clone(), type_info);
                    name
                };

                let expr = $crate::expressions::SymExpr::Variable(variable_name.clone());

                Self {
                    variable_name,
                    expr,
                    concrete_value: Some(value),
                    manager,
                }
            }

            /// Create a symbolic value from a concrete value
            pub fn from_concrete(
                value: $concrete_type,
                manager: std::sync::Arc<std::sync::Mutex<$crate::manager::SymExManager>>,
            ) -> Self {
                let variable_name = {
                    let mgr = manager.lock().unwrap();
                    mgr.fresh_variable($type_name)
                };

                let expr = $crate::expressions::SymExpr::Constant(
                    $crate::expressions::ConstValue::$const_variant(value),
                );

                Self {
                    variable_name,
                    expr,
                    concrete_value: Some(value),
                    manager,
                }
            }

            /// Create a symbolic value from an existing expression
            pub fn from_expr(
                expr: $crate::expressions::SymExpr,
                manager: std::sync::Arc<std::sync::Mutex<$crate::manager::SymExManager>>,
            ) -> Self {
                let variable_name = {
                    let mgr = manager.lock().unwrap();
                    mgr.fresh_variable($type_name)
                };

                // Try to evaluate the expression with empty bindings (works for constants)
                // and cast to the appropriate type, otherwise default to 0
                let concrete_value = {
                    let bindings = std::collections::HashMap::new();
                    expr.get_concrete_value(&bindings)
                        .and_then(|cv| match stringify!($const_variant) {
                            "U64" => cv.as_u64().map(|v| v as $concrete_type),
                            "I64" => cv.as_i64().map(|v| v as $concrete_type),
                            "U32" => cv.as_u32().map(|v| v as $concrete_type),
                            "I32" => cv.as_i32().map(|v| v as $concrete_type),
                            "U8" => cv.as_u8().map(|v| v as $concrete_type),
                            "F64" => cv.as_f64().map(|v| v as $concrete_type),
                            "F32" => cv.as_f32().map(|v| v as $concrete_type),
                            _ => None,
                        })
                        .or(Some(0 as $concrete_type))
                };

                Self {
                    variable_name,
                    expr,
                    concrete_value,
                    manager,
                }
            }

            /// Get the symbolic expression representing this value
            pub fn expr(&self) -> &$crate::expressions::SymExpr {
                &self.expr
            }

            /// Get the variable name
            pub fn variable_name(&self) -> &str {
                &self.variable_name
            }

            /// Get the concrete value if available
            pub fn concrete_value(&self) -> Option<$concrete_type> {
                self.concrete_value
            }

            /// Set the concrete value (for concolic execution updates)
            pub fn set_concrete_value(&mut self, value: $concrete_type) {
                self.concrete_value = Some(value);
            }

            /// Update concrete value from a model (for concolic execution)
            /// Returns true if the value was updated
            pub fn update_from_model(&mut self, model: &$crate::solver::Model) -> bool {
                if let Some(value) = model.$model_getter(&self.variable_name) {
                    self.concrete_value = Some(value);
                    true
                } else {
                    false
                }
            }

            /// Get a reference to the manager
            pub fn manager(
                &self,
            ) -> std::sync::Arc<std::sync::Mutex<$crate::manager::SymExManager>> {
                std::sync::Arc::clone(&self.manager)
            }

            /// Generate an equality constraint
            pub fn eq_constraint(&self, other: &Self) -> $crate::expressions::SymExpr {
                $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Eq,
                    self.expr.clone(),
                    other.expr.clone(),
                )
            }

            /// Generate a not-equal constraint
            pub fn ne_constraint(&self, other: &Self) -> $crate::expressions::SymExpr {
                $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Ne,
                    self.expr.clone(),
                    other.expr.clone(),
                )
            }

            /// Generate a less-than constraint
            pub fn lt_constraint(&self, other: &Self) -> $crate::expressions::SymExpr {
                $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Lt,
                    self.expr.clone(),
                    other.expr.clone(),
                )
            }

            /// Generate a less-than-or-equal constraint
            pub fn le_constraint(&self, other: &Self) -> $crate::expressions::SymExpr {
                $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Le,
                    self.expr.clone(),
                    other.expr.clone(),
                )
            }

            /// Generate a greater-than constraint
            pub fn gt_constraint(&self, other: &Self) -> $crate::expressions::SymExpr {
                $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Gt,
                    self.expr.clone(),
                    other.expr.clone(),
                )
            }

            /// Generate a greater-than-or-equal constraint
            pub fn ge_constraint(&self, other: &Self) -> $crate::expressions::SymExpr {
                $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Ge,
                    self.expr.clone(),
                    other.expr.clone(),
                )
            }

            /// Add an equality constraint to the manager
            pub fn assert_eq(&self, other: &Self) -> $crate::SymExResult<()> {
                let constraint = self.eq_constraint(other);
                let mut mgr = self.manager.lock().unwrap();
                mgr.add_constraint(constraint)
            }

            /// Add a not-equal constraint to the manager
            pub fn assert_ne(&self, other: &Self) -> $crate::SymExResult<()> {
                let constraint = self.ne_constraint(other);
                let mut mgr = self.manager.lock().unwrap();
                mgr.add_constraint(constraint)
            }

            /// Add a less-than constraint to the manager
            pub fn assert_lt(&self, other: &Self) -> $crate::SymExResult<()> {
                let constraint = self.lt_constraint(other);
                let mut mgr = self.manager.lock().unwrap();
                mgr.add_constraint(constraint)
            }

            /// Add a less-than-or-equal constraint to the manager
            pub fn assert_le(&self, other: &Self) -> $crate::SymExResult<()> {
                let constraint = self.le_constraint(other);
                let mut mgr = self.manager.lock().unwrap();
                mgr.add_constraint(constraint)
            }

            /// Add a greater-than constraint to the manager
            pub fn assert_gt(&self, other: &Self) -> $crate::SymExResult<()> {
                let constraint = self.gt_constraint(other);
                let mut mgr = self.manager.lock().unwrap();
                mgr.add_constraint(constraint)
            }

            /// Add a greater-than-or-equal constraint to the manager
            pub fn assert_ge(&self, other: &Self) -> $crate::SymExResult<()> {
                let constraint = self.ge_constraint(other);
                let mut mgr = self.manager.lock().unwrap();
                mgr.add_constraint(constraint)
            }
        }

        // Debug trait implementation
        impl std::fmt::Debug for $sym_type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!($sym_type))
                    .field("variable_name", &self.variable_name)
                    .field("expr", &self.expr)
                    .field("concrete_value", &self.concrete_value)
                    .finish()
            }
        }

        // Add trait for owned values
        impl std::ops::Add for $sym_type {
            type Output = $sym_type;

            fn add(self, rhs: Self) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Add,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) => Some(a.wrapping_add(b)),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: self.manager,
                }
            }
        }

        // Add trait for references
        impl std::ops::Add for &$sym_type {
            type Output = $sym_type;

            fn add(self, rhs: Self) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Add,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) => Some(a.wrapping_add(b)),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: std::sync::Arc::clone(&self.manager),
                }
            }
        }

        // Sub trait for owned values
        impl std::ops::Sub for $sym_type {
            type Output = $sym_type;

            fn sub(self, rhs: Self) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Sub,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) => Some(a.wrapping_sub(b)),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: self.manager,
                }
            }
        }

        impl std::ops::Sub for &$sym_type {
            type Output = $sym_type;

            fn sub(self, rhs: Self) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Sub,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) => Some(a.wrapping_sub(b)),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: std::sync::Arc::clone(&self.manager),
                }
            }
        }

        // Mul trait
        impl std::ops::Mul for $sym_type {
            type Output = $sym_type;

            fn mul(self, rhs: Self) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Mul,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) => Some(a.wrapping_mul(b)),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: self.manager,
                }
            }
        }

        impl std::ops::Mul for &$sym_type {
            type Output = $sym_type;

            fn mul(self, rhs: Self) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Mul,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) => Some(a.wrapping_mul(b)),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: std::sync::Arc::clone(&self.manager),
                }
            }
        }

        // Div trait
        impl std::ops::Div for $sym_type {
            type Output = $sym_type;

            fn div(self, rhs: Self) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Div,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) if b != 0 => Some(a / b),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: self.manager,
                }
            }
        }

        impl std::ops::Div for &$sym_type {
            type Output = $sym_type;

            fn div(self, rhs: Self) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Div,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) if b != 0 => Some(a / b),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: std::sync::Arc::clone(&self.manager),
                }
            }
        }

        // Rem trait
        impl std::ops::Rem for $sym_type {
            type Output = $sym_type;

            fn rem(self, rhs: Self) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Mod,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) if b != 0 => Some(a % b),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: self.manager,
                }
            }
        }

        impl std::ops::Rem for &$sym_type {
            type Output = $sym_type;

            fn rem(self, rhs: Self) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Mod,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) if b != 0 => Some(a % b),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: std::sync::Arc::clone(&self.manager),
                }
            }
        }

        // BitAnd trait
        impl std::ops::BitAnd for $sym_type {
            type Output = $sym_type;

            fn bitand(self, rhs: Self) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::BitAnd,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) => Some(a & b),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: self.manager,
                }
            }
        }

        impl std::ops::BitAnd for &$sym_type {
            type Output = $sym_type;

            fn bitand(self, rhs: Self) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::BitAnd,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) => Some(a & b),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: std::sync::Arc::clone(&self.manager),
                }
            }
        }

        // BitOr trait
        impl std::ops::BitOr for $sym_type {
            type Output = $sym_type;

            fn bitor(self, rhs: Self) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::BitOr,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) => Some(a | b),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: self.manager,
                }
            }
        }

        impl std::ops::BitOr for &$sym_type {
            type Output = $sym_type;

            fn bitor(self, rhs: Self) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::BitOr,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) => Some(a | b),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: std::sync::Arc::clone(&self.manager),
                }
            }
        }

        // BitXor trait
        impl std::ops::BitXor for $sym_type {
            type Output = $sym_type;

            fn bitxor(self, rhs: Self) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::BitXor,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) => Some(a ^ b),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: self.manager,
                }
            }
        }

        impl std::ops::BitXor for &$sym_type {
            type Output = $sym_type;

            fn bitxor(self, rhs: Self) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::BitXor,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) => Some(a ^ b),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: std::sync::Arc::clone(&self.manager),
                }
            }
        }

        // Shl trait
        impl std::ops::Shl<$sym_type> for $sym_type {
            type Output = $sym_type;

            fn shl(self, rhs: $sym_type) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Shl,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) if (b as u32) < $bit_width => Some(a << b),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: self.manager,
                }
            }
        }

        impl std::ops::Shl<&$sym_type> for &$sym_type {
            type Output = $sym_type;

            fn shl(self, rhs: &$sym_type) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Shl,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) if (b as u32) < $bit_width => Some(a << b),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: std::sync::Arc::clone(&self.manager),
                }
            }
        }

        // Shr trait
        impl std::ops::Shr<$sym_type> for $sym_type {
            type Output = $sym_type;

            fn shr(self, rhs: $sym_type) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Shr,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) if (b as u32) < $bit_width => Some(a >> b),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: self.manager,
                }
            }
        }

        impl std::ops::Shr<&$sym_type> for &$sym_type {
            type Output = $sym_type;

            fn shr(self, rhs: &$sym_type) -> Self::Output {
                let expr = $crate::expressions::SymExpr::binary_op(
                    $crate::expressions::BinOp::Shr,
                    self.expr.clone(),
                    rhs.expr.clone(),
                );
                let concrete_value = match (self.concrete_value, rhs.concrete_value) {
                    (Some(a), Some(b)) if (b as u32) < $bit_width => Some(a >> b),
                    _ => None,
                };

                $sym_type {
                    variable_name: {
                        let mgr = self.manager.lock().unwrap();
                        mgr.fresh_variable($type_name)
                    },
                    expr,
                    concrete_value,
                    manager: std::sync::Arc::clone(&self.manager),
                }
            }
        }

        // PartialEq trait with path forking support
        impl PartialEq for $sym_type {
            fn eq(&self, other: &Self) -> bool {
                // If we're in symbolic mode, this creates a branch point
                if $crate::symbolic_types::is_symbolic_mode() {
                    // Check if we have a predetermined branch decision
                    if let Some(decision) = $crate::symbolic_types::get_next_branch_decision() {
                        // Add the appropriate constraint to the manager
                        let constraint = if decision {
                            self.eq_constraint(other)
                        } else {
                            self.ne_constraint(other)
                        };

                        // Add constraint and check satisfiability immediately
                        let mut mgr = self.manager.lock().unwrap();
                        let _ = mgr.add_constraint(constraint);

                        // EAGER PATH PRUNING: Check if the path is still satisfiable
                        if let Ok(is_sat) = mgr.is_satisfiable() {
                            if !is_sat {
                                drop(mgr);
                                $crate::symbolic_types::mark_path_unsatisfiable();
                                std::panic::panic_any($crate::UnsatPanic);
                            }
                        }

                        return decision;
                    }

                    // No predetermined decision - use concrete result if available
                    if let (Some(a), Some(b)) = (self.concrete_value, other.concrete_value) {
                        return a == b;
                    }

                    false
                } else {
                    // Not in symbolic mode - use concrete values if available
                    if let (Some(a), Some(b)) = (self.concrete_value, other.concrete_value) {
                        return a == b;
                    }
                    self.expr == other.expr
                }
            }
        }

        impl Eq for $sym_type {}

        // PartialOrd and Ord traits
        impl PartialOrd for $sym_type {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for $sym_type {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                // If we have concrete values, use them
                if let (Some(a), Some(b)) = (self.concrete_value, other.concrete_value) {
                    return a.cmp(&b);
                }

                // If we're in symbolic mode, comparisons create branch points
                if $crate::symbolic_types::is_symbolic_mode() {
                    if let (Some(a), Some(b)) = (self.concrete_value, other.concrete_value) {
                        a.cmp(&b)
                    } else {
                        // Deterministic ordering based on variable names
                        self.variable_name.cmp(&other.variable_name)
                    }
                } else {
                    match (self.concrete_value, other.concrete_value) {
                        (Some(a), Some(b)) => a.cmp(&b),
                        _ => self.variable_name.cmp(&other.variable_name),
                    }
                }
            }
        }

        // From concrete type
        impl From<$concrete_type> for $sym_type {
            fn from(value: $concrete_type) -> Self {
                let manager = $crate::get_global_manager().expect("Failed to get global manager");
                $sym_type::from_concrete(value, manager)
            }
        }

        // TryFrom to concrete type
        impl TryFrom<$sym_type> for $concrete_type {
            type Error = &'static str;

            fn try_from(value: $sym_type) -> Result<Self, Self::Error> {
                value.concrete_value.ok_or("No concrete value available")
            }
        }

        impl TryFrom<&$sym_type> for $concrete_type {
            type Error = &'static str;

            fn try_from(value: &$sym_type) -> Result<Self, Self::Error> {
                value.concrete_value.ok_or("No concrete value available")
            }
        }

        // ============================================================================
        // Mixed-type operations: $sym_type with $concrete_type
        // ============================================================================

        // Add: $sym_type + $concrete_type
        impl std::ops::Add<$concrete_type> for $sym_type {
            type Output = $sym_type;
            fn add(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self + rhs_sym
            }
        }

        impl std::ops::Add<$concrete_type> for &$sym_type {
            type Output = $sym_type;
            fn add(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self + &rhs_sym
            }
        }

        impl std::ops::Add<$sym_type> for $concrete_type {
            type Output = $sym_type;
            fn add(self, rhs: $sym_type) -> Self::Output {
                let lhs_sym = $sym_type::from_concrete(self, std::sync::Arc::clone(&rhs.manager));
                lhs_sym + rhs
            }
        }

        impl std::ops::Add<&$sym_type> for $concrete_type {
            type Output = $sym_type;
            fn add(self, rhs: &$sym_type) -> Self::Output {
                let lhs_sym = $sym_type::from_concrete(self, std::sync::Arc::clone(&rhs.manager));
                &lhs_sym + rhs
            }
        }

        // Sub mixed-type
        impl std::ops::Sub<$concrete_type> for $sym_type {
            type Output = $sym_type;
            fn sub(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self - rhs_sym
            }
        }

        impl std::ops::Sub<$concrete_type> for &$sym_type {
            type Output = $sym_type;
            fn sub(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self - &rhs_sym
            }
        }

        impl std::ops::Sub<$sym_type> for $concrete_type {
            type Output = $sym_type;
            fn sub(self, rhs: $sym_type) -> Self::Output {
                let lhs_sym = $sym_type::from_concrete(self, std::sync::Arc::clone(&rhs.manager));
                lhs_sym - rhs
            }
        }

        impl std::ops::Sub<&$sym_type> for $concrete_type {
            type Output = $sym_type;
            fn sub(self, rhs: &$sym_type) -> Self::Output {
                let lhs_sym = $sym_type::from_concrete(self, std::sync::Arc::clone(&rhs.manager));
                &lhs_sym - rhs
            }
        }

        // Mul mixed-type
        impl std::ops::Mul<$concrete_type> for $sym_type {
            type Output = $sym_type;
            fn mul(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self * rhs_sym
            }
        }

        impl std::ops::Mul<$concrete_type> for &$sym_type {
            type Output = $sym_type;
            fn mul(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self * &rhs_sym
            }
        }

        impl std::ops::Mul<$sym_type> for $concrete_type {
            type Output = $sym_type;
            fn mul(self, rhs: $sym_type) -> Self::Output {
                let lhs_sym = $sym_type::from_concrete(self, std::sync::Arc::clone(&rhs.manager));
                lhs_sym * rhs
            }
        }

        impl std::ops::Mul<&$sym_type> for $concrete_type {
            type Output = $sym_type;
            fn mul(self, rhs: &$sym_type) -> Self::Output {
                let lhs_sym = $sym_type::from_concrete(self, std::sync::Arc::clone(&rhs.manager));
                &lhs_sym * rhs
            }
        }

        // Div mixed-type
        impl std::ops::Div<$concrete_type> for $sym_type {
            type Output = $sym_type;
            fn div(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self / rhs_sym
            }
        }

        impl std::ops::Div<$concrete_type> for &$sym_type {
            type Output = $sym_type;
            fn div(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self / &rhs_sym
            }
        }

        impl std::ops::Div<$sym_type> for $concrete_type {
            type Output = $sym_type;
            fn div(self, rhs: $sym_type) -> Self::Output {
                let lhs_sym = $sym_type::from_concrete(self, std::sync::Arc::clone(&rhs.manager));
                lhs_sym / rhs
            }
        }

        impl std::ops::Div<&$sym_type> for $concrete_type {
            type Output = $sym_type;
            fn div(self, rhs: &$sym_type) -> Self::Output {
                let lhs_sym = $sym_type::from_concrete(self, std::sync::Arc::clone(&rhs.manager));
                &lhs_sym / rhs
            }
        }

        // Rem mixed-type
        impl std::ops::Rem<$concrete_type> for $sym_type {
            type Output = $sym_type;
            fn rem(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self % rhs_sym
            }
        }

        impl std::ops::Rem<$concrete_type> for &$sym_type {
            type Output = $sym_type;
            fn rem(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self % &rhs_sym
            }
        }

        impl std::ops::Rem<$sym_type> for $concrete_type {
            type Output = $sym_type;
            fn rem(self, rhs: $sym_type) -> Self::Output {
                let lhs_sym = $sym_type::from_concrete(self, std::sync::Arc::clone(&rhs.manager));
                lhs_sym % rhs
            }
        }

        impl std::ops::Rem<&$sym_type> for $concrete_type {
            type Output = $sym_type;
            fn rem(self, rhs: &$sym_type) -> Self::Output {
                let lhs_sym = $sym_type::from_concrete(self, std::sync::Arc::clone(&rhs.manager));
                &lhs_sym % rhs
            }
        }

        // BitAnd mixed-type
        impl std::ops::BitAnd<$concrete_type> for $sym_type {
            type Output = $sym_type;
            fn bitand(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self & rhs_sym
            }
        }

        impl std::ops::BitAnd<$concrete_type> for &$sym_type {
            type Output = $sym_type;
            fn bitand(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self & &rhs_sym
            }
        }

        impl std::ops::BitAnd<$sym_type> for $concrete_type {
            type Output = $sym_type;
            fn bitand(self, rhs: $sym_type) -> Self::Output {
                let lhs_sym = $sym_type::from_concrete(self, std::sync::Arc::clone(&rhs.manager));
                lhs_sym & rhs
            }
        }

        impl std::ops::BitAnd<&$sym_type> for $concrete_type {
            type Output = $sym_type;
            fn bitand(self, rhs: &$sym_type) -> Self::Output {
                let lhs_sym = $sym_type::from_concrete(self, std::sync::Arc::clone(&rhs.manager));
                &lhs_sym & rhs
            }
        }

        // BitOr mixed-type
        impl std::ops::BitOr<$concrete_type> for $sym_type {
            type Output = $sym_type;
            fn bitor(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self | rhs_sym
            }
        }

        impl std::ops::BitOr<$concrete_type> for &$sym_type {
            type Output = $sym_type;
            fn bitor(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self | &rhs_sym
            }
        }

        impl std::ops::BitOr<$sym_type> for $concrete_type {
            type Output = $sym_type;
            fn bitor(self, rhs: $sym_type) -> Self::Output {
                let lhs_sym = $sym_type::from_concrete(self, std::sync::Arc::clone(&rhs.manager));
                lhs_sym | rhs
            }
        }

        impl std::ops::BitOr<&$sym_type> for $concrete_type {
            type Output = $sym_type;
            fn bitor(self, rhs: &$sym_type) -> Self::Output {
                let lhs_sym = $sym_type::from_concrete(self, std::sync::Arc::clone(&rhs.manager));
                &lhs_sym | rhs
            }
        }

        // BitXor mixed-type
        impl std::ops::BitXor<$concrete_type> for $sym_type {
            type Output = $sym_type;
            fn bitxor(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self ^ rhs_sym
            }
        }

        impl std::ops::BitXor<$concrete_type> for &$sym_type {
            type Output = $sym_type;
            fn bitxor(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self ^ &rhs_sym
            }
        }

        impl std::ops::BitXor<$sym_type> for $concrete_type {
            type Output = $sym_type;
            fn bitxor(self, rhs: $sym_type) -> Self::Output {
                let lhs_sym = $sym_type::from_concrete(self, std::sync::Arc::clone(&rhs.manager));
                lhs_sym ^ rhs
            }
        }

        impl std::ops::BitXor<&$sym_type> for $concrete_type {
            type Output = $sym_type;
            fn bitxor(self, rhs: &$sym_type) -> Self::Output {
                let lhs_sym = $sym_type::from_concrete(self, std::sync::Arc::clone(&rhs.manager));
                &lhs_sym ^ rhs
            }
        }

        // Shl mixed-type
        impl std::ops::Shl<$concrete_type> for $sym_type {
            type Output = $sym_type;
            fn shl(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self << rhs_sym
            }
        }

        impl std::ops::Shl<$concrete_type> for &$sym_type {
            type Output = $sym_type;
            fn shl(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self << &rhs_sym
            }
        }

        // Shr mixed-type
        impl std::ops::Shr<$concrete_type> for $sym_type {
            type Output = $sym_type;
            fn shr(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self >> rhs_sym
            }
        }

        impl std::ops::Shr<$concrete_type> for &$sym_type {
            type Output = $sym_type;
            fn shr(self, rhs: $concrete_type) -> Self::Output {
                let rhs_sym = $sym_type::from_concrete(rhs, std::sync::Arc::clone(&self.manager));
                self >> &rhs_sym
            }
        }
    };
}
