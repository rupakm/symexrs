//! Demonstration of SymBool branching and boolean ops.

use symexrs::{SymBool, explore_default};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== SymBool Demo ===\n");

    let result = explore_default(|| {
        let a = SymBool::new();
        let b = SymBool::new();

        let both = &a & &b;
        if both.holds() {
            println!("Path: a && b is true");
        } else {
            println!("Path: a && b is false");
        }

        Ok(())
    })?;

    println!(
        "Explored {} paths (sat: {}, unsat: {})",
        result.exploration_result.paths_explored,
        result.exploration_result.satisfiable_paths,
        result.exploration_result.unsatisfiable_paths
    );

    Ok(())
}
