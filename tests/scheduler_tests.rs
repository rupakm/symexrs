use symexrs::{explore, ExploreConfig, SchedulerKind, SymU64};

fn run_two_completed_paths(cfg: ExploreConfig) -> Vec<Vec<bool>> {
    use symexrs::runtime;
    use std::sync::{Arc, Mutex};

    let log: Arc<Mutex<Vec<Vec<bool>>>> = Arc::new(Mutex::new(Vec::new()));
    let log2 = Arc::clone(&log);

    let _ = explore(
        cfg.with_max_paths(2).with_sat_check_on_complete(false),
        move || {
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
        },
    )
    .unwrap();

    Arc::try_unwrap(log).unwrap().into_inner().unwrap()
}

#[test]
fn dfs_scheduler_picks_deeper_alternative_first() {
    let runs = run_two_completed_paths(ExploreConfig::new().with_scheduler(SchedulerKind::Dfs));
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0], vec![true, true]);
    assert_eq!(runs[1], vec![true, false]);
}

#[test]
fn bfs_scheduler_picks_shallow_alternative_first() {
    let runs = run_two_completed_paths(ExploreConfig::new().with_scheduler(SchedulerKind::Bfs));
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0], vec![true, true]);
    assert_eq!(runs[1], vec![false, true]);
}

#[test]
fn random_scheduler_is_seeded_and_deterministic() {
    // RandomScheduler consumes RNG once to pop the root work item (len=1).
    // With the current xorshift and push order, seed=2 picks index 1 next.
    let runs = run_two_completed_paths(
        ExploreConfig::new().with_scheduler(SchedulerKind::Random { seed: 2 }),
    );
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0], vec![true, true]);
    assert_eq!(runs[1], vec![true, false]);
}

#[test]
fn coverage_guided_prefers_deeper_when_no_novelty() {
    // After the root run, both branch sites have been seen, so this strategy falls back to depth.
    let runs =
        run_two_completed_paths(ExploreConfig::new().with_scheduler(SchedulerKind::CoverageGuided));
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0], vec![true, true]);
    assert_eq!(runs[1], vec![true, false]);
}
