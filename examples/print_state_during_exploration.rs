//! Demonstration of printing symbolic execution state during path exploration
//!
//! This example shows how to inspect the symbolic execution engine's state
//! during actual path exploration, where constraints are accumulated.

use symexrs::{ExploreConfig, SymExResult, SymU64, explore};

fn target_function() -> SymExResult<()> {
    let x = SymU64::new();
    let y = SymU64::new();

    println!(
        "\nCreated symbolic variables: {} and {}",
        x.variable_name(),
        y.variable_name()
    );

    // First branch
    if x.gt(&y) {
        println!("Branch 1: x > y");

        let sum = &x + &y;
        let hundred = SymU64::from_concrete(100);

        // Second branch
        if sum.gt(&hundred) {
            println!("Branch 2: x + y > 100");
            println!("Path: x > y AND x + y > 100");
        } else {
            println!("Branch 2: x + y <= 100");
            println!("Path: x > y AND x + y <= 100");
        }
    } else {
        println!("Branch 1: x <= y");

        let diff = &y - &x;
        let fifty = SymU64::from_concrete(50);

        // Second branch
        if diff.lt(&fifty) {
            println!("Branch 2: y - x < 50");
            println!("Path: x <= y AND y - x < 50");
        } else {
            println!("Branch 2: y - x >= 50");
            println!("Path: x <= y AND y - x >= 50");
        }
    }

    Ok(())
}

fn main() -> SymExResult<()> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║   Symbolic Execution State During Path Exploration           ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    let config = ExploreConfig::default()
        .with_max_depth(10)
        .with_max_paths(4);

    println!("Starting path exploration...\n");
    println!("Configuration:");
    println!("  Max depth: 10");
    println!("  Max paths: 4");
    println!("  Strategy: DFS (default)\n");

    let result = explore(config, target_function)?;

    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║                  Exploration Results                          ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    println!(
        "Paths explored: {}",
        result.exploration_result.paths_explored
    );
    println!(
        "Satisfiable paths: {}",
        result.exploration_result.satisfiable_paths
    );
    println!(
        "Unsatisfiable paths: {}",
        result.exploration_result.unsatisfiable_paths
    );
    println!(
        "Max depth reached: {}",
        result.exploration_result.max_depth_reached
    );
    println!("Total runs executed: {}", result.runs_executed);
    println!("Total execution time: {} ms", result.total_run_time_ms);

    if !result.bugs.is_empty() {
        println!("\nBugs found: {}", result.bugs.len());
        for (i, bug) in result.bugs.iter().enumerate() {
            println!("  Bug {}: {}", i + 1, bug.panic_message);
        }
    } else {
        println!("\nNo bugs found!");
    }

    Ok(())
}
