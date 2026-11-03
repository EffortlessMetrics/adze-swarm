#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Comprehensive tests for ADZE_EMIT_ARTIFACTS behavior in adze-tool.
//!
//! When `emit_artifacts` is true in `BuildOptions`, the pure-Rust builder
//! writes debug artifacts (grammar IR JSON, NODE_TYPES.json, .parsetable,
//! parser module) into `grammar_<name>/` under `out_dir`.
//! When false, only the parser module is written (in a temp or minimal dir).

use std::fs;
use std::path::Path;

use adze_tool::pure_rust_builder::{
    BuildOptions, BuildResult, build_parser_from_grammar_js, build_parser_from_json,
};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create `BuildOptions` pointing at the given temp dir with artifacts **disabled**.
fn opts_no_artifacts(dir: &TempDir) -> BuildOptions {
    BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    }
}

/// Create `BuildOptions` pointing at the given temp dir with artifacts **enabled**.
fn opts_with_artifacts(dir: &TempDir) -> BuildOptions {
    BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: true,
        compress_tables: false,
    }
}

/// A minimal grammar.js that defines a single grammar called "simple".
fn simple_grammar_js() -> &'static str {
    r#"
module.exports = grammar({
  name: 'simple',
  rules: {
    source_file: $ => $.expression,
    expression: $ => /\d+/
  }
});
"#
}

/// A second minimal grammar.js with a different name.
fn alpha_grammar_js() -> &'static str {
    r#"
module.exports = grammar({
  name: 'alpha',
  rules: {
    program: $ => $.item,
    item: $ => /[a-z]+/
  }
});
"#
}

/// A grammar.js with a more complex structure (choices).
fn complex_grammar_js() -> &'static str {
    r#"
module.exports = grammar({
  name: 'complex',
  rules: {
    source_file: $ => repeat($.statement),
    statement: $ => choice($.number, $.word),
    number: $ => /\d+/,
    word: $ => /[a-z]+/
  }
});
"#
}

/// Write a grammar.js to a temp dir and build with the given options.
fn build_js(js: &str, opts: BuildOptions) -> BuildResult {
    let src_dir = TempDir::new().unwrap();
    let path = src_dir.path().join("grammar.js");
    fs::write(&path, js).unwrap();
    build_parser_from_grammar_js(&path, opts).unwrap()
}

/// Return the grammar directory path for a given grammar name inside out_dir.
fn grammar_dir(out_dir: &Path, name: &str) -> std::path::PathBuf {
    out_dir.join(format!("grammar_{}", name))
}

// =========================================================================
// 1. Default behavior — no artifacts emitted
// =========================================================================

#[test]
fn no_artifacts_by_default_no_grammar_ir() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), opts_no_artifacts(&dir));
    let ir_path = grammar_dir(dir.path(), "simple").join("grammar.ir.json");
    assert!(
        !ir_path.exists(),
        "grammar.ir.json should NOT be written when emit_artifacts is false"
    );
}

#[test]
fn no_artifacts_by_default_no_node_types() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), opts_no_artifacts(&dir));
    let nt_path = grammar_dir(dir.path(), "simple").join("NODE_TYPES.json");
    assert!(
        !nt_path.exists(),
        "NODE_TYPES.json should NOT be written when emit_artifacts is false"
    );
}

#[test]
fn no_artifacts_parser_module_still_written() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), opts_no_artifacts(&dir));
    let parser_path = Path::new(&result.parser_path);
    assert!(
        parser_path.exists(),
        "Parser module should still be written even without artifacts"
    );
}

#[test]
fn no_artifacts_grammar_dir_exists_for_parser() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), opts_no_artifacts(&dir));
    // The grammar dir is created for the parser module regardless
    let gdir = grammar_dir(dir.path(), "simple");
    assert!(
        gdir.exists(),
        "Grammar directory should exist for parser module output"
    );
}

// =========================================================================
// 2. Artifact file creation when enabled
// =========================================================================

#[test]
fn artifacts_enabled_creates_grammar_ir_json() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let ir_path = grammar_dir(dir.path(), "simple").join("grammar.ir.json");
    assert!(ir_path.exists(), "grammar.ir.json should be created");
}

#[test]
fn artifacts_enabled_creates_node_types_json() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let nt_path = grammar_dir(dir.path(), "simple").join("NODE_TYPES.json");
    assert!(nt_path.exists(), "NODE_TYPES.json should be created");
}

#[test]
fn artifacts_enabled_creates_parser_module() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let parser_path = Path::new(&result.parser_path);
    assert!(parser_path.exists(), "Parser module should be created");
}

#[test]
fn artifacts_enabled_creates_grammar_directory() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let gdir = grammar_dir(dir.path(), "simple");
    assert!(gdir.is_dir(), "grammar_simple directory should be created");
}

// =========================================================================
// 3. Artifact file content — grammar JSON / IR
// =========================================================================

#[test]
fn grammar_ir_json_is_valid_json() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let ir_path = grammar_dir(dir.path(), "simple").join("grammar.ir.json");
    let content = fs::read_to_string(&ir_path).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&content).expect("grammar.ir.json should be valid JSON");
    assert!(parsed.is_object(), "grammar IR should be a JSON object");
}

#[test]
fn grammar_ir_json_contains_grammar_name() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let ir_path = grammar_dir(dir.path(), "simple").join("grammar.ir.json");
    let content = fs::read_to_string(&ir_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    let name = parsed.get("name").and_then(|v| v.as_str());
    assert_eq!(
        name,
        Some("simple"),
        "Grammar IR should contain the grammar name"
    );
}

#[test]
fn node_types_json_is_valid_json_array() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let nt_path = grammar_dir(dir.path(), "simple").join("NODE_TYPES.json");
    let content = fs::read_to_string(&nt_path).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&content).expect("NODE_TYPES.json should be valid JSON");
    assert!(parsed.is_array(), "NODE_TYPES should be a JSON array");
}

#[test]
fn node_types_json_has_entries() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let nt_path = grammar_dir(dir.path(), "simple").join("NODE_TYPES.json");
    let content = fs::read_to_string(&nt_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    let arr = parsed.as_array().unwrap();
    assert!(!arr.is_empty(), "NODE_TYPES should have at least one entry");
}

#[test]
fn grammar_ir_json_contains_rules() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(complex_grammar_js(), opts_with_artifacts(&dir));
    let ir_path = grammar_dir(dir.path(), "complex").join("grammar.ir.json");
    let content = fs::read_to_string(&ir_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    // The IR should contain rule definitions
    assert!(
        parsed.get("rules").is_some() || parsed.get("rule_names").is_some(),
        "Grammar IR should contain rules or rule_names"
    );
}

#[test]
fn grammar_ir_json_is_pretty_printed() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let ir_path = grammar_dir(dir.path(), "simple").join("grammar.ir.json");
    let content = fs::read_to_string(&ir_path).unwrap();
    // Pretty-printed JSON contains newlines and indentation
    assert!(
        content.contains('\n'),
        "grammar.ir.json should be pretty-printed with newlines"
    );
}

// =========================================================================
// 4. Artifact file naming convention
// =========================================================================

#[test]
fn grammar_dir_follows_naming_convention() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let expected = dir.path().join("grammar_simple");
    assert!(
        expected.is_dir(),
        "Directory should be named grammar_<name>"
    );
}

#[test]
fn grammar_dir_naming_for_different_name() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(alpha_grammar_js(), opts_with_artifacts(&dir));
    let expected = dir.path().join("grammar_alpha");
    assert!(expected.is_dir(), "Directory should be named grammar_alpha");
}

#[test]
fn ir_json_artifact_name_is_grammar_ir_json() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let gdir = grammar_dir(dir.path(), "simple");
    let files: Vec<String> = fs::read_dir(&gdir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        files.contains(&"grammar.ir.json".to_string()),
        "Should contain grammar.ir.json, got: {:?}",
        files
    );
}

#[test]
fn node_types_artifact_name_is_correct() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let gdir = grammar_dir(dir.path(), "simple");
    let files: Vec<String> = fs::read_dir(&gdir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        files.contains(&"NODE_TYPES.json".to_string()),
        "Should contain NODE_TYPES.json, got: {:?}",
        files
    );
}

// =========================================================================
// 5. Multiple grammars → multiple artifacts
// =========================================================================

#[test]
fn two_grammars_create_two_artifact_dirs() {
    let dir = TempDir::new().unwrap();
    let _r1 = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let _r2 = build_js(alpha_grammar_js(), opts_with_artifacts(&dir));
    assert!(grammar_dir(dir.path(), "simple").is_dir());
    assert!(grammar_dir(dir.path(), "alpha").is_dir());
}

#[test]
fn two_grammars_each_have_ir_json() {
    let dir = TempDir::new().unwrap();
    let _r1 = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let _r2 = build_js(alpha_grammar_js(), opts_with_artifacts(&dir));
    assert!(
        grammar_dir(dir.path(), "simple")
            .join("grammar.ir.json")
            .exists()
    );
    assert!(
        grammar_dir(dir.path(), "alpha")
            .join("grammar.ir.json")
            .exists()
    );
}

#[test]
fn two_grammars_each_have_node_types() {
    let dir = TempDir::new().unwrap();
    let _r1 = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let _r2 = build_js(alpha_grammar_js(), opts_with_artifacts(&dir));
    assert!(
        grammar_dir(dir.path(), "simple")
            .join("NODE_TYPES.json")
            .exists()
    );
    assert!(
        grammar_dir(dir.path(), "alpha")
            .join("NODE_TYPES.json")
            .exists()
    );
}

#[test]
fn two_grammars_ir_contents_differ() {
    let dir = TempDir::new().unwrap();
    let _r1 = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let _r2 = build_js(alpha_grammar_js(), opts_with_artifacts(&dir));
    let ir1 =
        fs::read_to_string(grammar_dir(dir.path(), "simple").join("grammar.ir.json")).unwrap();
    let ir2 = fs::read_to_string(grammar_dir(dir.path(), "alpha").join("grammar.ir.json")).unwrap();
    assert_ne!(
        ir1, ir2,
        "Different grammars should produce different IR JSON"
    );
}

// =========================================================================
// 6. Artifact directory structure
// =========================================================================

#[test]
fn artifact_dir_is_under_out_dir() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let gdir = grammar_dir(dir.path(), "simple");
    assert!(
        gdir.starts_with(dir.path()),
        "Grammar directory should be under out_dir"
    );
}

#[test]
fn artifact_dir_contains_only_expected_files() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let gdir = grammar_dir(dir.path(), "simple");
    let files: Vec<String> = fs::read_dir(&gdir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    // Expected files: grammar.ir.json, NODE_TYPES.json, parser module, possibly .parsetable
    for f in &files {
        assert!(
            f.ends_with(".json")
                || f.ends_with(".rs")
                || f.ends_with(".parsetable")
                || f.ends_with(".ir.json"),
            "Unexpected file in artifact dir: {}",
            f
        );
    }
}

#[test]
fn artifact_dir_has_no_nested_subdirectories() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let gdir = grammar_dir(dir.path(), "simple");
    let subdirs: Vec<_> = fs::read_dir(&gdir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();
    assert!(
        subdirs.is_empty(),
        "Artifact directory should be flat (no subdirectories), found: {:?}",
        subdirs.iter().map(|d| d.file_name()).collect::<Vec<_>>()
    );
}

#[test]
fn out_dir_only_contains_grammar_prefixed_dirs() {
    let dir = TempDir::new().unwrap();
    let _r1 = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let _r2 = build_js(alpha_grammar_js(), opts_with_artifacts(&dir));
    let entries: Vec<String> = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    for entry in &entries {
        assert!(
            entry.starts_with("grammar_"),
            "Directory '{}' should start with 'grammar_'",
            entry
        );
    }
}

// =========================================================================
// 7. Artifact cleanup behavior
// =========================================================================

#[test]
fn rebuild_refreshes_owned_artifacts_and_preserves_other_files() {
    let dir = TempDir::new().unwrap();
    // Keep the original first-build and unowned-file counterexample.
    let _r1 = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let gdir = grammar_dir(dir.path(), "simple");
    let stale = gdir.join("stale_artifact.txt");
    fs::write(&stale, "old data").unwrap();
    assert!(stale.exists());

    // A shared directory can also contain C output and authored scanner files.
    fs::create_dir_all(gdir.join("tree_sitter")).unwrap();
    let retained = [
        ("parser.c", "independent C parser"),
        ("tree_sitter/parser.h", "independent C header"),
        ("scanner.c", "authored scanner"),
    ];
    for (name, bytes) in retained {
        fs::write(gdir.join(name), bytes).unwrap();
    }
    let owned = ["parser_simple.rs", "grammar.ir.json", "NODE_TYPES.json"];
    for name in owned {
        assert!(gdir.join(name).is_file(), "first build must emit {name}");
        fs::write(gdir.join(name), "stale Rust output").unwrap();
    }

    // Rebuilding must refresh this writer's files without deleting other owners.
    let result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    assert_eq!(fs::read(&stale).unwrap(), b"old data");
    for (name, bytes) in retained {
        assert_eq!(fs::read(gdir.join(name)).unwrap(), bytes.as_bytes());
    }
    assert_eq!(Path::new(&result.parser_path), gdir.join("parser_simple.rs"));
    for name in owned {
        let bytes = fs::read(gdir.join(name)).unwrap();
        assert!(!bytes.is_empty(), "owned output {name} must be emitted");
        assert_ne!(
            bytes, b"stale Rust output",
            "owned output {name} must refresh"
        );
    }
    let ir: serde_json::Value =
        serde_json::from_slice(&fs::read(gdir.join("grammar.ir.json")).unwrap()).unwrap();
    assert_eq!(ir["name"], "simple");
    let node_types: serde_json::Value =
        serde_json::from_slice(&fs::read(gdir.join("NODE_TYPES.json")).unwrap()).unwrap();
    assert!(node_types.is_array());
}

#[test]
fn rebuild_recreates_ir_json() {
    let dir = TempDir::new().unwrap();
    // First build
    let _r1 = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let ir_path = grammar_dir(dir.path(), "simple").join("grammar.ir.json");
    assert!(ir_path.exists(), "IR should exist after first build");

    // Second build should recreate the file (cleanup + recreate)
    let _r2 = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    assert!(ir_path.exists(), "IR should exist after rebuild");

    // Both should be valid JSON with the same grammar name
    let content = fs::read_to_string(&ir_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(
        parsed.get("name").and_then(|v| v.as_str()),
        Some("simple"),
        "Rebuilt IR should still have the correct grammar name"
    );
}

#[test]
fn rebuild_does_not_leave_extra_files() {
    let dir = TempDir::new().unwrap();
    let _r1 = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let gdir = grammar_dir(dir.path(), "simple");
    let files_before: Vec<String> = fs::read_dir(&gdir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    let _r2 = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let files_after: Vec<String> = fs::read_dir(&gdir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(
        files_before.len(),
        files_after.len(),
        "Rebuild should not leave extra files"
    );
}

#[test]
fn cleanup_only_affects_matching_grammar_dir() {
    let dir = TempDir::new().unwrap();
    // Build both grammars
    let _r1 = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let _r2 = build_js(alpha_grammar_js(), opts_with_artifacts(&dir));

    // Plant a marker in alpha
    let marker = grammar_dir(dir.path(), "alpha").join("marker.txt");
    fs::write(&marker, "keep me").unwrap();

    // Rebuilding simple should not touch alpha
    let _r3 = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    assert!(
        marker.exists(),
        "Rebuilding 'simple' should not remove files from 'alpha' directory"
    );
}

// =========================================================================
// 8. build_parser_from_json artifact behavior
// =========================================================================

#[test]
fn json_build_creates_artifacts_when_enabled() {
    let dir = TempDir::new().unwrap();
    let grammar_json = serde_json::json!({
        "name": "json_test",
        "rules": {
            "source_file": {"type": "SYMBOL", "name": "value"},
            "value": {"type": "PATTERN", "value": "\\d+"}
        }
    });
    let opts = opts_with_artifacts(&dir);
    let _result =
        build_parser_from_json(serde_json::to_string(&grammar_json).unwrap(), opts).unwrap();
    let gdir = grammar_dir(dir.path(), "json_test");
    assert!(gdir.join("grammar.ir.json").exists());
    assert!(gdir.join("NODE_TYPES.json").exists());
}

#[test]
fn json_build_no_artifacts_when_disabled() {
    let dir = TempDir::new().unwrap();
    let grammar_json = serde_json::json!({
        "name": "json_test2",
        "rules": {
            "source_file": {"type": "SYMBOL", "name": "value"},
            "value": {"type": "PATTERN", "value": "\\d+"}
        }
    });
    let opts = opts_no_artifacts(&dir);
    let _result =
        build_parser_from_json(serde_json::to_string(&grammar_json).unwrap(), opts).unwrap();
    let ir_path = grammar_dir(dir.path(), "json_test2").join("grammar.ir.json");
    assert!(
        !ir_path.exists(),
        "IR artifact should not be created when disabled"
    );
}

// =========================================================================
// 9. BuildResult consistency with artifacts
// =========================================================================

#[test]
fn build_result_grammar_name_matches_artifacts() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    assert_eq!(result.grammar_name, "simple");
    let ir_path = grammar_dir(dir.path(), &result.grammar_name).join("grammar.ir.json");
    assert!(ir_path.exists());
}

#[test]
fn build_result_parser_path_inside_grammar_dir() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let parser_path = Path::new(&result.parser_path);
    let gdir = grammar_dir(dir.path(), "simple");
    assert!(
        parser_path.starts_with(&gdir),
        "Parser path {} should be inside grammar dir {}",
        parser_path.display(),
        gdir.display()
    );
}

#[test]
fn build_result_node_types_matches_file() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), opts_with_artifacts(&dir));
    let nt_path = grammar_dir(dir.path(), "simple").join("NODE_TYPES.json");
    let file_content = fs::read_to_string(&nt_path).unwrap();
    // Both should parse to the same JSON value
    let from_result: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    let from_file: serde_json::Value = serde_json::from_str(&file_content).unwrap();
    assert_eq!(
        from_result, from_file,
        "NODE_TYPES from result and file should match"
    );
}
