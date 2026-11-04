use adze_ir::{FieldId, Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use sha2::{Digest, Sha256};
use std::path::Path;

fn generated_sources(directory: &Path) -> Vec<serde_json::Value> {
    let mut pending = vec![directory.to_path_buf()];
    let mut sources = Vec::new();
    while let Some(path) = pending.pop() {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                sources.push(path);
            }
        }
    }
    sources.sort();
    sources
        .into_iter()
        .map(|path| {
            let bytes = std::fs::read(&path).unwrap();
            serde_json::json!({
                "path": path.strip_prefix(directory).unwrap().to_string_lossy(),
                "sha256": format!("{:x}", Sha256::digest(&bytes)),
            })
        })
        .collect()
}

fn main() {
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    println!("cargo::rustc-cfg=adze_unsafe_attrs");
    println!("cargo::rerun-if-changed=src/lib.rs");
    let source = Path::new("src/lib.rs");
    let annotated = adze_tool::generate_grammars(source).unwrap();
    adze_tool::build_parsers(&source.to_path_buf());

    let mut grammar = Grammar::new("occurrence_ir_repeat_baseline".into());
    grammar.start_symbol = Some(SymbolId(10));
    for (id, name, pattern) in [
        (1, "letter", TokenPattern::Regex("[a-z]".into())),
        (2, "open", TokenPattern::String("[".into())),
        (3, "close", TokenPattern::String("]".into())),
    ] {
        grammar.tokens.insert(
            SymbolId(id),
            Token {
                name: name.into(),
                pattern,
                fragile: false,
            },
        );
    }
    grammar
        .rule_names
        .insert(SymbolId(10), "source_file".into());
    grammar.rule_names.insert(SymbolId(11), "item".into());
    grammar.fields.insert(FieldId(1), "items".into());
    grammar.add_rule(Rule {
        lhs: SymbolId(10),
        rhs: vec![
            Symbol::Terminal(SymbolId(2)),
            Symbol::Repeat(Box::new(Symbol::NonTerminal(SymbolId(11)))),
            Symbol::Terminal(SymbolId(3)),
        ],
        fields: vec![(FieldId(1), 1)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
    });
    grammar.add_rule(Rule {
        lhs: SymbolId(11),
        rhs: vec![Symbol::Terminal(SymbolId(1))],
        fields: vec![],
        precedence: None,
        associativity: None,
        production_id: ProductionId(1),
    });
    let source_ir = serde_json::to_value(&grammar).unwrap();
    grammar.normalize();
    let normalized_ir = serde_json::to_value(&grammar).unwrap();
    let generated = adze_tool::pure_rust_builder::build_parser(
        grammar,
        adze_tool::pure_rust_builder::BuildOptions::default(),
    )
    .expect("generate the explicit IR repeat baseline");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(out.join("ir_repeat.rs"), generated.parser_code).unwrap();
    let origins = serde_json::json!({
        "annotated_source": annotated,
        "repeat_source_ir": source_ir,
        "repeat_normalized_ir": normalized_ir,
        "generated_rust": generated_sources(&out),
    });
    std::fs::write(
        out.join("origin-inputs.json"),
        serde_json::to_vec_pretty(&origins).unwrap(),
    )
    .unwrap();
}
