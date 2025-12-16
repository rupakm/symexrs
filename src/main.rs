use rust_project::{init, SymExResult};

fn main() -> SymExResult<()> {
    println!("Symbolic Execution Engine v{}", rust_project::VERSION);
    
    // Initialize the symbolic execution manager
    let _manager = init()?;
    println!("Symbolic execution engine initialized successfully!");
    
    Ok(())
}
