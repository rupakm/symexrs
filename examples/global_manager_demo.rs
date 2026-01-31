//! Demonstration of global manager usage for symbolic execution
//!
//! This example shows how to use the thread-local global manager
//! for seamless symbolic execution without explicitly passing managers.

use rust_project::{init_global, get_global_manager, reset_global_manager, SymU64};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Global Manager Demo ===\n");

    // Initialize the global manager for this thread
    println!("Initializing global manager...");
    init_global()?;

    // Create symbolic variables using the global manager
    println!("\nCreating symbolic variables using global manager:");
    let x = SymU64::new_global();
    let y = SymU64::new_global();
    println!("  x: {}", x.variable_name());
    println!("  y: {}", y.variable_name());

    // Use From trait (which uses global manager)
    println!("\nUsing From trait (auto-uses global manager):");
    let a: SymU64 = 10.into();
    let b: SymU64 = 20.into();
    println!("  a = 10 (concrete)");
    println!("  b = 20 (concrete)");

    // Perform operations
    println!("\nPerforming operations:");
    let sum = &x + &y;
    println!("  x + y = {:?}", sum.expr());

    let product = &a * &b;
    println!("  a * b = {:?} (concrete: {:?})", 
             product.expr(), product.concrete_value());

    // Add constraints using the new methods
    println!("\nAdding constraints:");
    x.assert_gt(&a)?;
    println!("  Added constraint: x > 10");

    y.assert_lt(&b)?;
    println!("  Added constraint: y < 20");

    // Check satisfiability
    println!("\nChecking satisfiability:");
    let manager = get_global_manager()?;
    let is_sat = {
        let mut mgr = manager.lock().unwrap();
        mgr.is_satisfiable()?
    };
    println!("  Constraints are satisfiable: {}", is_sat);

    // Get a model if satisfiable
    if is_sat {
        let model = {
            let mut mgr = manager.lock().unwrap();
            mgr.get_model()?
        };
        
        if let Some(model) = model {
            println!("\n  Found model:");
            for (var, value) in &model.assignments {
                println!("    {} = {:?}", var, value);
            }
        }
    }

    // Show constraint statistics
    println!("\nConstraint statistics:");
    let stats = {
        let mgr = manager.lock().unwrap();
        mgr.get_stats()
    };
    println!("  Total variables: {}", stats.variable_count);
    println!("  Total constraints: {}", stats.constraint_count);

    // Demonstrate reset
    println!("\nResetting global manager...");
    reset_global_manager();
    
    // After reset, new variables start from 0 again
    let z = SymU64::new_global();
    println!("  New variable after reset: {}", z.variable_name());

    println!("\n=== Demo Complete ===");
    Ok(())
}
