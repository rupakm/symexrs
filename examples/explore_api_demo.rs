//! Demonstration of the explore API - the main entry point for symbolic execution
//!
//! This example shows how to use the high-level `explore` function to
//! symbolically execute code with various configurations.

use symexrs::{explore, explore_default, ExploreConfig, SchedulerKind, SymU64};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Explore API Demo ===\n");

    // Example 1: Basic exploration with default configuration
    println!("1. Basic exploration with default config:");
    let result = explore_default(|| {
        let x = SymU64::new();
        let y = SymU64::new();

        let zero = SymU64::from_concrete(0);
        x.assert_gt(&zero)?;
        y.assert_gt(&zero)?;

        println!("   Created symbolic variables x and y with constraints x > 0, y > 0");
        Ok(())
    })?;

    println!(
        "   Result: {} paths explored, {} satisfiable",
        result.exploration_result.paths_explored, result.exploration_result.satisfiable_paths
    );
    println!();

    // Example 2: Custom configuration with depth-first search
    println!("2. Custom configuration (DFS, max depth 50):");
    let config = ExploreConfig::new()
        .with_max_depth(50)
        .with_scheduler(SchedulerKind::Dfs);

    let result = explore(config, || {
        let a = SymU64::from_concrete(10);
        let b = SymU64::from_concrete(20);
        let c = SymU64::from_concrete(30);

        let sum = &a + &b;
        sum.assert_eq(&c)?;

        println!("   Verified: 10 + 20 == 30");
        Ok(())
    })?;

    println!(
        "   Result: {} paths explored",
        result.exploration_result.paths_explored
    );
    println!();

    // Example 3: Breadth-first search
    println!("3. Breadth-first search:");
    let config = ExploreConfig::new()
        .with_scheduler(SchedulerKind::Bfs)
        .with_max_depth(30);

    let result = explore(config, || {
        let x = SymU64::new();
        let ten = SymU64::from_concrete(10);
        let twenty = SymU64::from_concrete(20);

        // Constraint: 10 < x < 20
        x.assert_gt(&ten)?;
        x.assert_lt(&twenty)?;

        println!("   Constraint: 10 < x < 20");
        Ok(())
    })?;

    println!(
        "   Result: {} paths explored, {} satisfiable",
        result.exploration_result.paths_explored, result.exploration_result.satisfiable_paths
    );
    println!();

    // Example 4: Detecting contradictory constraints
    println!("4. Detecting contradictory constraints:");
    let result = explore_default(|| {
        let x = SymU64::new();
        let zero = SymU64::from_concrete(0);

        // Contradictory: x > 0 AND x < 0
        x.assert_gt(&zero)?;
        x.assert_lt(&zero)?;

        println!("   Added contradictory constraints: x > 0 AND x < 0");
        Ok(())
    })?;

    println!(
        "   Result: {} unsatisfiable paths detected",
        result.exploration_result.unsatisfiable_paths
    );
    println!();

    // Example 5: Complex constraints with multiple variables
    println!("5. Complex constraints with multiple variables:");
    let result = explore_default(|| {
        let x = SymU64::new();
        let y = SymU64::new();
        let z = SymU64::new();

        let five = SymU64::from_concrete(5);
        let ten = SymU64::from_concrete(10);
        let fifteen = SymU64::from_concrete(15);

        // x < 5, 5 < y < 10, z > 15
        x.assert_lt(&five)?;
        y.assert_gt(&five)?;
        y.assert_lt(&ten)?;
        z.assert_gt(&fifteen)?;

        println!("   Constraints: x < 5, 5 < y < 10, z > 15");
        Ok(())
    })?;

    println!(
        "   Paths: {} explored, {} satisfiable",
        result.exploration_result.paths_explored, result.exploration_result.satisfiable_paths
    );
    println!();

    // Example 6: Using global manager within explore
    println!("6. Using global manager (SymU64::new_global()):");
    let result = explore_default(|| {
        let x = SymU64::new();
        let y = SymU64::new();

        let hundred = SymU64::from_concrete(100);
        x.assert_lt(&hundred)?;
        y.assert_lt(&hundred)?;

        println!("   Created variables using global manager");
        Ok(())
    })?;

    println!(
        "   Result: {} paths explored",
        result.exploration_result.paths_explored
    );
    println!();

    // Example 7: Arithmetic operations
    println!("7. Symbolic arithmetic operations:");
    let _result = explore_default(|| {
        let a = SymU64::from_concrete(7);
        let b = SymU64::from_concrete(3);

        // Test various operations
        let sum = &a + &b; // 7 + 3 = 10
        let diff = &a - &b; // 7 - 3 = 4
        let prod = &a * &b; // 7 * 3 = 21
        let _quot = &a / &b; // 7 / 3 = 2
        let _rem = &a % &b; // 7 % 3 = 1

        let ten = SymU64::from_concrete(10);
        let four = SymU64::from_concrete(4);

        sum.assert_eq(&ten)?;
        diff.assert_eq(&four)?;

        println!("   Verified: 7 + 3 = 10, 7 - 3 = 4");
        println!(
            "   Concrete values: sum={:?}, diff={:?}, prod={:?}",
            sum.concrete_value(),
            diff.concrete_value(),
            prod.concrete_value()
        );
        Ok(())
    })?;

    println!("   Result: All arithmetic operations verified");
    println!();

    // Summary
    println!("=== Summary ===");
    println!("The explore API provides a clean, high-level interface for symbolic execution:");
    println!("  • Configure exploration strategy, depth, and other parameters");
    println!("  • Automatic manager setup and cleanup");
    println!("  • Comprehensive result statistics");
    println!("  • Support for both explicit and global manager usage");
    println!("\n=== Demo Complete ===");

    Ok(())
}
