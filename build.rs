fn main() {
    // Configure Z3 library path for macOS Homebrew installation
    println!("cargo:rustc-link-search=native=/opt/homebrew/opt/z3/lib");
    println!("cargo:rustc-link-lib=z3");

    // Also try common Linux paths
    println!("cargo:rustc-link-search=native=/usr/lib");
    println!("cargo:rustc-link-search=native=/usr/local/lib");

    // On macOS, compile the coverage helper for weak symbol support
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rerun-if-changed=src/coverage_helper.c");
        cc::Build::new()
            .file("src/coverage_helper.c")
            .compile("coverage_helper");

        // Allow weak symbol references to LLVM coverage symbols
        println!("cargo:rustc-link-arg=-Wl,-undefined,dynamic_lookup");
    }
}
