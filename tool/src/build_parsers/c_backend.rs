//! Tree-sitter C backend generation and compilation.

use serde_json::Value;
use tree_sitter_generate::generate_parser_for_grammar;

use super::{BuildParserOptions, artifacts, scanner, sysroot};
use crate::GENERATED_SEMANTIC_VERSION;

pub(crate) fn ensure_tree_sitter_cli_if_required() {
    if std::env::var("ADZE_REQUIRE_TS_CLI").is_ok()
        && let Err(e) = std::process::Command::new("tree-sitter")
            .arg("--version")
            .output()
    {
        eprintln!("Warning: tree-sitter CLI not found or not executable");
        eprintln!("  Details: {}", e);
        eprintln!("  Hint: Install tree-sitter CLI >= 0.22 with: npm install -g tree-sitter-cli");
        eprintln!("  Then verify with: tree-sitter --version");
    }
}

pub(crate) fn compile_grammar(grammar: Value, options: &BuildParserOptions) {
    let grammar_str = serde_json::to_string(&grammar).unwrap();
    emit_grammar_json_if_requested(&grammar, options.emit_artifacts);

    let dump_path = artifacts::dump_path(options);
    let _ = std::fs::write(&dump_path, &grammar_str);

    let (grammar_name, grammar_c) =
        generate_c_parser(&grammar_str, options.emit_artifacts, &dump_path);
    let artifacts =
        artifacts::GrammarArtifacts::create(&grammar_name, &grammar, &grammar_c, options);
    let sysroot_dir = sysroot::write_wasm_sysroot_if_needed(&artifacts.dir);

    let mut c_config = cc::Build::new();
    configure_c_compiler(&mut c_config, &artifacts.dir, &sysroot_dir);
    scanner::add_first_available_scanner(&mut c_config, &artifacts.dir);

    c_config.compile(&library_name(&grammar_name));
}

fn emit_grammar_json_if_requested(grammar: &Value, emit_artifacts: bool) {
    if emit_artifacts {
        eprintln!(
            "Generated grammar JSON:\n{}",
            serde_json::to_string_pretty(grammar).unwrap()
        );
    }
}

fn generate_c_parser(
    grammar_str: &str,
    emit_artifacts: bool,
    dump_path: &std::path::Path,
) -> (String, String) {
    match generate_parser_for_grammar(grammar_str, GENERATED_SEMANTIC_VERSION) {
        Ok(result) => {
            let named_path = dump_path.with_file_name(format!("grammar_{}.json", result.0));
            let _ = std::fs::write(named_path, grammar_str);
            result
        }
        Err(e) => {
            eprintln!("ERROR: Tree-sitter C generation failed for grammar");
            eprintln!("  Error: {}", e);
            eprintln!(
                "  Hint: Ensure tree-sitter CLI >= 0.22 is on PATH (run `tree-sitter --version`)"
            );
            eprintln!("  Hint: Check that the grammar JSON is valid");
            if emit_artifacts {
                eprintln!("  Debug: See generated grammar JSON above");
            }
            eprintln!("  Debug: Wrote grammar JSON to {}", dump_path.display());
            panic!("C backend parser generation failed: {}", e);
        }
    }
}

fn configure_c_compiler(
    c_config: &mut cc::Build,
    dir: &std::path::Path,
    sysroot_dir: &std::path::Path,
) {
    c_config.std("c11").include(dir).include(sysroot_dir);
    c_config.warnings(false);

    if cfg!(target_env = "msvc") {
        c_config.flag_if_supported("/EHsc");
    } else {
        c_config.flag_if_supported("-fno-exceptions");
    }

    c_config.file(dir.join("parser.c"));
}

fn library_name(grammar_name: &str) -> String {
    grammar_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}
