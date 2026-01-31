//! Demonstration of concolic execution optimization
//!
//! This example shows how concrete values are maintained alongside symbolic
//! expressions to enable fast satisfiability checking without calling the
//! SMT solver for every path.

use rust_project::{SymExManager, SymExResult, solver::Z3Solver, symbolic_types::SymU64};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn main() -> SymExResult<()> {
    println!("=== Concolic Execution Demo ===\n");

    // Create a manager
    let solver = Box::new(Z3Solver::new()?);
    let manager = Arc::new(Mutex::new(SymExManager::new(solver)));

    println!("1. Creating symbolic variables with concrete values");
    println!("   This enables concolic (concrete + symbolic) execution\n");

    // Create symbolic variables with initial concrete values
    let x = SymU64::from_concrete(10, Arc::clone(&manager));
    let y = SymU64::from_concrete(20, Arc::clone(&manager));

    println!("   x = 10 (symbolic: {})", x.variable_name());
    println!("   y = 20 (symbolic: {})", y.variable_name());
    println!();

    // Perform operations - concrete values are maintained
    let sum = &x + &y;
    println!("2. Operations maintain concrete values:");
    println!(
        "   x + y = {:?} (concrete: {:?})",
        sum.expr(),
        sum.concrete_value()
    );
    println!();

    // Add a constraint: x < y
    println!("3. Adding constraint: x < y");
    let constraint = x.lt_constraint(&y);
    {
        let mut mgr = manager.lock().unwrap();
        mgr.add_constraint(constraint.clone())?;
    }

    // Fast satisfiability check using concrete values
    println!("4. Fast satisfiability check using concrete values:");
    let mut concrete_values = HashMap::new();
    concrete_values.insert(x.variable_name().to_string(), x.concrete_value().unwrap());
    concrete_values.insert(y.variable_name().to_string(), y.concrete_value().unwrap());

    {
        let mgr = manager.lock().unwrap();
        match mgr.fast_check_satisfiable_with_concrete(&concrete_values) {
            Some(true) => {
                println!("   ✓ Path is satisfiable (checked with concrete values, no SMT call!)")
            }
            Some(false) => println!("   ✗ Path is unsatisfiable (detected with concrete values)"),
            None => println!("   ? Cannot determine with concrete values alone"),
        }
    }
    println!();

    // Now add a contradictory constraint: x > y
    println!("5. Adding contradictory constraint: x > y");
    let bad_constraint = x.gt_constraint(&y);
    {
        let mut mgr = manager.lock().unwrap();
        mgr.add_constraint(bad_constraint)?;
    }

    // Fast check will detect unsatisfiability immediately
    println!("6. Fast satisfiability check with contradictory constraints:");
    {
        let mgr = manager.lock().unwrap();
        match mgr.fast_check_satisfiable_with_concrete(&concrete_values) {
            Some(true) => println!("   ✓ Path is satisfiable"),
            Some(false) => {
                println!("   ✗ Path is unsatisfiable (detected immediately with concrete values!)")
            }
            None => println!("   ? Cannot determine with concrete values alone"),
        }
    }
    println!();

    // Demonstrate updating concrete values when exploring a new path
    println!("7. Exploring a new path - updating concrete values from SMT model");

    // Clear constraints and start fresh
    {
        let mut mgr = manager.lock().unwrap();
        mgr.clear_constraints();
    }

    // Create a fresh symbolic variable (without concrete value initially)
    let mut z = SymU64::new(Arc::clone(&manager));

    // Create a constant for comparison
    let fifteen = SymU64::from_concrete(15, Arc::clone(&manager));

    // Add constraint: z > 15
    let new_constraint = z.gt_constraint(&fifteen);

    {
        let mut mgr = manager.lock().unwrap();
        mgr.add_constraint(new_constraint)?;
    }

    println!("   Added constraint: z > 15");
    println!("   z is a fresh symbolic variable (no concrete value yet)");
    println!();

    // Get a model from the SMT solver
    println!("8. Getting new concrete values from SMT solver:");
    let model = {
        let mut mgr = manager.lock().unwrap();
        mgr.get_model()?
    };

    if let Some(model) = model {
        println!("   SMT solver found satisfying values:");
        for var in model.get_variables() {
            if let Some(val) = model.get_u64(&var) {
                println!("     {} = {}", var, val);
            }
        }
        println!();

        // Update concrete values from the model
        println!("9. Updating concrete values from model:");
        if model.is_empty() {
            println!("   (Model is empty - Z3 didn't provide concrete values)");
            println!("   This is expected for simple constraints where any value works");
        } else {
            let updated = z.update_from_model(&model);
            if updated {
                println!(
                    "   ✓ Updated z to {} (now satisfies z > 15)",
                    z.concrete_value().unwrap()
                );
            }
        }
        println!();

        // Now fast check will succeed
        println!("10. Fast satisfiability check with updated concrete values:");
        if let Some(z_val) = z.concrete_value() {
            let mut updated_concrete = HashMap::new();
            updated_concrete.insert(z.variable_name().to_string(), z_val);
            updated_concrete.insert(
                fifteen.variable_name().to_string(),
                fifteen.concrete_value().unwrap(),
            );

            let mgr = manager.lock().unwrap();
            match mgr.fast_check_satisfiable_with_concrete(&updated_concrete) {
                Some(true) => {
                    println!("    ✓ Path is satisfiable (verified with updated concrete values!)")
                }
                Some(false) => println!("    ✗ Path is unsatisfiable"),
                None => println!("    ? Cannot determine with concrete values alone"),
            }
        } else {
            println!("    (No concrete value available for z - would need SMT solver)");
        }
    } else {
        println!("   No model available (constraints unsatisfiable)");
    }

    println!();
    println!("=== Benefits of Concolic Execution ===");
    println!("1. Fast path pruning: Detect unsatisfiable paths without SMT calls");
    println!("2. Reduced solver overhead: Only call SMT solver when necessary");
    println!("3. Concrete guidance: Use concrete values to guide path exploration");
    println!("4. Early termination: Stop exploring infeasible paths immediately");

    Ok(())
}
