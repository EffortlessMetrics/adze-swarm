//! Generated parser artifact materialization.

use std::{io::Write, path::PathBuf};

use serde_json::Value;

use super::BuildParserOptions;

pub(crate) struct GrammarArtifacts {
    pub(crate) dir: PathBuf,
    _tempdir: Option<tempfile::TempDir>,
}

impl GrammarArtifacts {
    pub(crate) fn create(
        grammar_name: &str,
        grammar: &Value,
        grammar_c: &str,
        options: &BuildParserOptions,
    ) -> Self {
        let tempfile = tempfile::Builder::new()
            .prefix("grammar")
            .tempdir()
            .unwrap();

        let (dir, _tempdir) = if options.emit_artifacts {
            let grammar_dir =
                PathBuf::from(options.out_dir.as_str()).join(format!("grammar_{grammar_name}"));
            if grammar_dir.is_dir() {
                std::fs::remove_dir_all(&grammar_dir).expect("Couldn't clear old artifacts");
            }
            std::fs::DirBuilder::new()
                .recursive(true)
                .create(grammar_dir.clone())
                .expect("Couldn't create grammar JSON directory");
            (grammar_dir, None)
        } else {
            (tempfile.path().into(), Some(tempfile))
        };

        write_parser_c(&dir, grammar_c);
        write_grammar_json(&dir, grammar_name, grammar);
        write_parser_header(&dir);

        Self { dir, _tempdir }
    }
}

pub(crate) fn dump_path(options: &BuildParserOptions) -> PathBuf {
    PathBuf::from(options.out_dir.as_str()).join("last_grammar.json")
}

fn write_parser_c(dir: &std::path::Path, grammar_c: &str) {
    let mut f = std::fs::File::create(dir.join("parser.c")).unwrap();
    f.write_all(grammar_c.as_bytes()).unwrap();
}

fn write_grammar_json(dir: &std::path::Path, grammar_name: &str, grammar: &Value) {
    let mut grammar_json_file =
        std::fs::File::create(dir.join(format!("{grammar_name}.json"))).unwrap();
    grammar_json_file
        .write_all(serde_json::to_string_pretty(grammar).unwrap().as_bytes())
        .unwrap();
}

fn write_parser_header(dir: &std::path::Path) {
    let header_dir = dir.join("tree_sitter");
    std::fs::create_dir(&header_dir).unwrap();
    let mut parser_file = std::fs::File::create(header_dir.join("parser.h")).unwrap();
    parser_file
        .write_all(tree_sitter::PARSER_HEADER.as_bytes())
        .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_c_artifacts_preserve_other_owners_and_refresh_idempotently() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("grammar_fixture");
        std::fs::create_dir_all(dir.join("tree_sitter")).unwrap();
        let retained = [
            ("parser_fixture.rs", "independent Rust parser"),
            ("grammar.ir.json", "independent Rust IR"),
            ("NODE_TYPES.json", "independent node types"),
            ("fixture.parsetable", "independent serialized table"),
            ("scanner.c", "independent scanner source"),
            ("tree_sitter/custom.h", "independent scanner header"),
        ];
        for (name, bytes) in retained {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
        let options = BuildParserOptions {
            out_dir: root.path().display().to_string(),
            emit_artifacts: true,
            enable_debug_file: false,
            use_pure_rust: false,
        };
        for c_source in ["first generated C", "updated generated C"] {
            let grammar = serde_json::json!({"name":"fixture", "source":c_source});
            let artifacts = GrammarArtifacts::create("fixture", &grammar, c_source, &options);
            assert_eq!(artifacts.dir, dir);
            for (name, bytes) in retained {
                assert_eq!(
                    std::fs::read_to_string(dir.join(name)).unwrap(),
                    bytes,
                    "{name}"
                );
            }
            assert_eq!(
                std::fs::read_to_string(dir.join("parser.c")).unwrap(),
                c_source
            );
            let emitted: Value =
                serde_json::from_str(&std::fs::read_to_string(dir.join("fixture.json")).unwrap())
                    .unwrap();
            assert_eq!(emitted, grammar);
            assert_eq!(
                std::fs::read(dir.join("tree_sitter/parser.h")).unwrap(),
                tree_sitter::PARSER_HEADER.as_bytes()
            );
        }
    }
}
