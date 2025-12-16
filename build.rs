fn main() {
    // Configure Z3 library path for macOS Homebrew installation
    println!("cargo:rustc-link-search=native=/opt/homebrew/opt/z3/lib");
    println!("cargo:rustc-link-lib=z3");
    
    // Also try common Linux paths
    println!("cargo:rustc-link-search=native=/usr/lib");
    println!("cargo:rustc-link-search=native=/usr/local/lib");
}