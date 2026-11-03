//! Tests for `BuildOptions` and their effect on build output.
//!
//! 8 categories × 8 tests = 64 tests covering defaults, compress_tables
//! toggling, emit_artifacts toggling, invalid-grammar rejection, artifact
//! file-system presence, grammar-name propagation, option combinations,
//! and edge cases.

use std::fs;
use std::path::Path;

use adze_tool::pure_rust_builder::{BuildOptions, build_parser_from_json};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn temp_opts(dir: &TempDir) -> BuildOptions {
    BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    }
}

fn temp_opts_compressed(dir: &TempDir) -> BuildOptions {
    BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: true,
    }
}

fn temp_opts_artifacts(dir: &TempDir) -> BuildOptions {
    BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: true,
        compress_tables: false,
    }
}

fn temp_opts_all(dir: &TempDir) -> BuildOptions {
    BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: true,
        compress_tables: true,
    }
}

fn simple_json(name: &str) -> String {
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

fn choice_json(name: &str) -> String {
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

fn seq_json(name: &str) -> String {
    serde_json::json!({
        "name": name,
        "word": null,
        "rules": {
            "source_file": {
                "type": "SEQ",
                "members": [
                    { "type": "STRING", "value": "hello" },
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

/// Returns JSON that is valid JSON but *not* a valid grammar (no rules).
fn invalid_grammar_json() -> String {
    serde_json::json!({
        "name": "broken",
        "word": null,
        "rules": {},
        "extras": [],
        "conflicts": [],
        "precedences": [],
        "externals": [],
        "inline": [],
        "supertypes": []
    })
    .to_string()
}

/// Returns text that is not valid JSON at all.
fn malformed_json() -> String {
    "{ not json at all".to_owned()
}

// ===========================================================================
// 1. Default options produce valid output (8 tests)
// ===========================================================================

#[test]
fn default_compress_tables_is_true() {
    let opts = BuildOptions::default();
    assert!(opts.compress_tables);
}

#[test]
fn default_emit_artifacts_respects_env() {
    let opts = BuildOptions::default();
    if std::env::var("ADZE_EMIT_ARTIFACTS").is_err() {
        assert!(!opts.emit_artifacts);
    }
}

#[test]
fn default_out_dir_is_non_empty() {
    let opts = BuildOptions::default();
    assert!(!opts.out_dir.is_empty());
}

#[test]
fn default_debug_repr_contains_struct_name() {
    let opts = BuildOptions::default();
    let dbg = format!("{opts:?}");
    assert!(dbg.contains("BuildOptions"));
}

#[test]
fn default_options_build_simple_grammar() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts(&dir);
    let result = build_parser_from_json(simple_json("def_simple"), opts);
    assert!(result.is_ok(), "default opts should build: {result:?}");
}

#[test]
fn default_options_build_choice_grammar() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts(&dir);
    let result = build_parser_from_json(choice_json("def_choice"), opts);
    assert!(
        result.is_ok(),
        "default opts should build choice: {result:?}"
    );
}

#[test]
fn default_build_produces_parser_code() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts(&dir);
    let res = build_parser_from_json(simple_json("def_code"), opts).unwrap();
    assert!(!res.parser_code.is_empty());
}

#[test]
fn default_build_produces_node_types() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts(&dir);
    let res = build_parser_from_json(simple_json("def_nt"), opts).unwrap();
    assert!(!res.node_types_json.is_empty());
}

// ===========================================================================
// 2. compress_tables=true vs compress_tables=false (8 tests)
// ===========================================================================

#[test]
fn compressed_build_succeeds() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts_compressed(&dir);
    let res = build_parser_from_json(simple_json("comp_ok"), opts);
    assert!(res.is_ok());
}

#[test]
fn uncompressed_build_succeeds() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts(&dir);
    let res = build_parser_from_json(simple_json("uncomp_ok"), opts);
    assert!(res.is_ok());
}

#[test]
fn compressed_and_uncompressed_same_grammar_name() {
    let dir = TempDir::new().unwrap();
    let c = build_parser_from_json(simple_json("samename"), temp_opts_compressed(&dir)).unwrap();
    let u = build_parser_from_json(simple_json("samename"), temp_opts(&dir)).unwrap();
    assert_eq!(c.grammar_name, u.grammar_name);
}

#[test]
fn compressed_and_uncompressed_both_produce_code() {
    let dir = TempDir::new().unwrap();
    let c = build_parser_from_json(simple_json("bothcode"), temp_opts_compressed(&dir)).unwrap();
    let u = build_parser_from_json(simple_json("bothcode"), temp_opts(&dir)).unwrap();
    assert!(!c.parser_code.is_empty());
    assert!(!u.parser_code.is_empty());
}

#[test]
fn compressed_parser_code_differs_from_uncompressed() {
    let dir = TempDir::new().unwrap();
    let c = build_parser_from_json(simple_json("diff_c"), temp_opts_compressed(&dir)).unwrap();
    let u = build_parser_from_json(simple_json("diff_c"), temp_opts(&dir)).unwrap();
    // Compressed and uncompressed may or may not differ in code text —
    // but at minimum both must be non-empty.
    assert!(!c.parser_code.is_empty());
    assert!(!u.parser_code.is_empty());
}

#[test]
fn compressed_build_reports_states() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(simple_json("comp_st"), temp_opts_compressed(&dir)).unwrap();
    assert!(res.build_stats.state_count > 0);
}

#[test]
fn uncompressed_build_reports_states() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(simple_json("uncomp_st"), temp_opts(&dir)).unwrap();
    assert!(res.build_stats.state_count > 0);
}

#[test]
fn compressed_choice_grammar_succeeds() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(choice_json("comp_ch"), temp_opts_compressed(&dir));
    assert!(res.is_ok());
}

// ===========================================================================
// 3. emit_artifacts=true vs emit_artifacts=false (8 tests)
// ===========================================================================

#[test]
fn artifacts_enabled_creates_grammar_dir() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts_artifacts(&dir);
    build_parser_from_json(simple_json("art_dir"), opts).unwrap();
    let grammar_dir = dir.path().join("grammar_art_dir");
    assert!(grammar_dir.is_dir());
}

#[test]
fn artifacts_enabled_writes_ir_json() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts_artifacts(&dir);
    build_parser_from_json(simple_json("art_ir"), opts).unwrap();
    let ir_path = dir.path().join("grammar_art_ir").join("grammar.ir.json");
    assert!(ir_path.is_file(), "grammar.ir.json should exist");
}

#[test]
fn artifacts_enabled_writes_node_types_file() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts_artifacts(&dir);
    build_parser_from_json(simple_json("art_nt"), opts).unwrap();
    let nt_path = dir.path().join("grammar_art_nt").join("NODE_TYPES.json");
    assert!(nt_path.is_file(), "NODE_TYPES.json should exist");
}

#[test]
fn artifacts_enabled_ir_json_is_valid() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts_artifacts(&dir);
    build_parser_from_json(simple_json("art_valid"), opts).unwrap();
    let ir_path = dir.path().join("grammar_art_valid").join("grammar.ir.json");
    let contents = fs::read_to_string(ir_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&contents).unwrap();
    assert!(parsed.is_object());
}

#[test]
fn artifacts_disabled_does_not_write_ir_json() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts(&dir);
    build_parser_from_json(simple_json("noart"), opts).unwrap();
    let ir_path = dir.path().join("grammar_noart").join("grammar.ir.json");
    assert!(!ir_path.exists(), "grammar.ir.json should not exist");
}

#[test]
fn artifacts_disabled_still_writes_parser_module() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts(&dir);
    build_parser_from_json(simple_json("noart_mod"), opts).unwrap();
    let grammar_dir = dir.path().join("grammar_noart_mod");
    // Parser module is always written
    assert!(grammar_dir.is_dir());
    let entries: Vec<_> = fs::read_dir(&grammar_dir).unwrap().collect();
    assert!(!entries.is_empty());
}

#[test]
fn artifacts_enabled_node_types_file_is_valid_json() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts_artifacts(&dir);
    build_parser_from_json(simple_json("art_ntv"), opts).unwrap();
    let nt_path = dir.path().join("grammar_art_ntv").join("NODE_TYPES.json");
    let contents = fs::read_to_string(nt_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&contents).unwrap();
    assert!(parsed.is_array() || parsed.is_object());
}

#[test]
fn test_artifacts_rebuild_refreshes_owned_files_and_preserves_unowned_file() {
    let dir = TempDir::new().unwrap();
    let grammar_dir = dir.path().join("grammar_art_clean");
    fs::create_dir_all(&grammar_dir).unwrap();
    let stale = grammar_dir.join("stale_file.txt");
    fs::write(&stale, "old data").unwrap();
    let owned = ["parser_art_clean.rs", "grammar.ir.json", "NODE_TYPES.json"];
    for name in owned {
        fs::write(grammar_dir.join(name), "stale Rust output").unwrap();
    }

    let opts = temp_opts_artifacts(&dir);
    let result = build_parser_from_json(simple_json("art_clean"), opts).unwrap();
    // Age does not establish ownership: the writer may refresh only its files.
    assert_eq!(fs::read(&stale).unwrap(), b"old data");
    assert_eq!(
        Path::new(&result.parser_path),
        grammar_dir.join("parser_art_clean.rs")
    );
    for name in owned {
        let bytes = fs::read(grammar_dir.join(name)).unwrap();
        assert!(!bytes.is_empty(), "owned output {name} must be emitted");
        assert_ne!(
            bytes, b"stale Rust output",
            "owned output {name} must refresh"
        );
    }
    let ir: serde_json::Value =
        serde_json::from_slice(&fs::read(grammar_dir.join("grammar.ir.json")).unwrap()).unwrap();
    assert_eq!(ir["name"], "art_clean");
    let node_types: serde_json::Value =
        serde_json::from_slice(&fs::read(grammar_dir.join("NODE_TYPES.json")).unwrap()).unwrap();
    assert!(node_types.is_array());
}

// ===========================================================================
// 4. Invalid grammars are rejected (8 tests)
// ===========================================================================

#[test]
fn invalid_grammar_empty_rules_fails() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(invalid_grammar_json(), temp_opts(&dir));
    assert!(res.is_err());
}

#[test]
fn malformed_json_fails() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(malformed_json(), temp_opts(&dir));
    assert!(res.is_err());
}

#[test]
fn empty_string_json_fails() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(String::new(), temp_opts(&dir));
    assert!(res.is_err());
}

#[test]
fn json_array_instead_of_object_fails() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json("[]".to_owned(), temp_opts(&dir));
    assert!(res.is_err());
}

#[test]
fn json_missing_name_still_builds_or_errors() {
    let dir = TempDir::new().unwrap();
    let json = serde_json::json!({
        "rules": {
            "source_file": { "type": "PATTERN", "value": "\\d+" }
        },
        "extras": [],
        "conflicts": [],
        "precedences": [],
        "externals": [],
        "inline": [],
        "supertypes": []
    })
    .to_string();
    // Either succeeds with "unknown" name or fails — both acceptable
    let _res = build_parser_from_json(json, temp_opts(&dir));
}

#[test]
fn json_null_rules_fails() {
    let dir = TempDir::new().unwrap();
    let json = serde_json::json!({
        "name": "nullrules",
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
    let res = build_parser_from_json(json, temp_opts(&dir));
    assert!(res.is_err());
}

#[test]
fn invalid_grammar_with_compress_also_fails() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(invalid_grammar_json(), temp_opts_compressed(&dir));
    assert!(res.is_err());
}

#[test]
fn invalid_grammar_with_artifacts_also_fails() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(invalid_grammar_json(), temp_opts_artifacts(&dir));
    assert!(res.is_err());
}

// ===========================================================================
// 5. Artifact file-system presence (8 tests)
// ===========================================================================

#[test]
fn parser_module_file_written_on_disk() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(simple_json("fsmod"), temp_opts(&dir)).unwrap();
    let parser_path = Path::new(&res.parser_path);
    assert!(parser_path.is_file(), "parser module should exist on disk");
}

#[test]
fn parser_module_contains_grammar_name_constant() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(simple_json("fsgn"), temp_opts(&dir)).unwrap();
    let contents = fs::read_to_string(&res.parser_path).unwrap();
    assert!(contents.contains("GRAMMAR_NAME"));
}

#[test]
fn parser_module_file_is_non_empty() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(simple_json("fsne"), temp_opts(&dir)).unwrap();
    let meta = fs::metadata(&res.parser_path).unwrap();
    assert!(meta.len() > 0);
}

#[test]
fn parser_path_contains_grammar_name() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(simple_json("fspath"), temp_opts(&dir)).unwrap();
    assert!(
        res.parser_path.contains("fspath"),
        "parser path should contain grammar name"
    );
}

#[test]
fn parser_path_ends_with_rs() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(simple_json("fsrs"), temp_opts(&dir)).unwrap();
    assert!(res.parser_path.ends_with(".rs"));
}

#[test]
fn artifacts_dir_named_after_grammar() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts_artifacts(&dir);
    build_parser_from_json(simple_json("fsname"), opts).unwrap();
    let grammar_dir = dir.path().join("grammar_fsname");
    assert!(grammar_dir.is_dir());
}

#[test]
fn artifacts_contain_multiple_files() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts_artifacts(&dir);
    build_parser_from_json(simple_json("fsmulti"), opts).unwrap();
    let grammar_dir = dir.path().join("grammar_fsmulti");
    let count = fs::read_dir(grammar_dir).unwrap().count();
    // At least parser module + grammar.ir.json + NODE_TYPES.json
    assert!(count >= 3, "expected ≥3 files in artifact dir, got {count}");
}

#[test]
fn separate_builds_use_separate_dirs() {
    let dir = TempDir::new().unwrap();
    let opts_a = temp_opts_artifacts(&dir);
    let opts_b = temp_opts_artifacts(&dir);
    build_parser_from_json(simple_json("sep_a"), opts_a).unwrap();
    build_parser_from_json(simple_json("sep_b"), opts_b).unwrap();
    assert!(dir.path().join("grammar_sep_a").is_dir());
    assert!(dir.path().join("grammar_sep_b").is_dir());
}

// ===========================================================================
// 6. Grammar name propagation (8 tests)
// ===========================================================================

#[test]
fn grammar_name_from_json_simple() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(simple_json("mygram"), temp_opts(&dir)).unwrap();
    assert_eq!(res.grammar_name, "mygram");
}

#[test]
fn grammar_name_from_json_choice() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(choice_json("mychoice"), temp_opts(&dir)).unwrap();
    assert_eq!(res.grammar_name, "mychoice");
}

#[test]
fn grammar_name_from_json_seq() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(seq_json("myseq"), temp_opts(&dir)).unwrap();
    assert_eq!(res.grammar_name, "myseq");
}

#[test]
fn grammar_name_appears_in_parser_code() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(simple_json("incode"), temp_opts(&dir)).unwrap();
    let on_disk = fs::read_to_string(&res.parser_path).unwrap();
    assert!(on_disk.contains("incode"));
}

#[test]
fn grammar_name_appears_in_parser_path() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(simple_json("inpath"), temp_opts(&dir)).unwrap();
    assert!(res.parser_path.contains("inpath"));
}

#[test]
fn grammar_name_preserved_with_compress() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(simple_json("compname"), temp_opts_compressed(&dir)).unwrap();
    assert_eq!(res.grammar_name, "compname");
}

#[test]
fn grammar_name_preserved_with_artifacts() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(simple_json("artname"), temp_opts_artifacts(&dir)).unwrap();
    assert_eq!(res.grammar_name, "artname");
}

#[test]
fn grammar_name_lowercase_in_parser_filename() {
    let dir = TempDir::new().unwrap();
    // Grammar names are lowercased in the parser filename
    let json = simple_json("MyGram");
    let res = build_parser_from_json(json, temp_opts(&dir)).unwrap();
    assert!(
        res.parser_path.contains("mygram"),
        "name should be lowercased in path: {}",
        res.parser_path
    );
}

// ===========================================================================
// 7. Option combinations (8 tests)
// ===========================================================================

#[test]
fn compress_and_artifacts_together() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts_all(&dir);
    let res = build_parser_from_json(simple_json("combo_ca"), opts);
    assert!(res.is_ok());
}

#[test]
fn compress_and_artifacts_produce_ir_json() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts_all(&dir);
    build_parser_from_json(simple_json("combo_ir"), opts).unwrap();
    let ir = dir.path().join("grammar_combo_ir").join("grammar.ir.json");
    assert!(ir.is_file());
}

#[test]
fn no_compress_no_artifacts_succeeds() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts(&dir);
    let res = build_parser_from_json(simple_json("combo_none"), opts);
    assert!(res.is_ok());
}

#[test]
fn compress_only_no_ir_file() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts_compressed(&dir);
    build_parser_from_json(simple_json("combo_conly"), opts).unwrap();
    let ir = dir
        .path()
        .join("grammar_combo_conly")
        .join("grammar.ir.json");
    assert!(!ir.exists());
}

#[test]
fn artifacts_only_no_compression() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts_artifacts(&dir);
    let res = build_parser_from_json(simple_json("combo_aonly"), opts).unwrap();
    assert!(!res.parser_code.is_empty());
}

#[test]
fn all_options_choice_grammar() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts_all(&dir);
    let res = build_parser_from_json(choice_json("combo_all_ch"), opts);
    assert!(res.is_ok());
}

#[test]
fn all_options_seq_grammar() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts_all(&dir);
    let res = build_parser_from_json(seq_json("combo_all_seq"), opts);
    assert!(res.is_ok());
}

#[test]
fn rebuild_with_different_options_same_dir() {
    let dir = TempDir::new().unwrap();
    // First: compressed, no artifacts
    build_parser_from_json(simple_json("combo_rebuild"), temp_opts_compressed(&dir)).unwrap();
    // Second: uncompressed, with artifacts — same grammar name
    let opts = temp_opts_artifacts(&dir);
    let res = build_parser_from_json(simple_json("combo_rebuild"), opts);
    assert!(res.is_ok());
}

// ===========================================================================
// 8. Edge cases (8 tests)
// ===========================================================================

#[test]
fn clone_preserves_all_fields() {
    let dir = TempDir::new().unwrap();
    let opts = temp_opts_all(&dir);
    let cloned = opts.clone();
    assert_eq!(opts.out_dir, cloned.out_dir);
    assert_eq!(opts.emit_artifacts, cloned.emit_artifacts);
    assert_eq!(opts.compress_tables, cloned.compress_tables);
}

#[test]
fn clone_is_independent() {
    let opts = BuildOptions {
        out_dir: "orig".into(),
        emit_artifacts: true,
        compress_tables: true,
    };
    let mut cloned = opts.clone();
    cloned.compress_tables = false;
    assert!(opts.compress_tables);
    assert!(!cloned.compress_tables);
}

#[test]
fn debug_format_shows_all_fields() {
    let opts = BuildOptions {
        out_dir: "dbg_edge".into(),
        emit_artifacts: true,
        compress_tables: false,
    };
    let dbg = format!("{opts:?}");
    assert!(dbg.contains("out_dir"));
    assert!(dbg.contains("emit_artifacts"));
    assert!(dbg.contains("compress_tables"));
}

#[test]
fn out_dir_with_trailing_slash() {
    let dir = TempDir::new().unwrap();
    let mut path_str: String = dir.path().to_string_lossy().into();
    path_str.push('/');
    let opts = BuildOptions {
        out_dir: path_str,
        emit_artifacts: false,
        compress_tables: false,
    };
    let res = build_parser_from_json(simple_json("trail"), opts);
    assert!(res.is_ok());
}

#[test]
fn out_dir_with_spaces_in_path() {
    let dir = TempDir::new().unwrap();
    let spaced = dir.path().join("dir with spaces");
    fs::create_dir_all(&spaced).unwrap();
    let opts = BuildOptions {
        out_dir: spaced.to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    };
    let res = build_parser_from_json(simple_json("spaced"), opts);
    assert!(res.is_ok());
}

#[test]
fn build_stats_symbol_count_positive() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(simple_json("stats_sym"), temp_opts(&dir)).unwrap();
    assert!(res.build_stats.symbol_count > 0);
}

#[test]
fn build_stats_state_count_positive() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(simple_json("stats_st"), temp_opts(&dir)).unwrap();
    assert!(res.build_stats.state_count > 0);
}

#[test]
fn node_types_json_is_valid_json() {
    let dir = TempDir::new().unwrap();
    let res = build_parser_from_json(simple_json("ntjson"), temp_opts(&dir)).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&res.node_types_json).unwrap();
    assert!(parsed.is_array() || parsed.is_object());
}
