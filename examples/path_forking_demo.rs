//! Demonstration of automatic path forking in symbolic execution
//!
//! This example shows how the symbolic execution engine automatically
//! explores all paths when encountering symbolic comparisons.

use rust_project::{explore_default, SymU64};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Path Forking Demo ===\n");

    // Example 1: Simple branch
    println!("Example 1: Simple if-else branch");
    let result = explore_default(|manager| {
        let x = SymU64::new(Arc::clone(manager));
        let zero = SymU64::from_concrete(0, Arc::clone(manager));

        if x == zero {
            println!("  Branch: x == 0");
        } else {
            println!("  Branch: x != 0");
        }

        Ok(())
    })?;

    println!("Paths explored: {}", result.exploration_result.paths_explored);
    println!("Satisfiable paths: {}", result.exploration_result.satisfiable_paths);
    println!();

    // Example 2: Multiple branches
    println!("Example 2: Multiple independent branches");
    let result = explore_default(|manager| {
        let x = SymU64::new(Arc::clone(manager));
        let y = SymU64::new(Arc::clone(manager));
        let zero = SymU64::from_concrete(0, Arc::clone(manager));

        if x == zero {
            println!("  x == 0");
        } else {
            println!("  x != 0");
        }

        if y == zero {
            println!("  y == 0");
        } else {
            println!("  y != 0");
        }

        Ok(())
    })?;

    println!("Paths explored: {} (2^2 = 4 paths)", result.exploration_result.paths_explored);
    println!();

    // Example 3: Nested branches
    println!("Example 3: Nested branches");
    let result = explore_default(|manager| {
        let x = SymU64::new(Arc::clone(manager));
        let y = SymU64::new(Arc::clone(manager));
        let zero = SymU64::from_concrete(0, Arc::clone(manager));
        let one = SymU64::from_concrete(1, Arc::clone(manager));

        if x == zero {
            println!("  x == 0");
            if y == one {
                println!("    y == 1");
            } else {
                println!("    y != 1");
            }
        } else {
            println!("  x != 0");
        }

        Ok(())
    })?;

    println!("Paths explored: {} (3 paths: x==0&&y==1, x==0&&y!=1, x!=0)", result.exploration_result.paths_explored);
    println!();

    // Example 4: Constraints with branches
    println!("Example 4: Branches with additional constraints");
    let result = explore_default(|manager| {
        let x = SymU64::new(Arc::clone(manager));
        let zero = SymU64::from_concrete(0, Arc::clone(manager));
        let ten = SymU64::from_concrete(10, Arc::clone(manager));

        if x == zero {
            println!("  Path 1: x == 0");
        } else {
            println!("  Path 2: x != 0");
            // Add additional constraint: x < 10
            x.assert_lt(&ten)?;
        }

        Ok(())
    })?;

    println!("Paths explored: {}", result.exploration_result.paths_explored);
    println!("Satisfiable paths: {}", result.exploration_result.satisfiable_paths);
    println!();

    // Example 5: Unsatisfiable path
    println!("Example 5: Branch with unsatisfiable path");
    let result = explore_default(|manager| {
        let x = SymU64::new(Arc::clone(manager));
        let zero = SymU64::from_concrete(0, Arc::clone(manager));
        let one = SymU64::from_concrete(1, Arc::clone(manager));

        if x == zero {
            println!("  Path 1: x == 0");
            // Add contradictory constraint: x == 1 (but we already have x == 0)
            x.assert_eq(&one)?;
        } else {
            println!("  Path 2: x != 0");
        }

        Ok(())
    })?;

    println!("Paths explored: {}", result.exploration_result.paths_explored);
    println!("Satisfiable paths: {}", result.exploration_result.satisfiable_paths);
    println!("Unsatisfiable paths: {}", result.exploration_result.unsatisfiable_paths);
    println!();

    println!("=== Demo Complete ===");
    Ok(())
}
