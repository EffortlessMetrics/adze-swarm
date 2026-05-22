//! Property-based tests for Grammar merge/combination operations.
//!
//! Tests grammar merging semantics across 8 categories (6 tests each, 48 total):
//! tokens, rules, precedence, fields, extras, validate, normalize, idempotent.

use adze_ir::builder::GrammarBuilder;
use adze_ir::validation::GrammarValidator;
use adze_ir::{Associativity, FieldId, Grammar, Rule, Symbol, SymbolId};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

fn arb_grammar_name() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{2,8}".prop_map(|s| s)
}

fn arb_rule_name() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{2,8}".prop_map(|s| s)
}

fn arb_pattern() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-z]+",
        Just(r"\d+".to_string()),
        Just(r"[a-zA-Z_]+".to_string()),
        Just(r"[0-9]+".to_string()),
    ]
}

fn arb_precedence_level() -> impl Strategy<Value = i16> {
    -10i16..=10i16
}

fn arb_associativity() -> impl Strategy<Value = Associativity> {
    prop_oneof![
        Just(Associativity::Left),
        Just(Associativity::Right),
        Just(Associativity::None),
    ]
}

/// Build a simple grammar with `n` unique tokens and a start rule referencing them.
fn build_grammar_with_tokens(name: &str, token_names: &[String], patterns: &[String]) -> Grammar {
    let mut builder = GrammarBuilder::new(name);
    for (tname, pat) in token_names.iter().zip(patterns.iter()) {
        builder = builder.token(tname, pat);
    }
    if let Some(first_tok) = token_names.first() {
        builder = builder
            .rule("start", vec![first_tok.as_str()])
            .start("start");
    }
    builder.build()
}

/// Merge tokens from `src` into `dst`, skipping duplicates by name.
fn merge_tokens(dst: &mut Grammar, src: &Grammar) {
    let existing_names: std::collections::HashSet<String> =
        dst.tokens.values().map(|t| t.name.clone()).collect();
    let max_id = dst
        .tokens
        .keys()
        .chain(dst.rules.keys())
        .chain(dst.rule_names.keys())
        .map(|id| id.0)
        .max()
        .unwrap_or(0);
    let mut next_id = max_id + 1;
    for (_sid, token) in &src.tokens {
        if !existing_names.contains(&token.name) {
            let new_id = SymbolId(next_id);
            next_id += 1;
            dst.tokens.insert(new_id, token.clone());
        }
    }
}

/// Merge rules from `src` into `dst`, remapping symbol IDs by name.
fn merge_rules(dst: &mut Grammar, src: &Grammar) {
    let max_id = dst
        .tokens
        .keys()
        .chain(dst.rules.keys())
        .chain(dst.rule_names.keys())
        .map(|id| id.0)
        .max()
        .unwrap_or(0);
    let mut next_id = max_id + 1;

    // Build name->id map for dst
    let mut name_to_id: std::collections::HashMap<String, SymbolId> =
        std::collections::HashMap::new();
    for (id, name) in &dst.rule_names {
        name_to_id.insert(name.clone(), *id);
    }
    for (id, tok) in &dst.tokens {
        name_to_id.insert(tok.name.clone(), *id);
    }

    for (src_lhs, src_rules) in &src.rules {
        let lhs_name = src
            .rule_names
            .get(src_lhs)
            .cloned()
            .unwrap_or_else(|| format!("_anon_{}", src_lhs.0));
        let dst_lhs = *name_to_id.entry(lhs_name.clone()).or_insert_with(|| {
            let id = SymbolId(next_id);
            next_id += 1;
            dst.rule_names.insert(id, lhs_name.clone());
            id
        });
        for src_rule in src_rules {
            let new_rhs: Vec<Symbol> = src_rule
                .rhs
                .iter()
                .map(|sym| match sym {
                    Symbol::Terminal(tid) => {
                        let tname = src
                            .tokens
                            .get(tid)
                            .map(|t| t.name.clone())
                            .unwrap_or_else(|| format!("_tok_{}", tid.0));
                        let mapped = *name_to_id.entry(tname).or_insert_with(|| {
                            let id = SymbolId(next_id);
                            next_id += 1;
                            id
                        });
                        Symbol::Terminal(mapped)
                    }
                    Symbol::NonTerminal(nid) => {
                        let nname = src
                            .rule_names
                            .get(nid)
                            .cloned()
                            .unwrap_or_else(|| format!("_nt_{}", nid.0));
                        let mapped = *name_to_id.entry(nname.clone()).or_insert_with(|| {
                            let id = SymbolId(next_id);
                            next_id += 1;
                            dst.rule_names.insert(id, nname);
                            id
                        });
                        Symbol::NonTerminal(mapped)
                    }
                    other => other.clone(),
                })
                .collect();
            let merged_rule = Rule {
                lhs: dst_lhs,
                rhs: new_rhs,
                precedence: src_rule.precedence,
                associativity: src_rule.associativity,
                fields: src_rule.fields.clone(),
                production_id: src_rule.production_id,
            };
            dst.rules.entry(dst_lhs).or_default().push(merged_rule);
        }
    }
}

/// Merge precedence declarations from `src` into `dst`.
fn merge_precedences(dst: &mut Grammar, src: &Grammar) {
    let existing_levels: std::collections::HashSet<i16> =
        dst.precedences.iter().map(|p| p.level).collect();
    for prec in &src.precedences {
        if !existing_levels.contains(&prec.level) {
            dst.precedences.push(prec.clone());
        }
    }
}

/// Merge fields from `src` into `dst`, skipping duplicates by name.
fn merge_fields(dst: &mut Grammar, src: &Grammar) {
    let existing: std::collections::HashSet<String> = dst.fields.values().cloned().collect();
    let max_field_id = dst.fields.keys().map(|f| f.0).max().unwrap_or(0);
    let mut next_fid = max_field_id + 1;
    for (_fid, fname) in &src.fields {
        if !existing.contains(fname) {
            dst.fields.insert(FieldId(next_fid), fname.clone());
            next_fid += 1;
        }
    }
}

/// Merge extras from `src` into `dst`, skipping duplicates.
fn merge_extras_into(dst: &mut Grammar, src: &Grammar) {
    // Merge extras by token name to avoid ID clashes
    let dst_extra_names: std::collections::HashSet<String> = dst
        .extras
        .iter()
        .filter_map(|id| dst.tokens.get(id).map(|t| t.name.clone()))
        .chain(
            dst.extras
                .iter()
                .filter_map(|id| dst.rule_names.get(id).cloned()),
        )
        .collect();

    for extra_id in &src.extras {
        let src_name = src
            .tokens
            .get(extra_id)
            .map(|t| t.name.clone())
            .or_else(|| src.rule_names.get(extra_id).cloned());
        if let Some(name) = src_name
            && !dst_extra_names.contains(&name)
        {
            // Find corresponding ID in dst
            if let Some((did, _)) = dst.tokens.iter().find(|(_, t)| t.name == name) {
                dst.extras.push(*did);
            }
        }
    }
}

/// Create a minimal valid grammar for merging tests.
fn minimal_grammar(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUM", r"\d+")
        .rule("root", vec!["NUM"])
        .start("root")
        .build()
}

// ---------------------------------------------------------------------------
// Category 1: prop_merge_tokens_*
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, ..Default::default() })]

    #[test]
    fn prop_merge_tokens_disjoint_union(
        name_a in arb_grammar_name(),
        name_b in arb_grammar_name(),
    ) {
        let ga = GrammarBuilder::new(&name_a)
            .token("ALPHA", "[a-z]+")
            .rule("sa", vec!["ALPHA"])
            .start("sa")
            .build();
        let gb = GrammarBuilder::new(&name_b)
            .token("DIGIT", r"\d+")
            .rule("sb", vec!["DIGIT"])
            .start("sb")
            .build();
        let mut merged = ga.clone();
        merge_tokens(&mut merged, &gb);
        // Both tokens must be present
        let names: Vec<String> = merged.tokens.values().map(|t| t.name.clone()).collect();
        prop_assert!(names.contains(&"ALPHA".to_string()));
        prop_assert!(names.contains(&"DIGIT".to_string()));
    }

    #[test]
    fn prop_merge_tokens_no_loss(
        tok_count_a in 1usize..=4,
        tok_count_b in 1usize..=4,
    ) {
        let token_names_a: Vec<String> = (0..tok_count_a).map(|i| format!("TA{}", i)).collect();
        let patterns_a: Vec<String> = (0..tok_count_a).map(|i| format!("a{}", i)).collect();
        let token_names_b: Vec<String> = (0..tok_count_b).map(|i| format!("TB{}", i)).collect();
        let patterns_b: Vec<String> = (0..tok_count_b).map(|i| format!("b{}", i)).collect();
        let ga = build_grammar_with_tokens("ga", &token_names_a, &patterns_a);
        let gb = build_grammar_with_tokens("gb", &token_names_b, &patterns_b);
        let mut merged = ga.clone();
        merge_tokens(&mut merged, &gb);
        // All original tokens preserved, plus new ones
        prop_assert!(merged.tokens.len() >= ga.tokens.len());
        prop_assert!(merged.tokens.len() <= ga.tokens.len() + gb.tokens.len());
    }

    #[test]
    fn prop_merge_tokens_duplicate_skipped(
        name in arb_grammar_name(),
    ) {
        let ga = GrammarBuilder::new(&name)
            .token("TOK", "abc")
            .rule("sa", vec!["TOK"])
            .start("sa")
            .build();
        let gb = GrammarBuilder::new("other")
            .token("TOK", "xyz")
            .rule("sb", vec!["TOK"])
            .start("sb")
            .build();
        let mut merged = ga.clone();
        let original_count = merged.tokens.len();
        merge_tokens(&mut merged, &gb);
        // Duplicate name should be skipped
        prop_assert_eq!(merged.tokens.len(), original_count);
    }

    #[test]
    fn prop_merge_tokens_preserves_pattern(
        pat in arb_pattern(),
    ) {
        let ga = GrammarBuilder::new("pa")
            .token("ORIG", &pat)
            .rule("sa", vec!["ORIG"])
            .start("sa")
            .build();
        let gb = Grammar::new("empty".to_string());
        let mut merged = ga.clone();
        merge_tokens(&mut merged, &gb);
        let orig_tok = merged.tokens.values().find(|t| t.name == "ORIG").unwrap();
        let ga_tok = ga.tokens.values().find(|t| t.name == "ORIG").unwrap();
        prop_assert_eq!(&orig_tok.pattern, &ga_tok.pattern);
    }

    #[test]
    fn prop_merge_tokens_empty_src(
        name in arb_grammar_name(),
    ) {
        let ga = GrammarBuilder::new(&name)
            .token("FOO", "foo")
            .rule("sa", vec!["FOO"])
            .start("sa")
            .build();
        let empty = Grammar::new("empty".to_string());
        let mut merged = ga.clone();
        merge_tokens(&mut merged, &empty);
        prop_assert_eq!(merged.tokens.len(), ga.tokens.len());
    }

    #[test]
    fn prop_merge_tokens_into_empty_dst(
        name in arb_grammar_name(),
    ) {
        let mut dst = Grammar::new("dst".to_string());
        let src = GrammarBuilder::new(&name)
            .token("BAR", "bar")
            .rule("sb", vec!["BAR"])
            .start("sb")
            .build();
        merge_tokens(&mut dst, &src);
        prop_assert!(!dst.tokens.is_empty());
        let names: Vec<String> = dst.tokens.values().map(|t| t.name.clone()).collect();
        prop_assert!(names.contains(&"BAR".to_string()));
    }
}

// ---------------------------------------------------------------------------
// Category 2: prop_merge_rules_*
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, ..Default::default() })]

    #[test]
    fn prop_merge_rules_union(
        name in arb_grammar_name(),
    ) {
        let ga = GrammarBuilder::new(&name)
            .token("NUM", r"\d+")
            .rule("expr", vec!["NUM"])
            .start("expr")
            .build();
        let gb = GrammarBuilder::new("gb")
            .token("NUM", r"\d+")
            .token("IDENT", "[a-z]+")
            .rule("expr", vec!["IDENT"])
            .start("expr")
            .build();
        let mut merged = ga.clone();
        merge_tokens(&mut merged, &gb);
        merge_rules(&mut merged, &gb);
        // Merged grammar should have rules for expr
        let has_expr = merged.rule_names.values().any(|n| n == "expr");
        prop_assert!(has_expr);
    }

    #[test]
    fn prop_merge_rules_preserves_original(
        name in arb_grammar_name(),
    ) {
        let ga = GrammarBuilder::new(&name)
            .token("NUM", r"\d+")
            .rule("val", vec!["NUM"])
            .start("val")
            .build();
        let original_rule_count: usize = ga.rules.values().map(|v| v.len()).sum();
        let empty = Grammar::new("empty".to_string());
        let mut merged = ga.clone();
        merge_rules(&mut merged, &empty);
        let merged_rule_count: usize = merged.rules.values().map(|v| v.len()).sum();
        prop_assert_eq!(merged_rule_count, original_rule_count);
    }

    #[test]
    fn prop_merge_rules_multiple_alternatives(
        name in arb_grammar_name(),
    ) {
        let ga = GrammarBuilder::new(&name)
            .token("A", "aaa")
            .token("B", "bbb")
            .rule("item", vec!["A"])
            .rule("item", vec!["B"])
            .start("item")
            .build();
        let gb = GrammarBuilder::new("gb")
            .token("C", "ccc")
            .rule("item", vec!["C"])
            .start("item")
            .build();
        let mut merged = ga.clone();
        merge_tokens(&mut merged, &gb);
        merge_rules(&mut merged, &gb);
        // Should have at least 3 alternatives (2 original + 1 merged)
        let total_rules: usize = merged.rules.values().map(|v| v.len()).sum();
        prop_assert!(total_rules >= 3);
    }

    #[test]
    fn prop_merge_rules_empty_rhs(
        name in arb_grammar_name(),
    ) {
        // Epsilon rules should survive merging
        let ga = GrammarBuilder::new(&name)
            .rule("opt", vec![])
            .start("opt")
            .build();
        let gb = GrammarBuilder::new("gb")
            .token("X", "xxx")
            .rule("opt", vec!["X"])
            .start("opt")
            .build();
        let mut merged = ga.clone();
        merge_tokens(&mut merged, &gb);
        merge_rules(&mut merged, &gb);
        let total_rules: usize = merged.rules.values().map(|v| v.len()).sum();
        prop_assert!(total_rules >= 2);
    }

    #[test]
    fn prop_merge_rules_disjoint_nonterminals(
        name_a in arb_rule_name(),
        name_b in arb_rule_name(),
    ) {
        prop_assume!(name_a != name_b);
        let ga = GrammarBuilder::new("ga")
            .token("T1", "ttt")
            .rule(&name_a, vec!["T1"])
            .start(&name_a)
            .build();
        let gb = GrammarBuilder::new("gb")
            .token("T2", "uuu")
            .rule(&name_b, vec!["T2"])
            .start(&name_b)
            .build();
        let mut merged = ga.clone();
        merge_tokens(&mut merged, &gb);
        merge_rules(&mut merged, &gb);
        let all_names: Vec<String> = merged.rule_names.values().cloned().collect();
        prop_assert!(all_names.contains(&name_a));
        prop_assert!(all_names.contains(&name_b));
    }

    #[test]
    fn prop_merge_rules_no_rule_loss(
        count_a in 1usize..=3,
        count_b in 1usize..=3,
    ) {
        let mut ba = GrammarBuilder::new("ga").token("V", "vvv");
        for i in 0..count_a {
            let rule_name = format!("ra{}", i);
            // Leak the string so we can use &str references
            let leaked: &'static str = Box::leak(rule_name.into_boxed_str());
            ba = ba.rule(leaked, vec!["V"]);
        }
        ba = ba.start("ra0");
        let ga = ba.build();

        let mut bb = GrammarBuilder::new("gb").token("W", "www");
        for i in 0..count_b {
            let rule_name = format!("rb{}", i);
            let leaked: &'static str = Box::leak(rule_name.into_boxed_str());
            bb = bb.rule(leaked, vec!["W"]);
        }
        bb = bb.start("rb0");
        let gb = bb.build();

        let ga_count: usize = ga.rules.values().map(|v| v.len()).sum();
        let gb_count: usize = gb.rules.values().map(|v| v.len()).sum();
        let mut merged = ga.clone();
        merge_tokens(&mut merged, &gb);
        merge_rules(&mut merged, &gb);
        let merged_count: usize = merged.rules.values().map(|v| v.len()).sum();
        prop_assert!(merged_count >= ga_count);
        prop_assert!(merged_count <= ga_count + gb_count);
    }
}

// ---------------------------------------------------------------------------
// Category 3: prop_merge_precedence_*
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, ..Default::default() })]

    #[test]
    fn prop_merge_precedence_disjoint_levels(
        level_a in arb_precedence_level(),
        level_b in arb_precedence_level(),
    ) {
        prop_assume!(level_a != level_b);
        let ga = GrammarBuilder::new("ga")
            .token("X", "xxx")
            .rule("ea", vec!["X"])
            .precedence(level_a, Associativity::Left, vec!["ea"])
            .start("ea")
            .build();
        let gb = GrammarBuilder::new("gb")
            .token("Y", "yyy")
            .rule("eb", vec!["Y"])
            .precedence(level_b, Associativity::Right, vec!["eb"])
            .start("eb")
            .build();
        let mut merged = ga.clone();
        merge_precedences(&mut merged, &gb);
        let levels: Vec<i16> = merged.precedences.iter().map(|p| p.level).collect();
        prop_assert!(levels.contains(&level_a));
        prop_assert!(levels.contains(&level_b));
    }

    #[test]
    fn prop_merge_precedence_same_level_skipped(
        level in arb_precedence_level(),
    ) {
        let ga = GrammarBuilder::new("ga")
            .token("X", "xxx")
            .rule("ea", vec!["X"])
            .precedence(level, Associativity::Left, vec!["ea"])
            .start("ea")
            .build();
        let gb = GrammarBuilder::new("gb")
            .token("Y", "yyy")
            .rule("eb", vec!["Y"])
            .precedence(level, Associativity::Right, vec!["eb"])
            .start("eb")
            .build();
        let mut merged = ga.clone();
        let original_count = merged.precedences.len();
        merge_precedences(&mut merged, &gb);
        // Same level should be skipped
        prop_assert_eq!(merged.precedences.len(), original_count);
    }

    #[test]
    fn prop_merge_precedence_preserves_associativity(
        assoc in arb_associativity(),
    ) {
        let ga = GrammarBuilder::new("ga")
            .token("X", "xxx")
            .rule("ea", vec!["X"])
            .precedence(1, assoc, vec!["ea"])
            .start("ea")
            .build();
        let empty = Grammar::new("empty".to_string());
        let mut merged = ga.clone();
        merge_precedences(&mut merged, &empty);
        prop_assert_eq!(merged.precedences[0].associativity, assoc);
    }

    #[test]
    fn prop_merge_precedence_empty_src(
        level in arb_precedence_level(),
    ) {
        let ga = GrammarBuilder::new("ga")
            .token("X", "xxx")
            .rule("ea", vec!["X"])
            .precedence(level, Associativity::Left, vec!["ea"])
            .start("ea")
            .build();
        let empty = Grammar::new("empty".to_string());
        let mut merged = ga.clone();
        merge_precedences(&mut merged, &empty);
        prop_assert_eq!(merged.precedences.len(), ga.precedences.len());
    }

    #[test]
    fn prop_merge_precedence_into_empty(
        level in arb_precedence_level(),
        assoc in arb_associativity(),
    ) {
        let mut dst = Grammar::new("dst".to_string());
        let src = GrammarBuilder::new("src")
            .token("X", "xxx")
            .rule("ea", vec!["X"])
            .precedence(level, assoc, vec!["ea"])
            .start("ea")
            .build();
        merge_precedences(&mut dst, &src);
        prop_assert!(!dst.precedences.is_empty());
        prop_assert_eq!(dst.precedences[0].level, level);
        prop_assert_eq!(dst.precedences[0].associativity, assoc);
    }

    #[test]
    fn prop_merge_precedence_multiple_levels(
        count in 1usize..=5,
    ) {
        let mut ga = GrammarBuilder::new("ga")
            .token("X", "xxx")
            .rule("ea", vec!["X"])
            .start("ea");
        for i in 0..count {
            ga = ga.precedence(i as i16, Associativity::Left, vec!["ea"]);
        }
        let grammar_a = ga.build();

        let gb = GrammarBuilder::new("gb")
            .token("Y", "yyy")
            .rule("eb", vec!["Y"])
            .precedence(100, Associativity::Right, vec!["eb"])
            .start("eb")
            .build();

        let mut merged = grammar_a.clone();
        merge_precedences(&mut merged, &gb);
        // The new level 100 should be added
        let levels: Vec<i16> = merged.precedences.iter().map(|p| p.level).collect();
        prop_assert!(levels.contains(&100));
        prop_assert!(merged.precedences.len() >= grammar_a.precedences.len());
    }
}

// ---------------------------------------------------------------------------
// Category 4: prop_merge_fields_*
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, ..Default::default() })]

    #[test]
    fn prop_merge_fields_disjoint(
        name_a in arb_rule_name(),
        name_b in arb_rule_name(),
    ) {
        prop_assume!(name_a != name_b);
        let mut ga = minimal_grammar("ga");
        ga.fields.insert(FieldId(0), name_a.clone());
        let mut gb = minimal_grammar("gb");
        gb.fields.insert(FieldId(0), name_b.clone());
        merge_fields(&mut ga, &gb);
        let field_names: Vec<&String> = ga.fields.values().collect();
        prop_assert!(field_names.contains(&&name_a));
        prop_assert!(field_names.contains(&&name_b));
    }

    #[test]
    fn prop_merge_fields_duplicate_skipped(
        fname in arb_rule_name(),
    ) {
        let mut ga = minimal_grammar("ga");
        ga.fields.insert(FieldId(0), fname.clone());
        let mut gb = minimal_grammar("gb");
        gb.fields.insert(FieldId(0), fname.clone());
        let original_count = ga.fields.len();
        merge_fields(&mut ga, &gb);
        prop_assert_eq!(ga.fields.len(), original_count);
    }

    #[test]
    fn prop_merge_fields_preserves_existing(
        fname in arb_rule_name(),
    ) {
        let mut ga = minimal_grammar("ga");
        ga.fields.insert(FieldId(0), fname.clone());
        let empty = Grammar::new("empty".to_string());
        merge_fields(&mut ga, &empty);
        prop_assert_eq!(ga.fields.len(), 1);
        prop_assert_eq!(ga.fields.get(&FieldId(0)).unwrap(), &fname);
    }

    #[test]
    fn prop_merge_fields_empty_into_empty(
        _seed in 0u32..100,
    ) {
        let mut dst = Grammar::new("dst".to_string());
        let src = Grammar::new("src".to_string());
        merge_fields(&mut dst, &src);
        prop_assert!(dst.fields.is_empty());
    }

    #[test]
    fn prop_merge_fields_into_empty(
        fname in arb_rule_name(),
    ) {
        let mut dst = Grammar::new("dst".to_string());
        let mut src = minimal_grammar("src");
        src.fields.insert(FieldId(0), fname.clone());
        merge_fields(&mut dst, &src);
        prop_assert!(!dst.fields.is_empty());
        let values: Vec<&String> = dst.fields.values().collect();
        prop_assert!(values.contains(&&fname));
    }

    #[test]
    fn prop_merge_fields_count_upper_bound(
        count_a in 1usize..=4,
        count_b in 1usize..=4,
    ) {
        let mut ga = minimal_grammar("ga");
        for i in 0..count_a {
            ga.fields.insert(FieldId(i as u16), format!("fa_{}", i));
        }
        let mut gb = minimal_grammar("gb");
        for i in 0..count_b {
            gb.fields.insert(FieldId(i as u16), format!("fb_{}", i));
        }
        merge_fields(&mut ga, &gb);
        prop_assert!(ga.fields.len() <= count_a + count_b);
    }
}

// ---------------------------------------------------------------------------
// Category 5: prop_merge_extras_*
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, ..Default::default() })]

    #[test]
    fn prop_merge_extras_from_src(
        name in arb_grammar_name(),
    ) {
        let ga = GrammarBuilder::new(&name)
            .token("WS", r"[ \t]+")
            .extra("WS")
            .token("NUM", r"\d+")
            .rule("root", vec!["NUM"])
            .start("root")
            .build();
        let gb = GrammarBuilder::new("gb")
            .token("COMMENT", r"//[^\n]*")
            .extra("COMMENT")
            .token("NUM", r"\d+")
            .rule("root", vec!["NUM"])
            .start("root")
            .build();
        let mut merged = ga.clone();
        merge_tokens(&mut merged, &gb);
        merge_extras_into(&mut merged, &gb);
        // Merged extras should include at least the original
        prop_assert!(!merged.extras.is_empty());
    }

    #[test]
    fn prop_merge_extras_no_duplicate(
        name in arb_grammar_name(),
    ) {
        let ga = GrammarBuilder::new(&name)
            .token("WS", r"[ \t]+")
            .extra("WS")
            .token("NUM", r"\d+")
            .rule("root", vec!["NUM"])
            .start("root")
            .build();
        // Same extra in both
        let gb = GrammarBuilder::new("gb")
            .token("WS", r"[ \t]+")
            .extra("WS")
            .token("NUM", r"\d+")
            .rule("root", vec!["NUM"])
            .start("root")
            .build();
        let mut merged = ga.clone();
        merge_tokens(&mut merged, &gb);
        let original_extra_count = merged.extras.len();
        merge_extras_into(&mut merged, &gb);
        // No duplicate extras should be added
        prop_assert_eq!(merged.extras.len(), original_extra_count);
    }

    #[test]
    fn prop_merge_extras_empty_src(
        name in arb_grammar_name(),
    ) {
        let ga = GrammarBuilder::new(&name)
            .token("WS", r"[ \t]+")
            .extra("WS")
            .token("NUM", r"\d+")
            .rule("root", vec!["NUM"])
            .start("root")
            .build();
        let empty = Grammar::new("empty".to_string());
        let mut merged = ga.clone();
        merge_extras_into(&mut merged, &empty);
        prop_assert_eq!(merged.extras.len(), ga.extras.len());
    }

    #[test]
    fn prop_merge_extras_preserves_original(
        name in arb_grammar_name(),
    ) {
        let ga = GrammarBuilder::new(&name)
            .token("WS", r"[ \t]+")
            .extra("WS")
            .token("NUM", r"\d+")
            .rule("root", vec!["NUM"])
            .start("root")
            .build();
        let empty = Grammar::new("empty".to_string());
        let mut merged = ga.clone();
        let original_extras = merged.extras.clone();
        merge_extras_into(&mut merged, &empty);
        prop_assert_eq!(merged.extras, original_extras);
    }

    #[test]
    fn prop_merge_extras_count_bound(
        has_ws in prop::bool::ANY,
        has_comment in prop::bool::ANY,
    ) {
        let mut ba = GrammarBuilder::new("ga")
            .token("NUM", r"\d+")
            .rule("root", vec!["NUM"])
            .start("root")
            .token("WS", r"[ \t]+")
            .token("COMMENT", r"//[^\n]*");
        if has_ws {
            ba = ba.extra("WS");
        }
        let ga = ba.build();

        let mut bb = GrammarBuilder::new("gb")
            .token("NUM", r"\d+")
            .rule("root", vec!["NUM"])
            .start("root")
            .token("WS", r"[ \t]+")
            .token("COMMENT", r"//[^\n]*");
        if has_comment {
            bb = bb.extra("COMMENT");
        }
        let gb = bb.build();

        let mut merged = ga.clone();
        merge_tokens(&mut merged, &gb);
        merge_extras_into(&mut merged, &gb);
        prop_assert!(merged.extras.len() <= ga.extras.len() + gb.extras.len());
    }

    #[test]
    fn prop_merge_extras_empty_both(
        _seed in 0u32..100,
    ) {
        let mut dst = Grammar::new("dst".to_string());
        let src = Grammar::new("src".to_string());
        merge_extras_into(&mut dst, &src);
        prop_assert!(dst.extras.is_empty());
    }
}

// ---------------------------------------------------------------------------
// Category 6: prop_merge_validate_*
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, ..Default::default() })]

    #[test]
    fn prop_merge_validate_single_grammar(
        name in arb_grammar_name(),
    ) {
        let grammar = GrammarBuilder::new(&name)
            .token("NUM", r"\d+")
            .rule("start_rule", vec!["NUM"])
            .start("start_rule")
            .build();
        // A simple valid grammar should pass validate
        prop_assert!(grammar.validate().is_ok());
    }

    #[test]
    fn prop_merge_validate_after_token_merge(
        name in arb_grammar_name(),
    ) {
        let ga = GrammarBuilder::new(&name)
            .token("NUM", r"\d+")
            .rule("root", vec!["NUM"])
            .start("root")
            .build();
        let gb = GrammarBuilder::new("gb")
            .token("STR", "[a-z]+")
            .rule("root", vec!["STR"])
            .start("root")
            .build();
        let mut merged = ga.clone();
        merge_tokens(&mut merged, &gb);
        // Token merge alone should preserve validity of the original grammar's rules
        prop_assert!(merged.validate().is_ok());
    }

    #[test]
    fn prop_merge_validate_empty_fields_ok(
        name in arb_grammar_name(),
    ) {
        let grammar = GrammarBuilder::new(&name)
            .token("TOK", "tok")
            .rule("root", vec!["TOK"])
            .start("root")
            .build();
        // Empty fields => lexicographic ordering trivially satisfied
        prop_assert!(grammar.fields.is_empty());
        prop_assert!(grammar.validate().is_ok());
    }

    #[test]
    fn prop_merge_validate_field_ordering(
        name_a in arb_rule_name(),
        name_b in arb_rule_name(),
    ) {
        prop_assume!(name_a != name_b);
        let mut grammar = minimal_grammar("test");
        let mut sorted_names = [name_a.clone(), name_b.clone()];
        sorted_names.sort();
        grammar.fields.insert(FieldId(0), sorted_names[0].clone());
        grammar.fields.insert(FieldId(1), sorted_names[1].clone());
        // Lexicographically sorted fields should validate
        prop_assert!(grammar.validate().is_ok());
    }

    #[test]
    fn prop_merge_validate_rejects_bad_field_order(
        name_a in arb_rule_name(),
        name_b in arb_rule_name(),
    ) {
        prop_assume!(name_a != name_b);
        let mut grammar = minimal_grammar("test");
        let mut sorted_names = [name_a.clone(), name_b.clone()];
        sorted_names.sort();
        // Insert in reverse lexicographic order (wrong)
        grammar
            .fields
            .insert(FieldId(0), sorted_names[1].clone());
        grammar
            .fields
            .insert(FieldId(1), sorted_names[0].clone());
        prop_assert!(grammar.validate().is_err());
    }

    #[test]
    fn prop_merge_validate_uses_grammar_validator(
        name in arb_grammar_name(),
    ) {
        let grammar = GrammarBuilder::new(&name)
            .token("NUM", r"\d+")
            .rule("root", vec!["NUM"])
            .start("root")
            .build();
        let mut validator = GrammarValidator::new();
        let result = validator.validate(&grammar);
        // Should not have fatal errors for a simple valid grammar
        let fatal_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|e| {
                !matches!(
                    e,
                    adze_ir::validation::ValidationError::NoExplicitStartRule
                )
            })
            .collect();
        prop_assert!(fatal_errors.is_empty(), "Unexpected errors: {:?}", fatal_errors);
    }
}

// ---------------------------------------------------------------------------
// Category 7: prop_merge_normalize_*
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, ..Default::default() })]

    #[test]
    fn prop_merge_normalize_preserves_terminals(
        name in arb_grammar_name(),
    ) {
        let mut grammar = GrammarBuilder::new(&name)
            .token("NUM", r"\d+")
            .rule("root", vec!["NUM"])
            .start("root")
            .build();
        let original_token_count = grammar.tokens.len();
        grammar.normalize();
        prop_assert_eq!(grammar.tokens.len(), original_token_count);
    }

    #[test]
    fn prop_merge_normalize_simple_is_noop(
        name in arb_grammar_name(),
    ) {
        let mut grammar = GrammarBuilder::new(&name)
            .token("NUM", r"\d+")
            .token("PLUS", "+")
            .rule("expr", vec!["NUM", "PLUS", "NUM"])
            .start("expr")
            .build();
        let original_rule_count: usize = grammar.rules.values().map(|v| v.len()).sum();
        grammar.normalize();
        let normalized_count: usize = grammar.rules.values().map(|v| v.len()).sum();
        // Simple grammar without complex symbols should be unchanged
        prop_assert_eq!(normalized_count, original_rule_count);
    }

    #[test]
    fn prop_merge_normalize_after_merge(
        name in arb_grammar_name(),
    ) {
        let ga = GrammarBuilder::new(&name)
            .token("A", "aaa")
            .rule("sa", vec!["A"])
            .start("sa")
            .build();
        let gb = GrammarBuilder::new("gb")
            .token("B", "bbb")
            .rule("sb", vec!["B"])
            .start("sb")
            .build();
        let mut merged = ga.clone();
        merge_tokens(&mut merged, &gb);
        merge_rules(&mut merged, &gb);
        // normalize should not panic
        merged.normalize();
        prop_assert!(!merged.rules.is_empty());
    }

    #[test]
    fn prop_merge_normalize_with_epsilon(
        name in arb_grammar_name(),
    ) {
        let mut grammar = GrammarBuilder::new(&name)
            .token("NUM", r"\d+")
            .rule("opt", vec![])
            .rule("opt", vec!["NUM"])
            .start("opt")
            .build();
        grammar.normalize();
        // Should still have rules
        prop_assert!(!grammar.rules.is_empty());
    }

    #[test]
    fn prop_merge_normalize_returns_rules(
        name in arb_grammar_name(),
    ) {
        let mut grammar = GrammarBuilder::new(&name)
            .token("NUM", r"\d+")
            .rule("root", vec!["NUM"])
            .start("root")
            .build();
        let produced = grammar.normalize();
        prop_assert!(!produced.is_empty());
    }

    #[test]
    fn prop_merge_normalize_idempotent(
        name in arb_grammar_name(),
    ) {
        let mut grammar = GrammarBuilder::new(&name)
            .token("NUM", r"\d+")
            .token("PLUS", "+")
            .rule("expr", vec!["NUM", "PLUS", "NUM"])
            .rule("expr", vec!["NUM"])
            .start("expr")
            .build();
        grammar.normalize();
        let after_first: Vec<Rule> = grammar.all_rules().cloned().collect();
        grammar.normalize();
        let after_second: Vec<Rule> = grammar.all_rules().cloned().collect();
        prop_assert_eq!(after_first.len(), after_second.len());
    }
}

// ---------------------------------------------------------------------------
// Category 8: prop_merge_idempotent_*
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, ..Default::default() })]

    #[test]
    fn prop_merge_idempotent_token_merge(
        name in arb_grammar_name(),
    ) {
        let ga = GrammarBuilder::new(&name)
            .token("TOK", "tok")
            .rule("root", vec!["TOK"])
            .start("root")
            .build();
        let mut merged_once = ga.clone();
        merge_tokens(&mut merged_once, &ga);
        let count_once = merged_once.tokens.len();
        merge_tokens(&mut merged_once, &ga);
        let count_twice = merged_once.tokens.len();
        prop_assert_eq!(count_once, count_twice);
    }

    #[test]
    fn prop_merge_idempotent_precedence_merge(
        level in arb_precedence_level(),
    ) {
        let ga = GrammarBuilder::new("ga")
            .token("X", "xxx")
            .rule("ea", vec!["X"])
            .precedence(level, Associativity::Left, vec!["ea"])
            .start("ea")
            .build();
        let mut merged = ga.clone();
        merge_precedences(&mut merged, &ga);
        let count_once = merged.precedences.len();
        merge_precedences(&mut merged, &ga);
        let count_twice = merged.precedences.len();
        prop_assert_eq!(count_once, count_twice);
    }

    #[test]
    fn prop_merge_idempotent_field_merge(
        fname in arb_rule_name(),
    ) {
        let mut ga = minimal_grammar("ga");
        ga.fields.insert(FieldId(0), fname.clone());
        let gb = ga.clone();
        merge_fields(&mut ga, &gb);
        let count_once = ga.fields.len();
        merge_fields(&mut ga, &gb);
        let count_twice = ga.fields.len();
        prop_assert_eq!(count_once, count_twice);
    }

    #[test]
    fn prop_merge_idempotent_extras_merge(
        name in arb_grammar_name(),
    ) {
        let ga = GrammarBuilder::new(&name)
            .token("WS", r"[ \t]+")
            .extra("WS")
            .token("NUM", r"\d+")
            .rule("root", vec!["NUM"])
            .start("root")
            .build();
        let mut merged = ga.clone();
        merge_extras_into(&mut merged, &ga);
        let count_once = merged.extras.len();
        merge_extras_into(&mut merged, &ga);
        let count_twice = merged.extras.len();
        prop_assert_eq!(count_once, count_twice);
    }

    #[test]
    fn prop_merge_idempotent_optimize(
        name in arb_grammar_name(),
    ) {
        let mut grammar = GrammarBuilder::new(&name)
            .token("NUM", r"\d+")
            .rule("root", vec!["NUM"])
            .start("root")
            .build();
        grammar.optimize();
        let after_first: Vec<Rule> = grammar.all_rules().cloned().collect();
        grammar.optimize();
        let after_second: Vec<Rule> = grammar.all_rules().cloned().collect();
        prop_assert_eq!(after_first, after_second);
    }

    #[test]
    fn prop_merge_idempotent_validate(
        name in arb_grammar_name(),
    ) {
        let grammar = GrammarBuilder::new(&name)
            .token("NUM", r"\d+")
            .rule("root", vec!["NUM"])
            .start("root")
            .build();
        let result_first = grammar.validate();
        let result_second = grammar.validate();
        // Validation should be deterministic
        prop_assert_eq!(result_first.is_ok(), result_second.is_ok());
    }
}
