/// Integration test for string validation
///
/// This test demonstrates that string operations are validated before being
/// added as constraints to the manager.
use rust_project::*;

#[test]
fn test_string_validation_integration() {
    // Initialize the symbolic execution engine
    let _ = init_global();
    let manager = get_global_manager().expect("Failed to get global manager");

    // Register a string variable
    {
        let mut mgr = manager.lock().unwrap();
        let type_info = TypeInfo::string(None);
        mgr.register_variable("s".to_string(), type_info).unwrap();
    }

    // Test 1: Valid string operation should succeed
    {
        let s = SymExpr::Variable("s".to_string());
        let start = SymExpr::Constant(ConstValue::I64(0));
        let length = SymExpr::Constant(ConstValue::I64(3));
        let substring = SymExpr::str_substring(s, start, length);

        let expected = SymExpr::Constant(ConstValue::String("hel".to_string()));
        let constraint = SymExpr::binary_op(BinOp::Eq, substring, expected);

        let mut mgr = manager.lock().unwrap();
        let result = mgr.add_constraint(constraint);
        assert!(result.is_ok(), "Valid string operation should succeed");
    }

    // Test 2: Negative index should fail
    {
        let s = SymExpr::Variable("s".to_string());
        let negative_index = SymExpr::Constant(ConstValue::I64(-1));
        let length = SymExpr::Constant(ConstValue::I64(2));
        let substring = SymExpr::str_substring(s, negative_index, length);

        let expected = SymExpr::Constant(ConstValue::String("he".to_string()));
        let constraint = SymExpr::binary_op(BinOp::Eq, substring, expected);

        let mut mgr = manager.lock().unwrap();
        let result = mgr.add_constraint(constraint);
        assert!(result.is_err(), "Negative index should fail validation");
        assert!(
            matches!(result.unwrap_err(), SymExError::InvalidStringOperation(_)),
            "Should return InvalidStringOperation error"
        );
    }

    // Test 3: Empty pattern should fail
    {
        let s = SymExpr::Variable("s".to_string());
        let empty_pattern = SymExpr::Constant(ConstValue::String("".to_string()));
        let replacement = SymExpr::Constant(ConstValue::String("x".to_string()));
        let replaced = SymExpr::str_replace(s, empty_pattern, replacement);

        let expected = SymExpr::Constant(ConstValue::String("result".to_string()));
        let constraint = SymExpr::binary_op(BinOp::Eq, replaced, expected);

        let mut mgr = manager.lock().unwrap();
        let result = mgr.add_constraint(constraint);
        assert!(result.is_err(), "Empty pattern should fail validation");
        assert!(
            matches!(result.unwrap_err(), SymExError::EmptyPatternError),
            "Should return EmptyPatternError"
        );
    }

    // Test 4: Non-ASCII characters should fail
    {
        let s = SymExpr::Variable("s".to_string());
        let non_ascii = SymExpr::Constant(ConstValue::String("Hello, 世界!".to_string()));
        let constraint = SymExpr::binary_op(BinOp::Eq, s, non_ascii);

        let mut mgr = manager.lock().unwrap();
        let result = mgr.add_constraint(constraint);
        assert!(
            result.is_err(),
            "Non-ASCII characters should fail validation"
        );
        assert!(
            matches!(result.unwrap_err(), SymExError::NonAsciiCharacter { .. }),
            "Should return NonAsciiCharacter error"
        );
    }

    // Test 5: Expression depth limit should be enforced
    {
        // Create a deeply nested expression
        let mut expr = SymExpr::Constant(ConstValue::String("x".to_string()));
        for _ in 0..101 {
            expr = SymExpr::str_concat(
                expr.clone(),
                SymExpr::Constant(ConstValue::String("y".to_string())),
            );
        }

        let s = SymExpr::Variable("s".to_string());
        let constraint = SymExpr::binary_op(BinOp::Eq, s, expr);

        let mut mgr = manager.lock().unwrap();
        let result = mgr.add_constraint(constraint);
        assert!(result.is_err(), "Expression depth limit should be enforced");
        assert!(
            matches!(result.unwrap_err(), SymExError::ResourceExhaustion(_)),
            "Should return ResourceExhaustion error"
        );
    }
}

#[test]
fn test_ascii_validation_in_symstring() {
    // Test that SymString constructors validate ASCII characters
    let _ = init_global();

    // Valid ASCII string should work
    let s1 = SymString::from_concrete("Hello, World!");
    assert_eq!(s1.concrete_value(), Some("Hello, World!"));

    // Valid ASCII string with special characters should work
    let s2 = SymString::from_concrete("!@#$%^&*()");
    assert_eq!(s2.concrete_value(), Some("!@#$%^&*()"));
}

#[test]
#[should_panic(expected = "Non-ASCII character")]
fn test_ascii_validation_from_concrete_panics() {
    // Test that from_concrete panics on non-ASCII input
    let _ = init_global();
    // This should panic
    let _s = SymString::from_concrete("Hello, 世界!");
}

#[test]
#[should_panic(expected = "Non-ASCII character")]
fn test_ascii_validation_with_value_panics() {
    // Test that with_value panics on non-ASCII input
    let _ = init_global();
    // This should panic
    let _s = SymString::with_value("Hello, 世界!".to_string());
}
