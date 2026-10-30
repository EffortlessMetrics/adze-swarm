//! Public-builder regression tests for #1077.

use adze_tool::pure_rust_builder::{BuildOptions, build_parser_from_json};

fn grammar_json(n: usize) -> String {
    serde_json::json!({
        "name": "long_rhs_1077",
        "rules": {
            "source_file": {
                "type": "SEQ",
                "members": vec![serde_json::json!({"type": "STRING", "value": "a"}); n]
            }
        }
    })
    .to_string()
}

fn opts(dir: &std::path::Path, compress: bool) -> BuildOptions {
    BuildOptions {
        out_dir: dir.to_string_lossy().to_string(),
        emit_artifacts: true,
        compress_tables: compress,
    }
}

#[test]
fn test_public_builder_rhs_256_rejected_before_artifacts_both_table_modes() {
    for compress in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let result = build_parser_from_json(grammar_json(256), opts(dir.path(), compress));
        let err = result.expect_err("256 RHS must be rejected, not wrapped to 0");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("256"),
            "error must name observed length: {msg}"
        );
        assert!(msg.contains("255"), "error must name ABI limit: {msg}");
        assert!(
            !dir.path().join("grammar_long_rhs_1077").exists(),
            "no parser artifacts may be committed on failure (compress={compress})"
        );
    }
}

#[test]
fn test_public_builder_rhs_255_at_limit_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let result = build_parser_from_json(grammar_json(255), opts(dir.path(), true))
        .expect("255 is at the ABI limit and must build");
    assert!(
        result.parser_code.contains("255"),
        "generated code must carry the exact 255 child count"
    );
}
