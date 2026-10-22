#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Property-based tests for the GrammarBuilder fluent API.
//! Covers: creation, naming, add_rule, add_token, add_external,
//! set_start_symbol, method chaining, and build() validation.

use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, Grammar, Symbol, SymbolId, TokenPattern};
use std::collections::HashSet;

// ── Helpers ─────────────────────────────────────────────────────────────

/// Resolve a non-terminal name to its SymbolId via `rule_names`.
fn sym(g: &Grammar, name: &str) -> SymbolId {
    g.rule_names
        .iter()
        .find(|(_, n)| n.as_str() == name)
        .map(|(id, _)| *id)
        .unwrap_or_else(|| panic!("symbol '{name}' not in rule_names"))
}

/// Resolve a token name to its SymbolId.
fn tok(g: &Grammar, name: &str) -> SymbolId {
    g.tokens
        .iter()
        .find(|(_, t)| t.name == name)
        .map(|(id, _)| *id)
        .unwrap_or_else(|| panic!("token '{name}' not found"))
}

// ═══════════════════════════════════════════════════════════════════════
// 1. Builder creates valid grammar
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn builder_minimal_grammar_validates() {
    let g = GrammarBuilder::new("minimal")
        .token("ID", r"[a-z]+")
        .rule("root", vec!["ID"])
        .start("root")
        .build();
    assert!(g.validate().is_ok());
}

#[test]
fn builder_empty_build_produces_default_fields() {
    let g = GrammarBuilder::new("defaults").build();
    assert!(g.fields.is_empty());
    assert!(g.alias_sequences.is_empty());
    assert_eq!(g.production_ids.len(), g.all_rules().count());
    assert_eq!(g.max_alias_sequence_length, 0);
    assert!(g.symbol_registry.is_none());
    assert!(g.inline_rules.is_empty());
    assert!(g.supertypes.is_empty());
}

#[test]
fn builder_grammar_with_all_features_validates() {
    let g = GrammarBuilder::new("full")
        .token("NUM", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .token("WS", r"[ \t]+")
        .fragile_token("ERR", "ERR")
        .external("INDENT")
        .extra("WS")
        .precedence(1, Associativity::Left, vec!["+"])
        .precedence(2, Associativity::Left, vec!["*"])
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    assert!(g.validate().is_ok());
    assert!(!g.extras.is_empty());
    assert!(!g.externals.is_empty());
    assert!(!g.precedences.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════
// 2. Builder with name
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn builder_name_preserved_verbatim() {
    for name in &["myLang", "x", "UPPER_CASE", "a1b2c3"] {
        let g = GrammarBuilder::new(name).build();
        assert_eq!(g.name, *name);
    }
}

#[test]
fn builder_name_empty_string_allowed() {
    let g = GrammarBuilder::new("").build();
    assert_eq!(g.name, "");
}

#[test]
fn builder_name_with_unicode() {
    let g = GrammarBuilder::new("日本語").build();
    assert_eq!(g.name, "日本語");
}

// ═══════════════════════════════════════════════════════════════════════
// 3. Builder add_rule
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn add_rule_single_terminal_rhs() {
    let g = GrammarBuilder::new("r")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let rules = &g.rules[&sym(&g, "s")];
    assert_eq!(rules.len(), 1);
    assert!(matches!(rules[0].rhs[0], Symbol::Terminal(_)));
}

#[test]
fn add_rule_mixed_terminal_nonterminal_rhs() {
    let g = GrammarBuilder::new("r")
        .token("PLUS", "+")
        .rule("atom", vec!["PLUS"])
        .rule("expr", vec!["atom", "PLUS", "atom"])
        .start("expr")
        .build();
    let eid = sym(&g, "expr");
    let rhs = &g.rules[&eid][0].rhs;
    // atom is NonTerminal, PLUS is Terminal, atom is NonTerminal
    assert!(matches!(rhs[0], Symbol::NonTerminal(_)));
    assert!(matches!(rhs[1], Symbol::Terminal(_)));
    assert!(matches!(rhs[2], Symbol::NonTerminal(_)));
}

#[test]
fn add_rule_epsilon_from_empty_vec() {
    let g = GrammarBuilder::new("r")
        .rule("nullable", vec![])
        .start("nullable")
        .build();
    let rules = &g.rules[&sym(&g, "nullable")];
    assert_eq!(rules[0].rhs, vec![Symbol::Epsilon]);
}

#[test]
fn add_rule_multiple_alternatives_accumulate() {
    let g = GrammarBuilder::new("r")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .rule("x", vec!["A"])
        .rule("x", vec!["B"])
        .rule("x", vec!["C"])
        .rule("x", vec!["A", "B"])
        .start("x")
        .build();
    assert_eq!(g.rules[&sym(&g, "x")].len(), 4);
}

#[test]
fn add_rule_production_ids_monotonically_increase() {
    let g = GrammarBuilder::new("r")
        .token("T", "t")
        .rule("a", vec!["T"])
        .rule("b", vec!["T"])
        .rule("c", vec!["T"])
        .rule("d", vec!["T"])
        .build();
    let ids: Vec<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    for i in 1..ids.len() {
        assert!(ids[i] > ids[i - 1], "IDs not monotonic: {:?}", ids);
    }
}

#[test]
fn add_rule_fields_default_empty() {
    let g = GrammarBuilder::new("r")
        .token("T", "t")
        .rule("s", vec!["T"])
        .build();
    for r in g.all_rules() {
        assert!(r.fields.is_empty());
    }
}

#[test]
fn add_rule_plain_has_no_precedence() {
    let g = GrammarBuilder::new("r")
        .token("T", "t")
        .rule("s", vec!["T"])
        .build();
    let r = g.all_rules().next().unwrap();
    assert!(r.precedence.is_none());
    assert!(r.associativity.is_none());
}

// ═══════════════════════════════════════════════════════════════════════
// 4. Builder add_token
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn add_token_literal_string_pattern() {
    let g = GrammarBuilder::new("t").token("while", "while").build();
    let t = g.tokens.values().next().unwrap();
    assert_eq!(t.pattern, TokenPattern::String("while".to_string()));
}

#[test]
fn add_token_regex_via_special_chars() {
    let g = GrammarBuilder::new("t").token("NUM", r"\d+").build();
    let t = g.tokens.values().next().unwrap();
    assert_eq!(t.pattern, TokenPattern::Regex(r"\d+".to_string()));
}

#[test]
fn add_token_slash_delimited_regex() {
    let g = GrammarBuilder::new("t")
        .token("FLOAT", r"/[0-9]+\.[0-9]+/")
        .build();
    let t = g.tokens.values().next().unwrap();
    assert_eq!(
        t.pattern,
        TokenPattern::Regex(r"[0-9]+\.[0-9]+".to_string())
    );
}

#[test]
fn add_token_fragile_flag() {
    let g = GrammarBuilder::new("t")
        .fragile_token("SEMI", ";")
        .token("COMMA", ",")
        .build();
    let semi = g.tokens.iter().find(|(_, t)| t.name == "SEMI").unwrap().1;
    let comma = g.tokens.iter().find(|(_, t)| t.name == "COMMA").unwrap().1;
    assert!(semi.fragile);
    assert!(!comma.fragile);
}

#[test]
fn add_token_duplicate_name_overwrites() {
    let g = GrammarBuilder::new("t")
        .token("X", "first")
        .token("X", "second")
        .build();
    assert_eq!(g.tokens.len(), 1);
    let t = g.tokens.values().next().unwrap();
    assert_eq!(t.pattern, TokenPattern::String("second".to_string()));
}

#[test]
fn add_token_symbol_ids_all_nonzero() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .build();
    for id in g.tokens.keys() {
        assert!(id.0 >= 1, "token SymbolId must be >= 1, got {}", id.0);
    }
}

#[test]
fn add_token_ids_unique() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .token("D", "d")
        .build();
    let ids: HashSet<u16> = g.tokens.keys().map(|id| id.0).collect();
    assert_eq!(ids.len(), 4);
}

// ═══════════════════════════════════════════════════════════════════════
// 5. Builder add_external
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn add_external_single() {
    let g = GrammarBuilder::new("e").external("INDENT").build();
    assert_eq!(g.externals.len(), 1);
    assert_eq!(g.externals[0].name, "INDENT");
}

#[test]
fn add_external_multiple_preserves_order() {
    let g = GrammarBuilder::new("e")
        .external("INDENT")
        .external("DEDENT")
        .external("NEWLINE")
        .build();
    assert_eq!(g.externals.len(), 3);
    assert_eq!(g.externals[0].name, "INDENT");
    assert_eq!(g.externals[1].name, "DEDENT");
    assert_eq!(g.externals[2].name, "NEWLINE");
}

#[test]
fn add_external_assigns_symbol_id() {
    let g = GrammarBuilder::new("e").external("HEREDOC").build();
    // The external should have a valid nonzero symbol_id
    assert!(g.externals[0].symbol_id.0 >= 1);
}

// ═══════════════════════════════════════════════════════════════════════
// 6. Builder set_start_symbol
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn start_symbol_reorders_rules_to_front() {
    let g = GrammarBuilder::new("s")
        .token("T", "t")
        .rule("beta", vec!["T"])
        .rule("alpha", vec!["beta"])
        .start("alpha")
        .build();
    let first = g.rules.keys().next().unwrap();
    assert_eq!(g.rule_names[first], "alpha");
}

#[test]
fn start_symbol_no_effect_when_unset() {
    let g = GrammarBuilder::new("s")
        .token("T", "t")
        .rule("first", vec!["T"])
        .rule("second", vec!["T"])
        .build();
    // Without .start(), rules appear in insertion order
    let first = g.rules.keys().next().unwrap();
    assert_eq!(g.rule_names[first], "first");
}

#[test]
fn start_symbol_with_multiple_alternatives() {
    let g = GrammarBuilder::new("s")
        .token("A", "a")
        .token("B", "b")
        .rule("other", vec!["A"])
        .rule("entry", vec!["A"])
        .rule("entry", vec!["B"])
        .rule("entry", vec!["A", "B"])
        .start("entry")
        .build();
    let first = g.rules.keys().next().unwrap();
    assert_eq!(g.rule_names[first], "entry");
    assert_eq!(g.rules[first].len(), 3);
}

// ═══════════════════════════════════════════════════════════════════════
// 7. Builder chain methods
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn chain_all_methods_in_single_expression() {
    // Verifies the entire fluent API compiles and runs in one chain.
    let g = GrammarBuilder::new("chain")
        .token("NUM", r"\d+")
        .token("+", "+")
        .token("WS", r"\s+")
        .fragile_token("ERR", "err")
        .external("EXT")
        .extra("WS")
        .precedence(1, Associativity::Left, vec!["+"])
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    assert_eq!(g.name, "chain");
    assert_eq!(g.tokens.len(), 4); // NUM, +, WS, ERR
    assert_eq!(g.externals.len(), 1);
    assert_eq!(g.extras.len(), 1);
    assert_eq!(g.precedences.len(), 1);
}

#[test]
fn chain_token_reuse_across_rules() {
    let g = GrammarBuilder::new("reuse")
        .token("ID", r"[a-z]+")
        .rule("a", vec!["ID"])
        .rule("b", vec!["ID"])
        .rule("c", vec!["ID"])
        .build();
    let id_sym = tok(&g, "ID");
    // All rules should reference the same token SymbolId
    for rules in g.rules.values() {
        for rule in rules {
            if let Symbol::Terminal(sid) = &rule.rhs[0] {
                assert_eq!(*sid, id_sym);
            }
        }
    }
}

#[test]
fn chain_extra_before_and_after_token() {
    // extra() can be called for a symbol that hasn't been token()-ed yet.
    let g = GrammarBuilder::new("extra_order")
        .extra("WS")
        .token("WS", r"\s+")
        .build();
    assert_eq!(g.extras.len(), 1);
    assert_eq!(g.tokens.len(), 1);
}

// ═══════════════════════════════════════════════════════════════════════
// 8. Builder build() validation
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn build_empty_grammar_validates() {
    let g = GrammarBuilder::new("empty").build();
    assert!(g.validate().is_ok());
}

#[test]
fn build_check_empty_terminals_ok() {
    let g = GrammarBuilder::new("nonempty")
        .token("A", "a")
        .token("B", "bb")
        .build();
    assert!(g.check_empty_terminals().is_ok());
}

#[test]
fn build_normalize_does_not_lose_rules() {
    let mut g = GrammarBuilder::new("norm")
        .token("X", "x")
        .rule("a", vec!["X"])
        .rule("b", vec!["a"])
        .rule("c", vec!["a", "b"])
        .start("c")
        .build();
    let before: usize = g.rules.values().map(|v| v.len()).sum();
    g.normalize();
    let after: usize = g.rules.values().map(|v| v.len()).sum();
    assert!(after >= before);
}

#[test]
fn build_normalize_idempotent() {
    let mut g = GrammarBuilder::new("idem")
        .token("T", "t")
        .rule("r", vec!["T"])
        .rule("s", vec!["r"])
        .start("s")
        .build();
    g.normalize();
    let snap1: Vec<_> = g.all_rules().map(|r| r.production_id).collect();
    g.normalize();
    let snap2: Vec<_> = g.all_rules().map(|r| r.production_id).collect();
    assert_eq!(snap1, snap2);
}

#[test]
fn build_python_like_validates() {
    let g = GrammarBuilder::python_like();
    assert!(g.validate().is_ok());
    assert_eq!(g.name, "python_like");
}

#[test]
fn build_javascript_like_validates() {
    let g = GrammarBuilder::javascript_like();
    assert!(g.validate().is_ok());
    assert_eq!(g.name, "javascript_like");
}

#[test]
fn build_all_rules_iterator_count() {
    let g = GrammarBuilder::new("iter")
        .token("T", "t")
        .rule("a", vec!["T"])
        .rule("a", vec!["T", "T"])
        .rule("b", vec!["T"])
        .build();
    let manual_count: usize = g.rules.values().map(|v| v.len()).sum();
    assert_eq!(g.all_rules().count(), manual_count);
    assert_eq!(manual_count, 3);
}

#[test]
fn build_find_symbol_by_name_works() {
    let g = GrammarBuilder::new("find")
        .token("T", "t")
        .rule("expr", vec!["T"])
        .rule("stmt", vec!["expr"])
        .start("stmt")
        .build();
    assert!(g.find_symbol_by_name("expr").is_some());
    assert!(g.find_symbol_by_name("stmt").is_some());
    assert!(g.find_symbol_by_name("nonexistent").is_none());
}
