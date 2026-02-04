//! Notes on symbolic assertions in this example
//!
//! This test intentionally demonstrates a subtle interaction between
//! symbolic comparisons and Rust `assert!`s inside an explored closure.
//!
//! In `symex_test`, we eventually reach this code:
//!
//! ```rust
//! if let Some(idx) = result {
//!     let arr_val = SymI32::from_concrete(arr[idx]);
//!     assert!(arr_val == target);
//! }
//! ```
//!
//! At first glance this looks like an invariant: if `binary_search` found an
//! index, then `arr[idx]` must equal `target`. Logically that is true.
//!
//! However, in this engine `==` on symbolic ints is a *branching* operation.
//! The `PartialEq` impl for `SymI32` (via `define_sym_int!`) works roughly as:
//!
//! - build a predicate `Eq(self.expr, other.expr)`;
//! - consult current concrete values to get a `concolic_choice` (true/false);
//! - call `runtime::choose_branch(&predicate, concolic_choice)` to decide
//!   which outcome to take for this run (`chosen`);
//! - add either `Eq(...)` (if `chosen == true`) or `Ne(...)` (if `chosen == false`)
//!   to the path constraints via the manager;
//! - if we flipped the concrete choice (i.e. `chosen != concolic_choice`), we
//!   call `refresh_concolic_from_model()` so the concrete values satisfy the
//!   accumulated constraints (when possible).
//!
//! Because exploration is *replay-based*, the engine will eventually try both
//! outcomes of each comparison, as long as they are satisfiable. This affects
//! the assertion above as follows:
//!
//! 1. In `binary_search`, inside the loop, we compute
//!    `mid_val = SymI32::from_concrete(arr[mid])`, i.e. a symbolic constant
//!    expression `Const(arr[mid])`.
//! 2. The comparison `if mid_val == *elem { ... }` yields a predicate
//!    `Eq(Const(arr[mid]), Var(target))`.
//! 3. On the path where the branch is taken, we add the constraint
//!    `target == arr[mid]`. Later, this leads to `result = Some(mid)`.
//! 4. Back in `symex_test`, we compute `arr_val = SymI32::from_concrete(arr[idx])`.
//!    This is again `Const(arr[idx])`, with the same concrete value as above.
//! 5. Logically, under the existing constraint `target == arr[idx]`, the
//!    comparison `arr_val == target` should *always* be true.
//!
//! The catch is that `assert!(arr_val == target)` *also* uses the same
//! branching `==` operator. During exploration, there will be a replay where
//! the engine forces this comparison to be `false` in order to explore the
//! alternative branch:
//!
//! - At the assertion site, it chooses `chosen = false`, adds
//!   `Ne(Const(arr[idx]), Var(target))` to the constraints, and tries to
//!   continue execution.
//! - Combined with the earlier constraint `Eq(Const(arr[idx]), Var(target))`,
//!   we now have an unsatisfiable set of constraints
//!   (`target == k` and `target != k` for the same `k`).
//! - The SMT solver would eventually report this path as UNSAT, but the Rust
//!   `assert!` macro *panics immediately* as soon as `arr_val == target`
//!   returns `false`, before the engine can classify the path as
//!   unsatisfiable and discard it.
//!
//! This is why you can see a panic like:
//!
//! ```text
//! thread 'symex_test' panicked at tests/binary_search.rs:95:13:
//! assertion failed: arr_val == target
//! ```
//!
//! even though there is no real model where `result = Some(idx)` and
//! `arr[idx] != target`. The failing assertion only occurs along paths that
//! are *logically impossible* given the accumulated constraints, but the
//! engine still concretely executes those paths when exploring the opposite
//! branch of a comparison.
//!
//! Takeaway: using `assert!(sym == sym2)` directly inside an explored closure
//! can produce panics on unsatisfiable paths, because `==` itself is a
//! branching operator. If you want to state logical invariants that must hold
//! under the current constraints, it is often better to:
//!
//! - phrase them using solver queries or dedicated `assert_*` helpers on the
//!   symbolic type, or
//! - avoid adding new branching comparisons at the point where you are
//!   checking the invariant.

use symexrs::{explore_default, SymI32};

fn binary_search(arr: &[i32], elem: &SymI32) -> Option<usize> {
    let mut size = arr.len();
    let mut base = 0;

    while size > 0 {
        size /= 2;
        let mid = base + size;

        // Convert i32 to SymI32 for comparison
        let mid_val = SymI32::from_concrete(arr[mid]);
        if mid_val == *elem {
            return Some(mid);
        } else if mid_val < *elem {
            base = mid;
        } else {
            base = base;
        }
    }

    None
}

fn binary_search_nonterminating(arr: &[i32], elem: &SymI32) -> Option<usize> {
    let mut cap = arr.len() - 1;
    let mut low = 0;

    while low < cap {
        let mut mid = (low + cap) / 2;
        if mid == low {
            mid += 1;
        }

        // Convert i32 to SymI32 for comparison
        let mid_val = SymI32::from_concrete(arr[mid]);
        if mid_val == *elem {
            return Some(mid);
        } else if mid_val < *elem {
            low = mid;
        } else {
            cap = mid;
        }
    }

    None
}

fn get_symbolic_array() -> [i32; 4] {
    [
        1, 2,
        3, 4,
    ]
}

fn symex_test(good: bool) {
    let result = explore_default(|| {
        let arr = get_symbolic_array();
        let target = SymI32::new();

        if target < 1.into() || target > 4.into() {
            return Ok(());
        }

        // Search for the target value
        let result = if good {
            binary_search(&arr, &target)
        } else {
            // this gets cut off by the run time budget
            binary_search_nonterminating(&arr, &target)
        };

        // Test that if we find something, it's actually at that index
        if let Some(idx) = result {
            // Verify the found element equals the target

            // output the state of the symex: the path constraint at this point
            // let _ = print_state();
            let arr_val = SymI32::from_concrete(arr[idx]);
            assert!(arr_val == target);
        } else {
            unreachable!()
        }

        Ok(())
    });

    match result {
        Ok(res) => {
            println!("Exploration completed successfully with {} executions", res.runs_executed);
        }
        Err(e) => {
            println!("Error during exploration: {e:?}");
        }
    }
}

#[test]
fn good() {
    symex_test(true);
}

#[test]
fn bad() {
    symex_test(false);
}
