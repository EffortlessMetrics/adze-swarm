#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Property-based tests for inline rule generation in adze-tool.
//!
//! Validates that enum variant inlining produces the correct grammar JSON:
//!   - Inline rules appear directly in CHOICE members
//!   - Inline rule naming conventions (no intermediate symbols)
//!   - Inline rule determinism (same input → same output)
//!   - Inline rules from nested types
//!   - Inline rules from Optional fields
//!   - Inline rules from Repeat/Vec fields
//!   - No inline rules when #[adze::no_inline] or precedence is used
//!   - Inline rule deduplication

use proptest::prelude::*;
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

// ===========================================================================
// Helpers
// ===========================================================================

fn extract_one(src: &str) -> Value {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("lib.rs");
    fs::write(&path, src).unwrap();
    let gs = adze_tool::generate_grammars(&path).unwrap();
    assert_eq!(gs.len(), 1, "expected exactly one grammar");
    gs.into_iter().next().unwrap()
}

fn rule_names(grammar: &Value) -> Vec<String> {
    grammar["rules"]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

/// Recursively collect every JSON node whose "type" equals `type_name`.
fn collect_nodes_of_type(val: &Value, type_name: &str) -> Vec<Value> {
    let mut out = Vec::new();
    match val {
        Value::Object(map) => {
            if map.get("type").and_then(|v| v.as_str()) == Some(type_name) {
                out.push(val.clone());
            }
            for v in map.values() {
                out.extend(collect_nodes_of_type(v, type_name));
            }
        }
        Value::Array(arr) => {
            for v in arr {
                out.extend(collect_nodes_of_type(v, type_name));
            }
        }
        _ => {}
    }
    out
}

/// Collect all SYMBOL name references recursively.
fn collect_symbol_names(val: &Value) -> Vec<String> {
    let mut out = Vec::new();
    match val {
        Value::Object(map) => {
            if map.get("type").and_then(|v| v.as_str()) == Some("SYMBOL")
                && let Some(n) = map.get("name").and_then(|v| v.as_str())
            {
                out.push(n.to_string());
            }
            for v in map.values() {
                out.extend(collect_symbol_names(v));
            }
        }
        Value::Array(arr) => {
            for v in arr {
                out.extend(collect_symbol_names(v));
            }
        }
        _ => {}
    }
    out
}

fn member_rule<'a>(grammar: &'a Value, member: &'a Value) -> &'a Value {
    if member["type"].as_str() == Some("SYMBOL")
        && let Some(name) = member["name"].as_str()
        && let Some(rule) = grammar["rules"].get(name)
    {
        rule
    } else {
        member
    }
}

// ===========================================================================
// Strategies
// ===========================================================================

fn grammar_name_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,10}".prop_filter("non-empty", |s| !s.is_empty())
}

fn type_name_strategy() -> impl Strategy<Value = String> {
    "[A-Z][a-z]{1,8}".prop_filter("valid Rust identifier in this edition", |s| {
        syn::parse_str::<syn::Ident>(s).is_ok()
    })
}

fn safe_pattern_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(r"[a-z]+".to_string()),
        Just(r"\d+".to_string()),
        Just(r"[a-zA-Z_][a-zA-Z0-9_]*".to_string()),
        Just(r"[0-9]+".to_string()),
        Just(r"[a-f0-9]+".to_string()),
    ]
}

fn text_token_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("+".to_string()),
        Just("-".to_string()),
        Just("*".to_string()),
        Just("=".to_string()),
        Just(";".to_string()),
        Just(",".to_string()),
    ]
}

// ===========================================================================
// Source-generation helpers
// ===========================================================================

/// Enum with two single-leaf variants (inlined by default).
fn enum_two_leaf_src(name: &str, ty: &str, p1: &str, p2: &str) -> String {
    format!(
        r##"
        #[adze::grammar("{name}")]
        mod grammar {{
            #[adze::language]
            pub enum {ty} {{
                Alpha(#[adze::leaf(pattern = r"{p1}")] String),
                Beta(#[adze::leaf(pattern = r"{p2}")] String),
            }}
        }}
        "##,
    )
}

/// Enum where one variant has two fields (multi-field SEQ).
fn enum_with_seq_variant_src(name: &str, ty: &str, pat: &str, tok: &str) -> String {
    format!(
        r##"
        #[adze::grammar("{name}")]
        mod grammar {{
            #[adze::language]
            pub enum {ty} {{
                Pair(
                    #[adze::leaf(pattern = r"{pat}")] String,
                    #[adze::leaf(text = "{tok}")] (),
                ),
                Single(#[adze::leaf(pattern = r"\d+")] String),
            }}
        }}
        "##,
    )
}

/// Enum with a no_inline variant.
fn enum_no_inline_src(name: &str, ty: &str, pat: &str) -> String {
    format!(
        r##"
        #[adze::grammar("{name}")]
        mod grammar {{
            #[adze::language]
            pub enum {ty} {{
                Alpha(#[adze::leaf(pattern = r"{pat}")] String),
                #[adze::no_inline]
                Beta(#[adze::leaf(pattern = r"\d+")] String),
            }}
        }}
        "##,
    )
}

/// Enum with a prec_left variant (not inlined).
fn enum_prec_left_src(name: &str, ty: &str) -> String {
    format!(
        r##"
        #[adze::grammar("{name}")]
        mod grammar {{
            #[adze::language]
            pub enum {ty} {{
                Number(#[adze::leaf(pattern = r"\d+")] String),
                #[adze::prec_left(1)]
                Add(
                    Box<{ty}>,
                    #[adze::leaf(text = "+")] (),
                    Box<{ty}>,
                ),
            }}
        }}
        "##,
    )
}

/// Enum with an Optional field inside a variant.
fn enum_optional_variant_src(name: &str, ty: &str, pat: &str) -> String {
    format!(
        r##"
        #[adze::grammar("{name}")]
        mod grammar {{
            #[adze::language]
            pub enum {ty} {{
                Maybe(
                    #[adze::leaf(pattern = r"{pat}")] Option<String>,
                    #[adze::leaf(pattern = r"\d+")] String,
                ),
                Plain(#[adze::leaf(pattern = r"[a-z]+")] String),
            }}
        }}
        "##,
    )
}

/// Enum with a Vec (repeat) field inside a variant.
fn enum_repeat_variant_src(name: &str, ty: &str) -> String {
    format!(
        r##"
        #[adze::grammar("{name}")]
        mod grammar {{
            pub struct Num {{
                #[adze::leaf(pattern = r"\d+")]
                v: String,
            }}

            #[adze::language]
            pub enum {ty} {{
                Nums(
                    #[adze::repeat(non_empty = true)]
                    Vec<Num>,
                ),
                Single(#[adze::leaf(pattern = r"[a-z]+")] String),
            }}
        }}
        "##,
    )
}

/// Struct-only grammar (no enum, no inlining).
fn struct_only_src(name: &str, ty: &str, pat: &str) -> String {
    format!(
        r##"
        #[adze::grammar("{name}")]
        mod grammar {{
            #[adze::language]
            pub struct {ty} {{
                #[adze::leaf(pattern = r"{pat}")]
                val: String,
            }}
        }}
        "##,
    )
}

/// Enum with three leaf variants that have the same pattern.
fn enum_dedup_src(name: &str, ty: &str, pat: &str) -> String {
    format!(
        r##"
        #[adze::grammar("{name}")]
        mod grammar {{
            #[adze::language]
            pub enum {ty} {{
                A(#[adze::leaf(pattern = r"{pat}")] String),
                B(#[adze::leaf(pattern = r"{pat}")] String),
                C(#[adze::leaf(pattern = r"{pat}")] String),
            }}
        }}
        "##,
    )
}

/// Enum with nested struct reference.
fn enum_nested_struct_src(name: &str, ty: &str) -> String {
    format!(
        r##"
        #[adze::grammar("{name}")]
        mod grammar {{
            pub struct Inner {{
                #[adze::leaf(pattern = r"\d+")]
                val: String,
            }}

            #[adze::language]
            pub enum {ty} {{
                Wrapped(Inner),
                Leaf(#[adze::leaf(pattern = r"[a-z]+")] String),
            }}
        }}
        "##,
    )
}

/// Enum with a text variant (STRING instead of PATTERN).
fn enum_text_variant_src(name: &str, ty: &str, tok: &str) -> String {
    format!(
        r##"
        #[adze::grammar("{name}")]
        mod grammar {{
            #[adze::language]
            pub enum {ty} {{
                Keyword(#[adze::leaf(text = "{tok}")] ()),
                Ident(#[adze::leaf(pattern = r"[a-z]+")] String),
            }}
        }}
        "##,
    )
}

// ===========================================================================
// 1. Inlined single-leaf variants produce PATTERN directly in CHOICE
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn inlined_leaf_variants_are_pattern_in_choice(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        p1 in safe_pattern_strategy(),
        p2 in safe_pattern_strategy(),
    ) {
        let src = enum_two_leaf_src(&name, &ty, &p1, &p2);
        let g = extract_one(&src);
        let choice = &g["rules"][&ty];
        prop_assert_eq!(choice["type"].as_str().unwrap(), "CHOICE");
        let members = choice["members"].as_array().unwrap();
        prop_assert_eq!(members.len(), 2);
        // Both members should be PATTERN (inlined directly)
        for m in members {
            prop_assert_eq!(m["type"].as_str().unwrap(), "PATTERN");
        }
    }
}

// ===========================================================================
// 2. Inlined variants don't create intermediate named rules
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn inlined_variants_no_intermediate_rules(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        p1 in safe_pattern_strategy(),
        p2 in safe_pattern_strategy(),
    ) {
        let src = enum_two_leaf_src(&name, &ty, &p1, &p2);
        let g = extract_one(&src);
        let names = rule_names(&g);
        // Should have source_file and the enum type — no variant rules
        let alpha_rule = format!("{ty}_Alpha");
        let beta_rule = format!("{ty}_Beta");
        prop_assert!(!names.contains(&alpha_rule), "found intermediate rule {alpha_rule}");
        prop_assert!(!names.contains(&beta_rule), "found intermediate rule {beta_rule}");
    }
}

// ===========================================================================
// 3. Inline rule determinism — same input yields identical JSON
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn inline_rule_determinism(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        p1 in safe_pattern_strategy(),
        p2 in safe_pattern_strategy(),
    ) {
        let src = enum_two_leaf_src(&name, &ty, &p1, &p2);
        let g1 = extract_one(&src);
        let g2 = extract_one(&src);
        prop_assert_eq!(g1, g2, "grammar generation must be deterministic");
    }
}

// ===========================================================================
// 4. Multi-field inlined variant produces SEQ in CHOICE
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn multifield_inlined_variant_is_seq(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        pat in safe_pattern_strategy(),
        tok in text_token_strategy(),
    ) {
        let src = enum_with_seq_variant_src(&name, &ty, &pat, &tok);
        let g = extract_one(&src);
        let members = g["rules"][&ty]["members"].as_array().unwrap();
        // First variant (Pair) has two fields → should be SEQ
        prop_assert_eq!(members[0]["type"].as_str().unwrap(), "SEQ");
        // Second variant (Single) is single leaf → PATTERN
        prop_assert_eq!(members[1]["type"].as_str().unwrap(), "PATTERN");
    }
}

// ===========================================================================
// 5. Inlined SEQ variant contains FIELD nodes
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn inlined_seq_contains_fields(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        pat in safe_pattern_strategy(),
        tok in text_token_strategy(),
    ) {
        let src = enum_with_seq_variant_src(&name, &ty, &pat, &tok);
        let g = extract_one(&src);
        let members = g["rules"][&ty]["members"].as_array().unwrap();
        let seq_members = members[0]["members"].as_array().unwrap();
        // Both children of the SEQ should be FIELD
        for m in seq_members {
            prop_assert_eq!(m["type"].as_str().unwrap(), "FIELD");
        }
    }
}

// ===========================================================================
// 6. no_inline variant creates an intermediate SYMBOL reference
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn no_inline_creates_symbol(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        pat in safe_pattern_strategy(),
    ) {
        let src = enum_no_inline_src(&name, &ty, &pat);
        let g = extract_one(&src);
        let members = g["rules"][&ty]["members"].as_array().unwrap();
        // Alpha is inlined (PATTERN), Beta has no_inline (SYMBOL)
        prop_assert_eq!(members[0]["type"].as_str().unwrap(), "PATTERN");
        prop_assert_eq!(members[1]["type"].as_str().unwrap(), "SYMBOL");
        let beta_name = format!("{ty}_Beta");
        prop_assert_eq!(members[1]["name"].as_str().unwrap(), beta_name.as_str());
    }
}

// ===========================================================================
// 7. no_inline variant generates a named rule in the rules map
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn no_inline_generates_named_rule(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        pat in safe_pattern_strategy(),
    ) {
        let src = enum_no_inline_src(&name, &ty, &pat);
        let g = extract_one(&src);
        let names = rule_names(&g);
        let beta_rule = format!("{ty}_Beta");
        prop_assert!(names.contains(&beta_rule), "expected rule {beta_rule}");
    }
}

// ===========================================================================
// 8. Precedence variant is not inlined (creates SYMBOL)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn prec_variant_not_inlined(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
    ) {
        let src = enum_prec_left_src(&name, &ty);
        let g = extract_one(&src);
        let members = g["rules"][&ty]["members"].as_array().unwrap();
        // Number is inlined (single leaf → PATTERN), Add has prec → SYMBOL
        prop_assert_eq!(members[0]["type"].as_str().unwrap(), "PATTERN");
        prop_assert_eq!(members[1]["type"].as_str().unwrap(), "SYMBOL");
    }
}

// ===========================================================================
// 9. Precedence variant rule has PREC_LEFT wrapper
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn prec_variant_has_prec_wrapper(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
    ) {
        let src = enum_prec_left_src(&name, &ty);
        let g = extract_one(&src);
        let add_rule_name = format!("{ty}_Add");
        let add_rule = &g["rules"][&add_rule_name];
        prop_assert_eq!(add_rule["type"].as_str().unwrap(), "PREC_LEFT");
        prop_assert_eq!(add_rule["value"].as_u64().unwrap(), 1);
    }
}

// ===========================================================================
// 10. Optional field inside inlined variant produces CHOICE with BLANK
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn optional_in_inlined_variant(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        pat in safe_pattern_strategy(),
    ) {
        let src = enum_optional_variant_src(&name, &ty, &pat);
        let g = extract_one(&src);
        let members = g["rules"][&ty]["members"].as_array().unwrap();
        // Maybe variant is inlined as SEQ (has two fields)
        let maybe = &members[0];
        prop_assert_eq!(maybe["type"].as_str().unwrap(), "SEQ");
        // First field is optional → wrapped in CHOICE with BLANK
        let seq_members = maybe["members"].as_array().unwrap();
        let first = &seq_members[0];
        prop_assert_eq!(first["type"].as_str().unwrap(), "CHOICE");
        let choice_members = first["members"].as_array().unwrap();
        let has_blank = choice_members.iter().any(|m| {
            m.get("type").and_then(|t| t.as_str()) == Some("BLANK")
        });
        prop_assert!(has_blank, "optional field should have BLANK in CHOICE");
    }
}

// ===========================================================================
// 11. Repeat field in inlined variant uses SYMBOL for vec_contents
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn repeat_in_inlined_variant(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
    ) {
        let src = enum_repeat_variant_src(&name, &ty);
        let g = extract_one(&src);
        let names = rule_names(&g);
        // The Vec creates a _vec_contents rule
        let has_vec_rule = names.iter().any(|n| n.contains("vec_contents"));
        prop_assert!(has_vec_rule, "repeat should generate vec_contents rule");
    }
}

// ===========================================================================
// 12. Struct-only grammar has no inlined rules at all
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn struct_only_no_inline_rules(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        pat in safe_pattern_strategy(),
    ) {
        let src = struct_only_src(&name, &ty, &pat);
        let g = extract_one(&src);
        let _choice_nodes = collect_nodes_of_type(&g["rules"], "CHOICE");
        // No CHOICE at the top level for a struct grammar
        let root_rule = &g["rules"][&ty];
        prop_assert_ne!(root_rule["type"].as_str().unwrap(), "CHOICE",
            "struct grammar should not have CHOICE root");
    }
}

// ===========================================================================
// 13. Deduplication: identical patterns still produce correct member count
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn dedup_same_pattern_correct_count(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        pat in safe_pattern_strategy(),
    ) {
        let src = enum_dedup_src(&name, &ty, &pat);
        let g = extract_one(&src);
        let members = g["rules"][&ty]["members"].as_array().unwrap();
        // All three variants should be present in CHOICE
        prop_assert_eq!(members.len(), 3);
    }
}

// ===========================================================================
// 14. Dedup variants are all PATTERN type (all inlined)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn dedup_variants_all_inlined(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        pat in safe_pattern_strategy(),
    ) {
        let src = enum_dedup_src(&name, &ty, &pat);
        let g = extract_one(&src);
        let members = g["rules"][&ty]["members"].as_array().unwrap();
        for m in members {
            prop_assert_eq!(m["type"].as_str().unwrap(), "PATTERN");
        }
    }
}

// ===========================================================================
// 15. Dedup variants share the same pattern value
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn dedup_variants_same_pattern_value(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        pat in safe_pattern_strategy(),
    ) {
        let src = enum_dedup_src(&name, &ty, &pat);
        let g = extract_one(&src);
        let members = g["rules"][&ty]["members"].as_array().unwrap();
        let first_val = members[0]["value"].as_str().unwrap();
        for i in 1..members.len() {
            prop_assert_eq!(members[i]["value"].as_str().unwrap(), first_val);
        }
    }
}

// ===========================================================================
// 16. Grammar JSON always has "rules" and "name" keys
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn grammar_json_has_required_keys(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        pat in safe_pattern_strategy(),
    ) {
        let src = enum_two_leaf_src(&name, &ty, &pat, r"\d+");
        let g = extract_one(&src);
        prop_assert!(g.get("rules").is_some(), "grammar must have rules");
        prop_assert!(g.get("name").is_some(), "grammar must have name");
        prop_assert_eq!(g["name"].as_str().unwrap(), name.as_str());
    }
}

// ===========================================================================
// 17. Nested struct reference in inlined variant uses SYMBOL
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn nested_struct_inlined_variant_uses_symbol(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
    ) {
        let src = enum_nested_struct_src(&name, &ty);
        let g = extract_one(&src);
        let members = g["rules"][&ty]["members"].as_array().unwrap();
        // Wrapped variant references Inner (non-leaf), either directly or via
        // the explicit generated wrapper needed for reduce/reduce identity.
        let wrapped = member_rule(&g, &members[0]);
        let inner_symbols = collect_symbol_names(wrapped);
        prop_assert!(inner_symbols.contains(&"Inner".to_string()),
            "wrapped variant should reference Inner struct");
    }
}

// ===========================================================================
// 18. Nested struct reference creates a named rule for the struct
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn nested_struct_creates_rule(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
    ) {
        let src = enum_nested_struct_src(&name, &ty);
        let g = extract_one(&src);
        let names = rule_names(&g);
        prop_assert!(names.contains(&"Inner".to_string()), "Inner struct should have its own rule");
    }
}

// ===========================================================================
// 19. Text (STRING) variant is inlined directly
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn text_variant_inlined_as_string(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        tok in text_token_strategy(),
    ) {
        let src = enum_text_variant_src(&name, &ty, &tok);
        let g = extract_one(&src);
        let members = g["rules"][&ty]["members"].as_array().unwrap();
        // Keyword variant with text → STRING
        prop_assert_eq!(members[0]["type"].as_str().unwrap(), "STRING");
        prop_assert_eq!(members[0]["value"].as_str().unwrap(), tok.as_str());
    }
}

// ===========================================================================
// 20. Text variant does not create intermediate rule
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn text_variant_no_intermediate_rule(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        tok in text_token_strategy(),
    ) {
        let src = enum_text_variant_src(&name, &ty, &tok);
        let g = extract_one(&src);
        let names = rule_names(&g);
        let kw_rule = format!("{ty}_Keyword");
        prop_assert!(!names.contains(&kw_rule), "inlined text variant should not have named rule");
    }
}

// ===========================================================================
// 21. source_file rule always exists and references root type
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn source_file_references_root(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        pat in safe_pattern_strategy(),
    ) {
        let src = enum_two_leaf_src(&name, &ty, &pat, r"\d+");
        let g = extract_one(&src);
        let sf = &g["rules"]["source_file"];
        prop_assert_eq!(sf["type"].as_str().unwrap(), "SYMBOL");
        prop_assert_eq!(sf["name"].as_str().unwrap(), ty.as_str());
    }
}

// ===========================================================================
// 22. Inlined variant with two fields has exactly 2 SEQ members
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn inlined_seq_has_correct_member_count(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        pat in safe_pattern_strategy(),
        tok in text_token_strategy(),
    ) {
        let src = enum_with_seq_variant_src(&name, &ty, &pat, &tok);
        let g = extract_one(&src);
        let members = g["rules"][&ty]["members"].as_array().unwrap();
        let seq_members = members[0]["members"].as_array().unwrap();
        prop_assert_eq!(seq_members.len(), 2);
    }
}

// ===========================================================================
// 23. Repeat variant's vec_contents rule has REPEAT1 type
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn repeat_vec_contents_is_repeat1(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
    ) {
        let src = enum_repeat_variant_src(&name, &ty);
        let g = extract_one(&src);
        let rules = g["rules"].as_object().unwrap();
        let vec_rule = rules.iter()
            .find(|(k, _)| k.contains("vec_contents"))
            .map(|(_, v)| v);
        prop_assert!(vec_rule.is_some(), "vec_contents rule should exist");
        prop_assert_eq!(vec_rule.unwrap()["type"].as_str().unwrap(), "REPEAT1");
    }
}

// ===========================================================================
// 24. Mixed inlined and non-inlined variants in same enum
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn mixed_inline_and_noinline(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        pat in safe_pattern_strategy(),
    ) {
        let src = enum_no_inline_src(&name, &ty, &pat);
        let g = extract_one(&src);
        let members = g["rules"][&ty]["members"].as_array().unwrap();
        // Alpha inlined → PATTERN, Beta not inlined → SYMBOL
        let types: Vec<&str> = members.iter()
            .map(|m| m["type"].as_str().unwrap())
            .collect();
        prop_assert!(types.contains(&"PATTERN"), "should have inlined PATTERN");
        prop_assert!(types.contains(&"SYMBOL"), "should have non-inlined SYMBOL");
    }
}

// ===========================================================================
// 25. Inline rule naming: no variant rules for fully-inlined enum
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn fully_inlined_enum_minimal_rules(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        p1 in safe_pattern_strategy(),
        p2 in safe_pattern_strategy(),
    ) {
        let src = enum_two_leaf_src(&name, &ty, &p1, &p2);
        let g = extract_one(&src);
        let names = rule_names(&g);
        // Only source_file and the enum type itself
        prop_assert_eq!(names.len(), 2, "fully inlined enum should only have 2 rules, got {:?}", names);
        prop_assert!(names.contains(&"source_file".to_string()));
        prop_assert!(names.contains(&ty));
    }
}

// ===========================================================================
// 26. Inlined SEQ variant does not produce top-level named rule
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn inlined_seq_no_toplevel_rule(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        pat in safe_pattern_strategy(),
        tok in text_token_strategy(),
    ) {
        let src = enum_with_seq_variant_src(&name, &ty, &pat, &tok);
        let g = extract_one(&src);
        let names = rule_names(&g);
        let pair_rule = format!("{ty}_Pair");
        prop_assert!(!names.contains(&pair_rule), "inlined Pair variant should not have named rule");
    }
}

// ===========================================================================
// 27. Enum with extras still inlines variants
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    #[test]
    fn enum_with_extras_still_inlines(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
    ) {
        let src = format!(
            r##"
            #[adze::grammar("{name}")]
            mod grammar {{
                #[adze::language]
                pub enum {ty} {{
                    Alpha(#[adze::leaf(pattern = r"[a-z]+")] String),
                    Digit(#[adze::leaf(pattern = r"\d+")] String),
                }}

                #[adze::extra]
                struct Whitespace {{
                    #[adze::leaf(pattern = r"\s")]
                    _whitespace: (),
                }}
            }}
            "##,
        );
        let g = extract_one(&src);
        let members = g["rules"][&ty]["members"].as_array().unwrap();
        for m in members {
            prop_assert_eq!(m["type"].as_str().unwrap(), "PATTERN");
        }
    }
}

// ===========================================================================
// 28. Determinism: two runs of same grammar produce byte-identical JSON
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    #[test]
    fn inline_determinism_json_identical(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        pat in safe_pattern_strategy(),
        tok in text_token_strategy(),
    ) {
        let src = enum_with_seq_variant_src(&name, &ty, &pat, &tok);
        let g1 = extract_one(&src);
        let g2 = extract_one(&src);
        let j1 = serde_json::to_string(&g1).unwrap();
        let j2 = serde_json::to_string(&g2).unwrap();
        prop_assert_eq!(j1, j2, "JSON serialization must be deterministic");
    }
}

// ===========================================================================
// 29. Inline rules from Optional: CHOICE/BLANK wraps inside inlined SEQ
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn optional_field_choice_blank_in_inlined(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        pat in safe_pattern_strategy(),
    ) {
        let src = enum_optional_variant_src(&name, &ty, &pat);
        let g = extract_one(&src);
        let blank_nodes = collect_nodes_of_type(&g["rules"][&ty], "BLANK");
        prop_assert!(!blank_nodes.is_empty(), "optional field should produce BLANK node");
    }
}

// ===========================================================================
// 30. Repeat inlined variant: FIELD wraps the SYMBOL reference
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    #[test]
    fn repeat_inlined_variant_has_field(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
    ) {
        let src = enum_repeat_variant_src(&name, &ty);
        let g = extract_one(&src);
        let members = g["rules"][&ty]["members"].as_array().unwrap();
        // Nums may be represented by an explicit generated wrapper so its
        // variant identity survives generated reduce/reduce conflicts.
        let nums = member_rule(&g, &members[0]);
        let field_nodes = collect_nodes_of_type(nums, "FIELD");
        prop_assert!(!field_nodes.is_empty(), "repeat variant should have FIELD nodes");
    }
}

// ===========================================================================
// 31. Dedup: no intermediate rules for any of the duplicate variants
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn dedup_no_intermediate_rules(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        pat in safe_pattern_strategy(),
    ) {
        let src = enum_dedup_src(&name, &ty, &pat);
        let g = extract_one(&src);
        let names = rule_names(&g);
        for variant in &["A", "B", "C"] {
            let rule = format!("{ty}_{variant}");
            prop_assert!(!names.contains(&rule), "dedup variant {rule} should not have named rule");
        }
    }
}

// ===========================================================================
// 32. Inlined SEQ variant pattern value matches input
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn inlined_seq_pattern_matches_input(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        pat in safe_pattern_strategy(),
        tok in text_token_strategy(),
    ) {
        let src = enum_with_seq_variant_src(&name, &ty, &pat, &tok);
        let g = extract_one(&src);
        let _members = g["rules"][&ty]["members"].as_array().unwrap();
        // First variant is SEQ; FIELD nodes reference named rules that have the PATTERN
        // Check the full grammar for the pattern and string values
        let all_patterns = collect_nodes_of_type(&g["rules"], "PATTERN");
        let has_input_pattern = all_patterns.iter().any(|p| p["value"].as_str() == Some(&pat));
        prop_assert!(has_input_pattern, "grammar should contain the input pattern");
        let all_strings = collect_nodes_of_type(&g["rules"], "STRING");
        let has_input_string = all_strings.iter().any(|s| s["value"].as_str() == Some(&tok));
        prop_assert!(has_input_string, "grammar should contain the input string");
    }
}

// ===========================================================================
// 33. Inline single-leaf pattern value matches input
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(15))]

    #[test]
    fn inlined_leaf_pattern_value_matches(
        name in grammar_name_strategy(),
        ty in type_name_strategy(),
        p1 in safe_pattern_strategy(),
        p2 in safe_pattern_strategy(),
    ) {
        let src = enum_two_leaf_src(&name, &ty, &p1, &p2);
        let g = extract_one(&src);
        let members = g["rules"][&ty]["members"].as_array().unwrap();
        prop_assert_eq!(members[0]["value"].as_str().unwrap(), p1.as_str());
        prop_assert_eq!(members[1]["value"].as_str().unwrap(), p2.as_str());
    }
}
