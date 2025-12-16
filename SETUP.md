# Setup Instructions

## Dependencies

This project requires the Z3 SMT solver to be installed on your system.

### Installing Z3

#### macOS
```bash
brew install z3
```

The project is configured to automatically find Z3 libraries installed via Homebrew at `/opt/homebrew/opt/z3/lib`.

#### Ubuntu/Debian
```bash
sudo apt-get install libz3-dev
```

#### Windows
Download and install Z3 from the [official releases page](https://github.com/Z3Prover/z3/releases).

### Verifying Installation

After installing Z3, you can verify the setup by running:

```bash
cargo test
```

All tests should pass, including the Z3 integration tests. The project includes:
- Basic Z3 satisfiability checking
- Model extraction and validation
- Unsatisfiable constraint detection
- QuickCheck property-based testing

## Project Structure

The symbolic execution engine is organized into the following modules:

- `error`: Comprehensive error types and result handling
- `expressions`: Symbolic expression abstract syntax trees
- `manager`: Global context management and execution state
- `solver`: SMT solver interface and Z3 integration
- `symbolic_types`: Symbolic numeric types with trait implementations

## Development

Use standard Rust development tools:

```bash
# Check code
cargo check

# Run tests
cargo test

# Format code
cargo fmt

# Lint code
cargo clippy
```