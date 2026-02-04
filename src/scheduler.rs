//! Scheduling strategies for replay-based exploration.

use crate::engine::{RunResult, WorkItem};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet, VecDeque};

#[derive(Debug, Clone)]
pub enum SchedulerKind {
    Dfs,
    Bfs,
    Random { seed: u64 },
    CoverageGuided,
}

pub trait Scheduler {
    fn push(&mut self, work: WorkItem);
    fn pop(&mut self) -> Option<WorkItem>;

    fn on_run_result(&mut self, _work: &WorkItem, _result: &RunResult) {}
}

pub fn make_scheduler(kind: SchedulerKind) -> Box<dyn Scheduler> {
    match kind {
        SchedulerKind::Dfs => Box::new(DfsScheduler::new()),
        SchedulerKind::Bfs => Box::new(BfsScheduler::new()),
        SchedulerKind::Random { seed } => Box::new(RandomScheduler::new(seed)),
        SchedulerKind::CoverageGuided => Box::new(CoverageGuidedScheduler::new()),
    }
}

pub struct DfsScheduler {
    stack: Vec<WorkItem>,
}

impl DfsScheduler {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }
}

impl Scheduler for DfsScheduler {
    fn push(&mut self, work: WorkItem) {
        self.stack.push(work);
    }

    fn pop(&mut self) -> Option<WorkItem> {
        self.stack.pop()
    }
}

pub struct BfsScheduler {
    queue: VecDeque<WorkItem>,
}

impl BfsScheduler {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }
}

impl Scheduler for BfsScheduler {
    fn push(&mut self, work: WorkItem) {
        self.queue.push_back(work);
    }

    fn pop(&mut self) -> Option<WorkItem> {
        self.queue.pop_front()
    }
}

pub struct RandomScheduler {
    pending: Vec<WorkItem>,
    rng: XorShift64,
}

impl RandomScheduler {
    pub fn new(seed: u64) -> Self {
        Self {
            pending: Vec::new(),
            rng: XorShift64::new(seed),
        }
    }
}

impl Scheduler for RandomScheduler {
    fn push(&mut self, work: WorkItem) {
        self.pending.push(work);
    }

    fn pop(&mut self) -> Option<WorkItem> {
        if self.pending.is_empty() {
            return None;
        }
        let idx = (self.rng.next_u64() as usize) % self.pending.len();
        Some(self.pending.swap_remove(idx))
    }
}

pub struct CoverageGuidedScheduler {
    heap: BinaryHeap<ScoredWork>,
    seen_sites: HashSet<u64>,
    seq: u64,
}

impl CoverageGuidedScheduler {
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            seen_sites: HashSet::new(),
            seq: 0,
        }
    }

    fn score(&self, work: &WorkItem) -> i64 {
        let depth = work.forced.len() as i64;
        let novelty = match work.fork_site_id {
            Some(id) if !self.seen_sites.contains(&id) => 1,
            _ => 0,
        };

        // Primary: novelty, Secondary: go deeper if no novelty.
        novelty * 10_000 + depth
    }
}

impl Scheduler for CoverageGuidedScheduler {
    fn push(&mut self, work: WorkItem) {
        let score = self.score(&work);
        let seq = self.seq;
        self.seq = self.seq.wrapping_add(1);
        self.heap.push(ScoredWork { score, seq, work });
    }

    fn pop(&mut self) -> Option<WorkItem> {
        self.heap.pop().map(|s| s.work)
    }

    fn on_run_result(&mut self, _work: &WorkItem, result: &RunResult) {
        for b in &result.branches {
            self.seen_sites.insert(b.site_id);
        }
    }
}

#[derive(Debug)]
struct ScoredWork {
    score: i64,
    seq: u64,
    work: WorkItem,
}

impl PartialEq for ScoredWork {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score && self.seq == other.seq && self.work.id == other.work.id
    }
}

impl Eq for ScoredWork {}

impl PartialOrd for ScoredWork {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScoredWork {
    fn cmp(&self, other: &Self) -> Ordering {
        // Max-heap: higher score first. For stability, lower seq first.
        match self.score.cmp(&other.score) {
            Ordering::Equal => other.seq.cmp(&self.seq),
            o => o,
        }
    }
}

#[derive(Debug, Clone)]
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0xdead_beef_u64 } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
}
