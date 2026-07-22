//! Comprehensive tests for `advanced_conflict` module.
//!
//! Covers `ConflictAnalyzer`, `ConflictStats`, `PrecedenceResolver`, and
//! `PrecedenceDecision` across a wide range of grammar shapes.
//!
//! Also tests `ConflictResolver::detect_conflicts` and `resolve_conflicts`
//! for shift-reduce, reduce-reduce, precedence, and associativity scenarios.

use adze_glr_core::advanced_conflict::{
    ConflictAnalyzer, ConflictStats, PrecedenceDecision, PrecedenceResolver,
};
use adze_glr_core::{
    Action, Conflict, ConflictResolver, ConflictType, FirstFollowSets, ItemSetCollection, LexMode,
    ParseTable, StateId,
};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{
    Associativity, Grammar, Precedence, PrecedenceKind, ProductionId, Rule, RuleId, Symbol,
    SymbolId, Token, TokenPattern,
};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal ParseTable from a Grammar (enough for `analyze_table`).
fn minimal_table(grammar: Grammar) -> ParseTable {
    ParseTable {
        action_table: vec![vec![vec![Action::Shift(StateId(1))]]],
        goto_table: vec![],
        symbol_metadata: vec![],
        state_count: 1,
        symbol_count: 1,
        symbol_to_index: BTreeMap::new(),
        index_to_symbol: vec![],
        external_scanner_states: vec![],
        rules: vec![],
        nonterminal_to_index: BTreeMap::new(),
        goto_indexing: adze_glr_core::GotoIndexing::NonterminalMap,
        eof_symbol: SymbolId(0),
        start_symbol: SymbolId(1),
        grammar,
        initial_state: StateId(0),
        token_count: 1,
        external_token_count: 0,
        lex_modes: vec![LexMode {
            lex_state: 0,
            external_lex_state: 0,
        }],
        extras: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
    }
}

/// Resolve a symbol name to its `SymbolId` via the grammar's rule_names or tokens.
fn sym(grammar: &Grammar, name: &str) -> SymbolId {
    for (&id, n) in &grammar.rule_names {
        if n == name {
            return id;
        }
    }
    for (&id, tok) in &grammar.tokens {
        if tok.name == name {
            return id;
        }
    }
    panic!("symbol `{name}` not found in grammar");
}

/// Build an ambiguous grammar: E → a | E E
/// This is inherently ambiguous and produces shift-reduce conflicts.
fn ambiguous_ee_grammar() -> Grammar {
    let mut grammar = Grammar::new("ambig".into());
    let a = SymbolId(1);
    let e = SymbolId(10);
    grammar.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(e, "E".into());
    grammar.rules.insert(
        e,
        vec![
            Rule {
                lhs: e,
                rhs: vec![Symbol::Terminal(a)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            },
            Rule {
                lhs: e,
                rhs: vec![Symbol::NonTerminal(e), Symbol::NonTerminal(e)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(1),
            },
        ],
    );
    grammar.set_start_symbol(e);
    grammar
}

/// Build a grammar with a shift-reduce conflict resolvable by precedence.
/// expr → expr '+' expr | expr '*' expr | NUM
fn arithmetic_prec_grammar(
    plus_prec: i16,
    plus_assoc: Associativity,
    star_prec: i16,
    star_assoc: Associativity,
) -> Grammar {
    GrammarBuilder::new("arith")
        .token("NUM", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .precedence(plus_prec, plus_assoc, vec!["+"])
        .precedence(star_prec, star_assoc, vec!["*"])
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], plus_prec, plus_assoc)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], star_prec, star_assoc)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build()
}

// =========================================================================
// ConflictStats tests
// =========================================================================

#[test]
fn stats_default_is_all_zeros() {
    let stats = ConflictStats::default();
    assert_eq!(stats.shift_reduce_conflicts, 0);
    assert_eq!(stats.reduce_reduce_conflicts, 0);
    assert_eq!(stats.precedence_resolved, 0);
    assert_eq!(stats.associativity_resolved, 0);
    assert_eq!(stats.explicit_glr, 0);
    assert_eq!(stats.default_resolved, 0);
}

#[test]
fn stats_clone_preserves_values() {
    let stats = ConflictStats {
        shift_reduce_conflicts: 3,
        reduce_reduce_conflicts: 7,
        precedence_resolved: 2,
        ..Default::default()
    };
    let cloned = stats.clone();
    assert_eq!(cloned.shift_reduce_conflicts, 3);
    assert_eq!(cloned.reduce_reduce_conflicts, 7);
    assert_eq!(cloned.precedence_resolved, 2);
}

#[test]
fn stats_debug_format() {
    let stats = ConflictStats::default();
    let dbg = format!("{stats:?}");
    assert!(dbg.contains("ConflictStats"));
    assert!(dbg.contains("shift_reduce_conflicts"));
}

#[test]
fn stats_all_fields_nonzero() {
    let stats = ConflictStats {
        shift_reduce_conflicts: 1,
        reduce_reduce_conflicts: 2,
        precedence_resolved: 3,
        associativity_resolved: 4,
        explicit_glr: 5,
        default_resolved: 6,
    };
    assert_eq!(stats.shift_reduce_conflicts, 1);
    assert_eq!(stats.reduce_reduce_conflicts, 2);
    assert_eq!(stats.precedence_resolved, 3);
    assert_eq!(stats.associativity_resolved, 4);
    assert_eq!(stats.explicit_glr, 5);
    assert_eq!(stats.default_resolved, 6);
}

#[test]
fn stats_clone_independence() {
    let stats = ConflictStats {
        shift_reduce_conflicts: 10,
        ..Default::default()
    };
    let mut cloned = stats.clone();
    cloned.shift_reduce_conflicts = 42;
    assert_eq!(stats.shift_reduce_conflicts, 10);
    assert_eq!(cloned.shift_reduce_conflicts, 42);
}

#[test]
fn stats_debug_contains_all_field_names() {
    let stats = ConflictStats {
        shift_reduce_conflicts: 1,
        reduce_reduce_conflicts: 2,
        precedence_resolved: 3,
        associativity_resolved: 4,
        explicit_glr: 5,
        default_resolved: 6,
    };
    let dbg = format!("{stats:?}");
    assert!(dbg.contains("reduce_reduce_conflicts"));
    assert!(dbg.contains("precedence_resolved"));
    assert!(dbg.contains("associativity_resolved"));
    assert!(dbg.contains("explicit_glr"));
    assert!(dbg.contains("default_resolved"));
}

// =========================================================================
// ConflictAnalyzer tests
// =========================================================================

#[test]
fn analyzer_new_has_zero_stats() {
    let analyzer = ConflictAnalyzer::new();
    let s = analyzer.get_stats();
    assert_eq!(s.shift_reduce_conflicts, 0);
    assert_eq!(s.reduce_reduce_conflicts, 0);
}

#[test]
fn analyzer_default_equals_new() {
    let a = ConflictAnalyzer::default();
    let b = ConflictAnalyzer::new();
    assert_eq!(
        a.get_stats().shift_reduce_conflicts,
        b.get_stats().shift_reduce_conflicts
    );
    assert_eq!(
        a.get_stats().reduce_reduce_conflicts,
        b.get_stats().reduce_reduce_conflicts
    );
}

#[test]
fn analyze_trivial_table_no_conflicts() {
    let grammar = GrammarBuilder::new("trivial")
        .token("A", "a")
        .rule("start", vec!["A"])
        .start("start")
        .build();
    let table = minimal_table(grammar);
    let mut analyzer = ConflictAnalyzer::new();
    let stats = analyzer.analyze_table(&table);
    assert_eq!(stats.shift_reduce_conflicts, 0);
    assert_eq!(stats.reduce_reduce_conflicts, 0);
}

#[test]
fn analyze_table_returns_same_as_get_stats() {
    let grammar = GrammarBuilder::new("t")
        .token("X", "x")
        .rule("s", vec!["X"])
        .start("s")
        .build();
    let table = minimal_table(grammar);
    let mut analyzer = ConflictAnalyzer::new();
    let returned = analyzer.analyze_table(&table);
    let stored = analyzer.get_stats().clone();
    assert_eq!(
        returned.shift_reduce_conflicts,
        stored.shift_reduce_conflicts
    );
    assert_eq!(
        returned.reduce_reduce_conflicts,
        stored.reduce_reduce_conflicts
    );
}

#[test]
fn analyze_table_resets_stats_on_second_call() {
    let grammar = GrammarBuilder::new("t")
        .token("X", "x")
        .rule("s", vec!["X"])
        .start("s")
        .build();
    let table = minimal_table(grammar);
    let mut analyzer = ConflictAnalyzer::new();
    let _ = analyzer.analyze_table(&table);
    let stats = analyzer.analyze_table(&table);
    assert_eq!(stats.shift_reduce_conflicts, 0);
}

#[test]
fn analyze_default_table() {
    let table = ParseTable::default();
    let mut analyzer = ConflictAnalyzer::new();
    let stats = analyzer.analyze_table(&table);
    assert_eq!(stats.shift_reduce_conflicts, 0);
}

#[test]
fn analyzer_all_stats_zero_after_analyze() {
    let grammar = GrammarBuilder::new("simple")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let table = minimal_table(grammar);
    let mut analyzer = ConflictAnalyzer::new();
    let stats = analyzer.analyze_table(&table);
    assert_eq!(stats.precedence_resolved, 0);
    assert_eq!(stats.associativity_resolved, 0);
    assert_eq!(stats.explicit_glr, 0);
    assert_eq!(stats.default_resolved, 0);
}

#[test]
fn analyzer_multiple_tables_independent() {
    let g1 = GrammarBuilder::new("g1")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let g2 = GrammarBuilder::new("g2")
        .token("B", "b")
        .rule("s", vec!["B"])
        .start("s")
        .build();
    let t1 = minimal_table(g1);
    let t2 = minimal_table(g2);
    let mut analyzer = ConflictAnalyzer::new();
    let s1 = analyzer.analyze_table(&t1);
    let s2 = analyzer.analyze_table(&t2);
    assert_eq!(s1.shift_reduce_conflicts, s2.shift_reduce_conflicts);
}

// =========================================================================
// PrecedenceDecision tests
// =========================================================================

#[test]
fn decision_eq() {
    assert_eq!(
        PrecedenceDecision::PreferShift,
        PrecedenceDecision::PreferShift
    );
    assert_eq!(
        PrecedenceDecision::PreferReduce,
        PrecedenceDecision::PreferReduce
    );
    assert_eq!(PrecedenceDecision::Error, PrecedenceDecision::Error);
}

#[test]
fn decision_ne() {
    assert_ne!(
        PrecedenceDecision::PreferShift,
        PrecedenceDecision::PreferReduce
    );
    assert_ne!(PrecedenceDecision::PreferReduce, PrecedenceDecision::Error);
    assert_ne!(PrecedenceDecision::PreferShift, PrecedenceDecision::Error);
}

#[test]
fn decision_clone() {
    let d = PrecedenceDecision::PreferShift;
    assert_eq!(d, d.clone());
}

#[test]
fn decision_debug_format() {
    let dbg = format!("{:?}", PrecedenceDecision::Error);
    assert!(dbg.contains("Error"));
}

#[test]
fn decision_debug_prefer_shift() {
    let dbg = format!("{:?}", PrecedenceDecision::PreferShift);
    assert!(dbg.contains("PreferShift"));
}

#[test]
fn decision_debug_prefer_reduce() {
    let dbg = format!("{:?}", PrecedenceDecision::PreferReduce);
    assert!(dbg.contains("PreferReduce"));
}

#[test]
fn decision_clone_all_variants() {
    for d in [
        PrecedenceDecision::PreferShift,
        PrecedenceDecision::PreferReduce,
        PrecedenceDecision::Error,
    ] {
        assert_eq!(d, d.clone());
    }
}

// =========================================================================
// PrecedenceResolver — single operator grammars
// =========================================================================

/// Build a single-operator grammar: `s → s OP s | NUM`
fn single_op_grammar(
    op_name: &str,
    prec: i16,
    assoc: Associativity,
) -> (Grammar, SymbolId, SymbolId) {
    let grammar = GrammarBuilder::new("single_op")
        .token("NUM", r"\d+")
        .token(op_name, op_name)
        .precedence(prec, assoc, vec![op_name])
        .rule_with_precedence("s", vec!["s", op_name, "s"], prec, assoc)
        .rule("s", vec!["NUM"])
        .start("s")
        .build();
    let op = sym(&grammar, op_name);
    let s = sym(&grammar, "s");
    (grammar, op, s)
}

#[test]
fn resolver_shift_higher_precedence_prefers_shift() {
    let grammar = GrammarBuilder::new("hi_shift")
        .token("A", "a")
        .token("B", "b")
        .precedence(5, Associativity::Left, vec!["B"])
        .rule_with_precedence("s", vec!["s", "A", "s"], 2, Associativity::Left)
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let b = sym(&grammar, "B");
    let s = sym(&grammar, "s");
    let resolver = PrecedenceResolver::new(&grammar);
    assert_eq!(
        resolver.can_resolve_shift_reduce(b, s),
        Some(PrecedenceDecision::PreferShift)
    );
}

#[test]
fn resolver_shift_lower_precedence_prefers_reduce() {
    let grammar = GrammarBuilder::new("lo_shift")
        .token("A", "a")
        .token("B", "b")
        .precedence(1, Associativity::Left, vec!["A"])
        .rule_with_precedence("s", vec!["s", "A", "s"], 3, Associativity::Left)
        .rule("s", vec!["B"])
        .start("s")
        .build();
    let a = sym(&grammar, "A");
    let s = sym(&grammar, "s");
    let resolver = PrecedenceResolver::new(&grammar);
    assert_eq!(
        resolver.can_resolve_shift_reduce(a, s),
        Some(PrecedenceDecision::PreferReduce)
    );
}

#[test]
fn resolver_same_prec_left_assoc_prefers_reduce() {
    let (grammar, op, s) = single_op_grammar("+", 1, Associativity::Left);
    let resolver = PrecedenceResolver::new(&grammar);
    assert_eq!(
        resolver.can_resolve_shift_reduce(op, s),
        Some(PrecedenceDecision::PreferReduce)
    );
}

#[test]
fn resolver_same_prec_right_assoc_prefers_shift() {
    let (grammar, op, s) = single_op_grammar("^", 1, Associativity::Right);
    let resolver = PrecedenceResolver::new(&grammar);
    assert_eq!(
        resolver.can_resolve_shift_reduce(op, s),
        Some(PrecedenceDecision::PreferShift)
    );
}

#[test]
fn resolver_same_prec_none_assoc_returns_error() {
    let (grammar, op, s) = single_op_grammar("~", 1, Associativity::None);
    let resolver = PrecedenceResolver::new(&grammar);
    assert_eq!(
        resolver.can_resolve_shift_reduce(op, s),
        Some(PrecedenceDecision::Error)
    );
}

#[test]
fn resolver_left_assoc_at_different_prec_levels() {
    // Both left-assoc but at different levels: higher wins regardless of assoc.
    let grammar = GrammarBuilder::new("levels")
        .token("NUM", r"\d+")
        .token("LO", "+")
        .token("HI", "*")
        .precedence(1, Associativity::Left, vec!["LO"])
        .precedence(5, Associativity::Left, vec!["HI"])
        .rule_with_precedence("s", vec!["s", "LO", "s"], 1, Associativity::Left)
        .rule("s", vec!["NUM"])
        .start("s")
        .build();
    let resolver = PrecedenceResolver::new(&grammar);
    let hi = sym(&grammar, "HI");
    let s = sym(&grammar, "s");
    assert_eq!(
        resolver.can_resolve_shift_reduce(hi, s),
        Some(PrecedenceDecision::PreferShift)
    );
}

#[test]
fn resolver_right_assoc_at_higher_level_prefers_shift() {
    let grammar = GrammarBuilder::new("r_hi")
        .token("NUM", r"\d+")
        .token("OP", "=")
        .precedence(10, Associativity::Right, vec!["OP"])
        .rule_with_precedence("s", vec!["s", "OP", "s"], 5, Associativity::Left)
        .rule("s", vec!["NUM"])
        .start("s")
        .build();
    let resolver = PrecedenceResolver::new(&grammar);
    let op = sym(&grammar, "OP");
    let s = sym(&grammar, "s");
    assert_eq!(
        resolver.can_resolve_shift_reduce(op, s),
        Some(PrecedenceDecision::PreferShift)
    );
}

#[test]
fn resolver_negative_precedence_levels() {
    let grammar = GrammarBuilder::new("neg")
        .token("NUM", r"\d+")
        .token("OP", "-")
        .precedence(-5, Associativity::Left, vec!["OP"])
        .rule_with_precedence("s", vec!["s", "OP", "s"], -5, Associativity::Left)
        .rule("s", vec!["NUM"])
        .start("s")
        .build();
    let resolver = PrecedenceResolver::new(&grammar);
    let op = sym(&grammar, "OP");
    let s = sym(&grammar, "s");
    // Same prec -5, left assoc → reduce
    assert_eq!(
        resolver.can_resolve_shift_reduce(op, s),
        Some(PrecedenceDecision::PreferReduce)
    );
}

#[test]
fn resolver_zero_precedence_level() {
    let grammar = GrammarBuilder::new("zero")
        .token("NUM", r"\d+")
        .token("OP", "|")
        .precedence(0, Associativity::Right, vec!["OP"])
        .rule_with_precedence("s", vec!["s", "OP", "s"], 0, Associativity::Right)
        .rule("s", vec!["NUM"])
        .start("s")
        .build();
    let resolver = PrecedenceResolver::new(&grammar);
    let op = sym(&grammar, "OP");
    let s = sym(&grammar, "s");
    assert_eq!(
        resolver.can_resolve_shift_reduce(op, s),
        Some(PrecedenceDecision::PreferShift)
    );
}

// =========================================================================
// PrecedenceResolver — missing information
// =========================================================================

#[test]
fn resolver_unknown_shift_symbol_returns_none() {
    let (grammar, _op, s) = single_op_grammar("+", 1, Associativity::Left);
    let resolver = PrecedenceResolver::new(&grammar);
    let unknown = SymbolId(999);
    assert_eq!(resolver.can_resolve_shift_reduce(unknown, s), None);
}

#[test]
fn resolver_unknown_reduce_symbol_returns_none() {
    let (grammar, op, _s) = single_op_grammar("+", 1, Associativity::Left);
    let resolver = PrecedenceResolver::new(&grammar);
    let unknown = SymbolId(999);
    assert_eq!(resolver.can_resolve_shift_reduce(op, unknown), None);
}

#[test]
fn resolver_both_unknown_returns_none() {
    let grammar = GrammarBuilder::new("empty")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let resolver = PrecedenceResolver::new(&grammar);
    assert_eq!(
        resolver.can_resolve_shift_reduce(SymbolId(100), SymbolId(200)),
        None
    );
}

// =========================================================================
// PrecedenceResolver — grammar with no precedence info
// =========================================================================

#[test]
fn resolver_no_precedence_always_returns_none() {
    let grammar = GrammarBuilder::new("noprec")
        .token("X", "x")
        .token("Y", "y")
        .rule("s", vec!["X", "Y"])
        .start("s")
        .build();
    let resolver = PrecedenceResolver::new(&grammar);
    let x = sym(&grammar, "X");
    let s = sym(&grammar, "s");
    assert_eq!(resolver.can_resolve_shift_reduce(x, s), None);
}

// =========================================================================
// PrecedenceResolver — complex grammars
// =========================================================================

#[test]
fn resolver_javascript_like_grammar() {
    let grammar = GrammarBuilder::javascript_like();
    let resolver = PrecedenceResolver::new(&grammar);
    let plus = sym(&grammar, "+");
    let expr = sym(&grammar, "expression");
    assert_eq!(resolver.can_resolve_shift_reduce(plus, expr), None);
}

#[test]
fn resolver_multi_level_precedence() {
    let grammar = GrammarBuilder::new("multi")
        .token("NUM", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .token("^", "^")
        .precedence(1, Associativity::Left, vec!["+"])
        .precedence(2, Associativity::Left, vec!["*"])
        .precedence(3, Associativity::Right, vec!["^"])
        .rule_with_precedence("add", vec!["add", "+", "add"], 1, Associativity::Left)
        .rule("add", vec!["NUM"])
        .rule_with_precedence("mul", vec!["mul", "*", "mul"], 2, Associativity::Left)
        .rule("mul", vec!["NUM"])
        .rule_with_precedence("pow", vec!["pow", "^", "pow"], 3, Associativity::Right)
        .rule("pow", vec!["NUM"])
        .start("add")
        .build();

    let resolver = PrecedenceResolver::new(&grammar);
    let plus = sym(&grammar, "+");
    let star = sym(&grammar, "*");
    let caret = sym(&grammar, "^");
    let add = sym(&grammar, "add");
    let mul = sym(&grammar, "mul");
    let pow = sym(&grammar, "pow");

    assert_eq!(
        resolver.can_resolve_shift_reduce(caret, add),
        Some(PrecedenceDecision::PreferShift)
    );
    assert_eq!(
        resolver.can_resolve_shift_reduce(star, add),
        Some(PrecedenceDecision::PreferShift)
    );
    assert_eq!(
        resolver.can_resolve_shift_reduce(plus, add),
        Some(PrecedenceDecision::PreferReduce)
    );
    assert_eq!(
        resolver.can_resolve_shift_reduce(plus, mul),
        Some(PrecedenceDecision::PreferReduce)
    );
    assert_eq!(
        resolver.can_resolve_shift_reduce(caret, pow),
        Some(PrecedenceDecision::PreferShift)
    );
}

// =========================================================================
// PrecedenceResolver — precedence from grammar.precedences only
// =========================================================================

#[test]
fn resolver_uses_precedences_from_grammar_declarations() {
    let mut grammar = Grammar::new("raw".to_string());
    let tok_a = SymbolId(1);
    let tok_b = SymbolId(2);
    let nt = SymbolId(10);

    grammar.tokens.insert(
        tok_a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        tok_b,
        Token {
            name: "b".into(),
            pattern: TokenPattern::String("b".into()),
            fragile: false,
        },
    );
    grammar.precedences.push(Precedence {
        level: 5,
        associativity: Associativity::Left,
        symbols: vec![tok_a],
    });
    grammar.precedences.push(Precedence {
        level: 10,
        associativity: Associativity::Right,
        symbols: vec![tok_b],
    });
    grammar.rules.insert(
        nt,
        vec![Rule {
            lhs: nt,
            rhs: vec![Symbol::Terminal(tok_a)],
            precedence: Some(PrecedenceKind::Static(5)),
            associativity: Some(Associativity::Left),
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );

    let resolver = PrecedenceResolver::new(&grammar);
    assert_eq!(
        resolver.can_resolve_shift_reduce(tok_b, nt),
        Some(PrecedenceDecision::PreferShift)
    );
    assert_eq!(
        resolver.can_resolve_shift_reduce(tok_a, nt),
        Some(PrecedenceDecision::PreferReduce)
    );
}

// =========================================================================
// PrecedenceResolver — multiple rules on the same nonterminal
// =========================================================================

#[test]
fn resolver_picks_first_annotated_rule_for_symbol_prec() {
    let grammar = GrammarBuilder::new("multi_rule")
        .token("A", "a")
        .token("B", "b")
        .precedence(1, Associativity::Left, vec!["A"])
        .precedence(2, Associativity::Left, vec!["B"])
        .rule_with_precedence("s", vec!["s", "A", "s"], 1, Associativity::Left)
        .rule_with_precedence("s", vec!["s", "B", "s"], 2, Associativity::Left)
        .rule("s", vec!["A"])
        .start("s")
        .build();

    let resolver = PrecedenceResolver::new(&grammar);
    let a = sym(&grammar, "A");
    let s = sym(&grammar, "s");
    assert!(resolver.can_resolve_shift_reduce(a, s).is_some());
}

// =========================================================================
// Edge cases
// =========================================================================

#[test]
fn resolver_empty_grammar_returns_none() {
    let grammar = Grammar::new("empty".to_string());
    let resolver = PrecedenceResolver::new(&grammar);
    assert_eq!(
        resolver.can_resolve_shift_reduce(SymbolId(0), SymbolId(0)),
        None
    );
}

#[test]
fn resolver_grammar_with_only_tokens_no_rules() {
    let grammar = GrammarBuilder::new("tokens_only")
        .token("X", "x")
        .precedence(1, Associativity::Left, vec!["X"])
        .build();
    let resolver = PrecedenceResolver::new(&grammar);
    let x = sym(&grammar, "X");
    assert_eq!(resolver.can_resolve_shift_reduce(x, SymbolId(999)), None);
}

#[test]
fn resolver_rule_without_associativity_is_not_stored() {
    let mut grammar = Grammar::new("no_assoc".to_string());
    let tok = SymbolId(1);
    let nt = SymbolId(10);
    grammar.tokens.insert(
        tok,
        Token {
            name: "t".into(),
            pattern: TokenPattern::String("t".into()),
            fragile: false,
        },
    );
    grammar.precedences.push(Precedence {
        level: 1,
        associativity: Associativity::Left,
        symbols: vec![tok],
    });
    grammar.rules.insert(
        nt,
        vec![Rule {
            lhs: nt,
            rhs: vec![Symbol::Terminal(tok)],
            precedence: Some(PrecedenceKind::Static(1)),
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );

    let resolver = PrecedenceResolver::new(&grammar);
    assert_eq!(resolver.can_resolve_shift_reduce(tok, nt), None);
}

#[test]
fn resolver_rule_without_precedence_is_not_stored() {
    let mut grammar = Grammar::new("no_prec_rule".to_string());
    let tok = SymbolId(1);
    let nt = SymbolId(10);
    grammar.tokens.insert(
        tok,
        Token {
            name: "t".into(),
            pattern: TokenPattern::String("t".into()),
            fragile: false,
        },
    );
    grammar.precedences.push(Precedence {
        level: 1,
        associativity: Associativity::Left,
        symbols: vec![tok],
    });
    grammar.rules.insert(
        nt,
        vec![Rule {
            lhs: nt,
            rhs: vec![Symbol::Terminal(tok)],
            precedence: None, // no precedence on the rule
            associativity: Some(Associativity::Left),
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );

    let resolver = PrecedenceResolver::new(&grammar);
    assert_eq!(resolver.can_resolve_shift_reduce(tok, nt), None);
}

#[test]
fn resolver_multiple_tokens_same_precedence_level() {
    let mut grammar = Grammar::new("multi_tok".to_string());
    let tok_a = SymbolId(1);
    let tok_b = SymbolId(2);
    let nt = SymbolId(10);
    grammar.tokens.insert(
        tok_a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        tok_b,
        Token {
            name: "b".into(),
            pattern: TokenPattern::String("b".into()),
            fragile: false,
        },
    );
    // Both tokens at the same precedence level
    grammar.precedences.push(Precedence {
        level: 3,
        associativity: Associativity::Left,
        symbols: vec![tok_a, tok_b],
    });
    grammar.rules.insert(
        nt,
        vec![Rule {
            lhs: nt,
            rhs: vec![Symbol::Terminal(tok_a)],
            precedence: Some(PrecedenceKind::Static(3)),
            associativity: Some(Associativity::Left),
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );

    let resolver = PrecedenceResolver::new(&grammar);
    // Both should resolve the same way (same prec, left assoc → reduce)
    assert_eq!(
        resolver.can_resolve_shift_reduce(tok_a, nt),
        Some(PrecedenceDecision::PreferReduce)
    );
    assert_eq!(
        resolver.can_resolve_shift_reduce(tok_b, nt),
        Some(PrecedenceDecision::PreferReduce)
    );
}

#[test]
fn analyzer_with_javascript_like_grammar() {
    let grammar = GrammarBuilder::javascript_like();
    let table = minimal_table(grammar);
    let mut analyzer = ConflictAnalyzer::new();
    let stats = analyzer.analyze_table(&table);
    assert_eq!(stats.shift_reduce_conflicts, 0);
}

#[test]
fn analyzer_with_python_like_grammar() {
    let grammar = GrammarBuilder::python_like();
    let table = minimal_table(grammar);
    let mut analyzer = ConflictAnalyzer::new();
    let stats = analyzer.analyze_table(&table);
    assert_eq!(stats.reduce_reduce_conflicts, 0);
}

// =========================================================================
// ConflictResolver::detect_conflicts — ambiguous grammar
// =========================================================================

#[test]
fn detect_conflicts_ambiguous_ee_grammar_has_conflicts() {
    let grammar = ambiguous_ee_grammar();
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    let resolver = ConflictResolver::detect_conflicts(&collection, &grammar, &ff);
    assert!(
        !resolver.conflicts.is_empty(),
        "E → a | E E should produce conflicts"
    );
}

#[test]
fn detect_conflicts_ambiguous_ee_has_shift_reduce() {
    let grammar = ambiguous_ee_grammar();
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    let resolver = ConflictResolver::detect_conflicts(&collection, &grammar, &ff);
    let has_sr = resolver
        .conflicts
        .iter()
        .any(|c| c.conflict_type == ConflictType::ShiftReduce);
    assert!(has_sr, "E → a | E E should have shift-reduce conflicts");
}

#[test]
fn detect_conflicts_ambiguous_ee_conflict_has_multiple_actions() {
    let grammar = ambiguous_ee_grammar();
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    let resolver = ConflictResolver::detect_conflicts(&collection, &grammar, &ff);
    for conflict in &resolver.conflicts {
        assert!(
            conflict.actions.len() > 1,
            "Each conflict should have >1 action"
        );
    }
}

// =========================================================================
// ConflictResolver::detect_conflicts — simple unambiguous grammar
// =========================================================================

#[test]
fn detect_conflicts_simple_unambiguous_no_conflicts() {
    // S → a b
    let mut grammar = Grammar::new("simple".into());
    let a = SymbolId(1);
    let b = SymbolId(2);
    let s = SymbolId(10);
    grammar.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        b,
        Token {
            name: "b".into(),
            pattern: TokenPattern::String("b".into()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(s, "S".into());
    grammar.rules.insert(
        s,
        vec![Rule {
            lhs: s,
            rhs: vec![Symbol::Terminal(a), Symbol::Terminal(b)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    grammar.set_start_symbol(s);
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    let resolver = ConflictResolver::detect_conflicts(&collection, &grammar, &ff);
    assert!(
        resolver.conflicts.is_empty(),
        "S → a b should have no conflicts"
    );
}

// =========================================================================
// ConflictResolver::detect_conflicts — reduce-reduce conflict
// =========================================================================

#[test]
fn detect_conflicts_reduce_reduce() {
    // S → A | B, A → a, B → a — same token reduces to two different rules
    let mut grammar = Grammar::new("rr".into());
    let tok_a = SymbolId(1);
    let s_sym = SymbolId(10);
    let a_sym = SymbolId(11);
    let b_sym = SymbolId(12);

    grammar.tokens.insert(
        tok_a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(s_sym, "S".into());
    grammar.rule_names.insert(a_sym, "A".into());
    grammar.rule_names.insert(b_sym, "B".into());
    grammar.rules.insert(
        s_sym,
        vec![
            Rule {
                lhs: s_sym,
                rhs: vec![Symbol::NonTerminal(a_sym)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            },
            Rule {
                lhs: s_sym,
                rhs: vec![Symbol::NonTerminal(b_sym)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(1),
            },
        ],
    );
    grammar.rules.insert(
        a_sym,
        vec![Rule {
            lhs: a_sym,
            rhs: vec![Symbol::Terminal(tok_a)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(2),
        }],
    );
    grammar.rules.insert(
        b_sym,
        vec![Rule {
            lhs: b_sym,
            rhs: vec![Symbol::Terminal(tok_a)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(3),
        }],
    );

    grammar.set_start_symbol(s_sym);
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    let resolver = ConflictResolver::detect_conflicts(&collection, &grammar, &ff);

    let has_rr = resolver
        .conflicts
        .iter()
        .any(|c| c.conflict_type == ConflictType::ReduceReduce);
    assert!(has_rr, "S → A | B, A → a, B → a should have RR conflict");
}

// =========================================================================
// ConflictResolver::resolve_conflicts
// =========================================================================

#[test]
fn resolve_conflicts_reduces_action_count_for_ambiguous_grammar() {
    let grammar = ambiguous_ee_grammar();
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    let mut resolver = ConflictResolver::detect_conflicts(&collection, &grammar, &ff);
    let before_count: usize = resolver.conflicts.iter().map(|c| c.actions.len()).sum();
    resolver.resolve_conflicts(&grammar);
    let after_count: usize = resolver.conflicts.iter().map(|c| c.actions.len()).sum();
    // resolve_conflicts should not increase total action count
    assert!(after_count <= before_count);
}

#[test]
fn resolve_conflicts_on_empty_conflicts_is_noop() {
    let grammar = Grammar::new("noop".into());
    let mut resolver = ConflictResolver { conflicts: vec![] };
    resolver.resolve_conflicts(&grammar);
    assert!(resolver.conflicts.is_empty());
}

#[test]
fn resolve_shift_reduce_with_no_prec_uses_fork() {
    // Manually construct a shift-reduce conflict with no precedence info
    let grammar = Grammar::new("no_prec".into());
    let mut resolver = ConflictResolver {
        conflicts: vec![Conflict {
            state: StateId(0),
            symbol: SymbolId(1),
            actions: vec![Action::Shift(StateId(5)), Action::Reduce(RuleId(2))],
            conflict_type: ConflictType::ShiftReduce,
        }],
    };
    resolver.resolve_conflicts(&grammar);
    // No precedence info → Fork
    assert_eq!(resolver.conflicts.len(), 1);
    let actions = &resolver.conflicts[0].actions;
    assert_eq!(actions.len(), 1);
    assert!(matches!(&actions[0], Action::Fork(_)));
}

#[test]
fn resolve_reduce_reduce_picks_lower_rule_id() {
    let grammar = Grammar::new("rr".into());
    let mut resolver = ConflictResolver {
        conflicts: vec![Conflict {
            state: StateId(0),
            symbol: SymbolId(1),
            actions: vec![Action::Reduce(RuleId(5)), Action::Reduce(RuleId(2))],
            conflict_type: ConflictType::ReduceReduce,
        }],
    };
    resolver.resolve_conflicts(&grammar);
    assert_eq!(resolver.conflicts.len(), 1);
    let actions = &resolver.conflicts[0].actions;
    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0], Action::Reduce(RuleId(2))));
}

#[test]
fn resolve_reduce_reduce_with_same_rule_id() {
    let grammar = Grammar::new("rr_same".into());
    let mut resolver = ConflictResolver {
        conflicts: vec![Conflict {
            state: StateId(0),
            symbol: SymbolId(1),
            actions: vec![Action::Reduce(RuleId(3)), Action::Reduce(RuleId(3))],
            conflict_type: ConflictType::ReduceReduce,
        }],
    };
    resolver.resolve_conflicts(&grammar);
    let actions = &resolver.conflicts[0].actions;
    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0], Action::Reduce(RuleId(3))));
}

#[test]
fn resolve_reduce_reduce_three_actions_picks_lowest() {
    let grammar = Grammar::new("rr3".into());
    let mut resolver = ConflictResolver {
        conflicts: vec![Conflict {
            state: StateId(0),
            symbol: SymbolId(1),
            actions: vec![
                Action::Reduce(RuleId(10)),
                Action::Reduce(RuleId(1)),
                Action::Reduce(RuleId(7)),
            ],
            conflict_type: ConflictType::ReduceReduce,
        }],
    };
    resolver.resolve_conflicts(&grammar);
    let actions = &resolver.conflicts[0].actions;
    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0], Action::Reduce(RuleId(1))));
}

// =========================================================================
// ConflictResolver with precedence-resolved grammars
// =========================================================================

#[test]
fn resolve_shift_reduce_with_precedence_left_assoc() {
    // Build a grammar where left-assoc at same prec should prefer reduce
    let grammar = arithmetic_prec_grammar(1, Associativity::Left, 2, Associativity::Left);
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    let mut resolver = ConflictResolver::detect_conflicts(&collection, &grammar, &ff);
    resolver.resolve_conflicts(&grammar);
    // After resolution, each conflict should have at least one action
    for conflict in &resolver.conflicts {
        assert!(
            !conflict.actions.is_empty(),
            "Resolved conflict should have at least one action"
        );
    }
}

#[test]
fn resolve_shift_reduce_with_precedence_right_assoc() {
    let grammar = arithmetic_prec_grammar(1, Associativity::Right, 2, Associativity::Right);
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    let mut resolver = ConflictResolver::detect_conflicts(&collection, &grammar, &ff);
    resolver.resolve_conflicts(&grammar);
    for conflict in &resolver.conflicts {
        assert!(!conflict.actions.is_empty());
    }
}

#[test]
fn resolve_shift_reduce_with_precedence_none_assoc() {
    let grammar = arithmetic_prec_grammar(1, Associativity::None, 2, Associativity::None);
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    let mut resolver = ConflictResolver::detect_conflicts(&collection, &grammar, &ff);
    resolver.resolve_conflicts(&grammar);
    // Conflicts should still exist, resolved to Fork or Error
    for conflict in &resolver.conflicts {
        assert!(!conflict.actions.is_empty());
    }
}

// =========================================================================
// Conflict struct & ConflictType tests
// =========================================================================

#[test]
fn conflict_type_eq() {
    assert_eq!(ConflictType::ShiftReduce, ConflictType::ShiftReduce);
    assert_eq!(ConflictType::ReduceReduce, ConflictType::ReduceReduce);
}

#[test]
fn conflict_type_ne() {
    assert_ne!(ConflictType::ShiftReduce, ConflictType::ReduceReduce);
}

#[test]
fn conflict_type_debug() {
    let dbg = format!("{:?}", ConflictType::ShiftReduce);
    assert!(dbg.contains("ShiftReduce"));
    let dbg2 = format!("{:?}", ConflictType::ReduceReduce);
    assert!(dbg2.contains("ReduceReduce"));
}

#[test]
fn conflict_type_clone() {
    let ct = ConflictType::ShiftReduce;
    assert_eq!(ct, ct.clone());
}

#[test]
fn conflict_struct_debug() {
    let c = Conflict {
        state: StateId(0),
        symbol: SymbolId(1),
        actions: vec![Action::Shift(StateId(2))],
        conflict_type: ConflictType::ShiftReduce,
    };
    let dbg = format!("{c:?}");
    assert!(dbg.contains("Conflict"));
}

#[test]
fn conflict_struct_clone() {
    let c = Conflict {
        state: StateId(3),
        symbol: SymbolId(7),
        actions: vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(2))],
        conflict_type: ConflictType::ShiftReduce,
    };
    let c2 = c.clone();
    assert_eq!(c2.state, StateId(3));
    assert_eq!(c2.symbol, SymbolId(7));
    assert_eq!(c2.actions.len(), 2);
    assert_eq!(c2.conflict_type, ConflictType::ShiftReduce);
}

// =========================================================================
// Action enum edge cases in conflict context
// =========================================================================

#[test]
fn resolve_conflict_with_accept_action_preserves_it() {
    // If one action is Accept and one is Reduce, Accept is not a reduce,
    // so resolve_reduce_reduce should still work normally.
    let grammar = Grammar::new("accept".into());
    let mut resolver = ConflictResolver {
        conflicts: vec![Conflict {
            state: StateId(0),
            symbol: SymbolId(0),
            actions: vec![Action::Accept, Action::Reduce(RuleId(1))],
            conflict_type: ConflictType::ShiftReduce,
        }],
    };
    // resolve_conflicts processes as shift-reduce but Accept doesn't match Shift
    resolver.resolve_conflicts(&grammar);
    // The original actions are kept since neither matches shift+reduce pair
    assert!(!resolver.conflicts[0].actions.is_empty());
}

#[test]
fn resolve_multiple_conflicts_independently() {
    let grammar = Grammar::new("multi".into());
    let mut resolver = ConflictResolver {
        conflicts: vec![
            Conflict {
                state: StateId(0),
                symbol: SymbolId(1),
                actions: vec![Action::Reduce(RuleId(10)), Action::Reduce(RuleId(3))],
                conflict_type: ConflictType::ReduceReduce,
            },
            Conflict {
                state: StateId(1),
                symbol: SymbolId(2),
                actions: vec![Action::Reduce(RuleId(7)), Action::Reduce(RuleId(1))],
                conflict_type: ConflictType::ReduceReduce,
            },
        ],
    };
    resolver.resolve_conflicts(&grammar);
    assert_eq!(resolver.conflicts.len(), 2);
    assert!(matches!(
        resolver.conflicts[0].actions[0],
        Action::Reduce(RuleId(3))
    ));
    assert!(matches!(
        resolver.conflicts[1].actions[0],
        Action::Reduce(RuleId(1))
    ));
}

// =========================================================================
// End-to-end: detect + resolve on realistic grammars
// =========================================================================

#[test]
fn detect_and_resolve_arithmetic_grammar() {
    let grammar = arithmetic_prec_grammar(1, Associativity::Left, 2, Associativity::Left);
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    let mut resolver = ConflictResolver::detect_conflicts(&collection, &grammar, &ff);
    let initial_conflicts = resolver.conflicts.len();
    resolver.resolve_conflicts(&grammar);
    // Conflict count should remain the same (entries don't get removed, actions get reduced)
    assert_eq!(resolver.conflicts.len(), initial_conflicts);
}

#[test]
fn detect_conflicts_single_token_grammar_no_conflicts() {
    // S → a
    let mut grammar = Grammar::new("single".into());
    let a = SymbolId(1);
    let s = SymbolId(10);
    grammar.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(s, "S".into());
    grammar.rules.insert(
        s,
        vec![Rule {
            lhs: s,
            rhs: vec![Symbol::Terminal(a)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    grammar.set_start_symbol(s);
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    let resolver = ConflictResolver::detect_conflicts(&collection, &grammar, &ff);
    assert!(resolver.conflicts.is_empty());
}

#[test]
fn detect_conflicts_chain_grammar_no_conflicts() {
    // S → A, A → B, B → c  (no ambiguity)
    let mut grammar = Grammar::new("chain".into());
    let c = SymbolId(1);
    let s = SymbolId(10);
    let a = SymbolId(11);
    let b = SymbolId(12);
    grammar.tokens.insert(
        c,
        Token {
            name: "c".into(),
            pattern: TokenPattern::String("c".into()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(s, "S".into());
    grammar.rule_names.insert(a, "A".into());
    grammar.rule_names.insert(b, "B".into());
    grammar.rules.insert(
        s,
        vec![Rule {
            lhs: s,
            rhs: vec![Symbol::NonTerminal(a)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    grammar.rules.insert(
        a,
        vec![Rule {
            lhs: a,
            rhs: vec![Symbol::NonTerminal(b)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(1),
        }],
    );
    grammar.rules.insert(
        b,
        vec![Rule {
            lhs: b,
            rhs: vec![Symbol::Terminal(c)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(2),
        }],
    );
    grammar.set_start_symbol(s);
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    let resolver = ConflictResolver::detect_conflicts(&collection, &grammar, &ff);
    assert!(
        resolver.conflicts.is_empty(),
        "Chain grammar S → A → B → c should have no conflicts"
    );
}

// =========================================================================
// ConflictResolver with single-action cells (no conflict)
// =========================================================================

#[test]
fn resolve_single_action_shift_is_noop() {
    let grammar = Grammar::new("sa".into());
    let mut resolver = ConflictResolver {
        conflicts: vec![Conflict {
            state: StateId(0),
            symbol: SymbolId(1),
            actions: vec![Action::Shift(StateId(1))],
            conflict_type: ConflictType::ShiftReduce,
        }],
    };
    resolver.resolve_conflicts(&grammar);
    // Single shift action should be preserved as-is
    assert_eq!(resolver.conflicts[0].actions.len(), 1);
    assert!(matches!(
        resolver.conflicts[0].actions[0],
        Action::Shift(StateId(1))
    ));
}

#[test]
fn resolve_single_action_reduce_is_noop() {
    let grammar = Grammar::new("sr".into());
    let mut resolver = ConflictResolver {
        conflicts: vec![Conflict {
            state: StateId(0),
            symbol: SymbolId(1),
            actions: vec![Action::Reduce(RuleId(0))],
            conflict_type: ConflictType::ReduceReduce,
        }],
    };
    resolver.resolve_conflicts(&grammar);
    assert_eq!(resolver.conflicts[0].actions.len(), 1);
    assert!(matches!(
        resolver.conflicts[0].actions[0],
        Action::Reduce(RuleId(0))
    ));
}

// =========================================================================
// ConflictResolver — shift-reduce Fork wraps both actions
// =========================================================================

#[test]
fn shift_reduce_fork_contains_both_actions() {
    let grammar = Grammar::new("fork_check".into());
    let mut resolver = ConflictResolver {
        conflicts: vec![Conflict {
            state: StateId(0),
            symbol: SymbolId(1),
            actions: vec![Action::Shift(StateId(9)), Action::Reduce(RuleId(4))],
            conflict_type: ConflictType::ShiftReduce,
        }],
    };
    resolver.resolve_conflicts(&grammar);
    let actions = &resolver.conflicts[0].actions;
    assert_eq!(actions.len(), 1);
    if let Action::Fork(inner) = &actions[0] {
        assert_eq!(inner.len(), 2);
        assert!(inner.iter().any(|a| matches!(a, Action::Shift(_))));
        assert!(inner.iter().any(|a| matches!(a, Action::Reduce(_))));
    } else {
        panic!("Expected Fork action for unresolvable shift-reduce");
    }
}

// =========================================================================
// Additional PrecedenceResolver edge cases
// =========================================================================

#[test]
fn resolver_large_prec_values() {
    let grammar = GrammarBuilder::new("large")
        .token("NUM", r"\d+")
        .token("OP", "+")
        .precedence(i16::MAX, Associativity::Left, vec!["OP"])
        .rule_with_precedence("s", vec!["s", "OP", "s"], i16::MIN, Associativity::Left)
        .rule("s", vec!["NUM"])
        .start("s")
        .build();
    let resolver = PrecedenceResolver::new(&grammar);
    let op = sym(&grammar, "OP");
    let s = sym(&grammar, "s");
    // MAX > MIN → shift
    assert_eq!(
        resolver.can_resolve_shift_reduce(op, s),
        Some(PrecedenceDecision::PreferShift)
    );
}

#[test]
fn resolver_min_prec_values() {
    let grammar = GrammarBuilder::new("min")
        .token("NUM", r"\d+")
        .token("OP", "+")
        .precedence(i16::MIN, Associativity::Right, vec!["OP"])
        .rule_with_precedence("s", vec!["s", "OP", "s"], i16::MAX, Associativity::Right)
        .rule("s", vec!["NUM"])
        .start("s")
        .build();
    let resolver = PrecedenceResolver::new(&grammar);
    let op = sym(&grammar, "OP");
    let s = sym(&grammar, "s");
    // MIN < MAX → reduce
    assert_eq!(
        resolver.can_resolve_shift_reduce(op, s),
        Some(PrecedenceDecision::PreferReduce)
    );
}

#[test]
fn resolver_precedence_with_multiple_nonterminals() {
    // Each nonterminal gets its own rule precedence
    let grammar = GrammarBuilder::new("multi_nt")
        .token("NUM", r"\d+")
        .token("A", "a")
        .token("B", "b")
        .precedence(1, Associativity::Left, vec!["A"])
        .precedence(2, Associativity::Left, vec!["B"])
        .rule_with_precedence("x", vec!["x", "A", "x"], 1, Associativity::Left)
        .rule("x", vec!["NUM"])
        .rule_with_precedence("y", vec!["y", "B", "y"], 2, Associativity::Left)
        .rule("y", vec!["NUM"])
        .start("x")
        .build();

    let resolver = PrecedenceResolver::new(&grammar);
    let a = sym(&grammar, "A");
    let b = sym(&grammar, "B");
    let x = sym(&grammar, "x");
    let y = sym(&grammar, "y");

    // A(1) vs x(1,left) → reduce
    assert_eq!(
        resolver.can_resolve_shift_reduce(a, x),
        Some(PrecedenceDecision::PreferReduce)
    );
    // B(2) vs y(2,left) → reduce
    assert_eq!(
        resolver.can_resolve_shift_reduce(b, y),
        Some(PrecedenceDecision::PreferReduce)
    );
    // A(1) vs y(2) → reduce (y prec is higher)
    assert_eq!(
        resolver.can_resolve_shift_reduce(a, y),
        Some(PrecedenceDecision::PreferReduce)
    );
    // B(2) vs x(1) → shift (B prec is higher)
    assert_eq!(
        resolver.can_resolve_shift_reduce(b, x),
        Some(PrecedenceDecision::PreferShift)
    );
}
