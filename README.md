# Symbolic Execution Engine for Rust

A symbolic execution tool that uses Rust traits to enable seamless switching between concrete and symbolic execution. The system executes Rust code normally while carrying symbolic expressions alongside concrete values.

## Features

- **Trait-based symbolic types**: Drop-in replacements for concrete types (SymU64, SymI32, etc.)
- **SMT solver integration**: Uses Z3 for constraint satisfiability and model generation
- **Replay-based path exploration**: Re-executes the closure with forced branch decisions; undecided branches follow concolic execution and record alternatives
- **Property-based testing**: Comprehensive testing using QuickCheck

## Prerequisites

### Z3 SMT Solver

This project requires the Z3 SMT solver to be installed on your system.

#### macOS (Homebrew)
```bash
brew install z3
```

#### Ubuntu/Debian
```bash
sudo apt-get install z3 libz3-dev
```

#### Other Systems
Download and install Z3 from the [official releases](https://github.com/Z3Prover/z3/releases).

## Building

```bash
cargo build
```

## Running

```bash
cargo run
```

## Testing

```bash
cargo test
```

## Project Structure

- `src/symbolic_types.rs` - Drop-in symbolic types (ints/bool/string)
- `src/expressions.rs` - Abstract syntax tree for symbolic expressions
- `src/runtime.rs` - Per-run runtime state (decisions + concolic inputs)
- `src/engine.rs` - Exploration engine and scheduling primitives
- `src/manager.rs` - Constraint/variable manager + solver integration helpers
- `src/solver.rs` - SMT solver interface and Z3 integration
- `src/error.rs` - Comprehensive error handling

## Development Status

This project is under active development. The current focus is making the execution/runtime architecture ergonomic and schedulable.

## License

This project is licensed under the MIT License.
