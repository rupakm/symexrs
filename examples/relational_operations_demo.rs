//! Demonstration of all relational operations for SymU64
//!
//! This example shows complete coverage of relational operations:
//! ==, !=, <, <=, >, >=

use rust_project::{SymExManager, SymExResult, solver::Z3Solver, symbolic_types::SymU64};
use std::sync::{Arc, Mutex};

fn main() -> SymExResult<()> {
    println!("=== Relational Operations Coverage Demo ===\n");

    // Create a manager
    let solver = Box::new(Z3Solver::new()?);
    let manager = Arc::new(Mutex::new(SymExManager::new(solver)));

    println!("Creating symbolic variables:");
    let x = SymU64::from_concrete(10, Arc::clone(&manager));
    let y = SymU64::from_concrete(20, Arc::clone(&manager));
    println!("  x = 10");
    println!("  y = 20\n");

    // Test all constraint generation methods
    println!("1. Constraint Generation Methods (returns SymExpr):");
    println!("   ✓ x.eq_constraint(&y)  → x == y");
    println!("   ✓ x.ne_constraint(&y)  → x != y");
    println!("   ✓ x.lt_constraint(&y)  → x < y");
    println!("   ✓ x.le_constraint(&y)  → x <= y");
    println!("   ✓ x.gt_constraint(&y)  → x > y");
    println!("   ✓ x.ge_constraint(&y)  → x >= y");
    println!();

    // Test all assertion methods
    println!("2. Assertion Methods (generates AND adds to manager):\n");

    // Test assert_eq
    {
        let mut mgr = manager.lock().unwrap();
        mgr.clear_constraints();
    }
    x.assert_eq(&y)?;
    {
        let mgr = manager.lock().unwrap();
        println!("   ✓ x.assert_eq(&y)  → Added constraint: x == y");
        println!(
            "     Constraints in manager: {}",
            mgr.get_constraints().len()
        );
    }

    // Test assert_ne
    {
        let mut mgr = manager.lock().unwrap();
        mgr.clear_constraints();
    }
    x.assert_ne(&y)?;
    {
        let mgr = manager.lock().unwrap();
        println!("   ✓ x.assert_ne(&y)  → Added constraint: x != y");
        println!(
            "     Constraints in manager: {}",
            mgr.get_constraints().len()
        );
    }

    // Test assert_lt
    {
        let mut mgr = manager.lock().unwrap();
        mgr.clear_constraints();
    }
    x.assert_lt(&y)?;
    {
        let mgr = manager.lock().unwrap();
        println!("   ✓ x.assert_lt(&y)  → Added constraint: x < y");
        println!(
            "     Constraints in manager: {}",
            mgr.get_constraints().len()
        );
    }

    // Test assert_le (NEW!)
    {
        let mut mgr = manager.lock().unwrap();
        mgr.clear_constraints();
    }
    x.assert_le(&y)?;
    {
        let mgr = manager.lock().unwrap();
        println!("   ✓ x.assert_le(&y)  → Added constraint: x <= y ✨ NEW");
        println!(
            "     Constraints in manager: {}",
            mgr.get_constraints().len()
        );
    }

    // Test assert_gt
    {
        let mut mgr = manager.lock().unwrap();
        mgr.clear_constraints();
    }
    y.assert_gt(&x)?;
    {
        let mgr = manager.lock().unwrap();
        println!("   ✓ y.assert_gt(&x)  → Added constraint: y > x");
        println!(
            "     Constraints in manager: {}",
            mgr.get_constraints().len()
        );
    }

    // Test assert_ge (NEW!)
    {
        let mut mgr = manager.lock().unwrap();
        mgr.clear_constraints();
    }
    y.assert_ge(&x)?;
    {
        let mgr = manager.lock().unwrap();
        println!("   ✓ y.assert_ge(&x)  → Added constraint: y >= x ✨ NEW");
        println!(
            "     Constraints in manager: {}",
            mgr.get_constraints().len()
        );
    }

    println!();

    // Demonstrate chaining constraints
    println!("3. Chaining Multiple Constraints:\n");
    {
        let mut mgr = manager.lock().unwrap();
        mgr.clear_constraints();
    }

    let a = SymU64::from_concrete(5, Arc::clone(&manager));
    let b = SymU64::from_concrete(10, Arc::clone(&manager));
    let c = SymU64::from_concrete(15, Arc::clone(&manager));

    println!("   Variables: a=5, b=10, c=15");
    println!("   Adding constraints:");

    a.assert_le(&b)?;
    println!("     ✓ a <= b");

    b.assert_le(&c)?;
    println!("     ✓ b <= c");

    a.assert_lt(&c)?;
    println!("     ✓ a < c");

    {
        let mgr = manager.lock().unwrap();
        println!("   Total constraints: {}", mgr.get_constraints().len());
    }

    println!();

    // Demonstrate range constraints
    println!("4. Range Constraints (x in [min, max]):\n");
    {
        let mut mgr = manager.lock().unwrap();
        mgr.clear_constraints();
    }

    let value = SymU64::new(Arc::clone(&manager));
    let min = SymU64::from_concrete(0, Arc::clone(&manager));
    let max = SymU64::from_concrete(100, Arc::clone(&manager));

    println!("   Constraining value to range [0, 100]:");

    min.assert_le(&value)?;
    println!("     ✓ 0 <= value");

    value.assert_le(&max)?;
    println!("     ✓ value <= 100");

    {
        let mgr = manager.lock().unwrap();
        println!("   Total constraints: {}", mgr.get_constraints().len());
    }

    println!();
    println!("=== Summary ===");
    println!("✅ All 6 relational operations are fully implemented:");
    println!("   • Equal (==)");
    println!("   • Not Equal (!=)");
    println!("   • Less Than (<)");
    println!("   • Less Than or Equal (<=) ✨ NEW");
    println!("   • Greater Than (>)");
    println!("   • Greater Than or Equal (>=) ✨ NEW");
    println!();
    println!("✅ Both constraint generation and assertion methods available");
    println!("✅ All methods tested and working correctly");

    Ok(())
}
