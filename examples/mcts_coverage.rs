//! Example: MCTS-style exploration with LLVM coverage rewards
//!
//! This example demonstrates how to use coverage-guided exploration where:
//! 1. Concrete tests are run
//! 2. Coverage is measured via LLVM's instrument-coverage counters
//! 3. The reward (newly covered sites) guides exploration strategy
//!
//! To run with coverage:
//! ```bash
//! RUSTFLAGS="-C instrument-coverage" cargo run --example mcts_coverage
//! ```

use std::sync::Arc;
use symexrs::coverage::{
    CountReward, CoverageDelta, CoverageTracker, RarityReward, RarityTracker, RewardStrategy,
};
use symexrs::coverage_source_map::{SourceMapper, SourceMapperBuilder};
use symexrs::{explore, ExploreConfig, SymU64};

/// A simple MCTS-style rollout
struct MCTSExploration {
    coverage: CoverageTracker,
    reward_strategy: Box<dyn RewardStrategy>,
    total_reward: f64,
    num_rollouts: usize,
}

impl MCTSExploration {
    /// Create new MCTS exploration with simple count-based reward
    fn new() -> Option<Self> {
        let coverage = CoverageTracker::new()?;
        Some(Self {
            coverage,
            reward_strategy: Box::new(CountReward),
            total_reward: 0.0,
            num_rollouts: 0,
        })
    }

    /// Create new MCTS exploration with rarity-aware reward
    fn with_rarity() -> Option<Self> {
        let coverage = CoverageTracker::new()?;
        let rarity_tracker = Arc::new(RarityTracker::new()?);
        let reward_strategy = Box::new(RarityReward::new(rarity_tracker).with_weight(0.5));
        Some(Self {
            coverage,
            reward_strategy,
            total_reward: 0.0,
            num_rollouts: 0,
        })
    }

    /// Create from a custom coverage tracker
    fn with_tracker(coverage: CoverageTracker) -> Self {
        Self {
            coverage,
            reward_strategy: Box::new(CountReward),
            total_reward: 0.0,
            num_rollouts: 0,
        }
    }

    /// Run a single rollout (batch of concrete tests)
    fn rollout<F>(&mut self, test_fn: F) -> (CoverageDelta, f64)
    where
        F: FnOnce(),
    {
        // Start fresh counters for this rollout
        self.coverage.start_rollout();

        // Run the concrete tests
        test_fn();

        // Compute coverage delta and reward
        let delta = self.coverage.end_rollout();
        let reward = self.reward_strategy.compute_reward(&delta);

        self.total_reward += reward;
        self.num_rollouts += 1;

        (delta, reward)
    }

    /// Get average reward per rollout
    fn average_reward(&self) -> f64 {
        if self.num_rollouts == 0 {
            0.0
        } else {
            self.total_reward / self.num_rollouts as f64
        }
    }

    /// Get current coverage ratio
    fn coverage_ratio(&self) -> f64 {
        self.coverage.current_coverage_ratio()
    }

    /// Get cumulative coverage statistics
    fn coverage_stats(&self) -> (usize, usize, f64) {
        let covered = self.coverage.covered_count();
        let total = self.coverage.num_counters();
        let ratio = if total > 0 {
            covered as f64 / total as f64
        } else {
            0.0
        };
        (covered, total, ratio)
    }
}

fn main() {
    // Check if coverage is available
    if !symexrs::coverage::is_coverage_available() {
        println!("Warning: Coverage not available.");
        println!(
            "Compile with: RUSTFLAGS='-C instrument-coverage' cargo run --example mcts_coverage"
        );
        println!("Falling back to simple exploration without coverage...\n");

        // Run simple exploration without coverage
        run_simple_exploration();
        return;
    }

    println!("LLVM Coverage is available!");
    println!("Running MCTS-style exploration with coverage rewards...\n");

    // Debug: Print detected crates
    print_detected_crates();

    // Example 1: Simple count-based rewards
    run_count_based_exploration();

    println!();

    // Example 2: Rarity-aware rewards
    run_rarity_based_exploration();

    println!();

    // Example 3: Filtered coverage (exclude external libraries)
    run_filtered_exploration();
}

fn print_detected_crates() {
    println!("=== Detected Crates from Coverage Data ===");

    // Try to get crate information from the source mapper
    use symexrs::coverage_source_map::SourceMapper;

    // Create a mapper for a common crate to test if parsing works
    let mapper = SourceMapper::from_crates(&["std"]);
    println!("  Counters in 'std' crate: {}", mapper.whitelisted_count());

    let mapper = SourceMapper::from_crates(&["symexrs"]);
    println!(
        "  Counters in 'symexrs' crate: {}",
        mapper.whitelisted_count()
    );

    let mapper = SourceMapper::from_crates(&["mcts_coverage"]);
    println!(
        "  Counters in 'mcts_coverage' crate: {}",
        mapper.whitelisted_count()
    );

    println!();
}

fn run_simple_exploration() {
    println!("=== Simple Symbolic Exploration ===");

    let config = ExploreConfig::new().with_max_paths(4);

    let result = explore(config, || {
        let x = SymU64::new();
        let y = SymU64::new();

        // Some branches
        if x > 10u64.into() {
            if y > 20u64.into() {
                println!("  Path: x > 10, y > 20");
            } else {
                println!("  Path: x > 10, y <= 20");
            }
        } else {
            println!("  Path: x <= 10");
        }

        Ok(())
    });

    match result {
        Ok(stats) => {
            println!("Completed {} runs", stats.runs_executed);
        }
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }
}

fn run_count_based_exploration() {
    println!("=== MCTS with Count-Based Rewards ===");

    let mut mcts = match MCTSExploration::new() {
        Some(m) => m,
        None => {
            println!("Failed to initialize coverage");
            return;
        }
    };

    // Run several rollouts
    for rollout_id in 0..3 {
        println!("\nRollout {}:", rollout_id + 1);

        let (delta, reward) = mcts.rollout(|| {
            // Simulate concrete test execution
            // In real use, this would run your actual test code
            concrete_test_scenario(rollout_id);
        });

        let (covered, total, cumulative_ratio) = mcts.coverage_stats();

        println!("  New coverage sites: {}", delta.newly_covered_count);
        println!("  Reward: {:.2}", reward);
        println!("  Total coverage: {:.2}%", delta.coverage_ratio * 100.0);
        println!(
            "  Cumulative: {}/{} sites ({:.2}%)",
            covered,
            total,
            cumulative_ratio * 100.0
        );
    }

    println!("\nSummary:");
    println!("  Total rollouts: {}", mcts.num_rollouts);
    println!("  Average reward: {:.2}", mcts.average_reward());
    println!("  Final coverage: {:.2}%", mcts.coverage_ratio() * 100.0);
}

fn run_rarity_based_exploration() {
    println!("=== MCTS with Rarity-Aware Rewards ===");

    let mut mcts = match MCTSExploration::with_rarity() {
        Some(m) => m,
        None => {
            println!("Failed to initialize coverage with rarity");
            return;
        }
    };

    // Run several rollouts
    for rollout_id in 0..3 {
        println!("\nRollout {}:", rollout_id + 1);

        let (delta, reward) = mcts.rollout(|| {
            concrete_test_scenario(rollout_id);
        });

        let (covered, total, cumulative_ratio) = mcts.coverage_stats();

        println!("  New coverage sites: {}", delta.newly_covered_count);
        println!("  Reward (rarity-weighted): {:.2}", reward);
        if let Some(rarity) = delta.rarity_score {
            println!("  Rarity score: {:.2}", rarity);
        }
        println!(
            "  Cumulative: {}/{} sites ({:.2}%)",
            covered,
            total,
            cumulative_ratio * 100.0
        );
    }

    println!("\nSummary:");
    println!("  Total rollouts: {}", mcts.num_rollouts);
    println!("  Average reward: {:.2}", mcts.average_reward());
    println!("  Final coverage: {:.2}%", mcts.coverage_ratio() * 100.0);
}

fn run_filtered_exploration() {
    println!("=== MCTS with Source-Based Filtering ===");
    println!();

    // Get total counters to understand the range
    let temp_tracker = CoverageTracker::new();
    let total_counters = temp_tracker.as_ref().map(|t| t.num_counters()).unwrap_or(0);
    println!("Total coverage counters in binary: {}", total_counters);
    println!();

    // Example 1: Using SourceMapper with heuristic (include first 30% of counters)
    println!("Example 1: Heuristic filtering (first 30% of counters)");
    let mapper = SourceMapper::from_heuristic(total_counters, 0.3);
    println!(
        "  Including counters 0-{} ({} total)",
        mapper.whitelisted_count() - 1,
        mapper.whitelisted_count()
    );

    let coverage = match CoverageTracker::new() {
        Some(t) => t.with_filter(mapper.into_filter()),
        None => {
            println!("Failed to initialize coverage");
            return;
        }
    };

    let mut mcts = MCTSExploration::with_tracker(coverage);

    for rollout_id in 0..3 {
        let (delta, reward) = mcts.rollout(|| {
            concrete_test_scenario(rollout_id);
        });
        println!(
            "  Rollout {}: {} new sites, reward={:.2}",
            rollout_id + 1,
            delta.newly_covered_count,
            reward
        );
    }

    println!();

    // Example 2: Using SourceMapperBuilder for complex filtering
    println!("Example 2: Complex filtering with SourceMapperBuilder");
    let mapper = SourceMapperBuilder::new()
        .include_range(100, 300) // Include counters 100-299
        .exclude_indices(&[150, 151, 152]) // But exclude these
        .build();
    println!(
        "  Including {} counters (100-299 excluding 3)",
        mapper.whitelisted_count()
    );

    let coverage = match CoverageTracker::new() {
        Some(t) => t.with_filter(mapper.into_filter()),
        None => {
            println!("Failed to initialize coverage");
            return;
        }
    };

    let mut mcts = MCTSExploration::with_tracker(coverage);

    for rollout_id in 0..3 {
        let (delta, reward) = mcts.rollout(|| {
            concrete_test_scenario(rollout_id);
        });
        println!(
            "  Rollout {}: {} new sites, reward={:.2}",
            rollout_id + 1,
            delta.newly_covered_count,
            reward
        );
    }

    println!();

    // Example 3: Using specific indices
    println!("Example 3: Specific counter indices");
    let mapper = SourceMapper::from_indices(&[0, 1, 2, 10, 20, 30, 40, 50]);
    println!(
        "  Including {} specific counters",
        mapper.whitelisted_count()
    );

    let coverage = match CoverageTracker::new() {
        Some(t) => t.with_filter(mapper.into_filter()),
        None => {
            println!("Failed to initialize coverage");
            return;
        }
    };

    let mut mcts = MCTSExploration::with_tracker(coverage);

    for rollout_id in 0..3 {
        let (delta, reward) = mcts.rollout(|| {
            concrete_test_scenario(rollout_id);
        });
        println!(
            "  Rollout {}: {} new sites, reward={:.2}",
            rollout_id + 1,
            delta.newly_covered_count,
            reward
        );
    }

    println!();

    // Example 4: True source-based filtering by crate name
    println!("Example 4: Source-based filtering by crate name");
    println!("  This parses LLVM profile data to extract crate names from mangled symbols");

    // Try to filter by the current crate (mcts_coverage example)
    // In practice, you would use your actual crate name
    let mapper = SourceMapper::from_crates(&["mcts_coverage", "symexrs"]);

    if mapper.whitelisted_count() > 0 {
        println!(
            "  Found {} counters from specified crates",
            mapper.whitelisted_count()
        );

        let coverage = match CoverageTracker::new() {
            Some(t) => t.with_filter(mapper.into_filter()),
            None => {
                println!("Failed to initialize coverage");
                return;
            }
        };

        let mut mcts = MCTSExploration::with_tracker(coverage);

        for rollout_id in 0..3 {
            let (delta, reward) = mcts.rollout(|| {
                concrete_test_scenario(rollout_id);
            });
            println!(
                "  Rollout {}: {} new sites, reward={:.2}",
                rollout_id + 1,
                delta.newly_covered_count,
                reward
            );
        }
    } else {
        println!("  No counters found from specified crates");
        println!("  (This can happen if symbol parsing fails or if no functions match)");
    }

    println!("\nNote: The crate-based filtering parses:");
    println!("  - __llvm_prf_names section (zlib-compressed function names)");
    println!("  - Demangles Rust symbols to extract crate names");
    println!("  - Supports both legacy (_ZN) and v0 (_R) mangling schemes");
}

/// Simulates a concrete test scenario
fn concrete_test_scenario(seed: usize) {
    // This would be your actual test code
    // Different seeds exercise different code paths
    match seed % 3 {
        0 => {
            // Path A: Common code
            let _ = common_function();
        }
        1 => {
            // Path B: Common + rare
            let _ = common_function();
            let _ = rare_function_a();
        }
        2 => {
            // Path C: Common + different rare
            let _ = common_function();
            let _ = rare_function_b();
        }
        _ => unreachable!(),
    }
}

fn common_function() -> i32 {
    42
}

fn rare_function_a() -> i32 {
    100
}

fn rare_function_b() -> i32 {
    200
}
