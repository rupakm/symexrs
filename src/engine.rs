//! Exploration engine (replay-based).
//!
//! The engine re-executes the user closure for different decision prefixes.
//! It relies on the runtime to consume forced decisions and to choose further
//! branches concolically (by concrete execution).

use crate::decision::Decision;
use crate::error::SymExResult;
use crate::expressions::ConstValue;
use crate::runtime::{
    set_current_runtime, BranchRecord, RunAbort, Runtime, RuntimeConfig, RuntimeMode,
};
use crate::scheduler::{make_scheduler, Scheduler, SchedulerKind};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

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
    pub scheduler: SchedulerKind,
    pub budget: RunBudget,
    /// If true, call the SMT solver at path completion to classify SAT/UNSAT.
    pub check_sat_on_complete: bool,
}

impl Default for ExploreConfig {
    fn default() -> Self {
        Self {
            max_depth: 100,
            max_paths: None,
            scheduler: SchedulerKind::Dfs,
            budget: RunBudget::default(),
            check_sat_on_complete: true,
        }
    }
}

impl ExploreConfig {
    pub fn with_random_scheduler(mut self, seed: u64) -> Self {
        self.scheduler = SchedulerKind::Random { seed };
        self
    }

    pub fn with_coverage_guided_scheduler(mut self) -> Self {
        self.scheduler = SchedulerKind::CoverageGuided;
        self
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
        self.scheduler = match strategy {
            ExplorationStrategy::DepthFirst => SchedulerKind::Dfs,
            ExplorationStrategy::BreadthFirst => SchedulerKind::Bfs,
        };
        self
    }

    pub fn with_scheduler(mut self, scheduler: SchedulerKind) -> Self {
        self.scheduler = scheduler;
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
    pub forced: Vec<Decision>,
    pub inputs: HashMap<String, ConstValue>,
    pub fork_site_id: Option<u64>,
    pub fork_branch_index: Option<usize>,
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
    /// Wall-clock time spent executing the user closure for this run.
    pub duration_ms: u128,
    /// If the run ended in a user panic or error, capture a message.
    pub panic_message: Option<String>,
    /// Best-effort backtrace (may be empty depending on environment).
    pub backtrace: Option<String>,
    pub decisions_taken: Vec<Decision>,
    pub branches: Vec<BranchRecord>,
    pub spawned: Vec<WorkItem>,
    /// Concolic input store at the end of the run (may be updated by model repair).
    pub final_inputs: HashMap<String, ConstValue>,
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
    /// Bug cases (user panics) discovered during exploration.
    pub bugs: Vec<BugCase>,
    /// Total number of runs executed (includes aborted/panicked runs).
    pub runs_executed: usize,
    /// Total wall-clock time in all runs.
    pub total_run_time_ms: u128,
    pub completed: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BugCase {
    pub work: WorkItem,
    pub decisions_taken: Vec<Decision>,
    pub final_inputs: HashMap<String, ConstValue>,
    pub branches: Vec<BranchRecord>,
    pub panic_message: String,
    pub backtrace: Option<String>,
}

pub struct Explorer {
    cfg: ExploreConfig,
    pub(crate) runtime: Arc<Runtime>,
    next_id: WorkId,
    scheduler: Box<dyn Scheduler>,
}

impl Explorer {
    pub fn new(cfg: ExploreConfig) -> SymExResult<Self> {
        let scheduler = make_scheduler(cfg.scheduler.clone());
        Ok(Self {
            cfg,
            runtime: Runtime::new()?,
            next_id: 1,
            scheduler,
        })
    }

    /// Create an explorer with a custom scheduler implementation.
    ///
    /// This keeps `ExploreConfig` ergonomic for the built-in schedulers while
    /// allowing advanced users to provide a bespoke scheduler (MCTS/RL/etc.).
    pub fn new_with_scheduler(
        cfg: ExploreConfig,
        scheduler: Box<dyn Scheduler>,
    ) -> SymExResult<Self> {
        Ok(Self {
            cfg,
            runtime: Runtime::new()?,
            next_id: 1,
            scheduler,
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
        forced: Vec<Decision>,
        inputs: &HashMap<String, ConstValue>,
        fork_site_id: Option<u64>,
        fork_branch_index: Option<usize>,
    ) -> WorkItem {
        WorkItem {
            id: self.alloc_id(),
            parent: Some(parent),
            forced,
            inputs: inputs.clone(),
            fork_site_id,
            fork_branch_index,
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
                duration_ms: 0,
                panic_message: None,
                backtrace: None,
                decisions_taken: Vec::new(),
                branches: Vec::new(),
                spawned: Vec::new(),
                final_inputs: HashMap::new(),
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
        let start = Instant::now();
        let exec = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        let duration_ms = start.elapsed().as_millis();
        set_current_runtime(None);

        fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
            if let Some(s) = payload.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "<non-string panic payload>".to_string()
            }
        }

        let mut panic_message_out: Option<String> = None;
        let mut backtrace_out: Option<String> = None;

        let (outcome, user_ok) = match exec {
            Ok(Ok(())) => (RunOutcome::Completed, true),
            Ok(Err(e)) => {
                panic_message_out = Some(e.to_string());
                (RunOutcome::PanickedUser, false)
            }
            Err(payload) => {
                if payload.downcast_ref::<RunAbort>().is_some() {
                    match *payload.downcast_ref::<RunAbort>().unwrap() {
                        RunAbort::BudgetReached => (RunOutcome::AbortedBudget, true),
                        RunAbort::PathUnsat => (RunOutcome::AbortedUnsat, true),
                    }
                } else {
                    panic_message_out = Some(panic_message(&payload));
                    let bt = std::backtrace::Backtrace::capture();
                    backtrace_out = Some(format!("{bt}"));
                    (RunOutcome::PanickedUser, false)
                }
            }
        };

        let branches = self.runtime.branches_snapshot();
        let decisions_taken = self.runtime.decisions_taken();
        let inputs = self.runtime.inputs_snapshot();
        let alternatives = self.runtime.spawn_alternatives_snapshot();

        let mut spawned = Vec::new();
        for alt in alternatives {
            if alt.index >= decisions_taken.len() {
                continue;
            }
            let mut forced = decisions_taken[..=alt.index].to_vec();
            forced[alt.index] = alt.decision.clone();
            let fork_site_id = Some(alt.fork_site_id);
            spawned.push(self.make_child(
                work.id,
                forced,
                &inputs,
                fork_site_id,
                alt.fork_branch_index,
            ));
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
            duration_ms,
            panic_message: panic_message_out,
            backtrace: backtrace_out,
            decisions_taken,
            branches,
            spawned,
            final_inputs: inputs,
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
            fork_site_id: None,
            fork_branch_index: None,
        };

        self.scheduler.push(root);

        let mut explored = 0usize;
        let mut sat = 0usize;
        let mut unsat = 0usize;
        let mut max_depth = 0usize;
        let mut bugs: Vec<BugCase> = Vec::new();
        let mut runs_executed = 0usize;
        let mut total_run_time_ms: u128 = 0;

        while let Some(work) = self.scheduler.pop() {
            if self.cfg.max_paths.is_some_and(|m| explored >= m) {
                break;
            }
            if work.forced.len() > self.cfg.max_depth {
                continue;
            }

            let rr = self.run_one(&work, &f)?;
            runs_executed += 1;
            total_run_time_ms = total_run_time_ms.saturating_add(rr.duration_ms);
            max_depth = max_depth.max(rr.decisions_taken.len());

            // Let the scheduler learn from this run before pushing children.
            self.scheduler.on_run_result(&work, &rr);

            // Always enqueue spawned continuations.
            for s in rr.spawned {
                if s.forced.len() <= self.cfg.max_depth {
                    self.scheduler.push(s);
                }
            }

            if rr.outcome == RunOutcome::PanickedUser {
                if let Some(msg) = rr.panic_message.clone() {
                    bugs.push(BugCase {
                        work: work.clone(),
                        decisions_taken: rr.decisions_taken.clone(),
                        final_inputs: rr.final_inputs.clone(),
                        branches: rr.branches.clone(),
                        panic_message: msg,
                        backtrace: rr.backtrace.clone(),
                    });
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
            bugs,
            runs_executed,
            total_run_time_ms,
            completed: true,
            error: None,
        })
    }
}

pub fn replay<F>(work: WorkItem, cfg: ExploreConfig, f: F) -> SymExResult<RunResult>
where
    F: Fn() -> SymExResult<()>,
{
    let mut ex = Explorer::new(cfg)?;
    ex.run_one(&work, &f)
}

pub fn explore(cfg: ExploreConfig, f: impl Fn() -> SymExResult<()>) -> SymExResult<ExploreResult> {
    let mut ex = Explorer::new(cfg)?;
    ex.explore(f)
}

pub fn explore_with_scheduler(
    cfg: ExploreConfig,
    scheduler: Box<dyn Scheduler>,
    f: impl Fn() -> SymExResult<()>,
) -> SymExResult<ExploreResult> {
    let mut ex = Explorer::new_with_scheduler(cfg, scheduler)?;
    ex.explore(f)
}
