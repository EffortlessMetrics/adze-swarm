//! Debug-artifact producers share a directory without deleting other owners.
use adze_tool::pure_rust_builder::{BuildOptions, build_parser_from_json};

#[test]
fn test_rust_debug_artifacts_preserve_c_outputs_and_unowned_files() {
    let output = tempfile::tempdir().unwrap();
    let dir = output.path().join("grammar_artifact_fixture");
    std::fs::create_dir_all(dir.join("tree_sitter")).unwrap();
    let retained = [
        ("parser.c", "independent compiled C parser source"),
        ("artifact_fixture.json", "independent C grammar JSON"),
        ("tree_sitter/parser.h", "independent C parser header"),
        ("scanner.c", "independent scanner source"),
        ("unowned-sentinel", "retain unrelated authored content"),
    ];
    for (name, bytes) in retained {
        std::fs::write(dir.join(name), bytes).unwrap();
    }
    for text in ["hello", "goodbye"] {
        let grammar = serde_json::json!({"name":"artifact_fixture", "rules":{
            "source_file":{"type":"STRING", "value":text}
        }});
        let result = build_parser_from_json(
            grammar.to_string(),
            BuildOptions {
                out_dir: output.path().display().to_string(),
                emit_artifacts: true,
                ..BuildOptions::default()
            },
        )
        .unwrap();
        for (name, bytes) in retained {
            assert_eq!(
                std::fs::read_to_string(dir.join(name)).unwrap(),
                bytes,
                "{name}"
            );
        }
        assert_eq!(
            std::path::Path::new(&result.parser_path).parent().unwrap(),
            dir
        );
        assert!(!std::fs::read(result.parser_path).unwrap().is_empty());
        let ir: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("grammar.ir.json")).unwrap())
                .unwrap();
        assert_eq!(ir["name"], "artifact_fixture");
        let node_types: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("NODE_TYPES.json")).unwrap())
                .unwrap();
        assert!(node_types.is_array());
    }
}
