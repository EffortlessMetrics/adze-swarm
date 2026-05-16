//! Build-script parser generation orchestration.
//!
//! This module keeps the `adze-tool` crate root focused on public API exports
//! while isolating the build-script responsibilities needed to generate and
//! compile parsers.

use std::env;
use std::error::Error as _;
use std::io::Write;
use std::path::{Path, PathBuf};

use tree_sitter_generate::generate_parser_for_grammar;

use crate::{GENERATED_SEMANTIC_VERSION, generate_grammars};

struct BuildParserConfig {
    emit_artifacts: bool,
    use_pure_rust: bool,
    write_debug_file: bool,
}

impl BuildParserConfig {
    fn from_env() -> Self {
        Self {
            emit_artifacts: env_bool("ADZE_EMIT_ARTIFACTS"),
            use_pure_rust: env::var("CARGO_FEATURE_PURE_RUST").is_ok()
                || env::var("ADZE_USE_PURE_RUST").is_ok(),
            write_debug_file: rust_log_contains("debug") || env::var("ADZE_DEBUG_FILE").is_ok(),
        }
    }
}

/// Using the `cc` crate, generates and compiles a C parser with Tree Sitter
/// for every Adze grammar found in the given module and recursive
/// submodules.
pub fn build_parsers(root_file: &Path) {
    let config = BuildParserConfig::from_env();

    if config.write_debug_file {
        write_debug_file(root_file, config.use_pure_rust);
    }

    if config.use_pure_rust {
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

    require_tree_sitter_cli_if_requested();

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    for grammar in generate_grammars(root_file).unwrap() {
        let grammar_str = serde_json::to_string(&grammar).unwrap();
        if config.emit_artifacts {
            eprintln!(
                "Generated grammar JSON:\n{}",
                serde_json::to_string_pretty(&grammar).unwrap()
            );
        }

        let dump_path = write_last_grammar_json(&out_dir, &grammar_str);
        let (grammar_name, grammar_c) =
            generate_c_parser(&grammar_str, dump_path.as_deref(), config.emit_artifacts);
        let grammar_dir = prepare_grammar_dir(&out_dir, &grammar_name, config.emit_artifacts);

        write_parser_sources(grammar_dir.path(), &grammar_name, &grammar, &grammar_c);
        write_wasm_sysroot_if_needed(grammar_dir.path());
        compile_parser(grammar_dir.path(), &grammar_name);
    }
}

fn env_bool(name: &str) -> bool {
    env::var(name)
        .map(|value| value.parse().unwrap_or(false))
        .unwrap_or(false)
}

fn rust_log_contains(needle: &str) -> bool {
    env::var("RUST_LOG")
        .ok()
        .unwrap_or_default()
        .contains(needle)
}

fn write_debug_file(root_file: &Path, use_pure_rust: bool) {
    if let Ok(mut f) = std::fs::File::create("adze_debug.txt") {
        writeln!(f, "build_parsers called for: {}", root_file.display()).ok();
        writeln!(
            f,
            "CARGO_FEATURE_PURE_RUST={:?}",
            env::var("CARGO_FEATURE_PURE_RUST")
        )
        .ok();
        writeln!(f, "ADZE_USE_PURE_RUST={:?}", env::var("ADZE_USE_PURE_RUST")).ok();
        writeln!(f, "use_pure_rust={use_pure_rust}").ok();
    }
}

fn require_tree_sitter_cli_if_requested() {
    if env::var("ADZE_REQUIRE_TS_CLI").is_ok()
        && let Err(e) = std::process::Command::new("tree-sitter")
            .arg("--version")
            .output()
    {
        eprintln!("Warning: tree-sitter CLI not found or not executable");
        eprintln!("  Details: {e}");
        eprintln!("  Hint: Install tree-sitter CLI >= 0.22 with: npm install -g tree-sitter-cli");
        eprintln!("  Then verify with: tree-sitter --version");
    }
}

fn write_last_grammar_json(out_dir: &Path, grammar_str: &str) -> Option<PathBuf> {
    let dump_path = out_dir.join("last_grammar.json");
    let _ = std::fs::write(&dump_path, grammar_str);
    Some(dump_path)
}

fn generate_c_parser(
    grammar_str: &str,
    dump_path: Option<&Path>,
    emit_artifacts: bool,
) -> (String, String) {
    match generate_parser_for_grammar(grammar_str, GENERATED_SEMANTIC_VERSION) {
        Ok(result) => {
            // Also save a per-grammar copy for easier debugging.
            if let Some(base_path) = dump_path {
                let named_path = base_path.with_file_name(format!("grammar_{}.json", result.0));
                let _ = std::fs::write(named_path, grammar_str);
            }
            result
        }
        Err(e) => {
            eprintln!("ERROR: Tree-sitter C generation failed for grammar");
            eprintln!("  Error: {e}");
            eprintln!(
                "  Hint: Ensure tree-sitter CLI >= 0.22 is on PATH (run `tree-sitter --version`)"
            );
            eprintln!("  Hint: Check that the grammar JSON is valid");
            if emit_artifacts {
                eprintln!("  Debug: See generated grammar JSON above");
            }
            if let Some(p) = dump_path {
                eprintln!("  Debug: Wrote grammar JSON to {}", p.display());
            }
            panic!("C backend parser generation failed: {e}");
        }
    }
}

enum GrammarWorkDir {
    Artifact(PathBuf),
    Temporary(tempfile::TempDir),
}

impl GrammarWorkDir {
    fn path(&self) -> &Path {
        match self {
            Self::Artifact(path) => path,
            Self::Temporary(tempdir) => tempdir.path(),
        }
    }
}

fn prepare_grammar_dir(out_dir: &Path, grammar_name: &str, emit_artifacts: bool) -> GrammarWorkDir {
    if !emit_artifacts {
        return GrammarWorkDir::Temporary(
            tempfile::Builder::new()
                .prefix("grammar")
                .tempdir()
                .unwrap(),
        );
    }

    let grammar_dir = out_dir.join(format!("grammar_{grammar_name}"));
    if grammar_dir.is_dir() {
        std::fs::remove_dir_all(&grammar_dir).expect("Couldn't clear old artifacts");
    }
    std::fs::DirBuilder::new()
        .recursive(true)
        .create(&grammar_dir)
        .expect("Couldn't create grammar JSON directory");
    GrammarWorkDir::Artifact(grammar_dir)
}

fn write_parser_sources(
    grammar_dir: &Path,
    grammar_name: &str,
    grammar: &serde_json::Value,
    grammar_c: &str,
) {
    let grammar_file = grammar_dir.join("parser.c");
    std::fs::File::create(grammar_file)
        .unwrap()
        .write_all(grammar_c.as_bytes())
        .unwrap();

    std::fs::File::create(grammar_dir.join(format!("{grammar_name}.json")))
        .unwrap()
        .write_all(serde_json::to_string_pretty(grammar).unwrap().as_bytes())
        .unwrap();

    let header_dir = grammar_dir.join("tree_sitter");
    std::fs::create_dir(&header_dir).unwrap();
    std::fs::File::create(header_dir.join("parser.h"))
        .unwrap()
        .write_all(tree_sitter::PARSER_HEADER.as_bytes())
        .unwrap();
}

fn write_wasm_sysroot_if_needed(grammar_dir: &Path) {
    let target = env::var("TARGET").unwrap_or_else(|_| {
        // Fallback to the current target if TARGET is not set.
        std::env::consts::ARCH.to_string() + "-" + std::env::consts::OS
    });
    if !target.starts_with("wasm32") {
        return;
    }

    let sysroot_dir = grammar_dir.join("sysroot");
    std::fs::create_dir(&sysroot_dir).unwrap();
    write_sysroot_header(
        &sysroot_dir,
        "stdint.h",
        include_bytes!("wasm-sysroot/stdint.h"),
    );
    write_sysroot_header(
        &sysroot_dir,
        "stdlib.h",
        include_bytes!("wasm-sysroot/stdlib.h"),
    );
    write_sysroot_header(
        &sysroot_dir,
        "stdio.h",
        include_bytes!("wasm-sysroot/stdio.h"),
    );
    write_sysroot_header(
        &sysroot_dir,
        "stdbool.h",
        include_bytes!("wasm-sysroot/stdbool.h"),
    );
}

fn write_sysroot_header(sysroot_dir: &Path, filename: &str, contents: &[u8]) {
    std::fs::File::create(sysroot_dir.join(filename))
        .unwrap()
        .write_all(contents)
        .unwrap();
}

fn compile_parser(grammar_dir: &Path, grammar_name: &str) {
    let mut c_config = cc::Build::new();
    c_config
        .std("c11")
        .include(grammar_dir)
        .include(grammar_dir.join("sysroot"));

    // Cross-platform warning suppression.
    c_config.warnings(false);

    // Platform-specific optimizations.
    if cfg!(target_env = "msvc") {
        c_config.flag_if_supported("/EHsc");
    } else {
        c_config.flag_if_supported("-fno-exceptions");
    }

    c_config.file(grammar_dir.join("parser.c"));
    add_first_available_scanner(&mut c_config, grammar_dir);
    c_config.compile(&archive_name(grammar_name));
}

fn add_first_available_scanner(c_config: &mut cc::Build, grammar_dir: &Path) {
    for (path, is_cpp) in scanner_candidates(grammar_dir) {
        if path.exists() {
            if is_cpp {
                c_config.cpp(true);
            }
            c_config.file(path);
            break;
        }
    }
}

fn scanner_candidates(grammar_dir: &Path) -> Vec<(PathBuf, bool)> {
    let mut candidates = vec![
        (grammar_dir.join("scanner.c"), false),
        (grammar_dir.join("scanner.cc"), true),
        (grammar_dir.join("scanner.cpp"), true),
    ];

    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let src_dir = Path::new(&manifest_dir).join("src");
        let scanner_subdir = src_dir.join("scanner");
        candidates.extend([
            (src_dir.join("scanner.c"), false),
            (src_dir.join("scanner.cc"), true),
            (src_dir.join("scanner.cpp"), true),
            (scanner_subdir.join("scanner.c"), false),
            (scanner_subdir.join("scanner.cc"), true),
            (scanner_subdir.join("scanner.cpp"), true),
        ]);
    }

    candidates
}

fn archive_name(grammar_name: &str) -> String {
    grammar_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn run_pure_rust_builder(root_file: &Path, strict: bool) {
    use crate::pure_rust_builder::{BuildOptions, build_parser_for_crate};
    let options = BuildOptions::default();
    match build_parser_for_crate(root_file, options) {
        Ok(results) => {
            for result in results {
                println!("cargo:rerun-if-changed={}", result.parser_path);
                if rust_log_contains("debug") {
                    println!("Built pure-Rust parser for {}", result.grammar_name);
                }
            }
        }
        Err(e) if strict => {
            eprintln!("Failed to build pure-Rust parser: {e}");
            let mut source = e.source();
            while let Some(err) = source {
                eprintln!("  Caused by: {err}");
                source = err.source();
            }
            panic!("FATAL: Pure-Rust parser generation failed: {e:#}");
        }
        Err(e) => {
            println!("cargo:warning=Failed to build optional pure-Rust parser modules: {e}");
        }
    }
}
