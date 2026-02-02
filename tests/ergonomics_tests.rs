use rust_project::{explore_default, explore_with_manager, ExploreConfig, SymBool, SymU64};

#[test]
fn explore_default_requires_no_manager_args() {
    let res = explore_default(|| {
        let x = SymU64::new();
        let zero: SymU64 = 0u64.into();

        if x == zero {
            // then
        } else {
            // else
        }

        Ok(())
    })
    .unwrap();

    assert_eq!(res.exploration_result.paths_explored, 2);
    assert_eq!(res.exploration_result.satisfiable_paths, 2);
}

#[test]
fn explore_default_can_discover_multiple_frontier_branches_per_run() {
    // Two branch points. One combination is unsatisfiable: (x==0) && (x==1).
    let res = explore_default(|| {
        let x = SymU64::new();

        if x == 0u64.into() {
            // x == 0
        } else {
            // x != 0
        }

        if x == 1u64.into() {
            // x == 1
        } else {
            // x != 1
        }

        Ok(())
    })
    .unwrap();

    assert_eq!(res.exploration_result.paths_explored, 4);
    assert_eq!(res.exploration_result.satisfiable_paths, 3);
    assert_eq!(res.exploration_result.unsatisfiable_paths, 1);
}

#[test]
fn symbool_holds_is_ergonomic_in_if() {
    let res = explore_default(|| {
        let flag = SymBool::new();
        if flag.holds() {
            // flag == true
        } else {
            // flag == false
        }
        Ok(())
    })
    .unwrap();

    assert_eq!(res.exploration_result.paths_explored, 2);
    assert_eq!(res.exploration_result.satisfiable_paths, 2);
}

#[test]
fn explore_with_manager_keeps_old_style_working() {
    // Compatibility API: tests can keep passing a manager explicitly.
    let res = explore_with_manager(ExploreConfig::default(), |manager| {
        let x = SymU64::new_in(manager.clone());
        let y = SymU64::new_in(manager.clone());
        if x == y {
            // one path
        } else {
            // other path
        }
        Ok(())
    })
    .unwrap();

    assert_eq!(res.exploration_result.paths_explored, 2);
}
