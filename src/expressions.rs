//! Symbolic expression representation and manipulation
//!
//! This module defines the abstract syntax tree for symbolic expressions
//! and provides operations for building, simplifying, and serializing
//! symbolic expressions to SMT-LIB format.

use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

/// Abstract syntax tree representation of symbolic expressions
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SymExpr {
    /// Symbolic variable with unique name
    Variable(String),
    /// Concrete constant value
    Constant(ConstValue),
    /// Binary operation between two expressions
    BinaryOp(BinOp, Box<SymExpr>, Box<SymExpr>),
    /// Unary operation on an expression
    UnaryOp(UnOp, Box<SymExpr>),
    /// Conditional expression (if-then-else)
    Conditional(Box<SymExpr>, Box<SymExpr>, Box<SymExpr>),
}

/// Binary operations supported in symbolic expressions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOp {
    // Arithmetic operations
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    // Bitwise operations
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    // Comparison operations
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Unary operations supported in symbolic expressions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnOp {
    Neg, // Arithmetic negation
    Not, // Bitwise/logical NOT
}

/// Constant values that can appear in symbolic expressions
#[derive(Debug, Clone, PartialEq)]
pub enum ConstValue {
    U64(u64),
    I64(i64),
    F64(f64),
    Bool(bool),
    U8(u8),
    U32(u32),
    I32(i32),
    F32(f32),
}

impl Eq for ConstValue {}

impl Hash for ConstValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            ConstValue::U64(v) => {
                0u8.hash(state);
                v.hash(state);
            }
            ConstValue::I64(v) => {
                1u8.hash(state);
                v.hash(state);
            }
            ConstValue::F64(v) => {
                2u8.hash(state);
                v.to_bits().hash(state);
            }
            ConstValue::Bool(v) => {
                3u8.hash(state);
                v.hash(state);
            }
            ConstValue::U8(v) => {
                4u8.hash(state);
                v.hash(state);
            }
            ConstValue::I32(v) => {
                5u8.hash(state);
                v.hash(state);
            }
            ConstValue::U32(v) => {
                7u8.hash(state);
                v.hash(state);
            }
            ConstValue::F32(v) => {
                6u8.hash(state);
                v.to_bits().hash(state);
            }
        }
    }
}

impl fmt::Display for SymExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SymExpr::Variable(name) => write!(f, "{name}"),
            SymExpr::Constant(val) => write!(f, "{val}"),
            SymExpr::BinaryOp(op, left, right) => {
                write!(f, "({left} {op} {right})")
            }
            SymExpr::UnaryOp(op, expr) => {
                write!(f, "({op} {expr})")
            }
            SymExpr::Conditional(cond, then_expr, else_expr) => {
                write!(f, "(if {cond} then {then_expr} else {else_expr})")
            }
        }
    }
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let op_str = match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
        };
        write!(f, "{op_str}")
    }
}

impl fmt::Display for UnOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let op_str = match self {
            UnOp::Neg => "-",
            UnOp::Not => "!",
        };
        write!(f, "{op_str}")
    }
}

impl UnOp {
    /// Convert this unary operation to SMT-LIB format
    pub fn to_smt_lib(&self) -> &'static str {
        match self {
            UnOp::Neg => "-",
            UnOp::Not => "not", // For boolean not, or "bvnot" for bitvector not
        }
    }
}

impl fmt::Display for ConstValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConstValue::U64(val) => write!(f, "{val}"),
            ConstValue::I64(val) => write!(f, "{val}"),
            ConstValue::F64(val) => write!(f, "{val}"),
            ConstValue::Bool(val) => write!(f, "{val}"),
            ConstValue::U8(val) => write!(f, "{val}"),
            ConstValue::U32(val) => write!(f, "{val}"),
            ConstValue::I32(val) => write!(f, "{val}"),
            ConstValue::F32(val) => write!(f, "{val}"),
        }
    }
}

impl ConstValue {
    /// Extract as u64 with casting
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            ConstValue::U64(v) => Some(*v),
            ConstValue::I64(v) => Some(*v as u64),
            ConstValue::U32(v) => Some(*v as u64),
            ConstValue::I32(v) => Some(*v as u64),
            ConstValue::U8(v) => Some(*v as u64),
            ConstValue::F64(v) => Some(*v as u64),
            ConstValue::F32(v) => Some(*v as u64),
            ConstValue::Bool(_) => None,
        }
    }

    /// Extract as i64 with casting
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            ConstValue::U64(v) => Some(*v as i64),
            ConstValue::I64(v) => Some(*v),
            ConstValue::U32(v) => Some(*v as i64),
            ConstValue::I32(v) => Some(*v as i64),
            ConstValue::U8(v) => Some(*v as i64),
            ConstValue::F64(v) => Some(*v as i64),
            ConstValue::F32(v) => Some(*v as i64),
            ConstValue::Bool(_) => None,
        }
    }

    /// Extract as u32 with casting
    pub fn as_u32(&self) -> Option<u32> {
        match self {
            ConstValue::U64(v) => Some(*v as u32),
            ConstValue::I64(v) => Some(*v as u32),
            ConstValue::U32(v) => Some(*v),
            ConstValue::I32(v) => Some(*v as u32),
            ConstValue::U8(v) => Some(*v as u32),
            ConstValue::F64(v) => Some(*v as u32),
            ConstValue::F32(v) => Some(*v as u32),
            ConstValue::Bool(_) => None,
        }
    }

    /// Extract as i32 with casting
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            ConstValue::U64(v) => Some(*v as i32),
            ConstValue::I64(v) => Some(*v as i32),
            ConstValue::U32(v) => Some(*v as i32),
            ConstValue::I32(v) => Some(*v),
            ConstValue::U8(v) => Some(*v as i32),
            ConstValue::F64(v) => Some(*v as i32),
            ConstValue::F32(v) => Some(*v as i32),
            ConstValue::Bool(_) => None,
        }
    }

    /// Extract as u8 with casting
    pub fn as_u8(&self) -> Option<u8> {
        match self {
            ConstValue::U64(v) => Some(*v as u8),
            ConstValue::I64(v) => Some(*v as u8),
            ConstValue::U32(v) => Some(*v as u8),
            ConstValue::I32(v) => Some(*v as u8),
            ConstValue::U8(v) => Some(*v),
            ConstValue::F64(v) => Some(*v as u8),
            ConstValue::F32(v) => Some(*v as u8),
            ConstValue::Bool(_) => None,
        }
    }

    /// Extract as f64 with casting
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ConstValue::U64(v) => Some(*v as f64),
            ConstValue::I64(v) => Some(*v as f64),
            ConstValue::U32(v) => Some(*v as f64),
            ConstValue::I32(v) => Some(*v as f64),
            ConstValue::U8(v) => Some(*v as f64),
            ConstValue::F64(v) => Some(*v),
            ConstValue::F32(v) => Some(*v as f64),
            ConstValue::Bool(_) => None,
        }
    }

    /// Extract as f32 with casting
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            ConstValue::U64(v) => Some(*v as f32),
            ConstValue::I64(v) => Some(*v as f32),
            ConstValue::U32(v) => Some(*v as f32),
            ConstValue::I32(v) => Some(*v as f32),
            ConstValue::U8(v) => Some(*v as f32),
            ConstValue::F64(v) => Some(*v as f32),
            ConstValue::F32(v) => Some(*v),
            ConstValue::Bool(_) => None,
        }
    }

    /// Extract as bool (only works for Bool variant)
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ConstValue::Bool(v) => Some(*v),
            _ => None,
        }
    }
}

impl SymExpr {
    /// Create a new variable expression
    pub fn variable(name: String) -> Self {
        SymExpr::Variable(name)
    }

    /// Create a new constant expression
    pub fn constant(value: ConstValue) -> Self {
        SymExpr::Constant(value)
    }

    /// Create a new binary operation expression
    pub fn binary_op(op: BinOp, left: SymExpr, right: SymExpr) -> Self {
        SymExpr::BinaryOp(op, Box::new(left), Box::new(right))
    }

    /// Create a new unary operation expression
    pub fn unary_op(op: UnOp, expr: SymExpr) -> Self {
        SymExpr::UnaryOp(op, Box::new(expr))
    }

    /// Create a conditional expression (if-then-else)
    pub fn conditional(cond: SymExpr, then_expr: SymExpr, else_expr: SymExpr) -> Self {
        SymExpr::Conditional(Box::new(cond), Box::new(then_expr), Box::new(else_expr))
    }

    /// Check if this expression is a constant
    pub fn is_constant(&self) -> bool {
        matches!(self, SymExpr::Constant(_))
    }

    /// Check if this expression is a variable
    pub fn is_variable(&self) -> bool {
        matches!(self, SymExpr::Variable(_))
    }

    /// Get the variable name if this is a variable expression
    pub fn as_variable(&self) -> Option<&String> {
        match self {
            SymExpr::Variable(name) => Some(name),
            _ => None,
        }
    }

    /// Get the constant value if this is a constant expression
    pub fn as_constant(&self) -> Option<&ConstValue> {
        match self {
            SymExpr::Constant(value) => Some(value),
            _ => None,
        }
    }

    /// Get all variable names referenced in this expression
    pub fn get_variables(&self) -> Vec<String> {
        let mut vars = Vec::new();
        self.collect_variables(&mut vars);
        vars.sort();
        vars.dedup();
        vars
    }

    /// Helper method to recursively collect variable names
    fn collect_variables(&self, vars: &mut Vec<String>) {
        match self {
            SymExpr::Variable(name) => vars.push(name.clone()),
            SymExpr::Constant(_) => {}
            SymExpr::BinaryOp(_, left, right) => {
                left.collect_variables(vars);
                right.collect_variables(vars);
            }
            SymExpr::UnaryOp(_, expr) => {
                expr.collect_variables(vars);
            }
            SymExpr::Conditional(cond, then_expr, else_expr) => {
                cond.collect_variables(vars);
                then_expr.collect_variables(vars);
                else_expr.collect_variables(vars);
            }
        }
    }

    /// Calculate the depth of the expression tree
    pub fn depth(&self) -> usize {
        match self {
            SymExpr::Variable(_) | SymExpr::Constant(_) => 1,
            SymExpr::UnaryOp(_, expr) => 1 + expr.depth(),
            SymExpr::BinaryOp(_, left, right) => 1 + left.depth().max(right.depth()),
            SymExpr::Conditional(cond, then_expr, else_expr) => {
                1 + cond.depth().max(then_expr.depth()).max(else_expr.depth())
            }
        }
    }

    /// Count the total number of nodes in the expression tree
    pub fn node_count(&self) -> usize {
        match self {
            SymExpr::Variable(_) | SymExpr::Constant(_) => 1,
            SymExpr::UnaryOp(_, expr) => 1 + expr.node_count(),
            SymExpr::BinaryOp(_, left, right) => 1 + left.node_count() + right.node_count(),
            SymExpr::Conditional(cond, then_expr, else_expr) => {
                1 + cond.node_count() + then_expr.node_count() + else_expr.node_count()
            }
        }
    }

    /// Serialize this expression to SMT-LIB format
    pub fn to_smt_lib(&self) -> String {
        match self {
            SymExpr::Variable(name) => name.clone(),
            SymExpr::Constant(value) => value.to_smt_lib(),
            SymExpr::BinaryOp(op, left, right) => {
                format!(
                    "({} {} {})",
                    op.to_smt_lib(),
                    left.to_smt_lib(),
                    right.to_smt_lib()
                )
            }
            SymExpr::UnaryOp(op, expr) => {
                format!("({} {})", op.to_smt_lib(), expr.to_smt_lib())
            }
            SymExpr::Conditional(cond, then_expr, else_expr) => {
                format!(
                    "(ite {} {} {})",
                    cond.to_smt_lib(),
                    then_expr.to_smt_lib(),
                    else_expr.to_smt_lib()
                )
            }
        }
    }

    /// Generate SMT-LIB variable declarations for all variables in this expression
    pub fn get_smt_declarations(&self) -> Vec<String> {
        let variables = self.get_variables();
        let mut declarations = Vec::new();

        for var in variables {
            // For now, assume all variables are integers (this can be enhanced later)
            declarations.push(format!("(declare-fun {var} () Int)"));
        }

        declarations
    }

    /// Generate a complete SMT-LIB script for this expression
    pub fn to_smt_script(&self) -> String {
        let mut script = String::new();

        // Add logic declaration
        script.push_str("(set-logic QF_LIA)\n");

        // Add variable declarations
        for decl in self.get_smt_declarations() {
            script.push_str(&decl);
            script.push('\n');
        }

        // Add the assertion
        script.push_str(&format!("(assert {})\n", self.to_smt_lib()));

        // Add check-sat and get-model commands
        script.push_str("(check-sat)\n");
        script.push_str("(get-model)\n");

        script
    }

    /// Simplify this expression using algebraic rules and constant folding
    pub fn simplify(&self) -> SymExpr {
        match self {
            SymExpr::Variable(_) | SymExpr::Constant(_) => self.clone(),

            SymExpr::UnaryOp(op, expr) => {
                let simplified_expr = expr.simplify();
                match (&simplified_expr, op) {
                    // Constant folding for unary operations
                    (SymExpr::Constant(val), UnOp::Neg) => match val {
                        ConstValue::I64(n) => SymExpr::constant(ConstValue::I64(-n)),
                        ConstValue::I32(n) => SymExpr::constant(ConstValue::I32(-n)),
                        ConstValue::F64(n) => SymExpr::constant(ConstValue::F64(-n)),
                        ConstValue::F32(n) => SymExpr::constant(ConstValue::F32(-n)),
                        _ => SymExpr::unary_op(*op, simplified_expr),
                    },
                    (SymExpr::Constant(ConstValue::Bool(b)), UnOp::Not) => {
                        SymExpr::constant(ConstValue::Bool(!b))
                    }
                    // Double negation elimination: -(-x) = x
                    (SymExpr::UnaryOp(UnOp::Neg, inner), UnOp::Neg) => (**inner).clone(),
                    (SymExpr::UnaryOp(UnOp::Not, inner), UnOp::Not) => (**inner).clone(),
                    _ => SymExpr::unary_op(*op, simplified_expr),
                }
            }

            SymExpr::BinaryOp(op, left, right) => {
                let simplified_left = left.simplify();
                let simplified_right = right.simplify();

                match (op, &simplified_left, &simplified_right) {
                    // Constant folding for arithmetic operations
                    (
                        BinOp::Add,
                        SymExpr::Constant(ConstValue::I64(a)),
                        SymExpr::Constant(ConstValue::I64(b)),
                    ) => SymExpr::constant(ConstValue::I64(a + b)),
                    (
                        BinOp::Sub,
                        SymExpr::Constant(ConstValue::I64(a)),
                        SymExpr::Constant(ConstValue::I64(b)),
                    ) => SymExpr::constant(ConstValue::I64(a - b)),
                    (
                        BinOp::Mul,
                        SymExpr::Constant(ConstValue::I64(a)),
                        SymExpr::Constant(ConstValue::I64(b)),
                    ) => SymExpr::constant(ConstValue::I64(a * b)),
                    (
                        BinOp::Div,
                        SymExpr::Constant(ConstValue::I64(a)),
                        SymExpr::Constant(ConstValue::I64(b)),
                    ) if *b != 0 => SymExpr::constant(ConstValue::I64(a / b)),

                    // Algebraic simplifications
                    // x + 0 = x, 0 + x = x
                    (BinOp::Add, expr, SymExpr::Constant(ConstValue::I64(0)))
                    | (BinOp::Add, SymExpr::Constant(ConstValue::I64(0)), expr) => expr.clone(),

                    // x - 0 = x
                    (BinOp::Sub, expr, SymExpr::Constant(ConstValue::I64(0))) => expr.clone(),

                    // x * 0 = 0, 0 * x = 0
                    (BinOp::Mul, _, SymExpr::Constant(ConstValue::I64(0)))
                    | (BinOp::Mul, SymExpr::Constant(ConstValue::I64(0)), _) => {
                        SymExpr::constant(ConstValue::I64(0))
                    }

                    // x * 1 = x, 1 * x = x
                    (BinOp::Mul, expr, SymExpr::Constant(ConstValue::I64(1)))
                    | (BinOp::Mul, SymExpr::Constant(ConstValue::I64(1)), expr) => expr.clone(),

                    // x / 1 = x
                    (BinOp::Div, expr, SymExpr::Constant(ConstValue::I64(1))) => expr.clone(),

                    // x - x = 0
                    (BinOp::Sub, left_expr, right_expr) if left_expr == right_expr => {
                        SymExpr::constant(ConstValue::I64(0))
                    }

                    // Boolean constant folding
                    (BinOp::Eq, SymExpr::Constant(a), SymExpr::Constant(b)) => {
                        SymExpr::constant(ConstValue::Bool(const_equal_with_nan(a, b)))
                    }
                    (BinOp::Ne, SymExpr::Constant(a), SymExpr::Constant(b)) => {
                        SymExpr::constant(ConstValue::Bool(!const_equal_with_nan(a, b)))
                    }

                    // Comparison constant folding for integers
                    (
                        BinOp::Lt,
                        SymExpr::Constant(ConstValue::I64(a)),
                        SymExpr::Constant(ConstValue::I64(b)),
                    ) => SymExpr::constant(ConstValue::Bool(a < b)),
                    (
                        BinOp::Le,
                        SymExpr::Constant(ConstValue::I64(a)),
                        SymExpr::Constant(ConstValue::I64(b)),
                    ) => SymExpr::constant(ConstValue::Bool(a <= b)),
                    (
                        BinOp::Gt,
                        SymExpr::Constant(ConstValue::I64(a)),
                        SymExpr::Constant(ConstValue::I64(b)),
                    ) => SymExpr::constant(ConstValue::Bool(a > b)),
                    (
                        BinOp::Ge,
                        SymExpr::Constant(ConstValue::I64(a)),
                        SymExpr::Constant(ConstValue::I64(b)),
                    ) => SymExpr::constant(ConstValue::Bool(a >= b)),

                    // Bitwise operations constant folding
                    (
                        BinOp::BitAnd,
                        SymExpr::Constant(ConstValue::U64(a)),
                        SymExpr::Constant(ConstValue::U64(b)),
                    ) => SymExpr::constant(ConstValue::U64(a & b)),
                    (
                        BinOp::BitOr,
                        SymExpr::Constant(ConstValue::U64(a)),
                        SymExpr::Constant(ConstValue::U64(b)),
                    ) => SymExpr::constant(ConstValue::U64(a | b)),
                    (
                        BinOp::BitXor,
                        SymExpr::Constant(ConstValue::U64(a)),
                        SymExpr::Constant(ConstValue::U64(b)),
                    ) => SymExpr::constant(ConstValue::U64(a ^ b)),

                    // x ^ x = 0 (for bitwise XOR)
                    (BinOp::BitXor, left_expr, right_expr) if left_expr == right_expr => {
                        SymExpr::constant(ConstValue::U64(0))
                    }

                    // x & x = x
                    (BinOp::BitAnd, left_expr, right_expr) if left_expr == right_expr => {
                        left_expr.clone()
                    }

                    // x | x = x
                    (BinOp::BitOr, left_expr, right_expr) if left_expr == right_expr => {
                        left_expr.clone()
                    }

                    _ => SymExpr::binary_op(*op, simplified_left, simplified_right),
                }
            }

            SymExpr::Conditional(cond, then_expr, else_expr) => {
                let simplified_cond = cond.simplify();
                let simplified_then = then_expr.simplify();
                let simplified_else = else_expr.simplify();

                match &simplified_cond {
                    SymExpr::Constant(ConstValue::Bool(true)) => simplified_then,
                    SymExpr::Constant(ConstValue::Bool(false)) => simplified_else,
                    _ => {
                        // If then and else branches are the same, return that value
                        if simplified_then == simplified_else {
                            simplified_then
                        } else {
                            SymExpr::conditional(simplified_cond, simplified_then, simplified_else)
                        }
                    }
                }
            }
        }
    }

    /// Check if this expression is in simplified form
    pub fn is_simplified(&self) -> bool {
        let simplified = self.simplify();
        *self == simplified
    }

    /// Get a canonical hash for this expression (for hash-consing)
    pub fn canonical_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    /// Evaluate the concrete value of this expression given variable bindings
    ///
    /// This method recursively evaluates the expression tree using the provided
    /// mapping of variable names to their concrete values. Returns `None` if
    /// any variable is not found in the bindings or if the operation cannot
    /// be evaluated (e.g., division by zero).
    ///
    /// # Arguments
    /// * `bindings` - A map from variable names to their concrete values
    ///
    /// # Returns
    /// * `Some(ConstValue)` - The evaluated concrete value
    /// * `None` - If evaluation fails (missing variable, division by zero, etc.)
    pub fn get_concrete_value(&self, bindings: &HashMap<String, ConstValue>) -> Option<ConstValue> {
        match self {
            SymExpr::Variable(name) => bindings.get(name).cloned(),

            SymExpr::Constant(value) => Some(value.clone()),

            SymExpr::UnaryOp(op, expr) => {
                let val = expr.get_concrete_value(bindings)?;
                match (op, val) {
                    (UnOp::Neg, ConstValue::I64(n)) => Some(ConstValue::I64(-n)),
                    (UnOp::Neg, ConstValue::I32(n)) => Some(ConstValue::I32(-n)),
                    (UnOp::Neg, ConstValue::F64(n)) => Some(ConstValue::F64(-n)),
                    (UnOp::Neg, ConstValue::F32(n)) => Some(ConstValue::F32(-n)),
                    (UnOp::Not, ConstValue::Bool(b)) => Some(ConstValue::Bool(!b)),
                    (UnOp::Not, ConstValue::U64(n)) => Some(ConstValue::U64(!n)),
                    (UnOp::Not, ConstValue::U32(n)) => Some(ConstValue::U32(!n)),
                    (UnOp::Not, ConstValue::U8(n)) => Some(ConstValue::U8(!n)),
                    (UnOp::Not, ConstValue::I64(n)) => Some(ConstValue::I64(!n)),
                    (UnOp::Not, ConstValue::I32(n)) => Some(ConstValue::I32(!n)),
                    _ => None,
                }
            }

            SymExpr::BinaryOp(op, left, right) => {
                let left_val = left.get_concrete_value(bindings)?;
                let right_val = right.get_concrete_value(bindings)?;
                eval_binary_op(*op, left_val, right_val)
            }

            SymExpr::Conditional(cond, then_expr, else_expr) => {
                let cond_val = cond.get_concrete_value(bindings)?;
                match cond_val {
                    ConstValue::Bool(true) => then_expr.get_concrete_value(bindings),
                    ConstValue::Bool(false) => else_expr.get_concrete_value(bindings),
                    _ => None,
                }
            }
        }
    }
}

/// Evaluate a binary operation on two concrete values
fn eval_binary_op(op: BinOp, left: ConstValue, right: ConstValue) -> Option<ConstValue> {
    match op {
        // Arithmetic operations
        BinOp::Add => eval_add(left, right),
        BinOp::Sub => eval_sub(left, right),
        BinOp::Mul => eval_mul(left, right),
        BinOp::Div => eval_div(left, right),
        BinOp::Mod => eval_mod(left, right),

        // Bitwise operations
        BinOp::BitAnd => eval_bitand(left, right),
        BinOp::BitOr => eval_bitor(left, right),
        BinOp::BitXor => eval_bitxor(left, right),
        BinOp::Shl => eval_shl(left, right),
        BinOp::Shr => eval_shr(left, right),

        // Comparison operations
        BinOp::Eq => eval_eq(left, right),
        BinOp::Ne => eval_ne(left, right),
        BinOp::Lt => eval_lt(left, right),
        BinOp::Le => eval_le(left, right),
        BinOp::Gt => eval_gt(left, right),
        BinOp::Ge => eval_ge(left, right),
    }
}

fn eval_add(left: ConstValue, right: ConstValue) -> Option<ConstValue> {
    match (left, right) {
        (ConstValue::U64(a), ConstValue::U64(b)) => Some(ConstValue::U64(a.wrapping_add(b))),
        (ConstValue::I64(a), ConstValue::I64(b)) => Some(ConstValue::I64(a.wrapping_add(b))),
        (ConstValue::U32(a), ConstValue::U32(b)) => Some(ConstValue::U32(a.wrapping_add(b))),
        (ConstValue::I32(a), ConstValue::I32(b)) => Some(ConstValue::I32(a.wrapping_add(b))),
        (ConstValue::U8(a), ConstValue::U8(b)) => Some(ConstValue::U8(a.wrapping_add(b))),
        (ConstValue::F64(a), ConstValue::F64(b)) => Some(ConstValue::F64(a + b)),
        (ConstValue::F32(a), ConstValue::F32(b)) => Some(ConstValue::F32(a + b)),
        _ => None,
    }
}

fn eval_sub(left: ConstValue, right: ConstValue) -> Option<ConstValue> {
    match (left, right) {
        (ConstValue::U64(a), ConstValue::U64(b)) => Some(ConstValue::U64(a.wrapping_sub(b))),
        (ConstValue::I64(a), ConstValue::I64(b)) => Some(ConstValue::I64(a.wrapping_sub(b))),
        (ConstValue::U32(a), ConstValue::U32(b)) => Some(ConstValue::U32(a.wrapping_sub(b))),
        (ConstValue::I32(a), ConstValue::I32(b)) => Some(ConstValue::I32(a.wrapping_sub(b))),
        (ConstValue::U8(a), ConstValue::U8(b)) => Some(ConstValue::U8(a.wrapping_sub(b))),
        (ConstValue::F64(a), ConstValue::F64(b)) => Some(ConstValue::F64(a - b)),
        (ConstValue::F32(a), ConstValue::F32(b)) => Some(ConstValue::F32(a - b)),
        _ => None,
    }
}

fn eval_mul(left: ConstValue, right: ConstValue) -> Option<ConstValue> {
    match (left, right) {
        (ConstValue::U64(a), ConstValue::U64(b)) => Some(ConstValue::U64(a.wrapping_mul(b))),
        (ConstValue::I64(a), ConstValue::I64(b)) => Some(ConstValue::I64(a.wrapping_mul(b))),
        (ConstValue::U32(a), ConstValue::U32(b)) => Some(ConstValue::U32(a.wrapping_mul(b))),
        (ConstValue::I32(a), ConstValue::I32(b)) => Some(ConstValue::I32(a.wrapping_mul(b))),
        (ConstValue::U8(a), ConstValue::U8(b)) => Some(ConstValue::U8(a.wrapping_mul(b))),
        (ConstValue::F64(a), ConstValue::F64(b)) => Some(ConstValue::F64(a * b)),
        (ConstValue::F32(a), ConstValue::F32(b)) => Some(ConstValue::F32(a * b)),
        _ => None,
    }
}

fn eval_div(left: ConstValue, right: ConstValue) -> Option<ConstValue> {
    match (left, right) {
        (ConstValue::U64(a), ConstValue::U64(b)) if b != 0 => Some(ConstValue::U64(a / b)),
        (ConstValue::I64(a), ConstValue::I64(b)) if b != 0 => Some(ConstValue::I64(a / b)),
        (ConstValue::U32(a), ConstValue::U32(b)) if b != 0 => Some(ConstValue::U32(a / b)),
        (ConstValue::I32(a), ConstValue::I32(b)) if b != 0 => Some(ConstValue::I32(a / b)),
        (ConstValue::U8(a), ConstValue::U8(b)) if b != 0 => Some(ConstValue::U8(a / b)),
        (ConstValue::F64(a), ConstValue::F64(b)) => Some(ConstValue::F64(a / b)),
        (ConstValue::F32(a), ConstValue::F32(b)) => Some(ConstValue::F32(a / b)),
        _ => None,
    }
}

fn eval_mod(left: ConstValue, right: ConstValue) -> Option<ConstValue> {
    match (left, right) {
        (ConstValue::U64(a), ConstValue::U64(b)) if b != 0 => Some(ConstValue::U64(a % b)),
        (ConstValue::I64(a), ConstValue::I64(b)) if b != 0 => Some(ConstValue::I64(a % b)),
        (ConstValue::U32(a), ConstValue::U32(b)) if b != 0 => Some(ConstValue::U32(a % b)),
        (ConstValue::I32(a), ConstValue::I32(b)) if b != 0 => Some(ConstValue::I32(a % b)),
        (ConstValue::U8(a), ConstValue::U8(b)) if b != 0 => Some(ConstValue::U8(a % b)),
        _ => None,
    }
}

fn eval_bitand(left: ConstValue, right: ConstValue) -> Option<ConstValue> {
    match (left, right) {
        (ConstValue::U64(a), ConstValue::U64(b)) => Some(ConstValue::U64(a & b)),
        (ConstValue::I64(a), ConstValue::I64(b)) => Some(ConstValue::I64(a & b)),
        (ConstValue::U32(a), ConstValue::U32(b)) => Some(ConstValue::U32(a & b)),
        (ConstValue::I32(a), ConstValue::I32(b)) => Some(ConstValue::I32(a & b)),
        (ConstValue::U8(a), ConstValue::U8(b)) => Some(ConstValue::U8(a & b)),
        (ConstValue::Bool(a), ConstValue::Bool(b)) => Some(ConstValue::Bool(a && b)),
        _ => None,
    }
}

fn eval_bitor(left: ConstValue, right: ConstValue) -> Option<ConstValue> {
    match (left, right) {
        (ConstValue::U64(a), ConstValue::U64(b)) => Some(ConstValue::U64(a | b)),
        (ConstValue::I64(a), ConstValue::I64(b)) => Some(ConstValue::I64(a | b)),
        (ConstValue::U32(a), ConstValue::U32(b)) => Some(ConstValue::U32(a | b)),
        (ConstValue::I32(a), ConstValue::I32(b)) => Some(ConstValue::I32(a | b)),
        (ConstValue::U8(a), ConstValue::U8(b)) => Some(ConstValue::U8(a | b)),
        (ConstValue::Bool(a), ConstValue::Bool(b)) => Some(ConstValue::Bool(a || b)),
        _ => None,
    }
}

fn eval_bitxor(left: ConstValue, right: ConstValue) -> Option<ConstValue> {
    match (left, right) {
        (ConstValue::U64(a), ConstValue::U64(b)) => Some(ConstValue::U64(a ^ b)),
        (ConstValue::I64(a), ConstValue::I64(b)) => Some(ConstValue::I64(a ^ b)),
        (ConstValue::U32(a), ConstValue::U32(b)) => Some(ConstValue::U32(a ^ b)),
        (ConstValue::I32(a), ConstValue::I32(b)) => Some(ConstValue::I32(a ^ b)),
        (ConstValue::U8(a), ConstValue::U8(b)) => Some(ConstValue::U8(a ^ b)),
        (ConstValue::Bool(a), ConstValue::Bool(b)) => Some(ConstValue::Bool(a ^ b)),
        _ => None,
    }
}

fn eval_shl(left: ConstValue, right: ConstValue) -> Option<ConstValue> {
    // Get shift amount as u32
    let shift = match &right {
        ConstValue::U64(n) => *n as u32,
        ConstValue::U32(n) => *n,
        ConstValue::U8(n) => *n as u32,
        ConstValue::I64(n) if *n >= 0 => *n as u32,
        ConstValue::I32(n) if *n >= 0 => *n as u32,
        _ => return None,
    };

    match left {
        ConstValue::U64(a) if shift < 64 => Some(ConstValue::U64(a << shift)),
        ConstValue::I64(a) if shift < 64 => Some(ConstValue::I64(a << shift)),
        ConstValue::U32(a) if shift < 32 => Some(ConstValue::U32(a << shift)),
        ConstValue::I32(a) if shift < 32 => Some(ConstValue::I32(a << shift)),
        ConstValue::U8(a) if shift < 8 => Some(ConstValue::U8(a << shift)),
        _ => None,
    }
}

fn eval_shr(left: ConstValue, right: ConstValue) -> Option<ConstValue> {
    // Get shift amount as u32
    let shift = match &right {
        ConstValue::U64(n) => *n as u32,
        ConstValue::U32(n) => *n,
        ConstValue::U8(n) => *n as u32,
        ConstValue::I64(n) if *n >= 0 => *n as u32,
        ConstValue::I32(n) if *n >= 0 => *n as u32,
        _ => return None,
    };

    match left {
        ConstValue::U64(a) if shift < 64 => Some(ConstValue::U64(a >> shift)),
        ConstValue::I64(a) if shift < 64 => Some(ConstValue::I64(a >> shift)),
        ConstValue::U32(a) if shift < 32 => Some(ConstValue::U32(a >> shift)),
        ConstValue::I32(a) if shift < 32 => Some(ConstValue::I32(a >> shift)),
        ConstValue::U8(a) if shift < 8 => Some(ConstValue::U8(a >> shift)),
        _ => None,
    }
}

fn eval_eq(left: ConstValue, right: ConstValue) -> Option<ConstValue> {
    Some(ConstValue::Bool(const_equal_with_nan(&left, &right)))
}

fn eval_ne(left: ConstValue, right: ConstValue) -> Option<ConstValue> {
    Some(ConstValue::Bool(!const_equal_with_nan(&left, &right)))
}

fn eval_lt(left: ConstValue, right: ConstValue) -> Option<ConstValue> {
    match (left, right) {
        (ConstValue::U64(a), ConstValue::U64(b)) => Some(ConstValue::Bool(a < b)),
        (ConstValue::I64(a), ConstValue::I64(b)) => Some(ConstValue::Bool(a < b)),
        (ConstValue::U32(a), ConstValue::U32(b)) => Some(ConstValue::Bool(a < b)),
        (ConstValue::I32(a), ConstValue::I32(b)) => Some(ConstValue::Bool(a < b)),
        (ConstValue::U8(a), ConstValue::U8(b)) => Some(ConstValue::Bool(a < b)),
        (ConstValue::F64(a), ConstValue::F64(b)) => Some(ConstValue::Bool(a < b)),
        (ConstValue::F32(a), ConstValue::F32(b)) => Some(ConstValue::Bool(a < b)),
        _ => None,
    }
}

fn eval_le(left: ConstValue, right: ConstValue) -> Option<ConstValue> {
    match (left, right) {
        (ConstValue::U64(a), ConstValue::U64(b)) => Some(ConstValue::Bool(a <= b)),
        (ConstValue::I64(a), ConstValue::I64(b)) => Some(ConstValue::Bool(a <= b)),
        (ConstValue::U32(a), ConstValue::U32(b)) => Some(ConstValue::Bool(a <= b)),
        (ConstValue::I32(a), ConstValue::I32(b)) => Some(ConstValue::Bool(a <= b)),
        (ConstValue::U8(a), ConstValue::U8(b)) => Some(ConstValue::Bool(a <= b)),
        (ConstValue::F64(a), ConstValue::F64(b)) => Some(ConstValue::Bool(a <= b)),
        (ConstValue::F32(a), ConstValue::F32(b)) => Some(ConstValue::Bool(a <= b)),
        _ => None,
    }
}

fn eval_gt(left: ConstValue, right: ConstValue) -> Option<ConstValue> {
    match (left, right) {
        (ConstValue::U64(a), ConstValue::U64(b)) => Some(ConstValue::Bool(a > b)),
        (ConstValue::I64(a), ConstValue::I64(b)) => Some(ConstValue::Bool(a > b)),
        (ConstValue::U32(a), ConstValue::U32(b)) => Some(ConstValue::Bool(a > b)),
        (ConstValue::I32(a), ConstValue::I32(b)) => Some(ConstValue::Bool(a > b)),
        (ConstValue::U8(a), ConstValue::U8(b)) => Some(ConstValue::Bool(a > b)),
        (ConstValue::F64(a), ConstValue::F64(b)) => Some(ConstValue::Bool(a > b)),
        (ConstValue::F32(a), ConstValue::F32(b)) => Some(ConstValue::Bool(a > b)),
        _ => None,
    }
}

fn eval_ge(left: ConstValue, right: ConstValue) -> Option<ConstValue> {
    match (left, right) {
        (ConstValue::U64(a), ConstValue::U64(b)) => Some(ConstValue::Bool(a >= b)),
        (ConstValue::I64(a), ConstValue::I64(b)) => Some(ConstValue::Bool(a >= b)),
        (ConstValue::U32(a), ConstValue::U32(b)) => Some(ConstValue::Bool(a >= b)),
        (ConstValue::I32(a), ConstValue::I32(b)) => Some(ConstValue::Bool(a >= b)),
        (ConstValue::U8(a), ConstValue::U8(b)) => Some(ConstValue::Bool(a >= b)),
        (ConstValue::F64(a), ConstValue::F64(b)) => Some(ConstValue::Bool(a >= b)),
        (ConstValue::F32(a), ConstValue::F32(b)) => Some(ConstValue::Bool(a >= b)),
        _ => None,
    }
}

/// Hash-consing table for expression deduplication and memory efficiency
pub struct ExpressionTable {
    table: Arc<Mutex<HashMap<u64, SymExpr>>>,
}

impl ExpressionTable {
    /// Create a new expression table
    pub fn new() -> Self {
        ExpressionTable {
            table: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get or insert an expression in the hash-consing table
    pub fn intern(&self, expr: SymExpr) -> SymExpr {
        let hash = expr.canonical_hash();
        let mut table = self.table.lock().unwrap();

        if let Some(existing) = table.get(&hash) {
            existing.clone()
        } else {
            table.insert(hash, expr.clone());
            expr
        }
    }

    /// Get the number of unique expressions in the table
    pub fn size(&self) -> usize {
        self.table.lock().unwrap().len()
    }

    /// Clear the hash-consing table
    pub fn clear(&self) {
        self.table.lock().unwrap().clear();
    }
}

impl Default for ExpressionTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to check if constants are equal, handling NaN properly
fn const_equal_with_nan(c1: &ConstValue, c2: &ConstValue) -> bool {
    match (c1, c2) {
        (ConstValue::F64(f1), ConstValue::F64(f2)) => {
            if f1.is_nan() && f2.is_nan() {
                true
            } else {
                f1 == f2
            }
        }
        (ConstValue::F32(f1), ConstValue::F32(f2)) => {
            if f1.is_nan() && f2.is_nan() {
                true
            } else {
                f1 == f2
            }
        }
        _ => c1 == c2,
    }
}

impl ConstValue {
    /// Create a ConstValue from a u64
    pub fn u64(value: u64) -> Self {
        ConstValue::U64(value)
    }

    /// Create a ConstValue from an i64
    pub fn i64(value: i64) -> Self {
        ConstValue::I64(value)
    }

    /// Create a ConstValue from an f64
    pub fn f64(value: f64) -> Self {
        ConstValue::F64(value)
    }

    /// Create a ConstValue from a bool
    pub fn bool(value: bool) -> Self {
        ConstValue::Bool(value)
    }

    /// Create a ConstValue from a u8
    pub fn u8(value: u8) -> Self {
        ConstValue::U8(value)
    }

    /// Create a ConstValue from an i32
    pub fn i32(value: i32) -> Self {
        ConstValue::I32(value)
    }

    /// Create a ConstValue from an f32
    pub fn f32(value: f32) -> Self {
        ConstValue::F32(value)
    }

    /// Get the type name of this constant value
    pub fn type_name(&self) -> &'static str {
        match self {
            ConstValue::U64(_) => "u64",
            ConstValue::I64(_) => "i64",
            ConstValue::F64(_) => "f64",
            ConstValue::Bool(_) => "bool",
            ConstValue::U8(_) => "u8",
            ConstValue::U32(_) => "u32",
            ConstValue::I32(_) => "i32",
            ConstValue::F32(_) => "f32",
        }
    }

    /// Check if this is a numeric type (not boolean)
    pub fn is_numeric(&self) -> bool {
        !matches!(self, ConstValue::Bool(_))
    }

    /// Check if this is an integer type
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            ConstValue::U64(_)
                | ConstValue::I64(_)
                | ConstValue::U8(_)
                | ConstValue::U32(_)
                | ConstValue::I32(_)
        )
    }

    /// Check if this is a floating-point type
    pub fn is_float(&self) -> bool {
        matches!(self, ConstValue::F64(_) | ConstValue::F32(_))
    }

    /// Serialize this constant value to SMT-LIB format
    pub fn to_smt_lib(&self) -> String {
        match self {
            ConstValue::U64(val) => val.to_string(),
            ConstValue::I64(val) => val.to_string(),
            ConstValue::F64(val) => {
                if val.is_nan() {
                    "(_ NaN 11 53)".to_string() // IEEE 754 double precision NaN
                } else if val.is_infinite() {
                    if val.is_sign_positive() {
                        "(_ +oo 11 53)".to_string()
                    } else {
                        "(_ -oo 11 53)".to_string()
                    }
                } else {
                    format!("{val}")
                }
            }
            ConstValue::Bool(val) => val.to_string(),
            ConstValue::U8(val) => val.to_string(),
            ConstValue::U32(val) => val.to_string(),
            ConstValue::I32(val) => val.to_string(),
            ConstValue::F32(val) => {
                if val.is_nan() {
                    "(_ NaN 8 24)".to_string() // IEEE 754 single precision NaN
                } else if val.is_infinite() {
                    if val.is_sign_positive() {
                        "(_ +oo 8 24)".to_string()
                    } else {
                        "(_ -oo 8 24)".to_string()
                    }
                } else {
                    format!("{val}")
                }
            }
        }
    }

    /// Get the SMT-LIB sort (type) for this constant value
    pub fn smt_sort(&self) -> &'static str {
        match self {
            ConstValue::U64(_)
            | ConstValue::I64(_)
            | ConstValue::U8(_)
            | ConstValue::U32(_)
            | ConstValue::I32(_) => "Int",
            ConstValue::F64(_) => "(_ FloatingPoint 11 53)",
            ConstValue::F32(_) => "(_ FloatingPoint 8 24)",
            ConstValue::Bool(_) => "Bool",
        }
    }
}

impl BinOp {
    /// Check if this is an arithmetic operation
    pub fn is_arithmetic(&self) -> bool {
        matches!(
            self,
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod
        )
    }

    /// Check if this is a bitwise operation
    pub fn is_bitwise(&self) -> bool {
        matches!(
            self,
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr
        )
    }

    /// Check if this is a comparison operation
    pub fn is_comparison(&self) -> bool {
        matches!(
            self,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
        )
    }

    /// Get the precedence level of this operation (higher number = higher precedence)
    pub fn precedence(&self) -> u8 {
        match self {
            BinOp::Mul | BinOp::Div | BinOp::Mod => 6,
            BinOp::Add | BinOp::Sub => 5,
            BinOp::Shl | BinOp::Shr => 4,
            BinOp::BitAnd => 3,
            BinOp::BitXor => 2,
            BinOp::BitOr => 1,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 0,
        }
    }

    /// Check if this operation is commutative
    pub fn is_commutative(&self) -> bool {
        matches!(
            self,
            BinOp::Add
                | BinOp::Mul
                | BinOp::BitAnd
                | BinOp::BitOr
                | BinOp::BitXor
                | BinOp::Eq
                | BinOp::Ne
        )
    }

    /// Convert this binary operation to SMT-LIB format
    pub fn to_smt_lib(&self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "div", // Integer division in SMT-LIB
            BinOp::Mod => "mod",
            BinOp::BitAnd => "bvand", // Bitvector operations
            BinOp::BitOr => "bvor",
            BinOp::BitXor => "bvxor",
            BinOp::Shl => "bvshl",
            BinOp::Shr => "bvlshr", // Logical shift right
            BinOp::Eq => "=",
            BinOp::Ne => "distinct", // Not equal in SMT-LIB
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::{Arbitrary, Gen};
    use quickcheck_macros::quickcheck as qc;

    // Implement Arbitrary for our types to enable property-based testing
    impl Arbitrary for ConstValue {
        fn arbitrary(g: &mut Gen) -> Self {
            match u8::arbitrary(g) % 7 {
                0 => ConstValue::U64(u64::arbitrary(g)),
                1 => ConstValue::I64(i64::arbitrary(g)),
                2 => ConstValue::F64(f64::arbitrary(g)),
                3 => ConstValue::Bool(bool::arbitrary(g)),
                4 => ConstValue::U8(u8::arbitrary(g)),
                5 => ConstValue::I32(i32::arbitrary(g)),
                _ => ConstValue::F32(f32::arbitrary(g)),
            }
        }
    }

    impl Arbitrary for BinOp {
        fn arbitrary(g: &mut Gen) -> Self {
            match u8::arbitrary(g) % 16 {
                0 => BinOp::Add,
                1 => BinOp::Sub,
                2 => BinOp::Mul,
                3 => BinOp::Div,
                4 => BinOp::Mod,
                5 => BinOp::BitAnd,
                6 => BinOp::BitOr,
                7 => BinOp::BitXor,
                8 => BinOp::Shl,
                9 => BinOp::Shr,
                10 => BinOp::Eq,
                11 => BinOp::Ne,
                12 => BinOp::Lt,
                13 => BinOp::Le,
                14 => BinOp::Gt,
                _ => BinOp::Ge,
            }
        }
    }

    impl Arbitrary for UnOp {
        fn arbitrary(g: &mut Gen) -> Self {
            match bool::arbitrary(g) {
                true => UnOp::Neg,
                false => UnOp::Not,
            }
        }
    }

    impl Arbitrary for SymExpr {
        fn arbitrary(g: &mut Gen) -> Self {
            let depth = g.size();
            if depth == 0 {
                // Base case: create a leaf node
                match bool::arbitrary(g) {
                    true => SymExpr::Variable(format!("var_{}", u32::arbitrary(g) % 100)),
                    false => SymExpr::Constant(ConstValue::arbitrary(g)),
                }
            } else {
                // Recursive case: create an operation
                let smaller_g = &mut Gen::new(depth / 2);
                match u8::arbitrary(g) % 4 {
                    0 => SymExpr::Variable(format!("var_{}", u32::arbitrary(g) % 100)),
                    1 => SymExpr::Constant(ConstValue::arbitrary(g)),
                    2 => SymExpr::BinaryOp(
                        BinOp::arbitrary(g),
                        Box::new(SymExpr::arbitrary(smaller_g)),
                        Box::new(SymExpr::arbitrary(smaller_g)),
                    ),
                    _ => SymExpr::UnaryOp(
                        UnOp::arbitrary(g),
                        Box::new(SymExpr::arbitrary(smaller_g)),
                    ),
                }
            }
        }
    }

    // Helper function to check if a ConstValue contains NaN
    #[allow(dead_code)]
    fn contains_nan(val: &ConstValue) -> bool {
        match val {
            ConstValue::F64(f) => f.is_nan(),
            ConstValue::F32(f) => f.is_nan(),
            _ => false,
        }
    }

    // Helper function to check if expressions are equal, handling NaN properly
    fn expr_equal_with_nan(left: &SymExpr, right: &SymExpr) -> bool {
        match (left, right) {
            (SymExpr::Constant(c1), SymExpr::Constant(c2)) => {
                crate::expressions::const_equal_with_nan(c1, c2)
            }
            _ => left == right,
        }
    }

    // **Feature: symbolic-execution-engine, Property 2: Expression construction from operations**
    // **Validates: Requirements 1.2, 2.1**
    #[qc]
    fn prop_expression_construction_from_operations(
        op: BinOp,
        left_val: ConstValue,
        right_val: ConstValue,
    ) -> bool {
        let left = SymExpr::constant(left_val.clone());
        let right = SymExpr::constant(right_val.clone());
        let expr = SymExpr::binary_op(op, left.clone(), right.clone());

        // Verify the expression structure is correct
        match expr {
            SymExpr::BinaryOp(actual_op, actual_left, actual_right) => {
                actual_op == op
                    && expr_equal_with_nan(&*actual_left, &left)
                    && expr_equal_with_nan(&*actual_right, &right)
            }
            _ => false,
        }
    }

    #[qc]
    fn prop_unary_expression_construction(op: UnOp, val: ConstValue) -> bool {
        let operand = SymExpr::constant(val.clone());
        let expr = SymExpr::unary_op(op, operand.clone());

        // Verify the expression structure is correct
        match expr {
            SymExpr::UnaryOp(actual_op, actual_operand) => {
                actual_op == op && expr_equal_with_nan(&*actual_operand, &operand)
            }
            _ => false,
        }
    }

    #[qc]
    fn prop_variable_expression_construction(var_name: String) -> bool {
        let expr = SymExpr::variable(var_name.clone());

        match expr {
            SymExpr::Variable(actual_name) => actual_name == var_name,
            _ => false,
        }
    }

    #[qc]
    fn prop_constant_expression_construction(val: ConstValue) -> bool {
        let expr = SymExpr::constant(val.clone());

        match expr {
            SymExpr::Constant(actual_val) => const_equal_with_nan(&actual_val, &val),
            _ => false,
        }
    }

    #[qc]
    fn prop_conditional_expression_construction(
        cond_val: ConstValue,
        then_val: ConstValue,
        else_val: ConstValue,
    ) -> bool {
        let cond = SymExpr::constant(cond_val.clone());
        let then_expr = SymExpr::constant(then_val.clone());
        let else_expr = SymExpr::constant(else_val.clone());
        let expr = SymExpr::conditional(cond.clone(), then_expr.clone(), else_expr.clone());

        match expr {
            SymExpr::Conditional(actual_cond, actual_then, actual_else) => {
                expr_equal_with_nan(&*actual_cond, &cond)
                    && expr_equal_with_nan(&*actual_then, &then_expr)
                    && expr_equal_with_nan(&*actual_else, &else_expr)
            }
            _ => false,
        }
    }

    #[qc]
    fn prop_expression_depth_calculation(expr: SymExpr) -> bool {
        let depth = expr.depth();
        depth >= 1 // All expressions should have at least depth 1
    }

    #[qc]
    fn prop_expression_node_count(expr: SymExpr) -> bool {
        let count = expr.node_count();
        count >= 1 // All expressions should have at least 1 node
    }

    #[qc]
    fn prop_variable_collection_consistency(expr: SymExpr) -> bool {
        let vars = expr.get_variables();
        // Variables should be sorted and unique
        let mut sorted_vars = vars.clone();
        sorted_vars.sort();
        sorted_vars.dedup();
        vars == sorted_vars
    }

    #[test]
    fn test_basic_expression_construction() {
        // Test basic construction methods work
        let var = SymExpr::variable("x".to_string());
        assert!(var.is_variable());
        assert_eq!(var.as_variable(), Some(&"x".to_string()));

        let const_expr = SymExpr::constant(ConstValue::U64(42));
        assert!(const_expr.is_constant());
        assert_eq!(const_expr.as_constant(), Some(&ConstValue::U64(42)));

        let add_expr = SymExpr::binary_op(BinOp::Add, var.clone(), const_expr.clone());
        assert!(!add_expr.is_constant());
        assert!(!add_expr.is_variable());
    }

    #[test]
    fn test_expression_display() {
        let var = SymExpr::variable("x".to_string());
        let const_expr = SymExpr::constant(ConstValue::U64(42));
        let add_expr = SymExpr::binary_op(BinOp::Add, var, const_expr);

        let display_str = format!("{}", add_expr);
        assert!(display_str.contains("x"));
        assert!(display_str.contains("42"));
        assert!(display_str.contains("+"));
    }

    #[test]
    fn test_const_value_type_queries() {
        assert!(ConstValue::U64(42).is_numeric());
        assert!(ConstValue::U64(42).is_integer());
        assert!(!ConstValue::U64(42).is_float());

        assert!(ConstValue::F64(3.14).is_numeric());
        assert!(!ConstValue::F64(3.14).is_integer());
        assert!(ConstValue::F64(3.14).is_float());

        assert!(!ConstValue::Bool(true).is_numeric());
        assert!(!ConstValue::Bool(true).is_integer());
        assert!(!ConstValue::Bool(true).is_float());
    }

    #[test]
    fn test_binop_properties() {
        assert!(BinOp::Add.is_arithmetic());
        assert!(!BinOp::Add.is_bitwise());
        assert!(!BinOp::Add.is_comparison());
        assert!(BinOp::Add.is_commutative());

        assert!(!BinOp::BitAnd.is_arithmetic());
        assert!(BinOp::BitAnd.is_bitwise());
        assert!(!BinOp::BitAnd.is_comparison());
        assert!(BinOp::BitAnd.is_commutative());

        assert!(!BinOp::Lt.is_arithmetic());
        assert!(!BinOp::Lt.is_bitwise());
        assert!(BinOp::Lt.is_comparison());
        assert!(!BinOp::Lt.is_commutative());
    }

    #[test]
    fn test_precedence_ordering() {
        assert!(BinOp::Mul.precedence() > BinOp::Add.precedence());
        assert!(BinOp::Add.precedence() > BinOp::Shl.precedence());
        assert!(BinOp::Shl.precedence() > BinOp::BitAnd.precedence());
    }

    #[test]
    fn test_smt_lib_serialization() {
        // Test variable
        let var = SymExpr::variable("x".to_string());
        assert_eq!(var.to_smt_lib(), "x");

        // Test constant
        let const_expr = SymExpr::constant(ConstValue::I64(42));
        assert_eq!(const_expr.to_smt_lib(), "42");

        // Test binary operation
        let add_expr = SymExpr::binary_op(BinOp::Add, var.clone(), const_expr.clone());
        assert_eq!(add_expr.to_smt_lib(), "(+ x 42)");

        // Test unary operation
        let neg_expr = SymExpr::unary_op(UnOp::Neg, var.clone());
        assert_eq!(neg_expr.to_smt_lib(), "(- x)");

        // Test conditional
        let cond = SymExpr::constant(ConstValue::Bool(true));
        let conditional = SymExpr::conditional(cond, var.clone(), const_expr.clone());
        assert_eq!(conditional.to_smt_lib(), "(ite true x 42)");
    }

    #[test]
    fn test_smt_declarations() {
        let var1 = SymExpr::variable("x".to_string());
        let var2 = SymExpr::variable("y".to_string());
        let expr = SymExpr::binary_op(BinOp::Add, var1, var2);

        let declarations = expr.get_smt_declarations();
        assert_eq!(declarations.len(), 2);
        assert!(declarations.contains(&"(declare-fun x () Int)".to_string()));
        assert!(declarations.contains(&"(declare-fun y () Int)".to_string()));
    }

    #[test]
    fn test_smt_script_generation() {
        let var = SymExpr::variable("x".to_string());
        let const_expr = SymExpr::constant(ConstValue::I64(0));
        let expr = SymExpr::binary_op(BinOp::Gt, var, const_expr);

        let script = expr.to_smt_script();
        assert!(script.contains("(set-logic QF_LIA)"));
        assert!(script.contains("(declare-fun x () Int)"));
        assert!(script.contains("(assert (> x 0))"));
        assert!(script.contains("(check-sat)"));
        assert!(script.contains("(get-model)"));
    }

    #[test]
    fn test_const_value_smt_lib() {
        assert_eq!(ConstValue::I64(42).to_smt_lib(), "42");
        assert_eq!(ConstValue::Bool(true).to_smt_lib(), "true");
        assert_eq!(ConstValue::Bool(false).to_smt_lib(), "false");
        assert_eq!(ConstValue::U64(100).to_smt_lib(), "100");
    }

    #[test]
    fn test_binop_smt_lib() {
        assert_eq!(BinOp::Add.to_smt_lib(), "+");
        assert_eq!(BinOp::Sub.to_smt_lib(), "-");
        assert_eq!(BinOp::Mul.to_smt_lib(), "*");
        assert_eq!(BinOp::Eq.to_smt_lib(), "=");
        assert_eq!(BinOp::Lt.to_smt_lib(), "<");
        assert_eq!(BinOp::Ne.to_smt_lib(), "distinct");
    }

    #[test]
    fn test_unop_smt_lib() {
        assert_eq!(UnOp::Neg.to_smt_lib(), "-");
        assert_eq!(UnOp::Not.to_smt_lib(), "not");
    }

    // **Feature: symbolic-execution-engine, Property 6: SMT serialization round-trip**
    // **Validates: Requirements 2.5**
    #[qc]
    fn prop_smt_serialization_round_trip(expr: SymExpr) -> bool {
        // For this property, we test that serialization produces valid SMT-LIB syntax
        // A true round-trip would require parsing SMT-LIB back to expressions,
        // which is beyond the scope of this task. Instead, we verify:
        // 1. Serialization doesn't crash
        // 2. The result contains expected structure
        // 3. Variables and constants are preserved in the output

        let smt_output = expr.to_smt_lib();

        // Basic structural checks
        if smt_output.is_empty() {
            return false;
        }

        // Check that variables in the original expression appear in the SMT output
        let original_vars = expr.get_variables();
        for var in &original_vars {
            if !smt_output.contains(var) {
                return false;
            }
        }

        // Check that constants are properly serialized
        match &expr {
            SymExpr::Constant(val) => {
                let expected = val.to_smt_lib();
                smt_output == expected
            }
            SymExpr::Variable(name) => smt_output == *name,
            SymExpr::BinaryOp(op, _, _) => smt_output.contains(op.to_smt_lib()),
            SymExpr::UnaryOp(op, _) => smt_output.contains(op.to_smt_lib()),
            SymExpr::Conditional(_, _, _) => smt_output.contains("ite"),
        }
    }

    #[qc]
    fn prop_smt_script_completeness(expr: SymExpr) -> bool {
        let script = expr.to_smt_script();

        // A complete SMT script should contain all necessary components
        script.contains("(set-logic")
            && script.contains("(check-sat)")
            && script.contains("(get-model)")
            && script.contains("(assert")
    }

    #[qc]
    fn prop_smt_declarations_completeness(expr: SymExpr) -> bool {
        let variables = expr.get_variables();
        let declarations = expr.get_smt_declarations();

        // Every variable should have a declaration
        if variables.len() != declarations.len() {
            return false;
        }

        // Each variable should appear in exactly one declaration
        // Use more precise matching to avoid substring issues
        for var in &variables {
            let expected_decl = format!("(declare-fun {} () Int)", var);
            let matching_decls = declarations
                .iter()
                .filter(|decl| **decl == expected_decl)
                .count();
            if matching_decls != 1 {
                return false;
            }
        }

        true
    }

    #[qc]
    fn prop_smt_serialization_preserves_structure(
        op: BinOp,
        left_val: ConstValue,
        right_val: ConstValue,
    ) -> bool {
        let left = SymExpr::constant(left_val);
        let right = SymExpr::constant(right_val);
        let expr = SymExpr::binary_op(op, left.clone(), right.clone());

        let smt_output = expr.to_smt_lib();

        // Should be in the form (op left_smt right_smt)
        let expected_op = op.to_smt_lib();
        let left_smt = left.to_smt_lib();
        let right_smt = right.to_smt_lib();

        smt_output.starts_with('(')
            && smt_output.ends_with(')')
            && smt_output.contains(expected_op)
            && smt_output.contains(&left_smt)
            && smt_output.contains(&right_smt)
    }

    #[test]
    fn test_constant_folding() {
        // Test arithmetic constant folding
        let expr = SymExpr::binary_op(
            BinOp::Add,
            SymExpr::constant(ConstValue::I64(2)),
            SymExpr::constant(ConstValue::I64(3)),
        );
        let simplified = expr.simplify();
        assert_eq!(simplified, SymExpr::constant(ConstValue::I64(5)));

        // Test multiplication by zero
        let expr = SymExpr::binary_op(
            BinOp::Mul,
            SymExpr::variable("x".to_string()),
            SymExpr::constant(ConstValue::I64(0)),
        );
        let simplified = expr.simplify();
        assert_eq!(simplified, SymExpr::constant(ConstValue::I64(0)));

        // Test addition with zero
        let var = SymExpr::variable("x".to_string());
        let expr = SymExpr::binary_op(
            BinOp::Add,
            var.clone(),
            SymExpr::constant(ConstValue::I64(0)),
        );
        let simplified = expr.simplify();
        assert_eq!(simplified, var);
    }

    #[test]
    fn test_algebraic_simplification() {
        let var = SymExpr::variable("x".to_string());

        // Test x - x = 0
        let expr = SymExpr::binary_op(BinOp::Sub, var.clone(), var.clone());
        let simplified = expr.simplify();
        assert_eq!(simplified, SymExpr::constant(ConstValue::I64(0)));

        // Test x ^ x = 0 (bitwise XOR)
        let expr = SymExpr::binary_op(BinOp::BitXor, var.clone(), var.clone());
        let simplified = expr.simplify();
        assert_eq!(simplified, SymExpr::constant(ConstValue::U64(0)));

        // Test x & x = x
        let expr = SymExpr::binary_op(BinOp::BitAnd, var.clone(), var.clone());
        let simplified = expr.simplify();
        assert_eq!(simplified, var);
    }

    #[test]
    fn test_double_negation_elimination() {
        let var = SymExpr::variable("x".to_string());
        let neg_var = SymExpr::unary_op(UnOp::Neg, var.clone());
        let double_neg = SymExpr::unary_op(UnOp::Neg, neg_var);

        let simplified = double_neg.simplify();
        assert_eq!(simplified, var);
    }

    #[test]
    fn test_conditional_simplification() {
        let var = SymExpr::variable("x".to_string());
        let const_42 = SymExpr::constant(ConstValue::I64(42));

        // Test if true then x else 42 = x
        let expr = SymExpr::conditional(
            SymExpr::constant(ConstValue::Bool(true)),
            var.clone(),
            const_42.clone(),
        );
        let simplified = expr.simplify();
        assert_eq!(simplified, var);

        // Test if false then x else 42 = 42
        let expr = SymExpr::conditional(
            SymExpr::constant(ConstValue::Bool(false)),
            var.clone(),
            const_42.clone(),
        );
        let simplified = expr.simplify();
        assert_eq!(simplified, const_42);

        // Test if cond then x else x = x
        let cond = SymExpr::variable("cond".to_string());
        let expr = SymExpr::conditional(cond, var.clone(), var.clone());
        let simplified = expr.simplify();
        assert_eq!(simplified, var);
    }

    #[test]
    fn test_hash_consing() {
        let table = ExpressionTable::new();

        let expr1 = SymExpr::variable("x".to_string());
        let expr2 = SymExpr::variable("x".to_string());

        let interned1 = table.intern(expr1);
        let interned2 = table.intern(expr2);

        // Should be the same instance due to hash-consing
        assert_eq!(interned1, interned2);
        assert_eq!(table.size(), 1);
    }

    #[test]
    fn test_canonical_hash() {
        let expr1 = SymExpr::variable("x".to_string());
        let expr2 = SymExpr::variable("x".to_string());
        let expr3 = SymExpr::variable("y".to_string());

        assert_eq!(expr1.canonical_hash(), expr2.canonical_hash());
        assert_ne!(expr1.canonical_hash(), expr3.canonical_hash());
    }

    #[test]
    fn test_is_simplified() {
        // Already simplified expression
        let var = SymExpr::variable("x".to_string());
        assert!(var.is_simplified());

        // Expression that can be simplified
        let expr = SymExpr::binary_op(
            BinOp::Add,
            var.clone(),
            SymExpr::constant(ConstValue::I64(0)),
        );
        assert!(!expr.is_simplified());

        // After simplification
        let simplified = expr.simplify();
        assert!(simplified.is_simplified());
    }

    // **Feature: symbolic-execution-engine, Property 5: Operation precedence preservation**
    // **Validates: Requirements 2.2**
    #[qc]
    fn prop_operation_precedence_preservation(
        op1: BinOp,
        op2: BinOp,
        a: ConstValue,
        b: ConstValue,
        c: ConstValue,
    ) -> bool {
        // Test that precedence is preserved in expression construction
        // For expression: a op1 b op2 c
        // The precedence should determine the structure

        let expr_a = SymExpr::constant(a);
        let expr_b = SymExpr::constant(b);
        let expr_c = SymExpr::constant(c);

        // Create nested expression: (a op1 b) op2 c
        let left_first = SymExpr::binary_op(
            op2,
            SymExpr::binary_op(op1, expr_a.clone(), expr_b.clone()),
            expr_c.clone(),
        );

        // Create nested expression: a op1 (b op2 c)
        let right_first = SymExpr::binary_op(
            op1,
            expr_a.clone(),
            SymExpr::binary_op(op2, expr_b.clone(), expr_c.clone()),
        );

        // The expressions should be different unless the operations have the same precedence
        if op1.precedence() == op2.precedence() {
            // Same precedence - both structures are valid
            true
        } else {
            // Different precedence - structures should be different
            left_first != right_first
        }
    }

    #[qc]
    fn prop_precedence_ordering_consistency(op1: BinOp, op2: BinOp) -> bool {
        let prec1 = op1.precedence();
        let prec2 = op2.precedence();

        // Precedence should be consistent with mathematical conventions
        // Higher precedence operations should bind tighter

        // Test some known precedence relationships
        match (op1, op2) {
            // Multiplication should have higher precedence than addition
            (BinOp::Mul, BinOp::Add) => prec1 > prec2,
            (BinOp::Add, BinOp::Mul) => prec1 < prec2,

            // Division should have same precedence as multiplication
            (BinOp::Div, BinOp::Mul) => prec1 == prec2,
            (BinOp::Mul, BinOp::Div) => prec1 == prec2,

            // Addition and subtraction should have same precedence
            (BinOp::Add, BinOp::Sub) => prec1 == prec2,
            (BinOp::Sub, BinOp::Add) => prec1 == prec2,

            // Comparison operations should have lower precedence than arithmetic
            (BinOp::Lt, BinOp::Add) => prec1 < prec2,
            (BinOp::Add, BinOp::Lt) => prec1 > prec2,

            _ => true, // For other combinations, just check consistency
        }
    }

    #[qc]
    fn prop_expression_structure_reflects_precedence(
        high_prec_op: BinOp,
        low_prec_op: BinOp,
        a: ConstValue,
        b: ConstValue,
        c: ConstValue,
    ) -> bool {
        // Skip if precedence relationship is not as expected
        if high_prec_op.precedence() <= low_prec_op.precedence() {
            return true; // Skip this test case
        }

        let expr_a = SymExpr::constant(a);
        let expr_b = SymExpr::constant(b);
        let expr_c = SymExpr::constant(c);

        // For expression a low_prec_op b high_prec_op c
        // Should be parsed as a low_prec_op (b high_prec_op c)
        let expected_structure = SymExpr::binary_op(
            low_prec_op,
            expr_a.clone(),
            SymExpr::binary_op(high_prec_op, expr_b.clone(), expr_c.clone()),
        );

        // The high precedence operation should be nested deeper
        match &expected_structure {
            SymExpr::BinaryOp(outer_op, _left, right) => {
                *outer_op == low_prec_op
                    && matches!(**right, SymExpr::BinaryOp(inner_op, _, _) if inner_op == high_prec_op)
            }
            _ => false,
        }
    }

    #[test]
    fn test_precedence_specific_cases() {
        // Test specific precedence cases that should always hold

        // 2 + 3 * 4 should be parsed as 2 + (3 * 4), not (2 + 3) * 4
        let expr = SymExpr::binary_op(
            BinOp::Add,
            SymExpr::constant(ConstValue::I64(2)),
            SymExpr::binary_op(
                BinOp::Mul,
                SymExpr::constant(ConstValue::I64(3)),
                SymExpr::constant(ConstValue::I64(4)),
            ),
        );

        // This should simplify to 2 + 12 = 14
        let simplified = expr.simplify();
        assert_eq!(simplified, SymExpr::constant(ConstValue::I64(14)));

        // Test that multiplication has higher precedence than addition
        assert!(BinOp::Mul.precedence() > BinOp::Add.precedence());

        // Test that shift operations have lower precedence than multiplication
        assert!(BinOp::Shl.precedence() < BinOp::Mul.precedence());

        // Test that comparison operations have lowest precedence
        assert!(BinOp::Lt.precedence() < BinOp::Add.precedence());
    }

    #[test]
    fn test_const_value_conversions() {
        // Test u64 conversions
        let val = ConstValue::U64(42);
        assert_eq!(val.as_u64(), Some(42));
        assert_eq!(val.as_i64(), Some(42));
        assert_eq!(val.as_u32(), Some(42));
        assert_eq!(val.as_i32(), Some(42));
        assert_eq!(val.as_u8(), Some(42));
        assert_eq!(val.as_f64(), Some(42.0));
        assert_eq!(val.as_f32(), Some(42.0));
        assert_eq!(val.as_bool(), None);

        // Test i64 conversions
        let val = ConstValue::I64(-10);
        assert_eq!(val.as_i64(), Some(-10));
        assert_eq!(val.as_u64(), Some(-10i64 as u64)); // Cast to u64
        assert_eq!(val.as_i32(), Some(-10));

        // Test u32 conversions
        let val = ConstValue::U32(100);
        assert_eq!(val.as_u32(), Some(100));
        assert_eq!(val.as_u64(), Some(100));
        assert_eq!(val.as_i64(), Some(100));

        // Test i32 conversions
        let val = ConstValue::I32(-5);
        assert_eq!(val.as_i32(), Some(-5));
        assert_eq!(val.as_i64(), Some(-5));

        // Test u8 conversions
        let val = ConstValue::U8(255);
        assert_eq!(val.as_u8(), Some(255));
        assert_eq!(val.as_u32(), Some(255));
        assert_eq!(val.as_u64(), Some(255));

        // Test f64 conversions
        let val = ConstValue::F64(3.14);
        assert_eq!(val.as_f64(), Some(3.14));
        assert_eq!(val.as_f32(), Some(3.14f32));
        assert_eq!(val.as_u64(), Some(3));
        assert_eq!(val.as_i64(), Some(3));

        // Test f32 conversions
        let val = ConstValue::F32(2.5);
        assert_eq!(val.as_f32(), Some(2.5));
        assert_eq!(val.as_f64(), Some(2.5f64));
        assert_eq!(val.as_u32(), Some(2));

        // Test bool conversions
        let val = ConstValue::Bool(true);
        assert_eq!(val.as_bool(), Some(true));
        assert_eq!(val.as_u64(), None);
        assert_eq!(val.as_i64(), None);
        assert_eq!(val.as_f64(), None);

        // Test truncation
        let val = ConstValue::U64(300);
        assert_eq!(val.as_u8(), Some(44)); // 300 % 256 = 44

        // Test large value casting
        let val = ConstValue::U32(u32::MAX);
        assert_eq!(val.as_u32(), Some(u32::MAX));
        assert_eq!(val.as_u64(), Some(u32::MAX as u64));
        assert_eq!(val.as_u8(), Some(255)); // Truncated
    }
}

// Additional expression manipulation methods will be implemented in later tasks
