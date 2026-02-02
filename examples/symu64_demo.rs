//! Demonstration of SymU64 symbolic type usage
//!
//! This example shows how to use SymU64 for symbolic execution
//! with arithmetic operations and constraint tracking.

use rust_project::symbolic_types::SymU64;
use rust_project::{get_global_manager, init_global};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== SymU64 Symbolic Execution Demo ===\n");

    // Initialize the global manager
    init_global()?;
    let manager = get_global_manager()?;

    // Create symbolic variables
    println!("Creating symbolic variables...");
    let x = SymU64::new();
    let y = SymU64::new();
    println!("  x: {}", x.variable_name());
    println!("  y: {}\n", y.variable_name());

    // Perform arithmetic operations
    println!("Performing arithmetic operations...");
    let sum = &x + &y;
    println!("  x + y = {:?}", sum.expr());

    let product = &x * &y;
    println!("  x * y = {:?}", product.expr());

    let complex = (&x + &y) * SymU64::from_concrete(2);
    println!("  (x + y) * 2 = {:?}\n", complex.expr());

    // Work with concrete values
    println!("Working with concrete values...");
    let a = SymU64::from_concrete(10);
    let b = SymU64::from_concrete(20);
    println!("  a = 10, b = 20");

    let result = &a + &b;
    println!(
        "  a + b = {:?} (concrete: {:?})",
        result.expr(),
        result.concrete_value()
    );

    let result = &a * &b;
    println!(
        "  a * b = {:?} (concrete: {:?})",
        result.expr(),
        result.concrete_value()
    );

    // Mixed symbolic and concrete
    println!("\nMixed symbolic and concrete operations...");
    let mixed = x + a;
    println!("  x + 10 = {:?}", mixed.expr());

    // Check variable registration
    println!("\nRegistered variables:");
    let mgr = manager.lock().unwrap();
    for var in mgr.get_registered_variables() {
        if let Some(info) = mgr.get_variable_info(&var) {
            println!(
                "  {} - type: {}, bits: {:?}, signed: {}",
                var, info.type_name, info.bit_width, info.is_signed
            );
        }
    }

    println!("\n=== Demo Complete ===");
    Ok(())
}
