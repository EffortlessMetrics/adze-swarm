//! Pure-Rust parser builder integration for build scripts.

use std::path::Path;

pub(crate) fn run_pure_rust_builder(root_file: &Path, strict: bool) {
    use crate::pure_rust_builder::{BuildOptions, build_parser_for_crate};
    let options = BuildOptions::default();
    match build_parser_for_crate(root_file, options) {
        Ok(results) => {
            for result in results {
                println!("cargo:rerun-if-changed={}", result.parser_path);
                if std::env::var("RUST_LOG")
                    .ok()
                    .unwrap_or_default()
                    .contains("debug")
                {
                    println!("Built pure-Rust parser for {}", result.grammar_name);
                }
            }
        }
        Err(e) if strict => {
            eprintln!("Failed to build pure-Rust parser: {}", e);
            let mut source = e.source();
            while let Some(err) = source {
                eprintln!("  Caused by: {}", err);
                source = err.source();
            }
            panic!("FATAL: Pure-Rust parser generation failed: {:#}", e);
        }
        Err(e) => {
            println!("cargo:warning=Failed to build optional pure-Rust parser modules: {e}");
        }
    }
}
