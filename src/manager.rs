//! Constraint/variable manager.
//!
//! `SymExManager` stores symbolic variables and path constraints for a single run,
//! and provides cached SAT/model queries via an SMT solver.

use crate::error::{SymExError, SymExResult};
use crate::expressions::SymExpr;
use crate::solver::{Model, SatResult, SmtSolver};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub type_name: String,
    pub bit_width: Option<usize>,
    pub is_signed: bool,
    pub creation_site: Option<String>,
}

impl TypeInfo {
    pub fn string(creation_site: Option<String>) -> Self {
        Self {
            type_name: "string".to_string(),
            bit_width: None,
            is_signed: false,
            creation_site,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub sat_cache_size: usize,
    pub model_cache_size: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub hit_rate: f64,
}

pub struct SymExManager {
    variable_counter: AtomicUsize,
    path_constraints: Vec<SymExpr>,
    solver: Box<dyn SmtSolver>,
    variable_registry: HashMap<String, TypeInfo>,

    sat_cache: HashMap<u64, (Vec<SymExpr>, SatResult)>,
    model_cache: HashMap<u64, (Vec<SymExpr>, Option<Model>)>,
    cache_hits: usize,
    cache_misses: usize,
}

impl SymExManager {
    pub fn new(solver: Box<dyn SmtSolver>) -> Self {
        Self {
            variable_counter: AtomicUsize::new(0),
            path_constraints: Vec::new(),
            solver,
            variable_registry: HashMap::new(),
            sat_cache: HashMap::new(),
            model_cache: HashMap::new(),
            cache_hits: 0,
            cache_misses: 0,
        }
    }

    pub fn fresh_variable(&self, type_name: &str) -> String {
        let id = self.variable_counter.fetch_add(1, Ordering::SeqCst);
        format!("{type_name}_{id}")
    }

    fn is_valid_variable_name(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        let mut chars = name.chars();
        let first = chars.next().unwrap();
        if !first.is_ascii_alphabetic() && first != '_' {
            return false;
        }
        chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    }

    pub fn register_variable(&mut self, name: String, type_info: TypeInfo) -> SymExResult<()> {
        if !Self::is_valid_variable_name(&name) {
            return Err(SymExError::InvalidOperation(format!(
                "Invalid variable name: {name}"
            )));
        }
        if self.variable_registry.contains_key(&name) {
            return Err(SymExError::DuplicateVariable(name));
        }
        self.variable_registry.insert(name, type_info);
        Ok(())
    }

    pub fn register_string_variable(&mut self, name: String) -> SymExResult<()> {
        self.register_variable(name, TypeInfo::string(None))
    }

    pub fn get_variable_info(&self, name: &str) -> Option<&TypeInfo> {
        self.variable_registry.get(name)
    }

    pub fn get_registered_variables(&self) -> Vec<String> {
        let mut vars: Vec<String> = self.variable_registry.keys().cloned().collect();
        vars.sort();
        vars
    }

    pub fn get_constraints(&self) -> &[SymExpr] {
        &self.path_constraints
    }

    pub fn clear_constraints(&mut self) {
        self.path_constraints.clear();
        self.clear_cache();
    }

    pub fn add_constraint(&mut self, constraint: SymExpr) -> SymExResult<()> {
        self.validate_constraint(&constraint)?;
        self.path_constraints.push(constraint);
        self.clear_cache();
        Ok(())
    }

    pub fn is_satisfiable(&mut self) -> SymExResult<bool> {
        let key = self.compute_constraints_hash(&self.path_constraints);
        if let Some((cached_constraints, cached_result)) = self.sat_cache.get(&key) {
            if self.constraints_match(cached_constraints, &self.path_constraints) {
                self.cache_hits += 1;
                return Ok(matches!(cached_result, SatResult::Sat));
            }
        }

        self.cache_misses += 1;
        let result = self.solver.check_sat(&self.path_constraints)?;
        self.sat_cache
            .insert(key, (self.path_constraints.clone(), result.clone()));
        Ok(matches!(result, SatResult::Sat))
    }

    pub fn get_model(&mut self) -> SymExResult<Option<Model>> {
        let key = self.compute_constraints_hash(&self.path_constraints);
        if let Some((cached_constraints, cached_model)) = self.model_cache.get(&key) {
            if self.constraints_match(cached_constraints, &self.path_constraints) {
                self.cache_hits += 1;
                return Ok(cached_model.clone());
            }
        }

        if !self.is_satisfiable()? {
            self.model_cache
                .insert(key, (self.path_constraints.clone(), None));
            return Ok(None);
        }

        let model = self.solver.get_model()?;
        self.model_cache
            .insert(key, (self.path_constraints.clone(), model.clone()));
        Ok(model)
    }

    pub fn check_satisfiability(&mut self, constraints: &[SymExpr]) -> SymExResult<bool> {
        let key = self.compute_constraints_hash(constraints);
        if let Some((cached_constraints, cached_result)) = self.sat_cache.get(&key) {
            if self.constraints_match(cached_constraints, constraints) {
                self.cache_hits += 1;
                return Ok(matches!(cached_result, SatResult::Sat));
            }
        }

        self.cache_misses += 1;
        let result = self.solver.check_sat(constraints)?;
        self.sat_cache
            .insert(key, (constraints.to_vec(), result.clone()));
        Ok(matches!(result, SatResult::Sat))
    }

    pub fn get_model_for_constraints(
        &mut self,
        constraints: &[SymExpr],
    ) -> SymExResult<Option<Model>> {
        let key = self.compute_constraints_hash(constraints);
        if let Some((cached_constraints, cached_model)) = self.model_cache.get(&key) {
            if self.constraints_match(cached_constraints, constraints) {
                self.cache_hits += 1;
                return Ok(cached_model.clone());
            }
        }

        self.cache_misses += 1;
        let result = self.solver.check_sat(constraints)?;
        if result != SatResult::Sat {
            self.model_cache.insert(key, (constraints.to_vec(), None));
            return Ok(None);
        }
        let model = self.solver.get_model()?;
        self.model_cache
            .insert(key, (constraints.to_vec(), model.clone()));
        Ok(model)
    }

    pub fn clear_cache(&mut self) {
        self.sat_cache.clear();
        self.model_cache.clear();
        self.cache_hits = 0;
        self.cache_misses = 0;
    }

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

    pub fn reset(&mut self) -> SymExResult<()> {
        self.variable_counter.store(0, Ordering::SeqCst);
        self.path_constraints.clear();
        self.variable_registry.clear();
        self.clear_cache();
        self.solver.reset()?;
        Ok(())
    }

    fn compute_constraints_hash(&self, constraints: &[SymExpr]) -> u64 {
        let mut hasher = DefaultHasher::new();
        constraints.len().hash(&mut hasher);
        for c in constraints {
            format!("{c:?}").hash(&mut hasher);
        }
        hasher.finish()
    }

    fn constraints_match(&self, a: &[SymExpr], b: &[SymExpr]) -> bool {
        a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x == y)
    }

    fn validate_constraint(&self, constraint: &SymExpr) -> SymExResult<()> {
        for var in constraint.get_variables() {
            if !self.variable_registry.contains_key(&var) {
                return Err(SymExError::UnboundVariable(var));
            }
        }

        if constraint.depth() > 100 {
            return Err(SymExError::ResourceExhaustion(
                "Constraint depth exceeds maximum limit (100)".to_string(),
            ));
        }
        if constraint.node_count() > 10000 {
            return Err(SymExError::ResourceExhaustion(
                "Constraint complexity exceeds maximum limit (10000 nodes)".to_string(),
            ));
        }

        constraint.validate_string_operations()?;
        constraint.validate_ascii()?;
        Ok(())
    }

    /// Fast satisfiability check using concrete values (limited to numeric constraints).
    pub fn fast_check_satisfiable_with_concrete(
        &self,
        concrete_values: &HashMap<String, u64>,
    ) -> Option<bool> {
        for c in &self.path_constraints {
            match self.evaluate_constraint_concrete(c, concrete_values) {
                Some(true) => continue,
                Some(false) => return Some(false),
                None => return None,
            }
        }
        Some(true)
    }

    fn evaluate_constraint_concrete(
        &self,
        constraint: &SymExpr,
        concrete_values: &HashMap<String, u64>,
    ) -> Option<bool> {
        use crate::expressions::{BinOp, ConstValue};
        match constraint {
            SymExpr::Constant(ConstValue::Bool(b)) => Some(*b),
            SymExpr::BinaryOp(op, left, right) => {
                let l = self.evaluate_expr_to_u64(left, concrete_values)?;
                let r = self.evaluate_expr_to_u64(right, concrete_values)?;
                Some(match op {
                    BinOp::Eq => l == r,
                    BinOp::Ne => l != r,
                    BinOp::Lt => l < r,
                    BinOp::Le => l <= r,
                    BinOp::Gt => l > r,
                    BinOp::Ge => l >= r,
                    _ => return None,
                })
            }
            SymExpr::UnaryOp(op, inner) => {
                use crate::expressions::UnOp;
                match op {
                    UnOp::Not => Some(!self.evaluate_constraint_concrete(inner, concrete_values)?),
                    _ => None,
                }
            }
            SymExpr::Conditional(cond, t, e) => {
                if self.evaluate_constraint_concrete(cond, concrete_values)? {
                    self.evaluate_constraint_concrete(t, concrete_values)
                } else {
                    self.evaluate_constraint_concrete(e, concrete_values)
                }
            }
            _ => None,
        }
    }

    fn evaluate_expr_to_u64(
        &self,
        expr: &SymExpr,
        concrete_values: &HashMap<String, u64>,
    ) -> Option<u64> {
        use crate::expressions::{BinOp, ConstValue, UnOp};
        match expr {
            SymExpr::Constant(val) => match val {
                ConstValue::U64(v) => Some(*v),
                ConstValue::U32(v) => Some(*v as u64),
                ConstValue::U8(v) => Some(*v as u64),
                ConstValue::I64(v) => Some(*v as u64),
                ConstValue::I32(v) => Some(*v as u64),
                _ => None,
            },
            SymExpr::Variable(name) => concrete_values.get(name).copied(),
            SymExpr::BinaryOp(op, left, right) => {
                let l = self.evaluate_expr_to_u64(left, concrete_values)?;
                let r = self.evaluate_expr_to_u64(right, concrete_values)?;
                Some(match op {
                    BinOp::Add => l.wrapping_add(r),
                    BinOp::Sub => l.wrapping_sub(r),
                    BinOp::Mul => l.wrapping_mul(r),
                    BinOp::Div if r != 0 => l / r,
                    BinOp::Mod if r != 0 => l % r,
                    BinOp::BitAnd => l & r,
                    BinOp::BitOr => l | r,
                    BinOp::BitXor => l ^ r,
                    BinOp::Shl if r < 64 => l << r,
                    BinOp::Shr if r < 64 => l >> r,
                    _ => return None,
                })
            }
            SymExpr::UnaryOp(op, inner) => {
                let v = self.evaluate_expr_to_u64(inner, concrete_values)?;
                match op {
                    UnOp::Neg => Some((-(v as i64)) as u64),
                    _ => None,
                }
            }
            SymExpr::Conditional(cond, t, e) => {
                if self.evaluate_constraint_concrete(cond, concrete_values)? {
                    self.evaluate_expr_to_u64(t, concrete_values)
                } else {
                    self.evaluate_expr_to_u64(e, concrete_values)
                }
            }
            _ => None,
        }
    }

    /// Format the symbolic execution state as a human-readable string.
    ///
    /// This outputs:
    /// - All registered symbolic variables with their type information
    /// - All path constraints accumulated so far
    /// - Cache statistics
    pub fn format_state(&self) -> String {
        let mut output = String::new();
        
        output.push_str("=== Symbolic Execution State ===\n\n");
        
        // Output symbolic variables
        output.push_str("Symbolic Variables:\n");
        output.push_str("-------------------\n");
        
        let mut vars: Vec<_> = self.variable_registry.iter().collect();
        vars.sort_by_key(|(name, _)| *name);
        
        if vars.is_empty() {
            output.push_str("  (no variables registered)\n");
        } else {
            for (name, type_info) in vars {
                output.push_str(&format!("  {}: {}", name, type_info.type_name));
                if let Some(width) = type_info.bit_width {
                    output.push_str(&format!(" ({width} bits"));
                    if type_info.is_signed {
                        output.push_str(", signed");
                    } else {
                        output.push_str(", unsigned");
                    }
                    output.push(')');
                }
                if let Some(site) = &type_info.creation_site {
                    output.push_str(&format!(" [created at: {site}]"));
                }
                output.push('\n');
            }
        }
        
        output.push('\n');
        
        // Output path constraints
        output.push_str("Path Constraints:\n");
        output.push_str("-----------------\n");
        
        if self.path_constraints.is_empty() {
            output.push_str("  (no constraints)\n");
        } else {
            for (i, constraint) in self.path_constraints.iter().enumerate() {
                output.push_str(&format!("  [{i}] {constraint}\n"));
            }
        }
        
        output.push('\n');
        
        // Output cache statistics
        let stats = self.get_cache_stats();
        output.push_str("Cache Statistics:\n");
        output.push_str("-----------------\n");
        output.push_str(&format!("  SAT cache entries: {}\n", stats.sat_cache_size));
        output.push_str(&format!("  Model cache entries: {}\n", stats.model_cache_size));
        output.push_str(&format!("  Cache hits: {}\n", stats.cache_hits));
        output.push_str(&format!("  Cache misses: {}\n", stats.cache_misses));
        output.push_str(&format!("  Hit rate: {:.2}%\n", stats.hit_rate * 100.0));
        
        output
    }

    /// Print the symbolic execution state to stdout.
    pub fn print_state(&self) {
        print!("{}", self.format_state());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expressions::{BinOp, ConstValue};
    use crate::solver::Z3Solver;

    fn manager() -> SymExManager {
        let solver = Box::new(Z3Solver::new().unwrap());
        SymExManager::new(solver)
    }

    #[test]
    fn registers_and_rejects_duplicates() {
        let mut m = manager();
        m.register_variable(
            "x".to_string(),
            TypeInfo {
                type_name: "u64".to_string(),
                bit_width: Some(64),
                is_signed: false,
                creation_site: None,
            },
        )
        .unwrap();

        let err = m
            .register_variable(
                "x".to_string(),
                TypeInfo {
                    type_name: "u64".to_string(),
                    bit_width: Some(64),
                    is_signed: false,
                    creation_site: None,
                },
            )
            .unwrap_err();
        assert!(matches!(err, SymExError::DuplicateVariable(_)));
    }

    #[test]
    fn rejects_unbound_variables_in_constraints() {
        let mut m = manager();
        let c = SymExpr::binary_op(
            BinOp::Eq,
            SymExpr::Variable("x".to_string()),
            SymExpr::Constant(ConstValue::U64(0)),
        );
        let err = m.add_constraint(c).unwrap_err();
        assert!(matches!(err, SymExError::UnboundVariable(_)));
    }
}
