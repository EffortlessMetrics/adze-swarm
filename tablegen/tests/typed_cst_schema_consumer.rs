//! #1061 generator-side schema consumer: byte-identical output proof.
//!
//! `NodeTypesGenerator` and `TypedCstGenerator` derive field facts from the
//! canonical `field_schema` module. These snapshots pin their full output on
//! a fixture exercising optional, repeat, multi-target choice, hidden-token,
//! and mandatory-plus-optional-same-field shapes, both raw and normalized.
//!
//! The raw-grammar snapshots were recorded BEFORE the single-derivation
//! refactor (which removes the generators' locally re-derived cardinality
//! helpers) and pass unchanged after it, proving the refactor changed
//! derivation, not results. The normalized node-types snapshot is likewise
//! byte-identical. The normalized typed-CST snapshot records the one
//! schema-mandated alignment: pre-change, hidden-token wrapper retention was
//! re-derived by walking raw field symbols, so after normalization replaced
//! wrappers with helpers the `_sep` target lost its wrapper and accessor;
//! post-change, retention follows the canonical schema and the accessor is
//! restored, agreeing with the node-types `_sep` type on the same grammar.

use adze_ir::{FieldId, Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use adze_tablegen::{NodeTypesGenerator, TypedCstGenerator};

fn token(name: &str, pattern: TokenPattern) -> Token {
    Token {
        name: name.to_string(),
        pattern,
        fragile: false,
    }
}

fn term(id: u16) -> Symbol {
    Symbol::Terminal(SymbolId(id))
}

fn non_term(id: u16) -> Symbol {
    Symbol::NonTerminal(SymbolId(id))
}

/// Fixture covering every shape the canonical schema resolves:
/// multi-target choice (`item`), repetition (`list`), a hidden-token target
/// (`_sep`), and a mandatory plus optional occurrence of one field in a
/// single alternative (`pair`).
fn schema_fixture() -> Grammar {
    let mut grammar = Grammar::new("schema_consumer".to_string());
    grammar.tokens.insert(
        SymbolId(1),
        token("num", TokenPattern::Regex(r"\d+".to_string())),
    );
    grammar.tokens.insert(
        SymbolId(2),
        token("word", TokenPattern::Regex("[a-z]+".to_string())),
    );
    grammar.tokens.insert(
        SymbolId(3),
        token("_sep", TokenPattern::Regex(r"\s+".to_string())),
    );
    for (id, name) in [(10u16, "start"), (11, "item"), (12, "list"), (13, "pair")] {
        grammar.rule_names.insert(SymbolId(id), name.to_string());
    }
    grammar.fields.insert(FieldId(1), "value".to_string());
    grammar.fields.insert(FieldId(2), "items".to_string());
    let mut pid = 0u16;
    let mut add =
        |grammar: &mut Grammar, lhs: u16, rhs: Vec<Symbol>, fields: Vec<(FieldId, usize)>| {
            grammar.add_rule(Rule {
                lhs: SymbolId(lhs),
                rhs,
                precedence: None,
                associativity: None,
                fields,
                production_id: ProductionId(pid),
            });
            pid += 1;
        };
    // start -> item (field value on the single child).
    add(&mut grammar, 10, vec![non_term(11)], vec![(FieldId(1), 0)]);
    // item -> num | word (same-named field, different target kinds).
    add(&mut grammar, 11, vec![term(1)], vec![(FieldId(1), 0)]);
    add(&mut grammar, 11, vec![term(2)], vec![(FieldId(1), 0)]);
    // list -> word* (zero or more repeated children).
    add(
        &mut grammar,
        12,
        vec![Symbol::Repeat(Box::new(term(2)))],
        vec![(FieldId(2), 0)],
    );
    // pair -> item _sep? (mandatory plus optional occurrence of one field
    // in a single alternative; hidden-token target).
    add(
        &mut grammar,
        13,
        vec![non_term(11), Symbol::Optional(Box::new(term(3)))],
        vec![(FieldId(1), 0), (FieldId(1), 1)],
    );
    grammar
}

fn normalized_schema_fixture() -> Grammar {
    let mut grammar = schema_fixture();
    grammar.normalize();
    grammar
}

#[test]
fn test_node_types_schema_fixture_output() {
    let grammar = schema_fixture();
    let json = NodeTypesGenerator::new(&grammar)
        .generate()
        .expect("node types generate");
    insta::assert_snapshot!(json);
}

#[test]
fn test_typed_cst_schema_fixture_output() {
    let grammar = schema_fixture();
    let generated = TypedCstGenerator::new(&grammar).generate().to_string();
    insta::assert_snapshot!(generated);
}

#[test]
fn test_normalized_typed_cst_agrees_with_node_types_on_hidden_target() {
    // The schema names `_sep` as a target of `pair.value` on the normalized
    // grammar (see the node-types snapshot). The typed-CST output must cover
    // the same fact: a retained `SepToken` wrapper plus a per-target
    // accessor, not a first-type-only shortcut.
    let grammar = normalized_schema_fixture();
    let json = NodeTypesGenerator::new(&grammar)
        .generate()
        .expect("node types generate");
    assert!(
        json.contains("\"_sep\""),
        "node-types must admit the hidden target: {json}"
    );
    let generated = TypedCstGenerator::new(&grammar).generate().to_string();
    assert!(
        generated.contains("pub struct SepToken"),
        "hidden target wrapper must be retained"
    );
    assert!(
        generated.contains("pub fn value_sep"),
        "every admitted target kind gets an accessor"
    );
    assert!(
        generated.contains("pub fn value_item"),
        "the named target kind keeps its accessor"
    );
    assert_eq!(
        generated.matches("pub fn value (").count(),
        1,
        "the only bare `value` accessor is the single-target one on `Start`; \
         multi-target `pair.value` must split per kind"
    );
}

#[test]
fn test_node_types_normalized_agrees_with_raw_schema_fixture() {
    // Normalization must preserve canonical field facts: byte-identical JSON.
    let raw = NodeTypesGenerator::new(&schema_fixture())
        .generate()
        .expect("node types generate");
    let normalized = NodeTypesGenerator::new(&normalized_schema_fixture())
        .generate()
        .expect("node types generate");
    assert_eq!(normalized, raw, "normalization must preserve field facts");
}

#[test]
fn test_typed_cst_normalized_schema_fixture_output() {
    let grammar = normalized_schema_fixture();
    let generated = TypedCstGenerator::new(&grammar).generate().to_string();
    insta::assert_snapshot!(generated);
}

#[test]
fn test_normalized_hidden_target_downstream_accessor_matches_document() {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture =
        std::env::temp_dir().join(format!("adze-hidden-cst-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(fixture.join("src")).unwrap();
    let manifest = format!(
        r#"[package]
name = "adze-hidden-cst-consumer"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
adze = {{ path = {:?}, default-features = false, features = ["pure-rust"] }}
[build-dependencies]
adze-tool = {{ path = {:?} }}
adze-ir = {{ path = {:?} }}
"#,
        repository.join("runtime"),
        repository.join("tool"),
        repository.join("ir")
    );
    std::fs::write(fixture.join("Cargo.toml"), manifest).unwrap();
    // Seed the standalone consumer with the reviewed repository's versions.
    std::fs::copy(repository.join("Cargo.lock"), fixture.join("Cargo.lock")).unwrap();
    std::fs::write(
        fixture.join("build.rs"),
        include_str!("fixtures/typed_cst_hidden/build.rs"),
    )
    .unwrap();
    std::fs::write(
        fixture.join("src/lib.rs"),
        include_str!("fixtures/typed_cst_hidden/lib.rs"),
    )
    .unwrap();
    // A distinct target avoids nested Cargo locking the parent target.
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repository.join("target"))
        .join("typed-cst-hidden-consumer");
    let output = std::process::Command::new(env!("CARGO"))
        .args(["test", "--offline", "--manifest-path"])
        .arg(fixture.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", target)
        .output()
        .expect("run actual downstream generated accessor control");
    assert!(
        output.status.success(),
        "downstream fixture retained at {}\nstdout:\n{}\nstderr:\n{}",
        fixture.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("{stdout}");
    assert!(
        stdout.contains("6 passed; 0 failed; 0 ignored"),
        "all six selected downstream controls must execute: {stdout}"
    );
    std::fs::remove_dir_all(fixture).unwrap();
}

#[test]
fn test_ordinary_optional_downstream_source_owner_and_selected_value() {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture = std::env::temp_dir().join(format!(
        "adze-ordinary-optional-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(fixture.join("src")).unwrap();
    let manifest = format!(
        r#"[package]
name = "adze-ordinary-optional-consumer"
version = "0.0.0"
edition = "2024"
[workspace]
[features]
default = ["pure-rust"]
pure-rust = ["adze/pure-rust"]
[dependencies]
adze = {{ path = {:?}, default-features = false, features = ["pure-rust"] }}
[build-dependencies]
adze-tool = {{ path = {:?} }}
adze-ir = {{ path = {:?} }}
"#,
        repository.join("runtime"),
        repository.join("tool"),
        repository.join("ir")
    );
    std::fs::write(fixture.join("Cargo.toml"), manifest).unwrap();
    // Seed the standalone consumer with the reviewed repository's versions.
    std::fs::copy(repository.join("Cargo.lock"), fixture.join("Cargo.lock")).unwrap();
    std::fs::write(
        fixture.join("build.rs"),
        include_str!("fixtures/typed_cst_ordinary_optional/build.rs"),
    )
    .unwrap();
    std::fs::write(
        fixture.join("src/lib.rs"),
        include_str!("fixtures/typed_cst_ordinary_optional/lib.rs"),
    )
    .unwrap();
    // A distinct target avoids nested Cargo locking the parent target.
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repository.join("target"))
        .join("typed-cst-ordinary-optional-consumer");
    let output = std::process::Command::new(env!("CARGO"))
        .args(["test", "--offline", "--manifest-path"])
        .arg(fixture.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", target)
        .output()
        .expect("run actual downstream generated accessor control");
    assert!(
        output.status.success(),
        "downstream fixture retained at {}\nstdout:\n{}\nstderr:\n{}",
        fixture.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("{stdout}");
    assert!(
        stdout.contains("4 passed; 0 failed; 0 ignored"),
        "all four selected downstream controls must execute: {stdout}"
    );
    std::fs::remove_dir_all(fixture).unwrap();
}
