//! Demonstration of printing symbolic execution state
//!
//! This example shows how to inspect the symbolic execution engine's state,
//! including symbolic variables, path constraints, and concrete values.

use symexrs::{SymExResult, SymU64, print_state};

fn example_with_branches(x: SymU64, y: SymU64) -> SymExResult<()> {
    println!("\n=== Initial State ===");
    print_state()?;

    // Create comparison: x > y
    if x.gt(&y) {
        println!("\n=== After first branch (x > y) ===");
        print_state()?;

        let z = &x + &y;
        let hundred = SymU64::from_concrete(100);

        if z.gt(&hundred) {
            println!("\n=== After second branch (x + y > 100) ===");
            print_state()?;
        } else {
            println!("\n=== After second branch (x + y <= 100) ===");
            print_state()?;
        }
    } else {
        println!("\n=== After first branch (x <= y) ===");
        print_state()?;

        let diff = &y - &x;
        let fifty = SymU64::from_concrete(50);

        if diff.lt(&fifty) {
            println!("\n=== After second branch (y - x < 50) ===");
            print_state()?;
        }
    }

    Ok(())
}

fn main() -> SymExResult<()> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║     Symbolic Execution State Printing Demo                   ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    println!("This demo shows how to inspect the symbolic execution state");
    println!("at different points during execution.\n");

    // Create symbolic variables
    let x = SymU64::new();
    let y = SymU64::new();

    println!("Created symbolic variables:");
    println!("  x: {}", x.variable_name());
    println!("  y: {}", y.variable_name());

    // Run the example
    example_with_branches(x, y)?;

    println!("\n=== Final State ===");
    print_state()?;

    Ok(())
}
