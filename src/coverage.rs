//! LLVM coverage counter integration for online reward computation.
//!
//! This module provides safe, in-process access to LLVM's instrumentation counters
//! without filesystem I/O. Designed for MCTS-style exploration where reward is
//! based on newly-covered code locations.
//!
//! # Features
//! - Zero filesystem I/O - reads counters directly from memory
//! - Thread-safe snapshot-based design (Option A: one snapshot per rollout)
//! - Rarity tracking with cumulative hit counts (strategy easily swappable)
//! - Dense storage (Vec<bool>) with path to sparse for large programs
//! - Filters to exclude external crates/system libraries
//!
//! # Usage
//! ```rust,no_run
//! use symexrs::coverage::{CoverageTracker, RarityTracker, CountReward, RewardStrategy};
//!
//! // Initialize (returns None if binary not compiled with -Z instrument-coverage)
//! let mut coverage = CoverageTracker::new().expect("Coverage not available");
//!
//! // Start a rollout
//! coverage.start_rollout();
//!
//! // ... run concrete tests ...
//!
//! // Get reward
//! let delta = coverage.end_rollout();
//! let reward = CountReward.compute_reward(&delta);
//! ```
//!
//! # Filtering External Libraries
//!
//! To exclude external libraries from coverage, use [`CoverageTracker::with_filter`]:
//!
//! ```rust,no_run
//! use symexrs::coverage::CoverageTracker;
//!
//! // Heuristic: only include first 50% of counters
//! // (In practice, you'd want source-based filtering by parsing __llvm_prf_data)
//! let tracker = CoverageTracker::new()
//!     .expect("Coverage not available")
//!     .with_filter(|idx| idx < 1000);
//! ```
//!
//! For more sophisticated filtering, use the [`SourceMapper`](crate::coverage_source_map::SourceMapper):
//!
//! ```rust,no_run
//! use symexrs::coverage::CoverageTracker;
//! use symexrs::coverage_source_map::SourceMapper;
//!
//! // Filter using counter index ranges
//! let mapper = SourceMapper::from_heuristic(1000, 0.3); // First 30%
//! let tracker = CoverageTracker::new()
//!     .expect("Coverage not available")
//!     .with_filter(mapper.into_filter());
//! ```
//!
//! For full source-based filtering (by crate name), you'd need to:
//! 1. Parse the `__llvm_prf_names` section (zlib-compressed function names)
//! 2. Map function names to counter indices via `__llvm_prf_data`
//! 3. Match function prefixes to crate names
//!
//! See the `mcts_coverage` example and [`coverage_source_map`](crate::coverage_source_map) module for details.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

// =============================================================================
// Configuration and Status
// =============================================================================

/// Whether coverage counters are available at runtime.
static COVERAGE_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Number of counters (cached after first detection).
static NUM_COUNTERS: AtomicU64 = AtomicU64::new(0);

/// Result of checking for coverage availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageStatus {
    /// Counters are available and functional.
    Available { num_counters: usize },
    /// Not compiled with coverage instrumentation.
    Unavailable,
}

/// Strategy for computing rewards from coverage deltas.
pub trait RewardStrategy: Send + Sync {
    /// Compute a scalar reward from a coverage delta.
    fn compute_reward(&self, delta: &CoverageDelta) -> f64;
}

/// Simple count-based reward (number of newly covered sites).
#[derive(Debug, Clone, Copy, Default)]
pub struct CountReward;

impl RewardStrategy for CountReward {
    fn compute_reward(&self, delta: &CoverageDelta) -> f64 {
        delta.newly_covered_count as f64
    }
}

/// Rarity-aware reward: rare paths score higher.
#[derive(Debug, Clone)]
pub struct RarityReward {
    tracker: Arc<RarityTracker>,
    /// Weight for raw count vs rarity (0.0 = pure rarity, 1.0 = pure count).
    count_weight: f64,
}

impl RarityReward {
    /// Create with equal weighting between count and rarity.
    pub fn new(tracker: Arc<RarityTracker>) -> Self {
        Self {
            tracker,
            count_weight: 0.5,
        }
    }

    /// Set the weight between count and rarity.
    pub fn with_weight(mut self, weight: f64) -> Self {
        self.count_weight = weight.clamp(0.0, 1.0);
        self
    }

    /// Get a reference to the rarity tracker.
    pub fn tracker(&self) -> &Arc<RarityTracker> {
        &self.tracker
    }
}

impl RewardStrategy for RarityReward {
    fn compute_reward(&self, delta: &CoverageDelta) -> f64 {
        let count_component = delta.newly_covered_count as f64;
        let rarity_component = delta.rarity_score.unwrap_or(1.0);
        self.count_weight * count_component + (1.0 - self.count_weight) * rarity_component
    }
}

// =============================================================================
// Coverage Delta
// =============================================================================

/// Represents new coverage discovered in a single rollout.
#[derive(Debug, Clone, Default)]
pub struct CoverageDelta {
    /// Number of newly covered sites (for quick access).
    pub newly_covered_count: usize,
    /// Indices of counters that went from 0 to >0.
    pub newly_covered_indices: Vec<usize>,
    /// Rarity score (if rarity tracking enabled).
    pub rarity_score: Option<f64>,
    /// Total coverage ratio after this rollout (0.0 to 1.0).
    pub coverage_ratio: f64,
}

impl CoverageDelta {
    /// Create an empty delta.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Returns true if any new coverage was found.
    pub fn has_new_coverage(&self) -> bool {
        self.newly_covered_count > 0
    }
}

// =============================================================================
// Coverage Tracker (Main API)
// =============================================================================

/// Tracks coverage across rollouts using dense storage.
/// Thread-safe: supports parallel test execution within a rollout.
pub struct CoverageTracker {
    /// Total number of counters in the binary.
    num_counters: usize,
    /// Which counters have been seen across all rollouts (dense bitset).
    /// Using RwLock to allow concurrent reads during reward computation.
    seen: RwLock<Vec<bool>>,
    /// Optional rarity tracker for biasing.
    rarity_tracker: Option<Arc<RarityTracker>>,
    /// Optional filter to exclude external crates.
    filter: Option<Box<dyn Fn(usize) -> bool + Send + Sync>>,
}

impl CoverageTracker {
    /// Initialize coverage tracking.
    /// Returns None if binary was not compiled with coverage instrumentation.
    pub fn new() -> Option<Self> {
        let status = init_coverage();
        match status {
            CoverageStatus::Available { num_counters } => {
                if num_counters == 0 {
                    return None;
                }
                Some(Self {
                    num_counters,
                    seen: RwLock::new(vec![false; num_counters]),
                    rarity_tracker: None,
                    filter: None,
                })
            }
            CoverageStatus::Unavailable => None,
        }
    }

    /// Enable rarity tracking with the given tracker.
    pub fn with_rarity_tracker(mut self, tracker: Arc<RarityTracker>) -> Self {
        self.rarity_tracker = Some(tracker);
        self
    }

    /// Set a filter to exclude certain counters (e.g., external crates).
    /// The function receives a counter index and should return true to include it.
    ///
    /// # Excluding External Libraries
    ///
    /// To exclude external libraries from coverage, you need to map counter indices
    /// to their source files. This requires parsing the `__llvm_prf_data` section
    /// which contains the mapping from counters to source locations.
    ///
    /// ## Current Approach (Heuristic)
    ///
    /// Without source mapping, you can use heuristics based on counter indices:
    /// - Run a calibration to identify which counters are in your code vs external libs
    /// - Use index ranges (e.g., first 50% are typically standard library)
    /// - Profile to identify "hot" counters that belong to your code
    ///
    /// ## Future Enhancement
    ///
    /// Full source-based filtering would require:
    /// 1. Reading the `__llvm_prf_data` section at initialization
    /// 2. Parsing the InstrProfRecord format (version-dependent)
    /// 3. Building a whitelist of counter indices from your crate's source files
    /// 4. Using `with_filter` with that whitelist
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use symexrs::coverage::CoverageTracker;
    ///
    /// // Only include counters in the first half (heuristic)
    /// let tracker = CoverageTracker::new()
    ///     .expect("Coverage not available")
    ///     .with_filter(|idx| idx < 100);
    /// ```
    pub fn with_filter<F>(mut self, filter: F) -> Self
    where
        F: Fn(usize) -> bool + Send + Sync + 'static,
    {
        self.filter = Some(Box::new(filter));
        self
    }

    /// Start a new rollout. Resets counters to zero.
    /// Call this before running concrete tests.
    pub fn start_rollout(&self) {
        reset_counters();
    }

    /// End a rollout and compute the coverage delta.
    /// Call this after concrete tests complete.
    pub fn end_rollout(&self) -> CoverageDelta {
        let counters = match read_counters() {
            Some(c) => c,
            None => return CoverageDelta::empty(),
        };

        let mut seen = self.seen.write().unwrap();
        let mut newly_covered = Vec::new();

        for (idx, &count) in counters.iter().enumerate() {
            // Apply filter if set
            if let Some(ref filter) = self.filter {
                if !filter(idx) {
                    continue;
                }
            }

            if count > 0 && !seen[idx] {
                seen[idx] = true;
                newly_covered.push(idx);
            }
        }

        let newly_covered_count = newly_covered.len();
        let total_covered = seen.iter().filter(|&&s| s).count();
        let coverage_ratio = if self.num_counters > 0 {
            total_covered as f64 / self.num_counters as f64
        } else {
            0.0
        };

        // Update rarity tracker if enabled
        let rarity_score = if let Some(ref tracker) = self.rarity_tracker {
            tracker.record_hits(&newly_covered);
            Some(tracker.compute_rarity_score(&newly_covered))
        } else {
            None
        };

        CoverageDelta {
            newly_covered_count,
            newly_covered_indices: newly_covered,
            rarity_score,
            coverage_ratio,
        }
    }

    /// Get current coverage ratio (0.0 to 1.0).
    pub fn current_coverage_ratio(&self) -> f64 {
        let seen = self.seen.read().unwrap();
        if self.num_counters == 0 {
            0.0
        } else {
            seen.iter().filter(|&&s| s).count() as f64 / self.num_counters as f64
        }
    }

    /// Get total number of counters.
    pub fn num_counters(&self) -> usize {
        self.num_counters
    }

    /// Get count of covered sites.
    pub fn covered_count(&self) -> usize {
        self.seen.read().unwrap().iter().filter(|&&s| s).count()
    }

    /// Reset all coverage state (for fresh exploration).
    pub fn reset(&self) {
        let mut seen = self.seen.write().unwrap();
        seen.fill(false);
        if let Some(ref tracker) = self.rarity_tracker {
            tracker.reset();
        }
    }
}

impl Default for CoverageTracker {
    fn default() -> Self {
        Self::new().expect("Coverage not available")
    }
}

// =============================================================================
// Rarity Tracker
// =============================================================================

/// Tracks how often each counter is hit for rarity-based biasing.
/// Uses cumulative counts (easy to change to decaying if needed).
#[derive(Debug)]
pub struct RarityTracker {
    /// Hit count per counter (how many rollouts covered this site).
    hit_counts: Vec<AtomicU64>,
    /// Total number of rollouts.
    total_rollouts: AtomicU64,
}

impl RarityTracker {
    /// Create a new rarity tracker.
    pub fn new() -> Option<Self> {
        let num_counters = get_num_counters()?;
        Some(Self {
            hit_counts: (0..num_counters).map(|_| AtomicU64::new(0)).collect(),
            total_rollouts: AtomicU64::new(0),
        })
    }

    /// Record that these counters were hit in a rollout.
    pub fn record_hits(&self, indices: &[usize]) {
        for &idx in indices {
            if idx < self.hit_counts.len() {
                self.hit_counts[idx].fetch_add(1, Ordering::Relaxed);
            }
        }
        self.total_rollouts.fetch_add(1, Ordering::Relaxed);
    }

    /// Compute rarity score for a set of counters.
    /// Higher score = rarer paths = more valuable.
    pub fn compute_rarity_score(&self, indices: &[usize]) -> f64 {
        if indices.is_empty() {
            return 0.0;
        }

        let total = self.total_rollouts.load(Ordering::Relaxed).max(1) as f64;
        let score: f64 = indices
            .iter()
            .map(|&idx| {
                let hits = self.hit_counts[idx].load(Ordering::Relaxed) as f64;
                // Inverse frequency: 1.0 if never seen, decreasing as hits increase
                1.0 / (1.0 + hits / total)
            })
            .sum();

        score / indices.len() as f64 // Average rarity
    }

    /// Get hit count for a specific counter.
    pub fn hit_count(&self, idx: usize) -> u64 {
        if idx < self.hit_counts.len() {
            self.hit_counts[idx].load(Ordering::Relaxed)
        } else {
            0
        }
    }

    /// Reset all statistics.
    pub fn reset(&self) {
        for counter in &self.hit_counts {
            counter.store(0, Ordering::Relaxed);
        }
        self.total_rollouts.store(0, Ordering::Relaxed);
    }
}

impl Default for RarityTracker {
    fn default() -> Self {
        Self::new().expect("Coverage not available")
    }
}

// =============================================================================
// Low-Level LLVM Runtime FFI (weak symbols for graceful degradation)
// =============================================================================

#[cfg(all(unix, not(target_os = "macos")))]
mod weak_symbols {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

    static BEGIN_PTR: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
    static END_PTR: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
    static INITIALIZED: AtomicBool = AtomicBool::new(false);

    fn init() {
        if INITIALIZED.load(Ordering::Relaxed) {
            return;
        }

        unsafe {
            let begin_name = "__llvm_profile_begin_counters\0";
            let end_name = "__llvm_profile_end_counters\0";

            let begin =
                libc::dlsym(libc::RTLD_DEFAULT, begin_name.as_ptr() as *const i8) as *mut c_void;
            let end =
                libc::dlsym(libc::RTLD_DEFAULT, end_name.as_ptr() as *const i8) as *mut c_void;

            BEGIN_PTR.store(begin, Ordering::Relaxed);
            END_PTR.store(end, Ordering::Relaxed);
        }

        INITIALIZED.store(true, Ordering::Relaxed);
    }

    pub fn begin_counters() -> *const u64 {
        init();
        BEGIN_PTR.load(Ordering::Relaxed) as *const u64
    }

    pub fn end_counters() -> *const u64 {
        init();
        END_PTR.load(Ordering::Relaxed) as *const u64
    }

    pub unsafe fn reset_counters() {
        let begin = begin_counters();
        let end = end_counters();
        if !begin.is_null() && !end.is_null() && begin <= end {
            let num_counters = (end as usize - begin as usize) / std::mem::size_of::<u64>();
            if num_counters > 0 {
                std::ptr::write_bytes(
                    begin as *mut u8,
                    0,
                    num_counters * std::mem::size_of::<u64>(),
                );
            }
        }
    }
}

// On macOS, instrument-coverage symbols are local (not exported via dlsym)
// We use a C helper with weak external references
#[cfg(target_os = "macos")]
mod weak_symbols {
    use std::ffi::c_void;

    unsafe extern "C" {
        fn coverage_begin_counters() -> *mut c_void;
        fn coverage_end_counters() -> *mut c_void;
        fn coverage_reset_counters();
    }

    pub fn begin_counters() -> *const u64 {
        unsafe { coverage_begin_counters() as *const u64 }
    }

    pub fn end_counters() -> *const u64 {
        unsafe { coverage_end_counters() as *const u64 }
    }

    pub unsafe fn reset_counters() {
        unsafe {
            coverage_reset_counters();
        }
    }
}

#[cfg(not(unix))]
mod weak_symbols {
    pub fn begin_counters() -> *const u64 {
        std::ptr::null()
    }

    pub fn end_counters() -> *const u64 {
        std::ptr::null()
    }

    pub unsafe fn reset_counters() {}
}

/// Initialize coverage detection.
fn init_coverage() -> CoverageStatus {
    // Already initialized?
    if COVERAGE_AVAILABLE.load(Ordering::Relaxed) {
        let num = NUM_COUNTERS.load(Ordering::Relaxed) as usize;
        return CoverageStatus::Available { num_counters: num };
    }

    // Try to detect coverage runtime using weak symbols
    let start = weak_symbols::begin_counters();
    let end = weak_symbols::end_counters();
    let available = !start.is_null() && !end.is_null() && start <= end;

    COVERAGE_AVAILABLE.store(available, Ordering::Relaxed);

    if available {
        let num = (end as usize - start as usize) / std::mem::size_of::<u64>();
        NUM_COUNTERS.store(num as u64, Ordering::Relaxed);
        CoverageStatus::Available { num_counters: num }
    } else {
        CoverageStatus::Unavailable
    }
}

/// Check if coverage is available.
pub fn is_coverage_available() -> bool {
    if !COVERAGE_AVAILABLE.load(Ordering::Relaxed) {
        // Try to initialize
        matches!(init_coverage(), CoverageStatus::Available { .. })
    } else {
        true
    }
}

/// Get number of counters (if available).
pub fn get_num_counters() -> Option<usize> {
    if !is_coverage_available() {
        return None;
    }
    Some(NUM_COUNTERS.load(Ordering::Relaxed) as usize)
}

/// Reset all counters to zero.
fn reset_counters() {
    if is_coverage_available() {
        unsafe {
            weak_symbols::reset_counters();
        }
    }
}

/// Read current counter values.
fn read_counters() -> Option<Vec<u64>> {
    let num = get_num_counters()?;
    if num == 0 {
        return Some(Vec::new());
    }

    let start = weak_symbols::begin_counters();
    if start.is_null() {
        return None;
    }

    let mut result = Vec::with_capacity(num);
    unsafe {
        for i in 0..num {
            result.push(*start.add(i));
        }
    }
    Some(result)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coverage_detection() {
        // This test works whether or not coverage is enabled
        let status = init_coverage();
        match status {
            CoverageStatus::Available { num_counters } => {
                assert!(num_counters > 0);
                assert!(is_coverage_available());
            }
            CoverageStatus::Unavailable => {
                assert!(!is_coverage_available());
            }
        }
    }

    #[test]
    fn test_tracker_creation() {
        // Returns Some if coverage available, None otherwise
        let _tracker = CoverageTracker::new();
        // Don't assert - depends on how test was compiled
    }

    #[test]
    fn test_count_reward() {
        let reward = CountReward;
        let delta = CoverageDelta {
            newly_covered_count: 5,
            newly_covered_indices: vec![1, 2, 3, 4, 5],
            rarity_score: None,
            coverage_ratio: 0.1,
        };
        assert_eq!(reward.compute_reward(&delta), 5.0);
    }

    #[test]
    fn test_rarity_reward() {
        let tracker = Arc::new(RarityTracker::new().unwrap_or_else(|| {
            // Create dummy tracker for test
            RarityTracker {
                hit_counts: vec![AtomicU64::new(10), AtomicU64::new(0), AtomicU64::new(5)],
                total_rollouts: AtomicU64::new(20),
            }
        }));

        let reward = RarityReward::new(tracker);
        let delta = CoverageDelta {
            newly_covered_count: 2,
            newly_covered_indices: vec![1, 2],
            rarity_score: Some(0.75),
            coverage_ratio: 0.1,
        };

        let r = reward.compute_reward(&delta);
        assert!(r > 0.0);
    }
}
