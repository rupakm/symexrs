use symexrs::{SymString, init_global};

fn main() {
    // Initialize the global symbolic execution manager
    init_global().expect("Failed to initialize global manager");

    println!("=== SymString Basic Demo ===\n");

    // Create symbolic strings
    println!("1. Creating symbolic strings:");
    let s1 = SymString::new_global();
    println!("   s1: variable_name = {}", s1.variable_name());
    println!("   s1: concrete_value = {:?}", s1.concrete_value());

    let s2 = SymString::from_concrete("hello");
    println!("   s2: variable_name = {}", s2.variable_name());
    println!("   s2: concrete_value = {:?}", s2.concrete_value());

    let s3 = SymString::with_value("world".to_string());
    println!("   s3: variable_name = {}", s3.variable_name());
    println!("   s3: concrete_value = {:?}", s3.concrete_value());

    println!("\n2. Accessing string properties:");
    println!("   s2.expr() = {:?}", s2.expr());
    println!("   s3.expr() = {:?}", s3.expr());

    println!("\n3. Modifying concrete values:");
    let mut s4 = SymString::new_global();
    println!("   s4 before: {:?}", s4.concrete_value());
    s4.set_concrete_value("modified".to_string());
    println!("   s4 after: {:?}", s4.concrete_value());

    println!("\n=== Demo Complete ===");
}
