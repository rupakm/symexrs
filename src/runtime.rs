//! Runtime state for symbolic execution.
//!
//! This module centralizes the per-run state needed by the symbolic types and
//! the exploration engine.

use crate::decision::Decision;
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
    /// Index in the unified decision stream.
    pub decision_index: usize,
    /// Stable identifier for the branch site (call location).
    pub site_id: u64,
    pub site_file: String,
    pub site_line: u32,
    pub site_column: u32,
    pub chosen: bool,
    pub was_forced: bool,
    pub predicate_hash: u64,
    pub vars: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ScheduleRecord {
    /// Index in the unified decision stream.
    pub decision_index: usize,
    pub site_id: u64,
    pub site_file: String,
    pub site_line: u32,
    pub site_column: u32,
    pub arity: u32,
    pub chosen_index: u32,
    pub was_forced: bool,
}

#[derive(Debug)]
pub enum RunAbort {
    BudgetReached,
    PathUnsat,
}

#[derive(Debug, Clone)]
pub struct AlternativeDecision {
    pub index: usize,
    pub decision: Decision,
    pub fork_site_id: u64,
    pub fork_branch_index: Option<usize>,
}

/// Central runtime object shared by symbolic values.
pub struct Runtime {
    pub(crate) manager: Arc<Mutex<SymExManager>>,
    config: RefCell<RuntimeConfig>,

    forced_decisions: RefCell<Vec<Decision>>,
    decision_index: Cell<usize>,
    decisions: RefCell<Vec<Decision>>,
    branch_ordinal: Cell<usize>,

    // Concolic seed inputs for this run.
    inputs: RefCell<HashMap<String, ConstValue>>,

    // Branch trace for the current run.
    branches: RefCell<Vec<BranchRecord>>,
    schedules: RefCell<Vec<ScheduleRecord>>,
    // Alternative decisions to spawn as new work.
    spawn_alternatives: RefCell<Vec<AlternativeDecision>>,
}

impl Runtime {
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new() -> SymExResult<Arc<Self>> {
        let solver = Box::new(Z3Solver::new()?);
        let manager = Arc::new(Mutex::new(SymExManager::new(solver)));
        Ok(Arc::new(Self {
            manager,
            config: RefCell::new(RuntimeConfig::default()),
            forced_decisions: RefCell::new(Vec::new()),
            decision_index: Cell::new(0),
            decisions: RefCell::new(Vec::new()),
            branch_ordinal: Cell::new(0),
            inputs: RefCell::new(HashMap::new()),
            branches: RefCell::new(Vec::new()),
            schedules: RefCell::new(Vec::new()),
            spawn_alternatives: RefCell::new(Vec::new()),
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
        forced_decisions: Vec<Decision>,
        inputs: HashMap<String, ConstValue>,
    ) -> SymExResult<()> {
        self.decision_index.set(0);
        self.branch_ordinal.set(0);
        *self.forced_decisions.borrow_mut() = forced_decisions;
        *self.inputs.borrow_mut() = inputs;
        self.decisions.borrow_mut().clear();
        self.branches.borrow_mut().clear();
        self.schedules.borrow_mut().clear();
        self.spawn_alternatives.borrow_mut().clear();

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

    pub fn schedules_snapshot(&self) -> Vec<ScheduleRecord> {
        self.schedules.borrow().clone()
    }

    pub fn spawn_alternatives_snapshot(&self) -> Vec<AlternativeDecision> {
        self.spawn_alternatives.borrow().clone()
    }

    pub fn decisions_taken(&self) -> Vec<Decision> {
        self.decisions.borrow().clone()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn next_decision_for_branch(
        &self,
        site_id: u64,
        site_file: String,
        site_line: u32,
        site_column: u32,
        predicate_hash: u64,
        vars: Vec<String>,
        concolic_choice: bool,
    ) -> bool {
        let cfg = self.config.borrow().clone();
        if cfg.mode != RuntimeMode::Explore {
            return concolic_choice;
        }

        let idx = self.decision_index.get();

        if let Some(max) = cfg.max_total_branches {
            if idx >= max {
                std::panic::panic_any(RunAbort::BudgetReached);
            }
        }

        let forced = self.forced_decisions.borrow();
        let (chosen, was_forced) = if let Some(d) = forced.get(idx).cloned() {
            match d {
                Decision::Bool(b) => (b, true),
                _ => (concolic_choice, false),
            }
        } else {
            (concolic_choice, false)
        };
        drop(forced);

        self.decisions.borrow_mut().push(Decision::Bool(chosen));

        let branch_ord = self.branch_ordinal.get();

        // Record branch.
        self.branches.borrow_mut().push(BranchRecord {
            decision_index: idx,
            site_id,
            site_file,
            site_line,
            site_column,
            chosen,
            was_forced,
            predicate_hash,
            vars,
        });

        // If this branch wasn't forced, register its alternative for scheduling.
        if !was_forced {
            let mut alts = self.spawn_alternatives.borrow_mut();
            if alts.len() < cfg.max_new_branches_to_record {
                alts.push(AlternativeDecision {
                    index: idx,
                    decision: Decision::Bool(!chosen),
                    fork_site_id: site_id,
                    fork_branch_index: Some(branch_ord),
                });
            } else {
                std::panic::panic_any(RunAbort::BudgetReached);
            }
        }

        self.decision_index.set(idx + 1);
        self.branch_ordinal.set(branch_ord + 1);
        chosen
    }

    pub fn next_decision_for_choice(
        &self,
        site_id: u64,
        site_file: String,
        site_line: u32,
        site_column: u32,
        arity: u32,
        concolic_index: u32,
    ) -> u32 {
        let cfg = self.config.borrow().clone();
        if cfg.mode != RuntimeMode::Explore {
            return concolic_index;
        }

        let idx = self.decision_index.get();

        if let Some(max) = cfg.max_total_branches {
            if idx >= max {
                std::panic::panic_any(RunAbort::BudgetReached);
            }
        }

        let forced = self.forced_decisions.borrow();
        let (chosen_index, was_forced) = if let Some(d) = forced.get(idx).cloned() {
            match d {
                Decision::Choice { arity: a, index } if a == arity && index < arity => {
                    (index, true)
                }
                _ => (concolic_index.min(arity.saturating_sub(1)), false),
            }
        } else {
            (concolic_index.min(arity.saturating_sub(1)), false)
        };
        drop(forced);

        self.decisions.borrow_mut().push(Decision::Choice {
            arity,
            index: chosen_index,
        });

        self.schedules.borrow_mut().push(ScheduleRecord {
            decision_index: idx,
            site_id,
            site_file,
            site_line,
            site_column,
            arity,
            chosen_index,
            was_forced,
        });

        if !was_forced {
            let mut alts = self.spawn_alternatives.borrow_mut();
            for j in 0..arity {
                if j == chosen_index {
                    continue;
                }
                if alts.len() < cfg.max_new_branches_to_record {
                    alts.push(AlternativeDecision {
                        index: idx,
                        decision: Decision::Choice { arity, index: j },
                        fork_site_id: site_id,
                        fork_branch_index: None,
                    });
                } else {
                    std::panic::panic_any(RunAbort::BudgetReached);
                }
            }
        }

        self.decision_index.set(idx + 1);
        chosen_index
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

    /// Format the complete runtime state as a human-readable string.
    ///
    /// This outputs:
    /// - Runtime configuration
    /// - Concrete input values
    /// - Symbolic variables and path constraints (from manager)
    /// - Decision trace
    /// - Branch trace
    pub fn format_state(&self) -> String {
        let mut output = String::new();
        
        output.push_str("╔═══════════════════════════════════════════════════════════════╗\n");
        output.push_str("║         SYMBOLIC EXECUTION ENGINE STATE                       ║\n");
        output.push_str("╚═══════════════════════════════════════════════════════════════╝\n\n");
        
        // Runtime configuration
        let cfg = self.config.borrow();
        output.push_str("Runtime Configuration:\n");
        output.push_str("----------------------\n");
        output.push_str(&format!("  Mode: {:?}\n", cfg.mode));
        output.push_str(&format!("  Max new branches to record: {}\n", cfg.max_new_branches_to_record));
        if let Some(max) = cfg.max_total_branches {
            output.push_str(&format!("  Max total branches: {max}\n"));
        } else {
            output.push_str("  Max total branches: unlimited\n");
        }
        drop(cfg); // Release borrow before locking manager
        output.push_str(&format!("  Current decision index: {}\n", self.decision_index.get()));
        output.push_str(&format!("  Current branch ordinal: {}\n", self.branch_ordinal.get()));
        output.push('\n');
        
        // Concrete input values
        output.push_str("Concrete Input Values:\n");
        output.push_str("----------------------\n");
        let inputs = self.inputs.borrow();
        if inputs.is_empty() {
            output.push_str("  (no concrete values)\n");
        } else {
            let mut sorted_inputs: Vec<_> = inputs.iter().collect();
            sorted_inputs.sort_by_key(|(k, _)| *k);
            for (name, value) in sorted_inputs {
                output.push_str(&format!("  {name} = {value}\n"));
            }
        }
        drop(inputs); // Release borrow before locking manager
        output.push('\n');
        
        // Symbolic state from manager
        if let Ok(mgr) = self.manager.try_lock() {
            output.push_str(&mgr.format_state());
        } else {
            output.push_str("Symbolic State:\n");
            output.push_str("---------------\n");
            output.push_str("  (manager is currently locked)\n");
        }
        output.push('\n');
        
        // Decision trace
        output.push_str("Decision Trace:\n");
        output.push_str("---------------\n");
        let decisions = self.decisions.borrow();
        if decisions.is_empty() {
            output.push_str("  (no decisions taken)\n");
        } else {
            for (i, decision) in decisions.iter().enumerate() {
                output.push_str(&format!("  [{i}] {decision:?}\n"));
            }
        }
        drop(decisions); // Release borrow
        output.push('\n');
        
        // Branch trace
        output.push_str("Branch Trace:\n");
        output.push_str("-------------\n");
        let branches = self.branches.borrow();
        if branches.is_empty() {
            output.push_str("  (no branches taken)\n");
        } else {
            for (i, branch) in branches.iter().enumerate() {
                output.push_str(&format!("  [{}] {}:{}:{} - ", 
                    i, branch.site_file, branch.site_line, branch.site_column));
                output.push_str(&format!("chose {}, ", branch.chosen));
                if branch.was_forced {
                    output.push_str("forced");
                } else {
                    output.push_str("concolic");
                }
                if !branch.vars.is_empty() {
                    output.push_str(&format!(", vars: [{}]", branch.vars.join(", ")));
                }
                output.push('\n');
            }
        }
        drop(branches); // Release borrow
        output.push('\n');
        
        // Alternative decisions to spawn
        output.push_str("Alternative Decisions (to spawn):\n");
        output.push_str("----------------------------------\n");
        let alts = self.spawn_alternatives.borrow();
        if alts.is_empty() {
            output.push_str("  (no alternatives)\n");
        } else {
            for (i, alt) in alts.iter().enumerate() {
                output.push_str(&format!("  [{}] At decision index {}: {:?}\n", 
                    i, alt.index, alt.decision));
            }
        }
        
        output.push_str("\n╔═══════════════════════════════════════════════════════════════╗\n");
        output.push_str("║                    END OF STATE                               ║\n");
        output.push_str("╚═══════════════════════════════════════════════════════════════╝\n");
        
        output
    }

    /// Print the complete runtime state to stdout.
    pub fn print_state(&self) {
        print!("{}", self.format_state());
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

#[track_caller]
pub fn choose_branch(predicate: &SymExpr, concolic_choice: bool) -> bool {
    with_current_runtime(|rt| {
        if let Some(rt) = rt {
            let mut vars = predicate.get_variables();
            vars.sort();
            vars.dedup();

            let loc = std::panic::Location::caller();
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            use std::hash::{Hash, Hasher};
            loc.file().hash(&mut hasher);
            loc.line().hash(&mut hasher);
            loc.column().hash(&mut hasher);
            let site_id = hasher.finish();

            let predicate_hash = predicate.canonical_hash();

            rt.next_decision_for_branch(
                site_id,
                loc.file().to_string(),
                loc.line(),
                loc.column(),
                predicate_hash,
                vars,
                concolic_choice,
            )
        } else {
            concolic_choice
        }
    })
}

#[track_caller]
pub fn choose_choice(arity: u32, concolic_index: u32) -> u32 {
    with_current_runtime(|rt| {
        if let Some(rt) = rt {
            let loc = std::panic::Location::caller();
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            use std::hash::{Hash, Hasher};
            loc.file().hash(&mut hasher);
            loc.line().hash(&mut hasher);
            loc.column().hash(&mut hasher);
            let site_id = hasher.finish();

            rt.next_decision_for_choice(
                site_id,
                loc.file().to_string(),
                loc.line(),
                loc.column(),
                arity,
                concolic_index,
            )
        } else {
            concolic_index
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
            decision_index: Cell::new(0),
            decisions: RefCell::new(Vec::new()),
            branch_ordinal: Cell::new(0),
            inputs: RefCell::new(HashMap::new()),
            branches: RefCell::new(Vec::new()),
            schedules: RefCell::new(Vec::new()),
            spawn_alternatives: RefCell::new(Vec::new()),
        });

        let mut tls = tls.borrow_mut();
        tls.default = Some(rt);
    });
}
