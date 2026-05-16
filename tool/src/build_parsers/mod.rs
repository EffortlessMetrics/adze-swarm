//! Build-script parser generation orchestration.

mod artifacts;
mod c_backend;
mod pure_rust;
mod scanner;
mod sysroot;

use std::path::Path;

pub(crate) use pure_rust::run_pure_rust_builder;

use crate::generate_grammars;

/// Using the `cc` crate, generates and compiles a C parser with Tree Sitter
/// for every Adze grammar found in the given module and recursive
/// submodules.
pub fn build_parsers(root_file: &Path) {
    let options = BuildParserOptions::from_env();
    options.write_debug_file(root_file);

    if options.use_pure_rust {
        // Use pure-Rust builder exclusively. Critical: don't fall through to C generation.
        run_pure_rust_builder(root_file, true);
        return;
    }

    // Build pure-Rust parser modules opportunistically even on the C-codegen path.
    //
    // Why: downstream crates can enable `adze/pure-rust` without having a
    // corresponding local Cargo feature. In that case `CARGO_FEATURE_PURE_RUST`
    // is not set for the build script, but the proc-macro still expands to an
    // `include!(.../parser_<grammar>.rs)` path. Generating the Rust parser here
    // avoids that mismatch and keeps C codegen behavior unchanged.
    run_pure_rust_builder(root_file, false);

    c_backend::ensure_tree_sitter_cli_if_required();

    for grammar in generate_grammars(root_file).unwrap() {
        c_backend::compile_grammar(grammar, &options);
    }
}

pub(crate) struct BuildParserOptions {
    out_dir: String,
    emit_artifacts: bool,
    enable_debug_file: bool,
    use_pure_rust: bool,
}

impl BuildParserOptions {
    fn from_env() -> Self {
        Self {
            out_dir: std::env::var("OUT_DIR").unwrap(),
            emit_artifacts: std::env::var("ADZE_EMIT_ARTIFACTS")
                .map(|s| s.parse().unwrap_or(false))
                .unwrap_or(false),
            enable_debug_file: std::env::var("RUST_LOG")
                .ok()
                .unwrap_or_default()
                .contains("debug")
                || std::env::var("ADZE_DEBUG_FILE").is_ok(),
            use_pure_rust: std::env::var("CARGO_FEATURE_PURE_RUST").is_ok()
                || std::env::var("ADZE_USE_PURE_RUST").is_ok(),
        }
    }

    fn write_debug_file(&self, root_file: &Path) {
        if !self.enable_debug_file {
            return;
        }

        use std::io::Write;
        if let Ok(mut f) = std::fs::File::create("adze_debug.txt") {
            writeln!(f, "build_parsers called for: {}", root_file.display()).ok();
            writeln!(
                f,
                "CARGO_FEATURE_PURE_RUST={:?}",
                std::env::var("CARGO_FEATURE_PURE_RUST")
            )
            .ok();
            writeln!(
                f,
                "ADZE_USE_PURE_RUST={:?}",
                std::env::var("ADZE_USE_PURE_RUST")
            )
            .ok();
            writeln!(f, "use_pure_rust={}", self.use_pure_rust).ok();
        }
    }
}
