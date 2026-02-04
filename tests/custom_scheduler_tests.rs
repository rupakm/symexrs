use symexrs::scheduler::Scheduler;
use symexrs::{explore_with_scheduler, ExploreConfig, SymU64};

// A tiny custom scheduler that always behaves like BFS.
struct AlwaysBfs {
    q: std::collections::VecDeque<symexrs::WorkItem>,
}

impl AlwaysBfs {
    fn new() -> Self {
        Self {
            q: std::collections::VecDeque::new(),
        }
    }
}

impl Scheduler for AlwaysBfs {
    fn push(&mut self, work: symexrs::WorkItem) {
        self.q.push_back(work);
    }

    fn pop(&mut self) -> Option<symexrs::WorkItem> {
        self.q.pop_front()
    }
}

#[test]
fn explore_with_custom_scheduler_works() {
    use symexrs::runtime;
    use std::sync::{Arc, Mutex};

    let log: Arc<Mutex<Vec<Vec<bool>>>> = Arc::new(Mutex::new(Vec::new()));
    let log2 = Arc::clone(&log);

    let cfg = ExploreConfig::new()
        .with_max_paths(2)
        .with_sat_check_on_complete(false);

    explore_with_scheduler(cfg, Box::new(AlwaysBfs::new()), move || {
        let x = SymU64::new();
        let y = SymU64::new();
        let zero: SymU64 = 0u64.into();

        let _ = x == zero;
        let _ = y == zero;

        runtime::with_current_runtime(|rt| {
            let rt = rt.expect("explore should set current runtime");
            let bools: Vec<bool> = rt
                .decisions_taken()
                .into_iter()
                .map(|d| d.as_bool().expect("expected bool decision"))
                .collect();
            log2.lock().unwrap().push(bools);
        });

        Ok(())
    })
    .unwrap();

    let runs = log.lock().unwrap();
    assert_eq!(runs.len(), 2);
    // Root path is the concolic default (true because initial concrete is 0).
    assert_eq!(runs[0], vec![true, true]);
    // BFS chooses the shallow alternative first.
    assert_eq!(runs[1], vec![false, true]);
}
