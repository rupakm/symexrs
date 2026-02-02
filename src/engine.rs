//! Exploration engine (replay-based).
//!
//! The engine re-executes the user closure for different decision prefixes.
//! It relies on the runtime to consume forced decisions and to choose further
//! branches concolically (by concrete execution).

use crate::error::SymExResult;
use crate::expressions::ConstValue;
use crate::runtime::{
    set_current_runtime, BranchRecord, RunAbort, Runtime, RuntimeConfig, RuntimeMode,
};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplorationStrategy {
    DepthFirst,
    BreadthFirst,
}

#[derive(Debug, Clone)]
pub struct RunBudget {
    pub max_new_branches_to_record: usize,
    pub max_total_branches: Option<usize>,
}

impl Default for RunBudget {
    fn default() -> Self {
        Self {
            max_new_branches_to_record: 1024,
            max_total_branches: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExploreConfig {
    pub max_depth: usize,
    pub max_paths: Option<usize>,
    pub strategy: ExplorationStrategy,
    pub budget: RunBudget,
    /// If true, call the SMT solver at path completion to classify SAT/UNSAT.
    pub check_sat_on_complete: bool,
}

impl Default for ExploreConfig {
    fn default() -> Self {
        Self {
            max_depth: 100,
            max_paths: None,
            strategy: ExplorationStrategy::DepthFirst,
            budget: RunBudget::default(),
            check_sat_on_complete: true,
        }
    }
}

impl ExploreConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    pub fn with_max_paths(mut self, max_paths: usize) -> Self {
        self.max_paths = Some(max_paths);
        self
    }

    pub fn with_strategy(mut self, strategy: ExplorationStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    pub fn with_budget(mut self, budget: RunBudget) -> Self {
        self.budget = budget;
        self
    }

    pub fn with_sat_check_on_complete(mut self, enabled: bool) -> Self {
        self.check_sat_on_complete = enabled;
        self
    }
}

pub type WorkId = u64;

#[derive(Debug, Clone)]
pub struct WorkItem {
    pub id: WorkId,
    pub parent: Option<WorkId>,
    pub forced: Vec<bool>,
    pub inputs: HashMap<String, ConstValue>,
    pub priority: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    Completed,
    AbortedBudget,
    AbortedUnsat,
    PanickedUser,
}

#[derive(Debug, Clone)]
pub struct RunStats {
    pub branches_seen: usize,
    pub forced_branches: usize,
    pub concolic_branches: usize,
}

#[derive(Debug, Clone)]
pub struct RunResult {
    pub work_id: WorkId,
    pub outcome: RunOutcome,
    pub decisions_taken: Vec<bool>,
    pub branches: Vec<BranchRecord>,
    pub spawned: Vec<WorkItem>,
    pub is_sat: Option<bool>,
    pub stats: RunStats,
}

#[derive(Debug, Clone)]
pub struct ExplorationResult {
    pub paths_explored: usize,
    pub satisfiable_paths: usize,
    pub unsatisfiable_paths: usize,
    pub max_depth_reached: usize,
}

#[derive(Debug, Clone)]
pub struct ExploreResult {
    pub exploration_result: ExplorationResult,
    pub completed: bool,
    pub error: Option<String>,
}

pub struct Explorer {
    cfg: ExploreConfig,
    pub(crate) runtime: Arc<Runtime>,
    next_id: WorkId,
}

impl Explorer {
    pub fn new(cfg: ExploreConfig) -> SymExResult<Self> {
        Ok(Self {
            cfg,
            runtime: Runtime::new()?,
            next_id: 1,
        })
    }

    pub fn runtime(&self) -> Arc<Runtime> {
        Arc::clone(&self.runtime)
    }

    fn alloc_id(&mut self) -> WorkId {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    fn make_child(
        &mut self,
        parent: WorkId,
        forced: Vec<bool>,
        inputs: &HashMap<String, ConstValue>,
    ) -> WorkItem {
        WorkItem {
            id: self.alloc_id(),
            parent: Some(parent),
            forced,
            inputs: inputs.clone(),
            priority: 0.0,
        }
    }

    pub fn run_one<F>(&mut self, work: &WorkItem, f: &F) -> SymExResult<RunResult>
    where
        F: Fn() -> SymExResult<()>,
    {
        if work.forced.len() > self.cfg.max_depth {
            return Ok(RunResult {
                work_id: work.id,
                outcome: RunOutcome::AbortedBudget,
                decisions_taken: Vec::new(),
                branches: Vec::new(),
                spawned: Vec::new(),
                is_sat: None,
                stats: RunStats {
                    branches_seen: 0,
                    forced_branches: 0,
                    concolic_branches: 0,
                },
            });
        }

        self.runtime.set_config(RuntimeConfig {
            mode: RuntimeMode::Explore,
            max_new_branches_to_record: self.cfg.budget.max_new_branches_to_record,
            max_total_branches: self.cfg.budget.max_total_branches,
        });
        self.runtime
            .reset_for_run(work.forced.clone(), work.inputs.clone())?;

        set_current_runtime(Some(Arc::clone(&self.runtime)));
        let exec = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f()));
        set_current_runtime(None);

        let (outcome, user_ok) = match exec {
            Ok(Ok(())) => (RunOutcome::Completed, true),
            Ok(Err(_e)) => (RunOutcome::PanickedUser, false),
            Err(payload) => {
                if payload.downcast_ref::<RunAbort>().is_some() {
                    match *payload.downcast_ref::<RunAbort>().unwrap() {
                        RunAbort::BudgetReached => (RunOutcome::AbortedBudget, true),
                        RunAbort::PathUnsat => (RunOutcome::AbortedUnsat, true),
                    }
                } else {
                    // Treat user panics as a path outcome (useful for bug-finding).
                    (RunOutcome::PanickedUser, false)
                }
            }
        };

        let branches = self.runtime.branches_snapshot();
        let decisions_taken = self.runtime.decisions_taken();
        let inputs = self.runtime.inputs_snapshot();
        let alt_indices = self.runtime.spawn_alternatives_snapshot();

        let mut spawned = Vec::new();
        for i in alt_indices {
            if i >= decisions_taken.len() {
                continue;
            }
            let mut forced = decisions_taken[..=i].to_vec();
            forced[i] = !forced[i];
            spawned.push(self.make_child(work.id, forced, &inputs));
        }

        let forced_branches = branches.iter().filter(|b| b.was_forced).count();
        let concolic_branches = branches.len().saturating_sub(forced_branches);

        let mut is_sat = None;
        if user_ok && outcome == RunOutcome::Completed && self.cfg.check_sat_on_complete {
            let mgr_arc = self.runtime.manager();
            let mut mgr = mgr_arc.lock().unwrap();
            is_sat = Some(mgr.is_satisfiable().unwrap_or(false));
        }

        Ok(RunResult {
            work_id: work.id,
            outcome,
            decisions_taken,
            branches,
            spawned,
            is_sat,
            stats: RunStats {
                branches_seen: concolic_branches + forced_branches,
                forced_branches,
                concolic_branches,
            },
        })
    }

    pub fn explore<F>(&mut self, f: F) -> SymExResult<ExploreResult>
    where
        F: Fn() -> SymExResult<()>,
    {
        let root = WorkItem {
            id: 0,
            parent: None,
            forced: Vec::new(),
            inputs: HashMap::new(),
            priority: 0.0,
        };

        let mut queue: VecDeque<WorkItem> = VecDeque::new();
        queue.push_back(root);

        let mut explored = 0usize;
        let mut sat = 0usize;
        let mut unsat = 0usize;
        let mut max_depth = 0usize;

        while let Some(work) = match self.cfg.strategy {
            ExplorationStrategy::DepthFirst => queue.pop_back(),
            ExplorationStrategy::BreadthFirst => queue.pop_front(),
        } {
            if self.cfg.max_paths.map_or(false, |m| explored >= m) {
                break;
            }
            if work.forced.len() > self.cfg.max_depth {
                continue;
            }

            let rr = self.run_one(&work, &f)?;
            max_depth = max_depth.max(rr.decisions_taken.len());

            // Always enqueue spawned continuations.
            for s in rr.spawned {
                if s.forced.len() <= self.cfg.max_depth {
                    queue.push_back(s);
                }
            }

            if rr.outcome == RunOutcome::Completed {
                explored += 1;
                match rr.is_sat {
                    Some(true) => sat += 1,
                    Some(false) => unsat += 1,
                    None => {}
                }
            }
        }

        Ok(ExploreResult {
            exploration_result: ExplorationResult {
                paths_explored: explored,
                satisfiable_paths: sat,
                unsatisfiable_paths: unsat,
                max_depth_reached: max_depth,
            },
            completed: true,
            error: None,
        })
    }
}

pub fn explore(cfg: ExploreConfig, f: impl Fn() -> SymExResult<()>) -> SymExResult<ExploreResult> {
    let mut ex = Explorer::new(cfg)?;
    ex.explore(f)
}
