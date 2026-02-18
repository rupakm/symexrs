//! Symbolic drop-in types.
//!
//! These types mirror common Rust types (ints, bool, string) and implement
//! standard traits. Under exploration, comparisons consume branch decisions
//! from the runtime and add the chosen constraint to the current path.

use crate::expressions::{BinOp, ConstValue, SymExpr, UnOp};
use crate::manager::{SymExManager, TypeInfo};
use std::cmp::Ordering;
use std::sync::{Arc, Mutex};

crate::define_sym_int!(SymU64, u64, U64, "u64", 64, false, get_u64);
crate::define_sym_int!(SymU32, u32, U32, "u32", 32, false, get_u32);
crate::define_sym_int!(SymI32, i32, I32, "i32", 32, true, get_i32);
crate::define_sym_int!(SymI64, i64, I64, "i64", 64, true, get_i64);
crate::define_sym_int!(SymU8, u8, U8, "u8", 8, false, get_u8);

#[derive(Clone)]
pub struct SymBool {
    variable_name: String,
    expr: SymExpr,
    concrete_value: Option<bool>,
    manager: Arc<Mutex<SymExManager>>,
}

impl Default for SymBool {
    fn default() -> Self {
        Self::new()
    }
}

impl SymBool {
    pub fn new() -> Self {
        let manager = crate::get_global_manager().expect("Failed to get global manager");
        Self::new_in(manager)
    }

    pub fn new_in(manager: Arc<Mutex<SymExManager>>) -> Self {
        let variable_name = {
            let mgr = manager.lock().unwrap();
            mgr.fresh_variable("bool")
        };
        let expr = SymExpr::Variable(variable_name.clone());

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

    pub fn new_global() -> Self {
        Self::new()
    }

    pub fn with_value(value: bool) -> Self {
        let manager = crate::get_global_manager().expect("Failed to get global manager");
        Self::with_value_in(value, manager)
    }

    pub fn with_value_in(value: bool, manager: Arc<Mutex<SymExManager>>) -> Self {
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

    pub fn from_concrete(value: bool) -> Self {
        let manager = crate::get_global_manager().expect("Failed to get global manager");
        Self::from_concrete_in(value, manager)
    }

    pub fn from_concrete_in(value: bool, manager: Arc<Mutex<SymExManager>>) -> Self {
        let variable_name = {
            let mgr = manager.lock().unwrap();
            mgr.fresh_variable("bool")
        };

        Self {
            variable_name,
            expr: SymExpr::Constant(ConstValue::Bool(value)),
            concrete_value: Some(value),
            manager,
        }
    }

    pub fn expr(&self) -> &SymExpr {
        &self.expr
    }

    pub fn variable_name(&self) -> &str {
        &self.variable_name
    }

    pub fn concrete_value(&self) -> Option<bool> {
        self.concrete_value
    }

    pub fn manager(&self) -> Arc<Mutex<SymExManager>> {
        Arc::clone(&self.manager)
    }

    pub fn eq_constraint(&self, other: &Self) -> SymExpr {
        SymExpr::binary_op(BinOp::Eq, self.expr.clone(), other.expr.clone())
    }

    pub fn ne_constraint(&self, other: &Self) -> SymExpr {
        SymExpr::binary_op(BinOp::Ne, self.expr.clone(), other.expr.clone())
    }

    pub fn assert_eq(&self, other: &Self) -> crate::SymExResult<()> {
        let constraint = self.eq_constraint(other);
        let mut mgr = self.manager.lock().unwrap();
        mgr.add_constraint(constraint)
    }

    pub fn assert_ne(&self, other: &Self) -> crate::SymExResult<()> {
        let constraint = self.ne_constraint(other);
        let mut mgr = self.manager.lock().unwrap();
        mgr.add_constraint(constraint)
    }

    pub fn assert_true(&self) -> crate::SymExResult<()> {
        let t = SymBool::from_concrete_in(true, Arc::clone(&self.manager));
        self.assert_eq(&t)
    }

    pub fn assert_false(&self) -> crate::SymExResult<()> {
        let f = SymBool::from_concrete_in(false, Arc::clone(&self.manager));
        self.assert_eq(&f)
    }

    pub fn implies(&self, other: &Self) -> SymBool {
        let not_self = !self;
        &not_self | other
    }

    pub fn iff(&self, other: &Self) -> SymBool {
        let forward = self.implies(other);
        let backward = other.implies(self);
        &forward & &backward
    }

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

    pub fn is_true(&self) -> bool {
        let t = SymBool::from_concrete_in(true, Arc::clone(&self.manager));
        self == &t
    }

    pub fn is_false(&self) -> bool {
        let f = SymBool::from_concrete_in(false, Arc::clone(&self.manager));
        self == &f
    }

    pub fn holds(&self) -> bool {
        self.is_true()
    }
}

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

impl std::ops::BitAnd for SymBool {
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

impl std::ops::BitAnd for &SymBool {
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

impl std::ops::BitOr for SymBool {
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

impl std::ops::BitOr for &SymBool {
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

impl std::ops::BitXor for SymBool {
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

impl std::ops::BitXor for &SymBool {
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

impl PartialEq for SymBool {
    fn eq(&self, other: &Self) -> bool {
        if !crate::runtime::is_exploring() {
            if let (Some(a), Some(b)) = (self.concrete_value, other.concrete_value) {
                return a == b;
            }
            return self.expr == other.expr;
        }

        let predicate = self.eq_constraint(other);
        let concolic_choice = match (self.concrete_value, other.concrete_value) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        };
        let chosen = crate::runtime::choose_branch(&predicate, concolic_choice);
        let constraint = if chosen {
            predicate
        } else {
            self.ne_constraint(other)
        };
        let mut mgr = self.manager.lock().unwrap();
        let _ = mgr.add_constraint(constraint);
        chosen
    }
}

impl Eq for SymBool {}

impl From<bool> for SymBool {
    fn from(value: bool) -> Self {
        SymBool::from_concrete(value)
    }
}

#[derive(Clone)]
pub struct SymString {
    variable_name: String,
    expr: SymExpr,
    concrete_value: Option<String>,
    manager: Arc<Mutex<SymExManager>>,
}

impl Default for SymString {
    fn default() -> Self {
        Self::new()
    }
}

impl SymString {
    pub fn new() -> Self {
        let manager = crate::get_global_manager().expect("Failed to get global manager");
        Self::new_in(manager)
    }

    pub fn new_in(manager: Arc<Mutex<SymExManager>>) -> Self {
        let variable_name = {
            let mgr = manager.lock().unwrap();
            mgr.fresh_variable("string")
        };
        let expr = SymExpr::Variable(variable_name.clone());
        {
            let mut mgr = manager.lock().unwrap();
            let _ = mgr.register_variable(variable_name.clone(), TypeInfo::string(None));
        }
        Self {
            variable_name,
            expr,
            concrete_value: None,
            manager,
        }
    }

    pub fn new_global() -> Self {
        Self::new()
    }

    pub fn with_value(value: String) -> Self {
        let manager = crate::get_global_manager().expect("Failed to get global manager");
        Self::with_value_in(value, manager)
    }

    pub fn with_value_in(value: String, manager: Arc<Mutex<SymExManager>>) -> Self {
        for (pos, ch) in value.chars().enumerate() {
            if !ch.is_ascii() {
                panic!("Non-ASCII character '{ch}' at position {pos} in string value");
            }
        }

        let variable_name = {
            let mut mgr = manager.lock().unwrap();
            let name = mgr.fresh_variable("string");
            let _ = mgr.register_variable(name.clone(), TypeInfo::string(None));
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

    pub fn from_concrete(value: &str) -> Self {
        let manager = crate::get_global_manager().expect("Failed to get global manager");
        Self::from_concrete_in(value, manager)
    }

    pub fn from_concrete_in(value: &str, manager: Arc<Mutex<SymExManager>>) -> Self {
        for (pos, ch) in value.chars().enumerate() {
            if !ch.is_ascii() {
                panic!("Non-ASCII character '{ch}' at position {pos} in string value");
            }
        }

        let variable_name = {
            let mgr = manager.lock().unwrap();
            mgr.fresh_variable("string")
        };

        Self {
            variable_name,
            expr: SymExpr::Constant(ConstValue::String(value.to_string())),
            concrete_value: Some(value.to_string()),
            manager,
        }
    }

    pub fn expr(&self) -> &SymExpr {
        &self.expr
    }

    pub fn variable_name(&self) -> &str {
        &self.variable_name
    }

    pub fn concrete_value(&self) -> Option<&str> {
        self.concrete_value.as_deref()
    }

    pub fn set_concrete_value(&mut self, value: String) {
        self.concrete_value = Some(value);
    }

    pub fn manager(&self) -> Arc<Mutex<SymExManager>> {
        Arc::clone(&self.manager)
    }

    pub fn eq_constraint(&self, other: &Self) -> SymExpr {
        SymExpr::binary_op(BinOp::Eq, self.expr.clone(), other.expr.clone())
    }

    pub fn ne_constraint(&self, other: &Self) -> SymExpr {
        SymExpr::binary_op(BinOp::Ne, self.expr.clone(), other.expr.clone())
    }

    pub fn concat(&self, other: &Self) -> SymString {
        let expr = SymExpr::str_concat(self.expr.clone(), other.expr.clone());
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

    pub fn length(&self) -> SymU64 {
        let expr = SymExpr::str_len(self.expr.clone());
        let concrete_length = self.concrete_value.as_ref().map(|s| s.len() as u64);
        let result = SymU64::from_expr_in(expr, Arc::clone(&self.manager));
        if let Some(len) = concrete_length {
            result.set_concrete_value(len);
        }
        result
    }
}

impl std::ops::Add for SymString {
    type Output = SymString;
    fn add(self, rhs: Self) -> Self::Output {
        self.concat(&rhs)
    }
}

impl std::ops::Add for &SymString {
    type Output = SymString;
    fn add(self, rhs: Self) -> Self::Output {
        self.concat(rhs)
    }
}

impl PartialEq for SymString {
    fn eq(&self, other: &Self) -> bool {
        if !crate::runtime::is_exploring() {
            if let (Some(a), Some(b)) =
                (self.concrete_value.as_ref(), other.concrete_value.as_ref())
            {
                return a == b;
            }
            return self.expr == other.expr;
        }

        let predicate = self.eq_constraint(other);
        let concolic_choice = match (self.concrete_value.as_ref(), other.concrete_value.as_ref()) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        };
        let chosen = crate::runtime::choose_branch(&predicate, concolic_choice);
        let constraint = if chosen {
            predicate
        } else {
            self.ne_constraint(other)
        };
        let mut mgr = self.manager.lock().unwrap();
        let _ = mgr.add_constraint(constraint);
        chosen
    }
}

impl Eq for SymString {}

impl PartialOrd for SymString {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if let (Some(a), Some(b)) = (self.concrete_value.as_ref(), other.concrete_value.as_ref()) {
            Some(a.cmp(b))
        } else {
            None
        }
    }
}

impl Ord for SymString {
    fn cmp(&self, other: &Self) -> Ordering {
        if let (Some(a), Some(b)) = (self.concrete_value.as_ref(), other.concrete_value.as_ref()) {
            return a.cmp(b);
        }
        self.variable_name.cmp(&other.variable_name)
    }
}
