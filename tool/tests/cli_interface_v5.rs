//! CLI interface and configuration tests for `adze-tool`.
//!
//! 8 categories × 8 tests = 64 tests covering BuildOptions defaults, custom
//! settings, build output file generation, error handling, output path
//! handling, grammar name configuration, artifact emission control,
//! and edge cases.

use std::fs;
use std::path::Path;

use adze_tool::cli::{Cli, Commands, OutputFormat};
use adze_tool::pure_rust_builder::{BuildOptions, build_parser_from_json};
use clap::Parser;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_opts(dir: &TempDir) -> BuildOptions {
    BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    }
}

fn make_opts_with_artifacts(dir: &TempDir) -> BuildOptions {
    BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: true,
        compress_tables: false,
    }
}

fn make_opts_compressed(dir: &TempDir) -> BuildOptions {
    BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: true,
    }
}

fn make_opts_full(dir: &TempDir) -> BuildOptions {
    BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: true,
        compress_tables: true,
    }
}

fn minimal_grammar(name: &str) -> String {
    serde_json::json!({
        "name": name,
        "word": null,
        "rules": {
            "source_file": {
                "type": "SYMBOL",
                "name": "value"
            },
            "value": {
                "type": "PATTERN",
                "value": "\\d+"
            }
        },
        "extras": [
            { "type": "PATTERN", "value": "\\s" }
        ],
        "conflicts": [],
        "precedences": [],
        "externals": [],
        "inline": [],
        "supertypes": []
    })
    .to_string()
}

fn choice_grammar(name: &str) -> String {
    serde_json::json!({
        "name": name,
        "word": null,
        "rules": {
            "source_file": {
                "type": "SYMBOL",
                "name": "item"
            },
            "item": {
                "type": "CHOICE",
                "members": [
                    { "type": "PATTERN", "value": "\\d+" },
                    { "type": "PATTERN", "value": "[a-z]+" }
                ]
            }
        },
        "extras": [
            { "type": "PATTERN", "value": "\\s" }
        ],
        "conflicts": [],
        "precedences": [],
        "externals": [],
        "inline": [],
        "supertypes": []
    })
    .to_string()
}

fn seq_grammar(name: &str) -> String {
    serde_json::json!({
        "name": name,
        "word": null,
        "rules": {
            "source_file": {
                "type": "SEQ",
                "members": [
                    { "type": "STRING", "value": "begin" },
                    { "type": "PATTERN", "value": "[a-z]+" }
                ]
            }
        },
        "extras": [
            { "type": "PATTERN", "value": "\\s" }
        ],
        "conflicts": [],
        "precedences": [],
        "externals": [],
        "inline": [],
        "supertypes": []
    })
    .to_string()
}

// ===========================================================================
// 1. BuildOptions default values (8 tests)
// ===========================================================================

#[test]
fn default_compress_tables_enabled() {
    let opts = BuildOptions::default();
    assert!(opts.compress_tables);
}

#[test]
fn default_emit_artifacts_disabled_without_env() {
    // When ADZE_EMIT_ARTIFACTS is not set, defaults to false
    if std::env::var("ADZE_EMIT_ARTIFACTS").is_err() {
        let opts = BuildOptions::default();
        assert!(!opts.emit_artifacts);
    }
}

#[test]
fn default_out_dir_non_empty() {
    let opts = BuildOptions::default();
    assert!(!opts.out_dir.is_empty());
}

#[test]
fn default_out_dir_fallback_without_env() {
    // Without OUT_DIR the default falls back to "target/debug"
    if std::env::var("OUT_DIR").is_err() {
        let opts = BuildOptions::default();
        assert_eq!(opts.out_dir, "target/debug");
    }
}

#[test]
fn default_debug_format_includes_struct_name() {
    let opts = BuildOptions::default();
    let debug_str = format!("{opts:?}");
    assert!(debug_str.contains("BuildOptions"));
}

#[test]
fn default_debug_format_includes_all_fields() {
    let opts = BuildOptions::default();
    let debug_str = format!("{opts:?}");
    assert!(debug_str.contains("out_dir"));
    assert!(debug_str.contains("emit_artifacts"));
    assert!(debug_str.contains("compress_tables"));
}

#[test]
fn default_options_build_minimal_grammar() {
    let dir = TempDir::new().unwrap();
    let opts = make_opts(&dir);
    let res = build_parser_from_json(minimal_grammar("cli_v5_def1"), opts);
    assert!(res.is_ok(), "default opts should build: {res:?}");
}

#[test]
fn default_options_produce_nonempty_parser_code() {
    let dir = TempDir::new().unwrap();
    let opts = make_opts(&dir);
    let res = build_parser_from_json(minimal_grammar("cli_v5_def2"), opts).unwrap();
    assert!(!res.parser_code.is_empty());
}

// ===========================================================================
// 2. BuildOptions with custom settings (8 tests)
// ===========================================================================

#[test]
fn custom_out_dir_is_preserved() {
    let opts = BuildOptions {
        out_dir: "/tmp/custom_cli_v5".into(),
        emit_artifacts: false,
        compress_tables: false,
    };
    assert_eq!(opts.out_dir, "/tmp/custom_cli_v5");
}

#[test]
fn custom_emit_artifacts_true() {
    let opts = BuildOptions {
        out_dir: "ignored".into(),
        emit_artifacts: true,
        compress_tables: false,
    };
    assert!(opts.emit_artifacts);
}

#[test]
fn custom_compress_tables_false() {
    let opts = BuildOptions {
        out_dir: "ignored".into(),
        emit_artifacts: false,
        compress_tables: false,
    };
    assert!(!opts.compress_tables);
}

#[test]
fn custom_all_fields_set() {
    let dir = TempDir::new().unwrap();
    let opts = make_opts_full(&dir);
    assert!(opts.emit_artifacts);
    assert!(opts.compress_tables);
    assert!(!opts.out_dir.is_empty());
}

#[test]
fn custom_compressed_build_succeeds() {
    let dir = TempDir::new().unwrap();
    let opts = make_opts_compressed(&dir);
    let res = build_parser_from_json(minimal_grammar("cli_v5_cust1"), opts);
    assert!(res.is_ok());
}

#[test]
fn custom_artifacts_build_succeeds() {
    let dir = TempDir::new().unwrap();
    let opts = make_opts_with_artifacts(&dir);
    let res = build_parser_from_json(minimal_grammar("cli_v5_cust2"), opts);
    assert!(res.is_ok());
}

#[test]
fn custom_full_options_build_succeeds() {
    let dir = TempDir::new().unwrap();
    let opts = make_opts_full(&dir);
    let res = build_parser_from_json(minimal_grammar("cli_v5_cust3"), opts);
    assert!(res.is_ok());
}

#[test]
fn custom_options_clone_preserves_values() {
    let opts = BuildOptions {
        out_dir: "clone_test".into(),
        emit_artifacts: true,
        compress_tables: true,
    };
    let cloned = opts.clone();
    assert_eq!(cloned.out_dir, "clone_test");
    assert!(cloned.emit_artifacts);
    assert!(cloned.compress_tables);
}

// ===========================================================================
// 3. Build output file generation (8 tests)
// ===========================================================================

#[test]
fn build_creates_parser_file_on_disk() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(minimal_grammar("cli_v5_out1"), make_opts(&dir)).unwrap();
    let parser_path = Path::new(&res.parser_path);
    assert!(parser_path.is_file());
}

#[test]
fn build_parser_file_is_nonempty() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(minimal_grammar("cli_v5_out2"), make_opts(&dir)).unwrap();
    let meta = fs::metadata(&res.parser_path).unwrap();
    assert!(meta.len() > 0);
}

#[test]
fn build_parser_file_contains_grammar_name_constant() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(minimal_grammar("cli_v5_out3"), make_opts(&dir)).unwrap();
    let contents = fs::read_to_string(&res.parser_path).unwrap();
    assert!(contents.contains("GRAMMAR_NAME"));
}

#[test]
fn build_parser_path_ends_with_rs() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(minimal_grammar("cli_v5_out4"), make_opts(&dir)).unwrap();
    assert!(res.parser_path.ends_with(".rs"));
}

#[test]
fn build_creates_grammar_directory() {
    let dir = TempDir::new().unwrap();
    build_parser_from_json(minimal_grammar("cli_v5_out5"), make_opts(&dir)).unwrap();
    let grammar_dir = dir.path().join("grammar_cli_v5_out5");
    assert!(grammar_dir.is_dir());
}

#[test]
fn build_result_has_nonempty_node_types() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(minimal_grammar("cli_v5_out6"), make_opts(&dir)).unwrap();
    assert!(!res.node_types_json.is_empty());
}

#[test]
fn build_result_node_types_is_valid_json() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(minimal_grammar("cli_v5_out7"), make_opts(&dir)).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&res.node_types_json).unwrap();
    assert!(parsed.is_array() || parsed.is_object());
}

#[test]
fn build_stats_has_positive_state_count() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(minimal_grammar("cli_v5_out8"), make_opts(&dir)).unwrap();
    assert!(res.build_stats.state_count > 0);
}

// ===========================================================================
// 4. Error handling for invalid inputs (8 tests)
// ===========================================================================

#[test]
fn error_on_empty_json_string() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(String::new(), make_opts(&dir));
    assert!(res.is_err());
}

#[test]
fn error_on_malformed_json() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json("{ not valid json".to_owned(), make_opts(&dir));
    assert!(res.is_err());
}

#[test]
fn error_on_json_array() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json("[]".to_owned(), make_opts(&dir));
    assert!(res.is_err());
}

#[test]
fn error_on_empty_rules() {
    let dir = TempDir::new().unwrap();
    let json = serde_json::json!({
        "name": "cli_v5_err1",
        "word": null,
        "rules": {},
        "extras": [],
        "conflicts": [],
        "precedences": [],
        "externals": [],
        "inline": [],
        "supertypes": []
    })
    .to_string();
    let res = build_parser_from_json(json, make_opts(&dir));
    assert!(res.is_err());
}

#[test]
fn error_on_null_rules() {
    let dir = TempDir::new().unwrap();
    let json = serde_json::json!({
        "name": "cli_v5_err2",
        "word": null,
        "rules": null,
        "extras": [],
        "conflicts": [],
        "precedences": [],
        "externals": [],
        "inline": [],
        "supertypes": []
    })
    .to_string();
    let res = build_parser_from_json(json, make_opts(&dir));
    assert!(res.is_err());
}

#[test]
fn error_on_json_number() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json("42".to_owned(), make_opts(&dir));
    assert!(res.is_err());
}

#[test]
fn error_on_json_string_literal() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json("\"hello\"".to_owned(), make_opts(&dir));
    assert!(res.is_err());
}

#[test]
fn error_on_json_null() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json("null".to_owned(), make_opts(&dir));
    assert!(res.is_err());
}

// ===========================================================================
// 5. Output path handling (8 tests)
// ===========================================================================

#[test]
fn output_path_contains_grammar_name() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(minimal_grammar("cli_v5_path1"), make_opts(&dir)).unwrap();
    assert!(
        res.parser_path.contains("cli_v5_path1"),
        "parser path should contain grammar name: {}",
        res.parser_path
    );
}

#[test]
fn output_dir_with_trailing_slash() {
    let dir = TempDir::new().unwrap();
    let mut path_str: String = dir.path().to_string_lossy().into();
    path_str.push('/');
    let opts = BuildOptions {
        out_dir: path_str,
        emit_artifacts: false,
        compress_tables: false,
    };
    let res = build_parser_from_json(minimal_grammar("cli_v5_path2"), opts);
    assert!(res.is_ok());
}

#[test]
fn output_dir_with_spaces() {
    let dir = TempDir::new().unwrap();
    let spaced = dir.path().join("path with spaces");
    fs::create_dir_all(&spaced).unwrap();
    let opts = BuildOptions {
        out_dir: spaced.to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    };
    let res = build_parser_from_json(minimal_grammar("cli_v5_path3"), opts);
    assert!(res.is_ok());
}

#[test]
fn separate_grammars_get_separate_dirs() {
    let dir = TempDir::new().unwrap();
    build_parser_from_json(minimal_grammar("cli_v5_patha"), make_opts(&dir)).unwrap();
    build_parser_from_json(minimal_grammar("cli_v5_pathb"), make_opts(&dir)).unwrap();
    assert!(dir.path().join("grammar_cli_v5_patha").is_dir());
    assert!(dir.path().join("grammar_cli_v5_pathb").is_dir());
}

#[test]
fn parser_module_filename_is_lowercased() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(minimal_grammar("MyLang"), make_opts(&dir)).unwrap();
    assert!(
        res.parser_path.contains("mylang"),
        "filename should be lowercased: {}",
        res.parser_path
    );
}

#[test]
fn parser_file_written_inside_grammar_dir() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(minimal_grammar("cli_v5_path5"), make_opts(&dir)).unwrap();
    let grammar_dir = dir.path().join("grammar_cli_v5_path5");
    assert!(Path::new(&res.parser_path).starts_with(&grammar_dir));
}

#[test]
fn deeply_nested_output_dir() {
    let dir = TempDir::new().unwrap();
    let nested = dir.path().join("a").join("b").join("c");
    fs::create_dir_all(&nested).unwrap();
    let opts = BuildOptions {
        out_dir: nested.to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    };
    let res = build_parser_from_json(minimal_grammar("cli_v5_path6"), opts);
    assert!(res.is_ok());
}

#[test]
fn output_path_uses_lowercase_for_mixed_case() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(minimal_grammar("MixedCaseLang"), make_opts(&dir)).unwrap();
    // Mixed-case grammar names are lowercased in the parser filename
    assert!(
        res.parser_path.contains("mixedcaselang"),
        "name should be lowercased in path: {}",
        res.parser_path
    );
}

// ===========================================================================
// 6. Grammar name configuration (8 tests)
// ===========================================================================

#[test]
fn grammar_name_propagated_to_result() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(minimal_grammar("cli_v5_name1"), make_opts(&dir)).unwrap();
    assert_eq!(res.grammar_name, "cli_v5_name1");
}

#[test]
fn grammar_name_from_choice_grammar() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(choice_grammar("cli_v5_name2"), make_opts(&dir)).unwrap();
    assert_eq!(res.grammar_name, "cli_v5_name2");
}

#[test]
fn grammar_name_from_seq_grammar() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(seq_grammar("cli_v5_name3"), make_opts(&dir)).unwrap();
    assert_eq!(res.grammar_name, "cli_v5_name3");
}

#[test]
fn grammar_name_in_parser_code_on_disk() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(minimal_grammar("cli_v5_name4"), make_opts(&dir)).unwrap();
    let contents = fs::read_to_string(&res.parser_path).unwrap();
    assert!(contents.contains("cli_v5_name4"));
}

#[test]
fn grammar_name_preserved_with_compression() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(minimal_grammar("cli_v5_name5"), make_opts_compressed(&dir))
        .unwrap();
    assert_eq!(res.grammar_name, "cli_v5_name5");
}

#[test]
fn grammar_name_preserved_with_artifacts() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(
        minimal_grammar("cli_v5_name6"),
        make_opts_with_artifacts(&dir),
    )
    .unwrap();
    assert_eq!(res.grammar_name, "cli_v5_name6");
}

#[test]
fn grammar_name_preserved_with_full_options() {
    let dir = TempDir::new().unwrap();
    let res =
        build_parser_from_json(minimal_grammar("cli_v5_name7"), make_opts_full(&dir)).unwrap();
    assert_eq!(res.grammar_name, "cli_v5_name7");
}

#[test]
fn grammar_name_appears_in_parser_path() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(minimal_grammar("cli_v5_name8"), make_opts(&dir)).unwrap();
    assert!(res.parser_path.contains("cli_v5_name8"));
}

// ===========================================================================
// 7. Artifact emission control (8 tests)
// ===========================================================================

#[test]
fn artifacts_enabled_creates_grammar_directory() {
    let dir = TempDir::new().unwrap();
    build_parser_from_json(
        minimal_grammar("cli_v5_art1"),
        make_opts_with_artifacts(&dir),
    )
    .unwrap();
    let grammar_dir = dir.path().join("grammar_cli_v5_art1");
    assert!(grammar_dir.is_dir());
}

#[test]
fn artifacts_enabled_writes_ir_json_file() {
    let dir = TempDir::new().unwrap();
    build_parser_from_json(
        minimal_grammar("cli_v5_art2"),
        make_opts_with_artifacts(&dir),
    )
    .unwrap();
    let ir = dir
        .path()
        .join("grammar_cli_v5_art2")
        .join("grammar.ir.json");
    assert!(ir.is_file());
}

#[test]
fn artifacts_enabled_writes_node_types_file() {
    let dir = TempDir::new().unwrap();
    build_parser_from_json(
        minimal_grammar("cli_v5_art3"),
        make_opts_with_artifacts(&dir),
    )
    .unwrap();
    let nt = dir
        .path()
        .join("grammar_cli_v5_art3")
        .join("NODE_TYPES.json");
    assert!(nt.is_file());
}

#[test]
fn artifacts_enabled_ir_json_is_valid() {
    let dir = TempDir::new().unwrap();
    build_parser_from_json(
        minimal_grammar("cli_v5_art4"),
        make_opts_with_artifacts(&dir),
    )
    .unwrap();
    let ir = dir
        .path()
        .join("grammar_cli_v5_art4")
        .join("grammar.ir.json");
    let contents = fs::read_to_string(ir).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&contents).unwrap();
    assert!(parsed.is_object());
}

#[test]
fn artifacts_disabled_no_ir_json() {
    let dir = TempDir::new().unwrap();
    build_parser_from_json(minimal_grammar("cli_v5_art5"), make_opts(&dir)).unwrap();
    let ir = dir
        .path()
        .join("grammar_cli_v5_art5")
        .join("grammar.ir.json");
    assert!(!ir.exists());
}

#[test]
fn artifacts_disabled_still_creates_parser_module() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(minimal_grammar("cli_v5_art6"), make_opts(&dir)).unwrap();
    assert!(Path::new(&res.parser_path).is_file());
}

#[test]
fn test_artifacts_rebuild_preserves_c_and_authored_files_and_refreshes_rust_parser() {
    let dir = TempDir::new().unwrap();
    let grammar_dir = dir.path().join("grammar_cli_v5_art7");
    fs::create_dir_all(grammar_dir.join("tree_sitter")).unwrap();
    let stale = grammar_dir.join("stale.txt");
    fs::write(&stale, "old").unwrap();
    let other_owners = [
        ("parser.c", "C parser owned by the C writer"),
        ("tree_sitter/parser.h", "C header owned by the C writer"),
    ];
    for (name, bytes) in other_owners {
        fs::write(grammar_dir.join(name), bytes).unwrap();
    }

    for _ in 0..2 {
        let parser_path = grammar_dir.join("parser_cli_v5_art7.rs");
        fs::write(&parser_path, "stale Rust parser").unwrap();
        let result = build_parser_from_json(
            minimal_grammar("cli_v5_art7"),
            make_opts_with_artifacts(&dir),
        )
        .unwrap();
        assert_eq!(fs::read(&stale).unwrap(), b"old");
        for (name, bytes) in other_owners {
            assert_eq!(fs::read(grammar_dir.join(name)).unwrap(), bytes.as_bytes());
        }
        assert_eq!(Path::new(&result.parser_path), parser_path);
        let parser = fs::read(parser_path).unwrap();
        assert!(!parser.is_empty());
        assert_ne!(parser, b"stale Rust parser");
    }
}

#[test]
fn artifacts_node_types_file_is_valid_json() {
    let dir = TempDir::new().unwrap();
    build_parser_from_json(
        minimal_grammar("cli_v5_art8"),
        make_opts_with_artifacts(&dir),
    )
    .unwrap();
    let nt = dir
        .path()
        .join("grammar_cli_v5_art8")
        .join("NODE_TYPES.json");
    let contents = fs::read_to_string(nt).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&contents).unwrap();
    assert!(parsed.is_array() || parsed.is_object());
}

// ===========================================================================
// 8. Edge cases: empty options, conflicting options, CLI parsing (8 tests)
// ===========================================================================

#[test]
fn clone_is_independent_of_original() {
    let opts = BuildOptions {
        out_dir: "original".into(),
        emit_artifacts: true,
        compress_tables: true,
    };
    let mut cloned = opts.clone();
    cloned.out_dir = "modified".into();
    cloned.emit_artifacts = false;
    assert_eq!(opts.out_dir, "original");
    assert!(opts.emit_artifacts);
    assert!(!cloned.emit_artifacts);
}

#[test]
fn rebuild_same_grammar_different_options() {
    let dir = TempDir::new().unwrap();
    // First build: uncompressed
    build_parser_from_json(minimal_grammar("cli_v5_edge1"), make_opts(&dir)).unwrap();
    // Second build: compressed with artifacts
    let res = build_parser_from_json(minimal_grammar("cli_v5_edge1"), make_opts_full(&dir));
    assert!(res.is_ok());
}

#[test]
fn build_stats_symbol_count_positive() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(minimal_grammar("cli_v5_edge2"), make_opts(&dir)).unwrap();
    assert!(res.build_stats.symbol_count > 0);
}

#[test]
fn cli_parse_generate_subcommand() {
    let cli = Cli::try_parse_from([
        "adze",
        "generate",
        "--grammar",
        "my_grammar.js",
        "--output",
        "out_dir",
    ])
    .unwrap();
    match cli.command {
        Commands::Generate {
            grammar, output, ..
        } => {
            assert_eq!(grammar.to_string_lossy(), "my_grammar.js");
            assert_eq!(output.to_string_lossy(), "out_dir");
        }
        _ => panic!("expected Generate command"),
    }
}

#[test]
fn cli_parse_generate_defaults() {
    let cli = Cli::try_parse_from(["adze", "generate"]).unwrap();
    match cli.command {
        Commands::Generate {
            grammar,
            output,
            debug,
            pure_rust,
        } => {
            assert_eq!(grammar.to_string_lossy(), "grammar.js");
            assert_eq!(output.to_string_lossy(), "src");
            assert!(!debug);
            assert!(pure_rust);
        }
        _ => panic!("expected Generate command"),
    }
}

#[test]
fn cli_parse_parse_subcommand() {
    let cli = Cli::try_parse_from(["adze", "parse", "input.txt"]).unwrap();
    match cli.command {
        Commands::Parse { file, parser, .. } => {
            assert_eq!(file.to_string_lossy(), "input.txt");
            assert!(parser.is_none());
        }
        _ => panic!("expected Parse command"),
    }
}

#[test]
fn cli_parse_output_format_variants() {
    // Verify all OutputFormat variants can be constructed
    let formats = [
        OutputFormat::Tree,
        OutputFormat::Sexp,
        OutputFormat::Json,
        OutputFormat::Dot,
    ];
    assert_eq!(formats.len(), 4);
    // Verify Debug is implemented
    for fmt in &formats {
        let debug_str = format!("{fmt:?}");
        assert!(!debug_str.is_empty());
    }
}

#[test]
fn cli_rejects_unknown_subcommand() {
    let result = Cli::try_parse_from(["adze", "nonexistent"]);
    assert!(result.is_err());
}
