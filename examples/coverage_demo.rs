//! Demo: Print coverage information after a symbolic execution run
//!
//! Shows how to use the CoverageTracker alongside symbolic exploration
//! to measure which code paths were exercised, filtered by crate.
//!
//! Run with:
//! ```bash
//! RUSTFLAGS="-C instrument-coverage" cargo run --example coverage_demo
//! ```

use symexrs::coverage::{CoverageTracker, CountReward, RewardStrategy};
use symexrs::{explore, ExploreConfig, SourceMapper, SymI32, SymU64};

/// A small function under test with several branches
fn target_function(x: i32, y: i32) -> &'static str {
    if x > 0 {
        if y > 0 {
            if x + y > 100 {
                "large positive sum"
            } else {
                "small positive sum"
            }
        } else {
            "x positive, y non-positive"
        }
    } else if x == 0 {
        "x is zero"
    } else {
        if y < -10 {
            "both very negative"
        } else {
            "x negative, y not very negative"
        }
    }
}

fn main() {
    println!("=== Coverage Demo: Symbolic Execution + Coverage Tracking ===\n");

    // --- Step 1: Check coverage availability ---
    if !symexrs::coverage::is_coverage_available() {
        println!("Coverage not available!");
        println!("Recompile with: RUSTFLAGS='-C instrument-coverage' cargo run --example coverage_demo");
        return;
    }

    let tracker = CoverageTracker::new().expect("coverage init failed");
    println!(
        "Total LLVM counters in binary: {}\n",
        tracker.num_counters()
    );

    // --- Step 2: Build a SourceMapper to filter to our crates ---
    let mapper = SourceMapper::from_crates(&["coverage_demo", "symexrs"]);
    println!(
        "Counters belonging to [coverage_demo, symexrs]: {}",
        mapper.whitelisted_count()
    );

    // Also show per-crate breakdown
    let demo_mapper = SourceMapper::from_crates(&["coverage_demo"]);
    let lib_mapper = SourceMapper::from_crates(&["symexrs"]);
    println!("  coverage_demo: {}", demo_mapper.whitelisted_count());
    println!("  symexrs:       {}", lib_mapper.whitelisted_count());
    println!();

    // --- Step 3: Create a filtered CoverageTracker ---
    let coverage = CoverageTracker::new()
        .unwrap()
        .with_filter(mapper.into_filter());

    // --- Step 4: Run symbolic exploration while tracking coverage ---
    println!("--- Running symbolic exploration ---\n");

    // Rollout 1: Explore the target_function via symbolic execution
    coverage.start_rollout();

    let result = explore(ExploreConfig::new().with_max_paths(16), || {
        let x = SymI32::new();
        let y = SymI32::new();

        let cx = x.concrete_value().unwrap_or(0);
        let cy = y.concrete_value().unwrap_or(0);
        let label = target_function(cx, cy);

        // Use the symbolic values in branches so the engine forks paths
        if x > 0.into() {
            if y > 0.into() {
                if x.clone() + y.clone() > 100.into() {
                    // large positive sum path
                }
            }
        } else if x == 0.into() {
            // zero path
        } else {
            if y < (-10).into() {
                // both very negative path
            }
        }

        let _ = label; // suppress unused warning
        Ok(())
    });

    let delta1 = coverage.end_rollout();

    match &result {
        Ok(res) => {
            println!("Exploration result:");
            println!(
                "  Paths explored:      {}",
                res.exploration_result.paths_explored
            );
            println!(
                "  Satisfiable paths:   {}",
                res.exploration_result.satisfiable_paths
            );
            println!(
                "  Unsatisfiable paths: {}",
                res.exploration_result.unsatisfiable_paths
            );
            println!("  Runs executed:       {}", res.runs_executed);
            println!(
                "  Wall-clock time:     {} ms",
                res.total_run_time_ms
            );
        }
        Err(e) => println!("Exploration error: {:?}", e),
    }

    println!();
    println!("Coverage after rollout 1 (symbolic exploration):");
    println!("  Newly covered sites: {}", delta1.newly_covered_count);
    println!("  Coverage ratio:      {:.2}%", delta1.coverage_ratio * 100.0);

    let reward = CountReward.compute_reward(&delta1);
    println!("  Reward (count):      {:.0}", reward);

    // --- Step 5: Run a second rollout with a different scenario ---
    println!("\n--- Running a second rollout (concrete calls) ---\n");

    coverage.start_rollout();

    // Exercise some additional paths concretely
    let _ = target_function(0, 0);
    let _ = target_function(50, 60); // sum > 100
    let _ = target_function(-5, -20); // both very negative

    // Also exercise some symexrs API to cover library code
    let a = SymU64::from_concrete(42);
    let b = SymU64::from_concrete(58);
    let _ = a + b;

    let delta2 = coverage.end_rollout();

    println!("Coverage after rollout 2 (concrete calls):");
    println!("  Newly covered sites: {}", delta2.newly_covered_count);
    println!("  Coverage ratio:      {:.2}%", delta2.coverage_ratio * 100.0);

    let reward2 = CountReward.compute_reward(&delta2);
    println!("  Reward (count):      {:.0}", reward2);

    // --- Step 6: Print cumulative summary ---
    println!("\n--- Cumulative Coverage Summary ---\n");
    println!(
        "  Total covered:  {} / {} filtered counters",
        coverage.covered_count(),
        coverage.num_counters()
    );
    println!(
        "  Coverage ratio: {:.2}%",
        coverage.current_coverage_ratio() * 100.0
    );
    println!(
        "  Newly covered indices (rollout 2, first 20): {:?}",
        &delta2.newly_covered_indices[..delta2.newly_covered_indices.len().min(20)]
    );
}
