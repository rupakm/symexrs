//! Error types and result handling for symbolic execution
//!
//! This module defines comprehensive error types for all layers of the
//! symbolic execution system and provides convenient result type aliases.

use std::fmt;

/// Result type alias for symbolic execution operations
pub type SymExResult<T> = Result<T, SymExError>;

/// Comprehensive error types for symbolic execution system
#[derive(Debug, Clone)]
pub enum SymExError {
    // Symbolic Type Errors
    /// Operations not supported for the symbolic type
    InvalidOperation(String),
    /// Incompatible types in operations
    TypeMismatch { expected: String, found: String },
    /// Arithmetic overflow in symbolic operations
    OverflowError(String),

    // Expression Errors
    /// Invalid expression structure
    MalformedExpression(String),
    /// Reference to undefined symbolic variable
    UnboundVariable(String),
    /// Circular dependencies in expressions
    CircularReference(String),

    // SMT Solver Errors
    /// Solver exceeded time limit
    SolverTimeout,
    /// Internal solver error or invalid input
    SolverError(String),
    /// Failed to extract concrete values from model
    ModelExtractionError(String),

    // Manager Errors
    /// Inconsistent internal state detected
    StateCorruption(String),
    /// Attempted to pop from empty backtracking stack
    StackUnderflow,
    /// Out of memory or other resources
    ResourceExhaustion(String),
    /// Duplicate variable registration
    DuplicateVariable(String),

    // String-specific Errors
    /// Invalid string operation
    InvalidStringOperation(String),
    /// String index or length out of bounds
    StringBoundsError {
        operation: String,
        index: i64,
        length: usize,
    },
    /// Non-ASCII character encountered
    NonAsciiCharacter { character: char, position: usize },
    /// Empty pattern in string operation
    EmptyPatternError,
    /// Z3 string theory not supported
    StringTheoryUnsupported,

    // General Errors
    /// Generic error with message
    Generic(String),
    /// I/O related errors
    IoError(String),
    /// Early UNSAT detection panic
    UnsatPanic,
    /// User function execution error
    ExecutionError(String),
}

impl fmt::Display for SymExError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Symbolic Type Errors
            SymExError::InvalidOperation(msg) => {
                write!(f, "Invalid operation: {msg}")
            }
            SymExError::TypeMismatch { expected, found } => {
                write!(f, "Type mismatch: expected {expected}, found {found}")
            }
            SymExError::OverflowError(msg) => {
                write!(f, "Arithmetic overflow: {msg}")
            }

            // Expression Errors
            SymExError::MalformedExpression(msg) => {
                write!(f, "Malformed expression: {msg}")
            }
            SymExError::UnboundVariable(var) => {
                write!(f, "Unbound variable: {var}")
            }
            SymExError::CircularReference(msg) => {
                write!(f, "Circular reference: {msg}")
            }

            // SMT Solver Errors
            SymExError::SolverTimeout => {
                write!(f, "SMT solver timeout")
            }
            SymExError::SolverError(msg) => {
                write!(f, "SMT solver error: {msg}")
            }
            SymExError::ModelExtractionError(msg) => {
                write!(f, "Model extraction error: {msg}")
            }

            // Manager Errors
            SymExError::StateCorruption(msg) => {
                write!(f, "State corruption: {msg}")
            }
            SymExError::StackUnderflow => {
                write!(f, "Backtracking stack underflow")
            }
            SymExError::ResourceExhaustion(msg) => {
                write!(f, "Resource exhaustion: {msg}")
            }
            SymExError::DuplicateVariable(var) => {
                write!(f, "Duplicate variable: {var}")
            }

            // String-specific Errors
            SymExError::InvalidStringOperation(msg) => {
                write!(f, "Invalid string operation: {msg}")
            }
            SymExError::StringBoundsError {
                operation,
                index,
                length,
            } => {
                write!(
                    f,
                    "String bounds error in {operation}: index {index} out of bounds for length {length}"
                )
            }
            SymExError::NonAsciiCharacter {
                character,
                position,
            } => {
                write!(
                    f,
                    "Non-ASCII character '{character}' at position {position}"
                )
            }
            SymExError::EmptyPatternError => {
                write!(f, "Empty pattern not allowed in string operation")
            }
            SymExError::StringTheoryUnsupported => {
                write!(f, "Z3 string theory not supported")
            }

            // General Errors
            SymExError::Generic(msg) => {
                write!(f, "Error: {msg}")
            }
            SymExError::IoError(msg) => {
                write!(f, "I/O error: {msg}")
            }
            SymExError::UnsatPanic => {
                write!(f, "Early UNSAT detection")
            }
            SymExError::ExecutionError(msg) => {
                write!(f, "Execution error: {msg}")
            }
        }
    }
}

impl std::error::Error for SymExError {}

impl From<std::io::Error> for SymExError {
    fn from(err: std::io::Error) -> Self {
        SymExError::IoError(err.to_string())
    }
}

// Convenience macros for error creation
#[macro_export]
macro_rules! symex_error {
    ($variant:ident, $msg:expr) => {
        SymExError::$variant($msg.to_string())
    };
    ($variant:ident) => {
        SymExError::$variant
    };
}

#[macro_export]
macro_rules! type_mismatch {
    ($expected:expr, $found:expr) => {
        SymExError::TypeMismatch {
            expected: $expected.to_string(),
            found: $found.to_string(),
        }
    };
}

// Error recovery strategies and utilities will be added in later tasks
