//! Test file to verify dependencies are working correctly

#[cfg(test)]
mod dependency_tests {
    use quickcheck::{TestResult, quickcheck};
    use quickcheck_macros::quickcheck as qc;
    use z3::{Context, Solver, ast::Int};

    #[test]
    fn test_z3_basic_functionality() {
        let cfg = z3::Config::new();
        let ctx = Context::new(&cfg);
        let solver = Solver::new(&ctx);

        // Create a simple constraint: x > 0
        let x = Int::new_const(&ctx, "x");
        let zero = Int::from_i64(&ctx, 0);
        let constraint = x.gt(&zero);

        solver.assert(&constraint);

        // Should be satisfiable
        assert_eq!(solver.check(), z3::SatResult::Sat);

        // Get a model and verify it satisfies the constraint
        if let Some(model) = solver.get_model() {
            if let Some(x_val) = model.eval(&x, true) {
                // The value should be greater than 0
                let x_int = x_val.as_i64().expect("Should be an integer");
                assert!(x_int > 0, "x should be greater than 0, got {}", x_int);
            }
        }
    }

    #[test]
    fn test_z3_unsatisfiable_constraint() {
        let cfg = z3::Config::new();
        let ctx = Context::new(&cfg);
        let solver = Solver::new(&ctx);

        // Create contradictory constraints: x > 0 AND x < 0
        let x = Int::new_const(&ctx, "x");
        let zero = Int::from_i64(&ctx, 0);
        let constraint1 = x.gt(&zero);
        let constraint2 = x.lt(&zero);

        solver.assert(&constraint1);
        solver.assert(&constraint2);

        // Should be unsatisfiable
        assert_eq!(solver.check(), z3::SatResult::Unsat);
    }

    #[test]
    fn test_quickcheck_basic_functionality() {
        fn prop_addition_commutative(a: i32, b: i32) -> TestResult {
            // Avoid overflow
            if a.checked_add(b).is_none() {
                return TestResult::discard();
            }
            TestResult::from_bool(a + b == b + a)
        }

        quickcheck(prop_addition_commutative as fn(i32, i32) -> TestResult);
    }

    #[qc]
    fn prop_multiplication_identity(x: i32) -> bool {
        x * 1 == x
    }
}
