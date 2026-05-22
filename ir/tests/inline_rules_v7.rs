//! Tests for inline rules and supertypes in adze-ir.
//!
//! Categories (8 × 8 = 64 tests):
//!   inline_basic_*       – fundamental inline rule behaviour
//!   inline_multiple_*    – multiple inline rules interacting
//!   inline_normalize_*   – inline rules survive grammar transforms
//!   supertype_basic_*    – fundamental supertype behaviour
//!   supertype_combined_* – supertypes combined with other features
//!   inline_validate_*    – validation-adjacent inline scenarios
//!   inline_serialize_*   – serialization round-trips for inline/supertype data
//!   inline_edge_*        – edge cases and boundary conditions

use adze_ir::builder::GrammarBuilder;
use adze_ir::{Grammar, SymbolId};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a small grammar with two tokens and a start rule.
fn base_builder(name: &str) -> GrammarBuilder {
    GrammarBuilder::new(name)
        .token("ID", r"[a-z]+")
        .token("NUM", r"[0-9]+")
        .token(";", ";")
        .rule("program", vec!["stmt"])
        .start("program")
}

/// Resolve a symbol name to its SymbolId.
fn sym(g: &Grammar, name: &str) -> SymbolId {
    g.find_symbol_by_name(name)
        .unwrap_or_else(|| panic!("symbol `{name}` not found"))
}

// ===========================================================================
// 1. inline_basic_* (8 tests)
// ===========================================================================

#[test]
fn inline_basic_empty_by_default() {
    let g = base_builder("ib1").rule("stmt", vec!["ID", ";"]).build();
    assert!(g.inline_rules.is_empty());
}

#[test]
fn inline_basic_single_rule() {
    let g = base_builder("ib2")
        .rule("stmt", vec!["ID", ";"])
        .rule("helper", vec!["ID"])
        .inline("helper")
        .build();
    assert_eq!(g.inline_rules.len(), 1);
    assert_eq!(g.inline_rules[0], sym(&g, "helper"));
}

#[test]
fn inline_basic_does_not_remove_from_rules() {
    let g = base_builder("ib3")
        .rule("stmt", vec!["ID", ";"])
        .rule("helper", vec!["ID"])
        .inline("helper")
        .build();
    assert!(g.rules.contains_key(&sym(&g, "helper")));
}

#[test]
fn inline_basic_preserves_start() {
    let g = base_builder("ib4")
        .rule("stmt", vec!["helper"])
        .rule("helper", vec!["ID", ";"])
        .inline("helper")
        .build();
    // start symbol's rules should still exist
    assert!(g.rules.contains_key(&sym(&g, "program")));
}

#[test]
fn inline_basic_symbol_id_matches() {
    let g = base_builder("ib5")
        .rule("stmt", vec!["ID", ";"])
        .rule("wrapper", vec!["ID"])
        .inline("wrapper")
        .build();
    let wrapper_id = sym(&g, "wrapper");
    assert!(g.inline_rules.contains(&wrapper_id));
}

#[test]
fn inline_basic_no_duplicate_on_single_call() {
    let g = base_builder("ib6")
        .rule("stmt", vec!["ID", ";"])
        .rule("node", vec!["ID"])
        .inline("node")
        .build();
    assert_eq!(
        g.inline_rules
            .iter()
            .filter(|&&id| id == sym(&g, "node"))
            .count(),
        1
    );
}

#[test]
fn inline_basic_rule_names_retained() {
    let g = base_builder("ib7")
        .rule("stmt", vec!["ID", ";"])
        .rule("aux", vec!["ID"])
        .inline("aux")
        .build();
    assert!(g.rule_names.values().any(|n| n == "aux"));
}

#[test]
fn inline_basic_grammar_name_unaffected() {
    let g = base_builder("ib8")
        .rule("stmt", vec!["ID", ";"])
        .rule("aux", vec!["NUM"])
        .inline("aux")
        .build();
    assert_eq!(g.name, "ib8");
}

// ===========================================================================
// 2. inline_multiple_* (8 tests)
// ===========================================================================

#[test]
fn inline_multiple_two_rules() {
    let g = base_builder("im1")
        .rule("stmt", vec!["ID", ";"])
        .rule("alpha", vec!["ID"])
        .rule("beta", vec!["NUM"])
        .inline("alpha")
        .inline("beta")
        .build();
    assert_eq!(g.inline_rules.len(), 2);
}

#[test]
fn inline_multiple_preserves_order() {
    let g = base_builder("im2")
        .rule("stmt", vec!["ID", ";"])
        .rule("first", vec!["ID"])
        .rule("second", vec!["NUM"])
        .inline("first")
        .inline("second")
        .build();
    assert_eq!(g.inline_rules[0], sym(&g, "first"));
    assert_eq!(g.inline_rules[1], sym(&g, "second"));
}

#[test]
fn inline_multiple_three_rules() {
    let g = base_builder("im3")
        .rule("stmt", vec!["ID", ";"])
        .rule("aa", vec!["ID"])
        .rule("bb", vec!["NUM"])
        .rule("cc", vec!["ID"])
        .inline("aa")
        .inline("bb")
        .inline("cc")
        .build();
    assert_eq!(g.inline_rules.len(), 3);
}

#[test]
fn inline_multiple_each_distinct() {
    let g = base_builder("im4")
        .rule("stmt", vec!["ID", ";"])
        .rule("rr", vec!["ID"])
        .rule("ss", vec!["NUM"])
        .inline("rr")
        .inline("ss")
        .build();
    let rr = sym(&g, "rr");
    let ss = sym(&g, "ss");
    assert_ne!(rr, ss);
    assert!(g.inline_rules.contains(&rr));
    assert!(g.inline_rules.contains(&ss));
}

#[test]
fn inline_multiple_does_not_affect_tokens() {
    let g = base_builder("im5")
        .rule("stmt", vec!["ID", ";"])
        .rule("pp", vec!["ID"])
        .rule("qq", vec!["NUM"])
        .inline("pp")
        .inline("qq")
        .build();
    // tokens should still exist
    assert!(!g.tokens.is_empty());
}

#[test]
fn inline_multiple_rules_map_unchanged() {
    let g_without = base_builder("im6a")
        .rule("stmt", vec!["ID", ";"])
        .rule("pp", vec!["ID"])
        .rule("qq", vec!["NUM"])
        .build();
    let g_with = base_builder("im6b")
        .rule("stmt", vec!["ID", ";"])
        .rule("pp", vec!["ID"])
        .rule("qq", vec!["NUM"])
        .inline("pp")
        .inline("qq")
        .build();
    assert_eq!(g_without.rules.len(), g_with.rules.len());
}

#[test]
fn inline_multiple_with_shared_rhs_symbol() {
    let g = base_builder("im7")
        .rule("stmt", vec!["aa", "bb"])
        .rule("aa", vec!["ID"])
        .rule("bb", vec!["ID"])
        .inline("aa")
        .inline("bb")
        .build();
    assert_eq!(g.inline_rules.len(), 2);
}

#[test]
fn inline_multiple_subset_check() {
    let g = base_builder("im8")
        .rule("stmt", vec!["ID", ";"])
        .rule("xx", vec!["ID"])
        .rule("yy", vec!["NUM"])
        .rule("zz", vec!["ID"])
        .inline("xx")
        .inline("zz")
        .build();
    assert!(g.inline_rules.contains(&sym(&g, "xx")));
    assert!(!g.inline_rules.contains(&sym(&g, "yy")));
    assert!(g.inline_rules.contains(&sym(&g, "zz")));
}

// ===========================================================================
// 3. inline_normalize_* (8 tests)
// ===========================================================================

#[test]
fn inline_normalize_survives_clone() {
    let g = base_builder("in1")
        .rule("stmt", vec!["ID", ";"])
        .rule("hlp", vec!["ID"])
        .inline("hlp")
        .build();
    let g2 = g.clone();
    assert_eq!(g.inline_rules, g2.inline_rules);
}

#[test]
fn inline_normalize_debug_format() {
    let g = base_builder("in2")
        .rule("stmt", vec!["ID", ";"])
        .rule("hlp", vec!["ID"])
        .inline("hlp")
        .build();
    let dbg = format!("{:?}", g.inline_rules);
    assert!(!dbg.is_empty());
}

#[test]
fn inline_normalize_equality() {
    let g1 = base_builder("in3")
        .rule("stmt", vec!["ID", ";"])
        .rule("hlp", vec!["ID"])
        .inline("hlp")
        .build();
    let g2 = base_builder("in3")
        .rule("stmt", vec!["ID", ";"])
        .rule("hlp", vec!["ID"])
        .inline("hlp")
        .build();
    assert_eq!(g1.inline_rules, g2.inline_rules);
}

#[test]
fn inline_normalize_not_in_extras() {
    let g = base_builder("in4")
        .rule("stmt", vec!["ID", ";"])
        .rule("hlp", vec!["ID"])
        .inline("hlp")
        .build();
    for id in &g.inline_rules {
        assert!(!g.extras.contains(id));
    }
}

#[test]
fn inline_normalize_not_in_supertypes() {
    let g = base_builder("in5")
        .rule("stmt", vec!["ID", ";"])
        .rule("hlp", vec!["ID"])
        .inline("hlp")
        .build();
    for id in &g.inline_rules {
        assert!(!g.supertypes.contains(id));
    }
}

#[test]
fn inline_normalize_stable_across_builds() {
    let make = || {
        base_builder("in6")
            .rule("stmt", vec!["ID", ";"])
            .rule("hlp", vec!["ID"])
            .inline("hlp")
            .build()
    };
    let a = make();
    let b = make();
    assert_eq!(a.inline_rules, b.inline_rules);
}

#[test]
fn inline_normalize_all_rules_still_accessible() {
    let g = base_builder("in7")
        .rule("stmt", vec!["ID", ";"])
        .rule("hlp", vec!["ID"])
        .inline("hlp")
        .build();
    let all: Vec<_> = g.all_rules().collect();
    assert!(!all.is_empty());
}

#[test]
fn inline_normalize_find_symbol_works() {
    let g = base_builder("in8")
        .rule("stmt", vec!["ID", ";"])
        .rule("hlp", vec!["ID"])
        .inline("hlp")
        .build();
    assert!(g.find_symbol_by_name("hlp").is_some());
}

// ===========================================================================
// 4. supertype_basic_* (8 tests)
// ===========================================================================

#[test]
fn supertype_basic_empty_by_default() {
    let g = base_builder("sb1").rule("stmt", vec!["ID", ";"]).build();
    assert!(g.supertypes.is_empty());
}

#[test]
fn supertype_basic_single() {
    let g = base_builder("sb2")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .supertype("expr")
        .build();
    assert_eq!(g.supertypes.len(), 1);
    assert_eq!(g.supertypes[0], sym(&g, "expr"));
}

#[test]
fn supertype_basic_does_not_remove_from_rules() {
    let g = base_builder("sb3")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .supertype("expr")
        .build();
    assert!(g.rules.contains_key(&sym(&g, "expr")));
}

#[test]
fn supertype_basic_preserves_start() {
    let g = base_builder("sb4")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .supertype("expr")
        .build();
    assert!(g.rules.contains_key(&sym(&g, "program")));
}

#[test]
fn supertype_basic_multiple() {
    let g = base_builder("sb5")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .rule("decl", vec!["ID", ";"])
        .supertype("expr")
        .supertype("decl")
        .build();
    assert_eq!(g.supertypes.len(), 2);
}

#[test]
fn supertype_basic_order_preserved() {
    let g = base_builder("sb6")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .rule("decl", vec!["ID", ";"])
        .supertype("decl")
        .supertype("expr")
        .build();
    assert_eq!(g.supertypes[0], sym(&g, "decl"));
    assert_eq!(g.supertypes[1], sym(&g, "expr"));
}

#[test]
fn supertype_basic_id_matches() {
    let g = base_builder("sb7")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .supertype("expr")
        .build();
    let expr_id = sym(&g, "expr");
    assert!(g.supertypes.contains(&expr_id));
}

#[test]
fn supertype_basic_rule_names_retained() {
    let g = base_builder("sb8")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .supertype("expr")
        .build();
    assert!(g.rule_names.values().any(|n| n == "expr"));
}

// ===========================================================================
// 5. supertype_combined_* (8 tests)
// ===========================================================================

#[test]
fn supertype_combined_with_inline() {
    let g = base_builder("sc1")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .rule("hlp", vec!["NUM"])
        .supertype("expr")
        .inline("hlp")
        .build();
    assert_eq!(g.supertypes.len(), 1);
    assert_eq!(g.inline_rules.len(), 1);
}

#[test]
fn supertype_combined_disjoint_sets() {
    let g = base_builder("sc2")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .rule("hlp", vec!["NUM"])
        .supertype("expr")
        .inline("hlp")
        .build();
    for id in &g.supertypes {
        assert!(!g.inline_rules.contains(id));
    }
    for id in &g.inline_rules {
        assert!(!g.supertypes.contains(id));
    }
}

#[test]
fn supertype_combined_multiple_of_each() {
    let g = base_builder("sc3")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .rule("decl", vec!["NUM"])
        .rule("hlp", vec!["ID"])
        .rule("aux", vec!["NUM"])
        .supertype("expr")
        .supertype("decl")
        .inline("hlp")
        .inline("aux")
        .build();
    assert_eq!(g.supertypes.len(), 2);
    assert_eq!(g.inline_rules.len(), 2);
}

#[test]
fn supertype_combined_with_extras() {
    let g = base_builder("sc4")
        .token("WS", r"[ \t]+")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .extra("WS")
        .supertype("expr")
        .build();
    assert!(!g.extras.is_empty());
    assert!(!g.supertypes.is_empty());
}

#[test]
fn supertype_combined_with_externals() {
    let g = base_builder("sc5")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .external("INDENT")
        .supertype("expr")
        .build();
    assert!(!g.externals.is_empty());
    assert!(!g.supertypes.is_empty());
}

#[test]
fn supertype_combined_with_precedence() {
    let g = GrammarBuilder::new("sc6")
        .token("ID", r"[a-z]+")
        .token("+", "+")
        .token("*", "*")
        .rule_with_precedence(
            "expr",
            vec!["expr", "+", "expr"],
            1,
            adze_ir::Associativity::Left,
        )
        .rule_with_precedence(
            "expr",
            vec!["expr", "*", "expr"],
            2,
            adze_ir::Associativity::Left,
        )
        .rule("expr", vec!["ID"])
        .supertype("expr")
        .start("expr")
        .build();
    assert!(!g.supertypes.is_empty());
    let expr_rules = g.rules.get(&sym(&g, "expr")).unwrap();
    assert!(expr_rules.len() >= 3);
}

#[test]
fn supertype_combined_three_supertypes_one_inline() {
    let g = base_builder("sc7")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .rule("decl", vec!["NUM"])
        .rule("lit", vec!["NUM"])
        .rule("hlp", vec!["ID"])
        .supertype("expr")
        .supertype("decl")
        .supertype("lit")
        .inline("hlp")
        .build();
    assert_eq!(g.supertypes.len(), 3);
    assert_eq!(g.inline_rules.len(), 1);
}

#[test]
fn supertype_combined_counts_correct() {
    let g = base_builder("sc8")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .rule("hlp", vec!["NUM"])
        .supertype("expr")
        .inline("hlp")
        .build();
    let total = g.supertypes.len() + g.inline_rules.len();
    assert_eq!(total, 2);
}

// ===========================================================================
// 6. inline_validate_* (8 tests)
// ===========================================================================

#[test]
fn inline_validate_symbol_exists_in_rule_names() {
    let g = base_builder("iv1")
        .rule("stmt", vec!["ID", ";"])
        .rule("hlp", vec!["ID"])
        .inline("hlp")
        .build();
    for id in &g.inline_rules {
        assert!(g.rule_names.contains_key(id));
    }
}

#[test]
fn inline_validate_all_ids_positive() {
    let g = base_builder("iv2")
        .rule("stmt", vec!["ID", ";"])
        .rule("hlp", vec!["ID"])
        .inline("hlp")
        .build();
    for id in &g.inline_rules {
        assert!(id.0 > 0);
    }
}

#[test]
fn inline_validate_supertype_ids_positive() {
    let g = base_builder("iv3")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .supertype("expr")
        .build();
    for id in &g.supertypes {
        assert!(id.0 > 0);
    }
}

#[test]
fn inline_validate_inline_has_rules() {
    let g = base_builder("iv4")
        .rule("stmt", vec!["ID", ";"])
        .rule("hlp", vec!["ID"])
        .inline("hlp")
        .build();
    for id in &g.inline_rules {
        assert!(g.rules.contains_key(id));
    }
}

#[test]
fn inline_validate_supertype_has_rules() {
    let g = base_builder("iv5")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .supertype("expr")
        .build();
    for id in &g.supertypes {
        assert!(g.rules.contains_key(id));
    }
}

#[test]
fn inline_validate_no_overlap_with_tokens() {
    let g = base_builder("iv6")
        .rule("stmt", vec!["ID", ";"])
        .rule("hlp", vec!["ID"])
        .inline("hlp")
        .build();
    for id in &g.inline_rules {
        assert!(!g.tokens.contains_key(id));
    }
}

#[test]
fn inline_validate_supertype_not_token() {
    let g = base_builder("iv7")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .supertype("expr")
        .build();
    for id in &g.supertypes {
        assert!(!g.tokens.contains_key(id));
    }
}

#[test]
fn inline_validate_empty_terminals_ok() {
    let g = base_builder("iv8")
        .rule("stmt", vec!["ID", ";"])
        .rule("hlp", vec!["ID"])
        .inline("hlp")
        .build();
    assert!(g.check_empty_terminals().is_ok());
}

// ===========================================================================
// 7. inline_serialize_* (8 tests)
// ===========================================================================

#[test]
fn inline_serialize_json_roundtrip() {
    let g = base_builder("is1")
        .rule("stmt", vec!["ID", ";"])
        .rule("hlp", vec!["ID"])
        .inline("hlp")
        .build();
    let json = serde_json::to_string(&g).unwrap();
    let g2: Grammar = serde_json::from_str(&json).unwrap();
    assert_eq!(g.inline_rules, g2.inline_rules);
}

#[test]
fn inline_serialize_supertype_roundtrip() {
    let g = base_builder("is2")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .supertype("expr")
        .build();
    let json = serde_json::to_string(&g).unwrap();
    let g2: Grammar = serde_json::from_str(&json).unwrap();
    assert_eq!(g.supertypes, g2.supertypes);
}

#[test]
fn inline_serialize_both_roundtrip() {
    let g = base_builder("is3")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .rule("hlp", vec!["NUM"])
        .supertype("expr")
        .inline("hlp")
        .build();
    let json = serde_json::to_string(&g).unwrap();
    let g2: Grammar = serde_json::from_str(&json).unwrap();
    assert_eq!(g.inline_rules, g2.inline_rules);
    assert_eq!(g.supertypes, g2.supertypes);
}

#[test]
fn inline_serialize_empty_inline_roundtrip() {
    let g = base_builder("is4").rule("stmt", vec!["ID", ";"]).build();
    let json = serde_json::to_string(&g).unwrap();
    let g2: Grammar = serde_json::from_str(&json).unwrap();
    assert!(g2.inline_rules.is_empty());
}

#[test]
fn inline_serialize_empty_supertypes_roundtrip() {
    let g = base_builder("is5").rule("stmt", vec!["ID", ";"]).build();
    let json = serde_json::to_string(&g).unwrap();
    let g2: Grammar = serde_json::from_str(&json).unwrap();
    assert!(g2.supertypes.is_empty());
}

#[test]
fn inline_serialize_json_contains_field() {
    let g = base_builder("is6")
        .rule("stmt", vec!["ID", ";"])
        .rule("hlp", vec!["ID"])
        .inline("hlp")
        .build();
    let json = serde_json::to_string(&g).unwrap();
    assert!(json.contains("inline_rules"));
}

#[test]
fn inline_serialize_json_contains_supertypes_field() {
    let g = base_builder("is7")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .supertype("expr")
        .build();
    let json = serde_json::to_string(&g).unwrap();
    assert!(json.contains("supertypes"));
}

#[test]
fn inline_serialize_pretty_json_roundtrip() {
    let g = base_builder("is8")
        .rule("stmt", vec!["ID", ";"])
        .rule("hlp", vec!["ID"])
        .rule("expr", vec!["ID"])
        .inline("hlp")
        .supertype("expr")
        .build();
    let json = serde_json::to_string_pretty(&g).unwrap();
    let g2: Grammar = serde_json::from_str(&json).unwrap();
    assert_eq!(g.inline_rules, g2.inline_rules);
    assert_eq!(g.supertypes, g2.supertypes);
}

// ===========================================================================
// 8. inline_edge_* (8 tests)
// ===========================================================================

#[test]
fn inline_edge_inline_start_symbol() {
    let g = base_builder("ie1")
        .rule("stmt", vec!["ID", ";"])
        .inline("program")
        .build();
    assert!(g.inline_rules.contains(&sym(&g, "program")));
}

#[test]
fn inline_edge_supertype_start_symbol() {
    let g = base_builder("ie2")
        .rule("stmt", vec!["ID", ";"])
        .supertype("program")
        .build();
    assert!(g.supertypes.contains(&sym(&g, "program")));
}

#[test]
fn inline_edge_same_symbol_inline_twice() {
    let g = base_builder("ie3")
        .rule("stmt", vec!["ID", ";"])
        .rule("hlp", vec!["ID"])
        .inline("hlp")
        .inline("hlp")
        .build();
    // builder does not deduplicate — both entries present
    assert!(g.inline_rules.len() >= 2);
}

#[test]
fn inline_edge_same_symbol_supertype_twice() {
    let g = base_builder("ie4")
        .rule("stmt", vec!["ID", ";"])
        .rule("expr", vec!["ID"])
        .supertype("expr")
        .supertype("expr")
        .build();
    assert!(g.supertypes.len() >= 2);
}

#[test]
fn inline_edge_many_inline_rules() {
    let mut builder = base_builder("ie5").rule("stmt", vec!["ID", ";"]);
    for i in 0..10 {
        let name: String = format!("r{i}");
        // Leak the string so we get a &'static str for the builder API.
        let leaked: &'static str = Box::leak(name.into_boxed_str());
        builder = builder.rule(leaked, vec!["ID"]).inline(leaked);
    }
    let g = builder.build();
    assert_eq!(g.inline_rules.len(), 10);
}

#[test]
fn inline_edge_many_supertypes() {
    let mut builder = base_builder("ie6").rule("stmt", vec!["ID", ";"]);
    for i in 0..10 {
        let name: String = format!("s{i}");
        let leaked: &'static str = Box::leak(name.into_boxed_str());
        builder = builder.rule(leaked, vec!["ID"]).supertype(leaked);
    }
    let g = builder.build();
    assert_eq!(g.supertypes.len(), 10);
}

#[test]
fn inline_edge_inline_with_epsilon_rule() {
    let g = base_builder("ie7")
        .rule("stmt", vec!["ID", ";"])
        .rule("maybe", vec![])
        .inline("maybe")
        .build();
    assert!(g.inline_rules.contains(&sym(&g, "maybe")));
}

#[test]
fn inline_edge_default_grammar_has_no_inline_or_supertypes() {
    let g = Grammar::default();
    assert!(g.inline_rules.is_empty());
    assert!(g.supertypes.is_empty());
}
