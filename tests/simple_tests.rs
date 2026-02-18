use symexrs::{SymI32, SymU32, SymU64, explore_default};

#[test]
fn test_arith() {
    // NOTE: This test uses from_concrete for y to ensure it has a fixed value.
    // If both x and y are created with new(), their concrete values may not be
    // updated correctly when arithmetic operations are involved, because we don't
    // have a global mechanism to update all variables when we get a model.
    let result = explore_default(|| {
        let x = SymU64::new();
        let y = SymU64::from_concrete(1u64);

        if x != y.clone() {
            if x == y.clone() + 10 {
                let cx = x.concrete_value().unwrap();
                let cy = y.concrete_value().unwrap();
                assert_eq!(cx, cy + 10);
            }
        } else {
            assert_eq!(x.concrete_value(), y.concrete_value());
        }
        Ok(())
    });
    assert!(result.is_ok());
}

#[test]
fn test_path() {
    println!("\n=== Testing Path Exploration with Comparison Operators ===\n");

    let result = explore_default(|| {
        let target = SymI32::new();

        println!("Before comparison: target = {:?}", target.concrete_value());

        // Now comparison operators should trigger path forking!

        if target < 1.into() || target > 10.into() {
            println!("Target is outside [1, 10]: {:?}", target.concrete_value());
            return Ok(());
        }

        // If we reach here, target should be between 1 and 10
        println!("Target is in [1, 10]");
        println!("  Concrete value: {:?}", target.concrete_value());
        println!("  Expected: value between 1 and 10");

        // Check if concrete value was updated
        if let Some(val) = target.concrete_value() {
            println!("  Actual value: {val}");
            if (1..=10).contains(&val) {
                println!("  ✓ Concrete value correctly updated!");
                // Verify the value actually satisfies the constraints
                assert!(
                    (1..=10).contains(&val),
                    "Concrete value {val} should be in [1, 10]"
                );
            } else {
                panic!("✗ Concrete value NOT updated correctly (got {val})");
            }
        } else {
            panic!("✗ No concrete value available");
        }

        Ok(())
    });

    match result {
        Ok(res) => {
            println!("\nExploration completed successfully:");
            println!(
                "  Paths explored: {}",
                res.exploration_result.paths_explored
            );
            println!(
                "  Satisfiable paths: {}",
                res.exploration_result.satisfiable_paths
            );
            println!(
                "  Unsatisfiable paths: {}",
                res.exploration_result.unsatisfiable_paths
            );

            // We should have explored multiple paths and found at least one satisfiable path
            // where target is in [1, 10]
            assert!(
                res.exploration_result.satisfiable_paths > 0,
                "Should have found at least one satisfiable path"
            );
        }
        Err(e) => {
            panic!("Error during exploration: {e:?}");
        }
    }
}

#[test]
fn symu32_from_concrete_behaves_as_literal() {
    let res = explore_default(|| {
        let x = SymU32::new();
        let y: u32 = 5;
        let z = SymU32::from_concrete(y);

        // On the branch where x == z, z is a constant 5, so x must also be 5.
        if x == z {
            assert!(x == 5u32.into());
        } else {
            assert!(x != 5u32.into());
        }

        Ok(())
    })
    .unwrap();

    assert_eq!(res.exploration_result.paths_explored, 2);
    assert_eq!(res.exploration_result.satisfiable_paths, 2);
}

#[test]
fn symu64_membership_over_concrete_array() {
    let res = explore_default(|| {
        // Concrete array of length 10
        let a: [u64; 10] = [3, 7, 11, 19, 23, 42, 100, 101, 2024, 9999];
        // One symbolic value
        let sym = SymU64::new();
        // Check whether sym is in a
        let mut in_array = false;
        for &v in &a {
            if sym == v.into() {
                in_array = true;
                break;
            }
        }
        // Sanity: the concrete model agrees with the membership result
        if let Some(c) = sym.concrete_value() {
            if in_array {
                assert!(a.contains(&c));
            } else {
                assert!(!a.contains(&c));
            }
        }
        Ok(())
    })
    .unwrap();
    // Expect n+1 (= 10+1) distinct satisfiable paths:
    // - 10 where sym == a[i]
    // - 1 where sym != any element of a
    assert_eq!(res.exploration_result.satisfiable_paths, 11);
    assert_eq!(res.exploration_result.unsatisfiable_paths, 0);
}

#[test]
fn no_sym() {
    let res = explore_default(|| {
        let a = 10;
        let b = 20;
        if a == b {
            unreachable!();
        }

        if a != b - 10 {
            unreachable!();
        }
        Ok(())
    })
    .unwrap();
    assert!(res.runs_executed == 1);
}
