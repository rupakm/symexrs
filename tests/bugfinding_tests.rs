use symexrs::{explore_default, replay, ExploreConfig, SymU64};

#[test]
fn exploration_records_user_panics_as_bug_cases() {
    let res = explore_default(|| {
        let x = SymU64::new();
        if x == 0u64.into() {
            panic!("boom");
        }
        Ok(())
    })
    .unwrap();

    assert_eq!(res.bugs.len(), 1);
    assert!(res.bugs[0].panic_message.contains("boom"));

    // We should still have explored the non-panicking path as a completed run.
    assert_eq!(res.exploration_result.paths_explored, 1);
    assert_eq!(res.exploration_result.satisfiable_paths, 1);

    // Branch site metadata should be populated.
    let br = res.bugs[0].branches.first().unwrap();
    assert!(!br.site_file.is_empty());
    assert!(br.site_id != 0);
}

#[test]
fn bug_case_is_replayable_from_decisions_and_inputs() {
    let res = explore_default(|| {
        let x = SymU64::new();
        if x == 0u64.into() {
            panic!("boom");
        }
        Ok(())
    })
    .unwrap();

    let bug = res.bugs.first().unwrap().clone();

    // Use the final concolic inputs captured for determinism.
    let mut work = bug.work.clone();
    work.inputs = bug.final_inputs.clone();

    let rr = replay(work, ExploreConfig::default(), || {
        let x = SymU64::new();
        if x == 0u64.into() {
            panic!("boom");
        }
        Ok(())
    })
    .unwrap();

    assert_eq!(rr.outcome, symexrs::RunOutcome::PanickedUser);
    assert!(rr.panic_message.unwrap_or_default().contains("boom"));
}
