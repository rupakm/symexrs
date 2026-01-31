//! Benchmark demonstrating early unsatisfiability detection optimization
//!
//! This example shows how early detection of unsatisfiable paths
//! significantly reduces execution time by avoiding unnecessary work.

use rust_project::{SymU64, explore_default};
use std::sync::Arc;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Early Unsatisfiability Detection Benchmark ===\n");

    // Scenario 1: Early contradiction
    println!("Scenario 1: Early Contradiction");
    println!("Code: if x == 0 {{ x.assert_eq(&1)?; /* 100 more ops */ }}");

    let start = Instant::now();
    let result = explore_default(|manager| {
        let x = SymU64::new(Arc::clone(manager));
        let zero = SymU64::from_concrete(0, Arc::clone(manager));
        let one = SymU64::from_concrete(1, Arc::clone(manager));

        if x == zero {
            // Immediate contradiction - should terminate early
            x.assert_eq(&one)?;

            // Simulate 100 more operations that would be skipped
            for i in 0..100 {
                let y = SymU64::new(Arc::clone(manager));
                let val = SymU64::from_concrete(i, Arc::clone(manager));
                y.assert_gt(&val)?;
            }
        } else {
            x.assert_ne(&zero)?;
        }

        Ok(())
    })?;

    let duration = start.elapsed();
    println!("Time: {:?}", duration);
    println!(
        "Paths explored: {}",
        result.exploration_result.paths_explored
    );
    println!(
        "Satisfiable: {}",
        result.exploration_result.satisfiable_paths
    );
    println!(
        "Unsatisfiable: {}",
        result.exploration_result.unsatisfiable_paths
    );
    println!("Note: Unsatisfiable path terminated early, skipping 100 operations\n");

    // Scenario 2: Multiple contradictions
    println!("Scenario 2: Multiple Contradictory Paths");
    println!("Code: Multiple branches with contradictions");

    let start = Instant::now();
    let result = explore_default(|manager| {
        let x = SymU64::new(Arc::clone(manager));
        let y = SymU64::new(Arc::clone(manager));
        let zero = SymU64::from_concrete(0, Arc::clone(manager));
        let one = SymU64::from_concrete(1, Arc::clone(manager));

        // First branch
        if x == zero {
            x.assert_eq(&one)?; // Contradiction
        } else {
            x.assert_ne(&zero)?;
        }

        // Second branch
        if y == zero {
            y.assert_eq(&one)?; // Contradiction
        } else {
            y.assert_ne(&zero)?;
        }

        Ok(())
    })?;

    let duration = start.elapsed();
    println!("Time: {:?}", duration);
    println!(
        "Paths explored: {}",
        result.exploration_result.paths_explored
    );
    println!(
        "Satisfiable: {}",
        result.exploration_result.satisfiable_paths
    );
    println!(
        "Unsatisfiable: {}",
        result.exploration_result.unsatisfiable_paths
    );
    println!("Note: Multiple paths terminated early\n");

    // Scenario 3: Deep nesting with early contradiction
    println!("Scenario 3: Deep Nesting with Early Contradiction");
    println!("Code: Nested branches with contradiction at depth 1");

    let start = Instant::now();
    let result = explore_default(|manager| {
        let x = SymU64::new(Arc::clone(manager));
        let zero = SymU64::from_concrete(0, Arc::clone(manager));
        let one = SymU64::from_concrete(1, Arc::clone(manager));

        if x == zero {
            x.assert_eq(&one)?; // Contradiction at depth 1

            // These nested branches would create exponential paths
            // but are skipped due to early termination
            for _ in 0..5 {
                let z = SymU64::new(Arc::clone(manager));
                if z == zero {
                    z.assert_eq(&zero)?;
                } else {
                    z.assert_ne(&zero)?;
                }
            }
        } else {
            x.assert_ne(&zero)?;
        }

        Ok(())
    })?;

    let duration = start.elapsed();
    println!("Time: {:?}", duration);
    println!(
        "Paths explored: {}",
        result.exploration_result.paths_explored
    );
    println!(
        "Satisfiable: {}",
        result.exploration_result.satisfiable_paths
    );
    println!(
        "Unsatisfiable: {}",
        result.exploration_result.unsatisfiable_paths
    );
    println!("Note: Avoided exploring 2^5 = 32 nested paths due to early termination\n");

    // Scenario 4: Comparison - No contradictions
    println!("Scenario 4: Baseline - No Contradictions");
    println!("Code: Simple branches without contradictions");

    let start = Instant::now();
    let result = explore_default(|manager| {
        let x = SymU64::new(Arc::clone(manager));
        let y = SymU64::new(Arc::clone(manager));
        let zero = SymU64::from_concrete(0, Arc::clone(manager));

        if x == zero {
            x.assert_eq(&zero)?;
        } else {
            x.assert_ne(&zero)?;
        }

        if y == zero {
            y.assert_eq(&zero)?;
        } else {
            y.assert_ne(&zero)?;
        }

        Ok(())
    })?;

    let duration = start.elapsed();
    println!("Time: {:?}", duration);
    println!(
        "Paths explored: {}",
        result.exploration_result.paths_explored
    );
    println!(
        "Satisfiable: {}",
        result.exploration_result.satisfiable_paths
    );
    println!(
        "Unsatisfiable: {}",
        result.exploration_result.unsatisfiable_paths
    );
    println!("Note: All paths are satisfiable, no early termination\n");

    println!("=== Benchmark Complete ===");
    println!("\nKey Takeaway:");
    println!("Early unsatisfiability detection prevents wasted work on paths");
    println!("that are guaranteed to be unsatisfiable, significantly improving");
    println!("performance for code with contradictory constraints.");

    Ok(())
}
