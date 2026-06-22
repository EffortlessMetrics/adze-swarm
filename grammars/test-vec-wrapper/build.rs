use std::path::Path;

fn main() {
    // Tell rustc this cfg is intentional so it doesn't warn
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");

    // Use pure Rust parser generation
    // SAFETY: This is safe in a build script as it runs in a single-threaded context
    unsafe {
        std::env::set_var("ADZE_USE_PURE_RUST", "1");
    }

    // Enable debug output
    // SAFETY: This is safe in a build script as it runs in a single-threaded context
    unsafe {
        std::env::set_var("ADZE_EMIT_ARTIFACTS", "true");
    }

    // Build the parsers
    adze_tool::build_parsers(Path::new("src/lib.rs"));
}
