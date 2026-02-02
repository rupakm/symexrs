//! Runtime state for symbolic execution.
//!
//! This module centralizes the per-run state needed by the symbolic types and
//! the exploration engine.

use crate::error::SymExResult;
use crate::expressions::ConstValue;
use crate::expressions::SymExpr;
use crate::manager::SymExManager;
use crate::solver::Z3Solver;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    /// No path exploration; comparisons should behave concretely.
    Concrete,
    /// Under exploration; comparisons consume decisions and record branches.
    Explore,
}

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub mode: RuntimeMode,
    /// Stop the run after recording this many newly discovered branches.
    pub max_new_branches_to_record: usize,
    /// Stop the run after consuming this many total branch points.
    pub max_total_branches: Option<usize>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            mode: RuntimeMode::Concrete,
            max_new_branches_to_record: usize::MAX,
            max_total_branches: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BranchRecord {
    pub index: usize,
    pub chosen: bool,
    pub was_forced: bool,
    pub predicate_hash: u64,
    pub vars: Vec<String>,
}

#[derive(Debug)]
pub enum RunAbort {
    BudgetReached,
    PathUnsat,
}

/// Central runtime object shared by symbolic values.
pub struct Runtime {
    pub(crate) manager: Arc<Mutex<SymExManager>>,
    config: RefCell<RuntimeConfig>,

    forced_decisions: RefCell<Vec<bool>>,
    branch_index: Cell<usize>,

    // Concolic seed inputs for this run.
    inputs: RefCell<HashMap<String, ConstValue>>,

    // Branch trace for the current run.
    branches: RefCell<Vec<BranchRecord>>,
    // Indices of branches for which we should spawn the alternative work item.
    spawn_alternatives_for: RefCell<Vec<usize>>,
}

impl Runtime {
    pub fn new() -> SymExResult<Arc<Self>> {
        let solver = Box::new(Z3Solver::new()?);
        let manager = Arc::new(Mutex::new(SymExManager::new(solver)));
        Ok(Arc::new(Self {
            manager,
            config: RefCell::new(RuntimeConfig::default()),
            forced_decisions: RefCell::new(Vec::new()),
            branch_index: Cell::new(0),
            inputs: RefCell::new(HashMap::new()),
            branches: RefCell::new(Vec::new()),
            spawn_alternatives_for: RefCell::new(Vec::new()),
        }))
    }

    pub fn manager(&self) -> Arc<Mutex<SymExManager>> {
        Arc::clone(&self.manager)
    }

    pub fn set_config(&self, cfg: RuntimeConfig) {
        *self.config.borrow_mut() = cfg;
    }

    pub fn config(&self) -> RuntimeConfig {
        self.config.borrow().clone()
    }

    pub fn reset_for_run(
        &self,
        forced_decisions: Vec<bool>,
        inputs: HashMap<String, ConstValue>,
    ) -> SymExResult<()> {
        self.branch_index.set(0);
        *self.forced_decisions.borrow_mut() = forced_decisions;
        *self.inputs.borrow_mut() = inputs;
        self.branches.borrow_mut().clear();
        self.spawn_alternatives_for.borrow_mut().clear();

        let mut mgr = self.manager.lock().unwrap();
        mgr.reset()?;
        Ok(())
    }

    pub fn inputs_snapshot(&self) -> HashMap<String, ConstValue> {
        self.inputs.borrow().clone()
    }

    pub fn concolic_value(&self, name: &str) -> Option<ConstValue> {
        self.inputs.borrow().get(name).cloned()
    }

    pub fn branches_snapshot(&self) -> Vec<BranchRecord> {
        self.branches.borrow().clone()
    }

    pub fn spawn_alternatives_snapshot(&self) -> Vec<usize> {
        self.spawn_alternatives_for.borrow().clone()
    }

    pub fn decisions_taken(&self) -> Vec<bool> {
        self.branches.borrow().iter().map(|b| b.chosen).collect()
    }

    pub fn next_decision_for_branch(
        &self,
        predicate_hash: u64,
        vars: Vec<String>,
        concolic_choice: bool,
    ) -> bool {
        let cfg = self.config.borrow().clone();
        if cfg.mode != RuntimeMode::Explore {
            return concolic_choice;
        }

        let idx = self.branch_index.get();

        if let Some(max) = cfg.max_total_branches {
            if idx >= max {
                std::panic::panic_any(RunAbort::BudgetReached);
            }
        }

        let forced = self.forced_decisions.borrow();
        let (chosen, was_forced) = if let Some(d) = forced.get(idx).copied() {
            (d, true)
        } else {
            (concolic_choice, false)
        };
        drop(forced);

        // Record branch.
        self.branches.borrow_mut().push(BranchRecord {
            index: idx,
            chosen,
            was_forced,
            predicate_hash,
            vars,
        });

        // If this branch wasn't forced, register its alternative for scheduling.
        if !was_forced {
            let mut alts = self.spawn_alternatives_for.borrow_mut();
            let already = alts.len();
            if already < cfg.max_new_branches_to_record {
                alts.push(idx);
            } else {
                std::panic::panic_any(RunAbort::BudgetReached);
            }
        }

        self.branch_index.set(idx + 1);
        chosen
    }

    pub fn concolic_value_for_var_u64(&self, name: &str) -> Option<u64> {
        match self.inputs.borrow().get(name) {
            Some(ConstValue::U64(v)) => Some(*v),
            Some(ConstValue::U32(v)) => Some(*v as u64),
            Some(ConstValue::U8(v)) => Some(*v as u64),
            Some(ConstValue::I64(v)) => Some(*v as u64),
            Some(ConstValue::I32(v)) => Some(*v as u64),
            _ => None,
        }
    }

    pub fn set_concolic_value(&self, name: String, value: ConstValue) {
        self.inputs.borrow_mut().insert(name, value);
    }

    pub fn refresh_inputs_from_model(&self) -> SymExResult<()> {
        let mgr_arc = self.manager();
        let mut mgr = mgr_arc.lock().unwrap();
        if !mgr.is_satisfiable()? {
            return Ok(());
        }
        if let Some(model) = mgr.get_model()? {
            for (k, v) in model.assignments {
                self.inputs.borrow_mut().insert(k, v);
            }
        }
        Ok(())
    }

    pub fn abort_unsat(&self) {
        std::panic::panic_any(RunAbort::PathUnsat);
    }
}

struct RuntimeTls {
    default: Option<Arc<Runtime>>,
    current: Option<Arc<Runtime>>,
}

thread_local! {
    static TLS: RefCell<RuntimeTls> = const { RefCell::new(RuntimeTls { default: None, current: None }) };
}

pub fn with_current_runtime<R>(f: impl FnOnce(Option<&Arc<Runtime>>) -> R) -> R {
    TLS.with(|tls| {
        let tls = tls.borrow();
        f(tls.current.as_ref())
    })
}

pub fn set_current_runtime(rt: Option<Arc<Runtime>>) {
    TLS.with(|tls| {
        tls.borrow_mut().current = rt;
    });
}

pub fn get_or_create_default_runtime() -> SymExResult<Arc<Runtime>> {
    TLS.with(|tls| {
        let mut tls = tls.borrow_mut();
        if tls.default.is_none() {
            tls.default = Some(Runtime::new()?);
        }
        Ok(Arc::clone(tls.default.as_ref().unwrap()))
    })
}

pub fn current_runtime_or_default() -> SymExResult<Arc<Runtime>> {
    TLS.with(|tls| {
        if let Some(rt) = tls.borrow().current.as_ref() {
            Ok(Arc::clone(rt))
        } else {
            get_or_create_default_runtime()
        }
    })
}

pub fn is_exploring() -> bool {
    with_current_runtime(|rt| {
        rt.map(|r| r.config.borrow().mode == RuntimeMode::Explore)
            .unwrap_or(false)
    })
}

pub fn global_manager() -> SymExResult<Arc<Mutex<SymExManager>>> {
    Ok(current_runtime_or_default()?.manager())
}

pub fn concolic_value_for(name: &str) -> Option<ConstValue> {
    with_current_runtime(|rt| rt.and_then(|rt| rt.concolic_value(name)))
}

pub fn refresh_concolic_from_model() -> SymExResult<()> {
    with_current_runtime(|rt| {
        if let Some(rt) = rt {
            rt.refresh_inputs_from_model()
        } else {
            Ok(())
        }
    })
}

pub fn choose_branch(predicate: &SymExpr, concolic_choice: bool) -> bool {
    with_current_runtime(|rt| {
        if let Some(rt) = rt {
            let mut vars = predicate.get_variables();
            vars.sort();
            vars.dedup();

            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            use std::hash::{Hash, Hasher};
            format!("{predicate:?}").hash(&mut hasher);
            let predicate_hash = hasher.finish();

            rt.next_decision_for_branch(predicate_hash, vars, concolic_choice)
        } else {
            concolic_choice
        }
    })
}

pub fn reset_global_runtime() {
    TLS.with(|tls| {
        let mut tls = tls.borrow_mut();
        tls.default = None;
        tls.current = None;
    });
}

pub fn set_global_manager(manager: Arc<Mutex<SymExManager>>) {
    TLS.with(|tls| {
        // Wrap an externally managed SymExManager into a Runtime.
        let rt = Arc::new(Runtime {
            manager,
            config: RefCell::new(RuntimeConfig::default()),
            forced_decisions: RefCell::new(Vec::new()),
            branch_index: Cell::new(0),
            inputs: RefCell::new(HashMap::new()),
            branches: RefCell::new(Vec::new()),
            spawn_alternatives_for: RefCell::new(Vec::new()),
        });

        let mut tls = tls.borrow_mut();
        tls.default = Some(rt);
    });
}
