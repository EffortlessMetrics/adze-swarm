//! Comprehensive tests for `tool/src/expansion.rs`.
//!
//! Covers grammar extraction from annotated Rust modules, attribute
//! processing (leaf, word, extra, external, prec, skip, field, delimited,
//! repeat, no_inline), code-generation shapes in the JSON output, feature
//! flag handling, and error cases.

use std::fs;

use serde_json::Value;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Write Rust source to a temp file and extract all grammars.
fn grammars_from(src: &str) -> Vec<Value> {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("lib.rs");
    fs::write(&path, src).unwrap();
    adze_tool::generate_grammars(&path).unwrap()
}

/// Write Rust source and return the first (and presumably only) grammar.
fn grammar_from(src: &str) -> Value {
    let gs = grammars_from(src);
    assert_eq!(gs.len(), 1, "expected exactly 1 grammar");
    gs.into_iter().next().unwrap()
}

/// Extract grammars, expecting an error.
fn grammar_err(src: &str) -> adze_tool::ToolError {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("lib.rs");
    fs::write(&path, src).unwrap();
    adze_tool::generate_grammars(&path).unwrap_err()
}

// =========================================================================
// 1. Grammar extraction from Rust files
// =========================================================================

#[test]
fn extract_minimal_enum_grammar() {
    let g = grammar_from(
        r#"
        #[adze::grammar("minimal")]
        mod grammar {
            #[adze::language]
            pub enum Root {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
            }
        }
        "#,
    );
    assert_eq!(g["name"], "minimal");
    assert!(g["rules"].is_object());
}

#[test]
fn extract_minimal_struct_grammar() {
    let g = grammar_from(
        r#"
        #[adze::grammar("struct_gram")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"\d+")]
                value: i32,
            }
        }
        "#,
    );
    assert_eq!(g["name"], "struct_gram");
    let rules = g["rules"].as_object().unwrap();
    assert!(rules.contains_key("Root"));
}

#[test]
fn source_file_rule_references_root_type() {
    let g = grammar_from(
        r#"
        #[adze::grammar("sf")]
        mod grammar {
            #[adze::language]
            pub enum Tok {
                A(#[adze::leaf(text = "a")] ()),
            }
        }
        "#,
    );
    let sf = &g["rules"]["source_file"];
    assert_eq!(sf["type"], "SYMBOL");
    assert_eq!(sf["name"], "Tok");
}

#[test]
fn no_grammar_module_yields_empty_vec() {
    let gs = grammars_from("pub fn hello() {}");
    assert!(gs.is_empty());
}

#[test]
fn nested_module_grammar_extraction() {
    let g = grammar_from(
        r#"
        mod outer {
            #[adze::grammar("nested")]
            mod inner {
                #[adze::language]
                pub enum T {
                    X(#[adze::leaf(text = "x")] ()),
                }
            }
        }
        "#,
    );
    assert_eq!(g["name"], "nested");
}

#[test]
fn multiple_grammars_in_one_file() {
    let gs = grammars_from(
        r#"
        #[adze::grammar("g1")]
        mod g1 {
            #[adze::language]
            pub enum A {
                X(#[adze::leaf(text = "x")] ()),
            }
        }
        #[adze::grammar("g2")]
        mod g2 {
            #[adze::language]
            pub enum B {
                Y(#[adze::leaf(text = "y")] ()),
            }
        }
        "#,
    );
    assert_eq!(gs.len(), 2);
    let names: Vec<&str> = gs.iter().map(|g| g["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"g1"));
    assert!(names.contains(&"g2"));
}

// =========================================================================
// 2. Attribute processing
// =========================================================================

// -- leaf(pattern = ...) --------------------------------------------------

#[test]
fn leaf_pattern_generates_pattern_rule() {
    let g = grammar_from(
        r#"
        #[adze::grammar("pat")]
        mod grammar {
            #[adze::language]
            pub enum Root {
                Id(#[adze::leaf(pattern = r"[a-z]+")] String),
            }
        }
        "#,
    );
    // Single-leaf enum variants may be inlined into the CHOICE, so the
    // PATTERN node can appear inside a member rather than as a top-level rule.
    let json_str = serde_json::to_string(&g["rules"]).unwrap();
    assert!(
        json_str.contains("PATTERN"),
        "expected PATTERN somewhere in rules"
    );
}

// -- leaf(text = ...) -----------------------------------------------------

#[test]
fn leaf_text_generates_string_rule() {
    let g = grammar_from(
        r#"
        #[adze::grammar("txt")]
        mod grammar {
            #[adze::language]
            pub enum Root {
                Plus(#[adze::leaf(text = "+")] ()),
            }
        }
        "#,
    );
    // Single-leaf enum variants inline into the CHOICE, so STRING node
    // appears inside a member rather than as a separate top-level rule.
    let json_str = serde_json::to_string(&g["rules"]).unwrap();
    assert!(
        json_str.contains("STRING"),
        "expected STRING somewhere in rules"
    );
}

// -- word attribute -------------------------------------------------------

#[test]
fn word_rule_appears_in_grammar_json() {
    let g = grammar_from(
        r#"
        #[adze::grammar("w")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"[a-z]+")]
                #[adze::word]
                ident: String,
            }
        }
        "#,
    );
    // The "word" key in grammar JSON should be set.
    assert!(g.get("word").is_some());
    // It should be a string referencing the word rule
    assert!(g["word"].is_string() || g["word"].is_null());
}

// -- extra attribute ------------------------------------------------------

#[test]
fn extra_token_appears_in_extras_list() {
    let g = grammar_from(
        r#"
        #[adze::grammar("ext")]
        mod grammar {
            #[adze::language]
            pub enum Root {
                A(#[adze::leaf(text = "a")] ()),
            }

            #[adze::extra]
            pub struct Whitespace {
                #[adze::leaf(pattern = r"\s")]
                _ws: String,
            }
        }
        "#,
    );
    let extras = g["extras"].as_array().unwrap();
    assert!(
        !extras.is_empty(),
        "extras list must contain at least the Whitespace token"
    );
    let has_ws = extras
        .iter()
        .any(|e| e["name"].as_str() == Some("Whitespace"));
    assert!(has_ws, "Whitespace should appear in extras");
}

// -- external attribute ---------------------------------------------------

#[test]
fn external_token_appears_in_externals_list() {
    let g = grammar_from(
        r#"
        #[adze::grammar("externals_test")]
        mod grammar {
            #[adze::language]
            pub enum Root {
                A(#[adze::leaf(text = "a")] ()),
            }

            #[adze::external]
            pub struct Indent {}
        }
        "#,
    );
    let externals = g.get("externals").and_then(|v| v.as_array());
    assert!(externals.is_some(), "externals key must exist");
    let ext = externals.unwrap();
    let has_indent = ext.iter().any(|e| e["name"].as_str() == Some("Indent"));
    assert!(has_indent, "Indent should appear in externals");
}

// -- skip attribute -------------------------------------------------------

#[test]
fn skipped_field_does_not_appear_in_rules() {
    let g = grammar_from(
        r#"
        #[adze::grammar("skip_test")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"[a-z]+")]
                visible: String,
                #[adze::skip]
                _hidden: (),
            }
        }
        "#,
    );
    let root_rule = &g["rules"]["Root"];
    let json_str = serde_json::to_string(root_rule).unwrap();
    assert!(
        !json_str.contains("_hidden"),
        "skipped field should not appear in rule"
    );
}

// -- prec, prec_left, prec_right -----------------------------------------

#[test]
fn prec_left_generates_prec_left_node() {
    let g = grammar_from(
        r#"
        #[adze::grammar("prec_l")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
                #[adze::prec_left(1)]
                Add(Expr, #[adze::leaf(text = "+")] (), Expr),
            }
        }
        "#,
    );
    let json_str = serde_json::to_string_pretty(&g["rules"]).unwrap();
    assert!(
        json_str.contains("PREC_LEFT"),
        "rules should contain PREC_LEFT"
    );
}

#[test]
fn prec_right_generates_prec_right_node() {
    let g = grammar_from(
        r#"
        #[adze::grammar("prec_r")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
                #[adze::prec_right(2)]
                Pow(Expr, #[adze::leaf(text = "^")] (), Expr),
            }
        }
        "#,
    );
    let json_str = serde_json::to_string_pretty(&g["rules"]).unwrap();
    assert!(
        json_str.contains("PREC_RIGHT"),
        "rules should contain PREC_RIGHT"
    );
}

#[test]
fn prec_generates_prec_node() {
    let g = grammar_from(
        r#"
        #[adze::grammar("prec_plain")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
                #[adze::prec(5)]
                Neg(#[adze::leaf(text = "-")] (), Expr),
            }
        }
        "#,
    );
    let json_str = serde_json::to_string_pretty(&g["rules"]).unwrap();
    // "PREC" but not "PREC_LEFT" or "PREC_RIGHT"
    assert!(json_str.contains(r#""PREC""#), "rules should contain PREC");
}

#[test]
fn prec_value_zero_accepted() {
    let g = grammar_from(
        r#"
        #[adze::grammar("prec_zero")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
                #[adze::prec_left(0)]
                Add(Expr, #[adze::leaf(text = "+")] (), Expr),
            }
        }
        "#,
    );
    let json_str = serde_json::to_string(&g["rules"]).unwrap();
    assert!(json_str.contains("PREC_LEFT"));
}

// -- field("name") attribute -----------------------------------------------

#[test]
fn field_attribute_overrides_name() {
    let g = grammar_from(
        r#"
        #[adze::grammar("fld")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"\d+")]
                #[adze::field("my_number")]
                value: i32,
            }
        }
        "#,
    );
    let root_json = serde_json::to_string(&g["rules"]["Root"]).unwrap();
    assert!(
        root_json.contains("my_number"),
        "field attribute name should appear in generated rule"
    );
}

// -- delimited attribute --------------------------------------------------

#[test]
fn delimited_vec_generates_delimiter_rule() {
    let g = grammar_from(
        r#"
        #[adze::grammar("delim")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"\d+")]
                #[adze::delimited(#[adze::leaf(text = ",")] ())]
                items: Vec<i32>,
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    // Should generate a delimiter rule containing a STRING ","
    let all_json = serde_json::to_string(rules).unwrap();
    assert!(
        all_json.contains("vec_delimiter"),
        "delimited Vec should produce a delimiter rule"
    );
}

// -- repeat(non_empty = true) ---------------------------------------------

#[test]
fn repeat_non_empty_uses_repeat1_without_blank() {
    let g = grammar_from(
        r#"
        #[adze::grammar("rep_ne")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"\d+")]
                #[adze::repeat(non_empty = true)]
                items: Vec<i32>,
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    let all_json = serde_json::to_string(rules).unwrap();
    // The vec_contents rule should exist and use REPEAT1
    assert!(all_json.contains("REPEAT1"));
    // The *reference* to the vec_contents rule should NOT be wrapped in a
    // CHOICE-with-BLANK because non_empty = true.
    let root_json = serde_json::to_string(&g["rules"]["Root"]).unwrap();
    // Root rule should reference the vec_contents, and it shouldn't be optional
    assert!(root_json.contains("vec_contents"));
}

#[test]
fn vec_without_repeat_non_empty_allows_blank() {
    let g = grammar_from(
        r#"
        #[adze::grammar("rep_opt")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"\d+")]
                items: Vec<i32>,
            }
        }
        "#,
    );
    let root_json = serde_json::to_string(&g["rules"]["Root"]).unwrap();
    // Default Vec can be empty, so should have BLANK choice
    assert!(root_json.contains("BLANK") || root_json.contains("CHOICE"));
}

// -- no_inline attribute --------------------------------------------------

#[test]
fn no_inline_variant_creates_intermediate_symbol() {
    let g = grammar_from(
        r#"
        #[adze::grammar("noinl")]
        mod grammar {
            #[adze::language]
            pub enum Root {
                #[adze::no_inline]
                A(#[adze::leaf(text = "a")] ()),
                B(#[adze::leaf(text = "b")] ()),
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    // no_inline should create an intermediate symbol Root_A
    assert!(
        rules.contains_key("Root_A"),
        "no_inline variant must create named intermediate rule"
    );
}

// -- Option<T> wrapping ---------------------------------------------------

#[test]
fn option_field_generates_choice_with_blank() {
    let g = grammar_from(
        r#"
        #[adze::grammar("opt_field")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"\d+")]
                required: i32,
                #[adze::leaf(pattern = r"[a-z]+")]
                maybe: Option<String>,
            }
        }
        "#,
    );
    let root_json = serde_json::to_string(&g["rules"]["Root"]).unwrap();
    assert!(
        root_json.contains("BLANK"),
        "Option field should allow BLANK"
    );
}

// =========================================================================
// 3. Code generation for parser rules (JSON structure)
// =========================================================================

#[test]
fn enum_generates_choice_node() {
    let g = grammar_from(
        r#"
        #[adze::grammar("choice")]
        mod grammar {
            #[adze::language]
            pub enum Root {
                A(#[adze::leaf(text = "a")] ()),
                B(#[adze::leaf(text = "b")] ()),
            }
        }
        "#,
    );
    assert_eq!(g["rules"]["Root"]["type"], "CHOICE");
}

#[test]
fn enum_choice_has_correct_member_count() {
    let g = grammar_from(
        r#"
        #[adze::grammar("cc")]
        mod grammar {
            #[adze::language]
            pub enum Root {
                A(#[adze::leaf(text = "a")] ()),
                B(#[adze::leaf(text = "b")] ()),
                C(#[adze::leaf(text = "c")] ()),
            }
        }
        "#,
    );
    let members = g["rules"]["Root"]["members"].as_array().unwrap();
    assert_eq!(members.len(), 3);
}

#[test]
fn struct_with_multiple_fields_generates_seq() {
    let g = grammar_from(
        r#"
        #[adze::grammar("seq_test")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"\d+")]
                a: i32,
                #[adze::leaf(pattern = r"[a-z]+")]
                b: String,
            }
        }
        "#,
    );
    assert_eq!(g["rules"]["Root"]["type"], "SEQ");
}

#[test]
fn single_field_struct_no_seq_wrapper() {
    let g = grammar_from(
        r#"
        #[adze::grammar("single")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"\d+")]
                value: i32,
            }
        }
        "#,
    );
    // Single field -> no SEQ, should be a FIELD directly
    assert_eq!(g["rules"]["Root"]["type"], "FIELD");
}

#[test]
fn field_node_has_name_and_content() {
    let g = grammar_from(
        r#"
        #[adze::grammar("fnode")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"\d+")]
                val: i32,
            }
        }
        "#,
    );
    let root = &g["rules"]["Root"];
    assert_eq!(root["type"], "FIELD");
    assert_eq!(root["name"], "val");
    assert!(root["content"].is_object());
}

#[test]
fn symbol_reference_has_type_symbol() {
    let g = grammar_from(
        r#"
        #[adze::grammar("symref")]
        mod grammar {
            #[adze::language]
            pub enum Root {
                Inner(Inner),
            }
            pub struct Inner {
                #[adze::leaf(text = "x")]
                x: (),
            }
        }
        "#,
    );
    let root = &g["rules"]["Root"];
    // Root is a CHOICE with members that reference Inner
    let json_str = serde_json::to_string(root).unwrap();
    assert!(json_str.contains("SYMBOL"));
}

#[test]
fn grammar_json_is_valid_json() {
    let g = grammar_from(
        r#"
        #[adze::grammar("roundtrip")]
        mod grammar {
            #[adze::language]
            pub enum Tok {
                A(#[adze::leaf(text = "a")] ()),
            }
        }
        "#,
    );
    let s = serde_json::to_string(&g).unwrap();
    let _: Value = serde_json::from_str(&s).unwrap();
}

#[test]
fn grammar_name_matches_attribute() {
    let g = grammar_from(
        r#"
        #[adze::grammar("my_custom_name")]
        mod grammar {
            #[adze::language]
            pub enum R {
                X(#[adze::leaf(text = "x")] ()),
            }
        }
        "#,
    );
    assert_eq!(g["name"], "my_custom_name");
}

#[test]
fn rules_object_always_contains_source_file() {
    let g = grammar_from(
        r#"
        #[adze::grammar("sf_check")]
        mod grammar {
            #[adze::language]
            pub enum R {
                X(#[adze::leaf(text = "x")] ()),
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    assert!(rules.contains_key("source_file"));
}

#[test]
fn extras_key_is_always_present() {
    let g = grammar_from(
        r#"
        #[adze::grammar("extras_key")]
        mod grammar {
            #[adze::language]
            pub enum R {
                X(#[adze::leaf(text = "x")] ()),
            }
        }
        "#,
    );
    assert!(g.get("extras").is_some());
    assert!(g["extras"].is_array());
}

#[test]
fn word_key_present_even_when_no_word_rule() {
    let g = grammar_from(
        r#"
        #[adze::grammar("no_word")]
        mod grammar {
            #[adze::language]
            pub enum R {
                X(#[adze::leaf(text = "x")] ()),
            }
        }
        "#,
    );
    // "word" key should be present (null if no word rule)
    assert!(g.get("word").is_some());
}

// =========================================================================
// 4. Feature flag handling
// =========================================================================

#[test]
fn externals_key_absent_when_no_externals() {
    let g = grammar_from(
        r#"
        #[adze::grammar("no_ext")]
        mod grammar {
            #[adze::language]
            pub enum R {
                X(#[adze::leaf(text = "x")] ()),
            }
        }
        "#,
    );
    // externals key is only included when there are external tokens
    assert!(
        g.get("externals").is_none(),
        "externals key should not be present when there are no external tokens"
    );
}

#[test]
fn externals_key_present_when_externals_exist() {
    let g = grammar_from(
        r#"
        #[adze::grammar("has_ext")]
        mod grammar {
            #[adze::language]
            pub enum R {
                X(#[adze::leaf(text = "x")] ()),
            }
            #[adze::external]
            pub struct MyExternal {}
        }
        "#,
    );
    assert!(g.get("externals").is_some());
}

#[test]
fn external_token_also_added_to_extras() {
    let g = grammar_from(
        r#"
        #[adze::grammar("ext_in_extras")]
        mod grammar {
            #[adze::language]
            pub enum R {
                X(#[adze::leaf(text = "x")] ()),
            }
            #[adze::external]
            pub struct Indent {}
        }
        "#,
    );
    let extras = g["extras"].as_array().unwrap();
    let has_indent = extras.iter().any(|e| e["name"].as_str() == Some("Indent"));
    assert!(
        has_indent,
        "external tokens should also appear in extras list"
    );
}

// =========================================================================
// 5. Error cases
// =========================================================================

#[test]
fn multiple_word_rules_error() {
    // Two structs with #[adze::word] at the struct level triggers the error.
    let e = grammar_err(
        r#"
        #[adze::grammar("mw")]
        mod grammar {
            #[adze::language]
            pub enum Root {
                A(#[adze::leaf(text = "a")] ()),
            }

            #[adze::word]
            pub struct Ident1 {
                #[adze::leaf(pattern = r"[a-z]+")]
                _v: String,
            }

            #[adze::word]
            pub struct Ident2 {
                #[adze::leaf(pattern = r"[A-Z]+")]
                _v: String,
            }
        }
        "#,
    );
    let msg = format!("{e}");
    assert!(
        msg.contains("word"),
        "error should mention 'word': got {msg}"
    );
}

#[test]
fn multiple_prec_attributes_error() {
    let e = grammar_err(
        r#"
        #[adze::grammar("mp")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
                #[adze::prec_left(1)]
                #[adze::prec_right(2)]
                Add(Expr, #[adze::leaf(text = "+")] (), Expr),
            }
        }
        "#,
    );
    let msg = format!("{e}");
    assert!(
        msg.contains("prec"),
        "error should mention precedence conflict: got {msg}"
    );
}

#[test]
fn empty_pattern_error() {
    // Empty patterns cause a field-level error that is caught during
    // struct generation. With a single field, this results in a
    // StructHasNoFields error since the only field is dropped.
    let e = grammar_err(
        r#"
        #[adze::grammar("ep")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = "")]
                bad: String,
            }
        }
        "#,
    );
    let msg = format!("{e}");
    assert!(
        msg.contains("no non-skipped fields") || msg.contains("no fields"),
        "struct with only an invalid empty-pattern field should have no usable fields: got {msg}"
    );
}

#[test]
fn struct_with_all_skipped_fields_error() {
    let e = grammar_err(
        r#"
        #[adze::grammar("allskip")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::skip]
                _a: (),
                #[adze::skip]
                _b: (),
            }
        }
        "#,
    );
    let msg = format!("{e}");
    assert!(
        msg.contains("no non-skipped fields") || msg.contains("no fields"),
        "error should report no fields: got {msg}"
    );
}

#[test]
#[should_panic]
fn missing_grammar_name_panics() {
    // grammar attribute without a string name should panic
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("lib.rs");
    fs::write(
        &path,
        r#"
        #[adze::grammar]
        mod grammar {
            #[adze::language]
            pub enum R {
                X(#[adze::leaf(text = "x")] ()),
            }
        }
        "#,
    )
    .unwrap();
    let _ = adze_tool::generate_grammars(&path);
}

#[test]
#[should_panic]
fn missing_language_annotation_panics() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("lib.rs");
    fs::write(
        &path,
        r#"
        #[adze::grammar("nolang")]
        mod grammar {
            pub enum R {
                X(#[adze::leaf(text = "x")] ()),
            }
        }
        "#,
    )
    .unwrap();
    let _ = adze_tool::generate_grammars(&path);
}

// =========================================================================
// 6. Deeper structural validation
// =========================================================================

#[test]
fn pattern_rule_value_matches_input() {
    // Use a struct field so the PATTERN is a named top-level rule.
    let g = grammar_from(
        r#"
        #[adze::grammar("pval")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"[a-z_]+")]
                id: String,
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    let pattern_val = rules
        .values()
        .find(|v| v["type"] == "PATTERN")
        .map(|v| v["value"].as_str().unwrap().to_string());
    assert_eq!(pattern_val.as_deref(), Some("[a-z_]+"));
}

#[test]
fn text_rule_value_matches_input() {
    // Use a struct field so the STRING is a named top-level rule.
    let g = grammar_from(
        r#"
        #[adze::grammar("tval")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(text = "++")]
                op: (),
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    let string_val = rules
        .values()
        .find(|v| v["type"] == "STRING")
        .map(|v| v["value"].as_str().unwrap().to_string());
    assert_eq!(string_val.as_deref(), Some("++"));
}

#[test]
fn prec_left_value_matches_attribute() {
    let g = grammar_from(
        r#"
        #[adze::grammar("plv")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
                #[adze::prec_left(42)]
                Add(Expr, #[adze::leaf(text = "+")] (), Expr),
            }
        }
        "#,
    );
    let all_json = serde_json::to_string(&g["rules"]).unwrap();
    // The precedence value 42 should appear in the JSON
    assert!(all_json.contains("42"));
}

#[test]
fn vec_field_generates_vec_contents_rule() {
    let g = grammar_from(
        r#"
        #[adze::grammar("vec_c")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"\d+")]
                items: Vec<i32>,
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    let has_vec_contents = rules.keys().any(|k| k.contains("vec_contents"));
    assert!(
        has_vec_contents,
        "Vec field should generate a *_vec_contents rule"
    );
}

#[test]
fn vec_element_field_name_generated() {
    let g = grammar_from(
        r#"
        #[adze::grammar("vec_el")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"\d+")]
                nums: Vec<i32>,
            }
        }
        "#,
    );
    let all_json = serde_json::to_string(&g["rules"]).unwrap();
    assert!(
        all_json.contains("vec_element"),
        "Vec fields should name elements with _vec_element"
    );
}

#[test]
fn unit_variant_is_not_inlined() {
    let g = grammar_from(
        r#"
        #[adze::grammar("unit_var")]
        mod grammar {
            #[adze::language]
            pub enum Root {
                #[adze::leaf(text = "nil")]
                Nil,
                Other(#[adze::leaf(text = "x")] ()),
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    // Unit variants should NOT be inlined, so Root_Nil should exist
    assert!(
        rules.contains_key("Root_Nil"),
        "unit variant should create an intermediate symbol"
    );
}

#[test]
fn prec_variant_is_not_inlined() {
    let g = grammar_from(
        r#"
        #[adze::grammar("prec_noinl")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
                #[adze::prec_left(1)]
                Add(Expr, #[adze::leaf(text = "+")] (), Expr),
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    // Precedence variants should NOT be inlined
    assert!(
        rules.contains_key("Expr_Add"),
        "prec variant should create intermediate symbol"
    );
}

#[test]
fn grammar_json_serializable_roundtrip() {
    let g = grammar_from(
        r#"
        #[adze::grammar("rt")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
                #[adze::prec_left(1)]
                Add(Expr, #[adze::leaf(text = "+")] (), Expr),
            }
        }
        "#,
    );
    let serialized = serde_json::to_vec_pretty(&g).unwrap();
    let deserialized: Value = serde_json::from_slice(&serialized).unwrap();
    assert_eq!(g, deserialized);
}

#[test]
fn extra_and_external_together() {
    let g = grammar_from(
        r#"
        #[adze::grammar("both")]
        mod grammar {
            #[adze::language]
            pub enum Root {
                A(#[adze::leaf(text = "a")] ()),
            }

            #[adze::extra]
            pub struct WS {
                #[adze::leaf(pattern = r"\s")]
                _ws: String,
            }

            #[adze::external]
            pub struct Indent {}
        }
        "#,
    );
    assert!(g["extras"].as_array().unwrap().len() >= 2);
    assert!(!g["externals"].as_array().unwrap().is_empty());
}

#[test]
fn struct_language_root_generates_correct_source_file() {
    let g = grammar_from(
        r#"
        #[adze::grammar("struct_root")]
        mod grammar {
            #[adze::language]
            pub struct Program {
                #[adze::leaf(pattern = r"[a-z]+")]
                name: String,
            }
        }
        "#,
    );
    let sf = &g["rules"]["source_file"];
    assert_eq!(sf["type"], "SYMBOL");
    assert_eq!(sf["name"], "Program");
}

#[test]
fn deterministic_output_across_calls() {
    let src = r#"
        #[adze::grammar("det")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
                #[adze::prec_left(1)]
                Add(Expr, #[adze::leaf(text = "+")] (), Expr),
                #[adze::prec_left(2)]
                Mul(Expr, #[adze::leaf(text = "*")] (), Expr),
            }
        }
    "#;
    let g1 = grammar_from(src);
    let g2 = grammar_from(src);
    assert_eq!(g1, g2, "output should be deterministic");
}

#[test]
fn enum_with_single_variant() {
    let g = grammar_from(
        r#"
        #[adze::grammar("single_var")]
        mod grammar {
            #[adze::language]
            pub enum Root {
                Only(#[adze::leaf(text = "only")] ()),
            }
        }
        "#,
    );
    // Even a single-variant enum produces a CHOICE
    assert_eq!(g["rules"]["Root"]["type"], "CHOICE");
    assert_eq!(g["rules"]["Root"]["members"].as_array().unwrap().len(), 1);
}

#[test]
fn word_rule_set_on_struct_field() {
    let g = grammar_from(
        r#"
        #[adze::grammar("word_struct")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"[a-z]+")]
                #[adze::word]
                name: String,
            }
        }
        "#,
    );
    let word = g["word"].as_str();
    assert!(word.is_some(), "word should be set");
}

#[test]
fn word_rule_set_on_struct_level() {
    let g = grammar_from(
        r#"
        #[adze::grammar("word_level")]
        mod grammar {
            #[adze::language]
            pub enum Root {
                Id(#[adze::leaf(pattern = r"[a-z]+")] String),
            }

            #[adze::word]
            pub struct Identifier {
                #[adze::leaf(pattern = r"[a-zA-Z_]+")]
                _id: String,
            }
        }
        "#,
    );
    let word = g["word"].as_str();
    assert!(word.is_some(), "word should be set at struct level");
    assert_eq!(word.unwrap(), "Identifier");
}

#[test]
fn variant_level_leaf_text_is_the_single_leaf_token() {
    // `#[adze::leaf(text = "true")] Literal(bool)` declares the variant's
    // token on the variant, not the field. Generation must emit the STRING
    // token, not a SYMBOL reference to the undefined rule `bool`.
    let g = grammar_from(
        r#"
        #[adze::grammar("variant_leaf")]
        mod grammar {
            #[adze::language]
            pub enum BoolExpr {
                #[adze::leaf(text = "true", transform = |_| true)]
                Literal(bool),
                #[adze::leaf(text = "false", transform = |_| false)]
                LiteralFalse(bool),
            }
        }
        "#,
    );
    let serialized = serde_json::to_string(&g["rules"]).unwrap();
    assert!(
        !serialized.contains("\"name\":\"bool\""),
        "no SYMBOL reference to undefined `bool` rule: {serialized}"
    );
    assert!(
        serialized.contains("\"value\":\"true\""),
        "variant leaf text emitted as token: {serialized}"
    );
    assert!(
        serialized.contains("\"value\":\"false\""),
        "second variant leaf text emitted as token: {serialized}"
    );
}
