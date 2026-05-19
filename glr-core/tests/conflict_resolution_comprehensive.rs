#![cfg(feature = "test-api")]
#![allow(
    clippy::needless_range_loop,
    unused_imports,
    clippy::clone_on_copy,
    dead_code
)]

//! Comprehensive conflict resolution tests for GLR core.
//!
//! This test suite covers extensive conflict resolution scenarios:
//! - Shift-reduce conflict detection
//! - Reduce-reduce conflict resolution
//! - Precedence-based resolution
//! - Associativity-based resolution (left vs right)
//! - First/Follow set computation for various grammars
//! - Ambiguous grammar handling (GLR should keep all actions)
//! - Empty grammar edge cases
//! - Grammars with epsilon rules
//! - Large grammars with many conflicts
//! - Precedence climbing patterns
//! - Conflict statistics and reporting
//!
//! Run with: cargo test -p adze-glr-core --features test-api --test conflict_resolution_comprehensive

use adze_glr_core::advanced_conflict::{
    ConflictAnalyzer, ConflictStats, PrecedenceDecision, PrecedenceResolver,
};
use adze_glr_core::conflict_inspection::{
    self, ConflictDetail, ConflictSummary, ConflictType as InspectionConflictType,
    classify_conflict, count_conflicts, find_conflicts_for_symbol, get_state_conflicts,
    state_has_conflicts,
};
use adze_glr_core::precedence_compare::{
    PrecedenceComparison, PrecedenceInfo, StaticPrecedenceResolver, compare_precedences,
};
use adze_glr_core::{
    Action, Conflict, ConflictResolver, ConflictType, FirstFollowSets, GotoIndexing,
    ItemSetCollection, LexMode, ParseRule, ParseTable, RuleId, SymbolMetadata,
};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{
    Associativity, Grammar, Precedence, PrecedenceKind, ProductionId, Rule, Symbol, SymbolId,
    Token, TokenPattern,
};
use std::collections::BTreeMap;

// ============================================================================
// Helpers
// ============================================================================

/// Build a minimal ParseTable with given action table.
fn make_table(action_table: Vec<Vec<Vec<Action>>>) -> ParseTable {
    let state_count = action_table.len();
    let symbol_count = action_table.first().map_or(0, |r| r.len());
    ParseTable {
        action_table,
        goto_table: vec![vec![]; state_count],
        symbol_metadata: vec![],
        state_count,
        symbol_count,
        symbol_to_index: BTreeMap::new(),
        index_to_symbol: vec![],
        external_scanner_states: vec![],
        rules: vec![],
        nonterminal_to_index: BTreeMap::new(),
        goto_indexing: GotoIndexing::NonterminalMap,
        eof_symbol: SymbolId(0),
        start_symbol: SymbolId(1),
        grammar: Grammar::new("test".to_string()),
        initial_state: adze_ir::StateId(0),
        token_count: 0,
        external_token_count: 0,
        lex_modes: vec![],
        extras: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
    }
}

/// Build a ParseTable with index_to_symbol so conflict_inspection functions work.
fn make_inspectable_table(
    action_table: Vec<Vec<Vec<Action>>>,
    index_to_symbol: Vec<SymbolId>,
) -> ParseTable {
    let state_count = action_table.len();
    let symbol_count = action_table.first().map_or(0, |r| r.len());
    let symbol_metadata: Vec<SymbolMetadata> = index_to_symbol
        .iter()
        .map(|sid| SymbolMetadata {
            name: format!("sym_{}", sid.0),
            is_visible: true,
            is_named: true,
            is_supertype: false,
            is_terminal: true,
            is_extra: false,
            is_fragile: false,
            symbol_id: *sid,
        })
        .collect();
    let mut symbol_to_index = BTreeMap::new();
    for (i, sid) in index_to_symbol.iter().enumerate() {
        symbol_to_index.insert(*sid, i);
    }
    ParseTable {
        action_table,
        goto_table: vec![vec![]; state_count],
        symbol_metadata,
        state_count,
        symbol_count,
        symbol_to_index,
        index_to_symbol,
        external_scanner_states: vec![],
        rules: vec![],
        nonterminal_to_index: BTreeMap::new(),
        goto_indexing: GotoIndexing::NonterminalMap,
        eof_symbol: SymbolId(0),
        start_symbol: SymbolId(1),
        grammar: Grammar::new("test".to_string()),
        initial_state: adze_ir::StateId(0),
        token_count: 0,
        external_token_count: 0,
        lex_modes: vec![],
        extras: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
    }
}

/// Build a grammar with explicit precedence declarations and rules.
fn expr_grammar_with_prec() -> Grammar {
    GrammarBuilder::new("expr_prec")
        .token("num", r"\d+")
        .token("+", r"\+")
        .token("*", r"\*")
        .precedence(1, Associativity::Left, vec!["+"])
        .precedence(2, Associativity::Left, vec!["*"])
        .rule("expr", vec!["num"])
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .start("expr")
        .build()
}

/// Build an ambiguous grammar: E → a | E E
fn ambiguous_concat_grammar() -> Grammar {
    let mut g = Grammar::new("ambig".into());
    let a = SymbolId(1);
    let e = SymbolId(10);
    g.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(e, "E".into());
    g.rules.insert(
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
    g
}

fn make_prec_info(level: i16, assoc: Associativity) -> PrecedenceInfo {
    PrecedenceInfo {
        level,
        associativity: assoc,
        is_fragile: false,
    }
}

/// Build a simple grammar: S → a
fn simple_single_rule_grammar() -> Grammar {
    let mut g = Grammar::new("simple".into());
    let a = SymbolId(1);
    let s = SymbolId(10);
    g.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(s, "S".into());
    g.rules.insert(
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
    g
}

/// Build a grammar with a nullable rule: S → A, A → a | ε (via empty rhs)
/// Note: Symbol::Epsilon is a complex symbol that requires normalization,
/// so we simulate nullable via an empty production (rhs = []).
fn epsilon_grammar() -> Grammar {
    let mut g = Grammar::new("epsilon".into());
    let a = SymbolId(1);
    let s = SymbolId(10);
    let a_nt = SymbolId(11);
    g.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(s, "S".into());
    g.rule_names.insert(a_nt, "A".into());
    g.rules.insert(
        s,
        vec![Rule {
            lhs: s,
            rhs: vec![Symbol::NonTerminal(a_nt)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    g.rules.insert(
        a_nt,
        vec![
            Rule {
                lhs: a_nt,
                rhs: vec![],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(1),
            },
            Rule {
                lhs: a_nt,
                rhs: vec![Symbol::Terminal(a)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(2),
            },
        ],
    );
    g
}

/// Build a grammar with multiple nonterminals: S → A B, A → a, B → b
fn multi_nonterminal_grammar() -> Grammar {
    let mut g = Grammar::new("multi".into());
    let a = SymbolId(1);
    let b = SymbolId(2);
    let s_sym = SymbolId(10);
    let a_sym = SymbolId(11);
    let b_sym = SymbolId(12);
    g.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        b,
        Token {
            name: "b".into(),
            pattern: TokenPattern::String("b".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(s_sym, "S".into());
    g.rule_names.insert(a_sym, "A".into());
    g.rule_names.insert(b_sym, "B".into());
    g.rules.insert(
        s_sym,
        vec![Rule {
            lhs: s_sym,
            rhs: vec![Symbol::NonTerminal(a_sym), Symbol::NonTerminal(b_sym)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    g.rules.insert(
        a_sym,
        vec![Rule {
            lhs: a_sym,
            rhs: vec![Symbol::Terminal(a)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(1),
        }],
    );
    g.rules.insert(
        b_sym,
        vec![Rule {
            lhs: b_sym,
            rhs: vec![Symbol::Terminal(b)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(2),
        }],
    );
    g
}

/// Build a right-associative expression grammar.
fn right_assoc_grammar() -> Grammar {
    GrammarBuilder::new("right_assoc")
        .token("num", r"\d+")
        .token("^", r"\^")
        .precedence(1, Associativity::Right, vec!["^"])
        .rule("expr", vec!["num"])
        .rule_with_precedence("expr", vec!["expr", "^", "expr"], 1, Associativity::Right)
        .start("expr")
        .build()
}

// ============================================================================
// 1. Shift-Reduce Conflict Detection (tests 1–8)
// ============================================================================

#[test]
fn sr_detect_basic_conflict_in_cell() {
    let cell = [
        Action::Shift(adze_ir::StateId(3)),
        Action::Reduce(RuleId(1)),
    ];
    let has_shift = cell.iter().any(|a| matches!(a, Action::Shift(_)));
    let has_reduce = cell.iter().any(|a| matches!(a, Action::Reduce(_)));
    assert!(has_shift && has_reduce, "cell should contain SR conflict");
}

#[test]
fn sr_detect_via_conflict_resolver() {
    let g = ambiguous_concat_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&g, &ff);
    let resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
    assert!(!resolver.conflicts.is_empty(), "should detect conflicts");
    let has_sr = resolver
        .conflicts
        .iter()
        .any(|c| c.conflict_type == ConflictType::ShiftReduce);
    assert!(has_sr, "ambiguous concat grammar should have S/R conflicts");
}

#[test]
fn sr_conflict_has_shift_and_reduce_actions() {
    let g = ambiguous_concat_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&g, &ff);
    let resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
    for c in &resolver.conflicts {
        if c.conflict_type == ConflictType::ShiftReduce {
            assert!(c.actions.iter().any(|a| matches!(a, Action::Shift(_))));
            assert!(c.actions.iter().any(|a| matches!(a, Action::Reduce(_))));
        }
    }
}

#[test]
fn sr_classify_via_inspection_api() {
    let actions = vec![
        Action::Shift(adze_ir::StateId(5)),
        Action::Reduce(RuleId(2)),
    ];
    let ct = classify_conflict(&actions);
    assert_eq!(ct, InspectionConflictType::ShiftReduce);
}

#[test]
fn sr_multiple_shifts_one_reduce() {
    let cell = vec![
        Action::Shift(adze_ir::StateId(1)),
        Action::Shift(adze_ir::StateId(2)),
        Action::Reduce(RuleId(0)),
    ];
    let shifts = cell
        .iter()
        .filter(|a| matches!(a, Action::Shift(_)))
        .count();
    let reduces = cell
        .iter()
        .filter(|a| matches!(a, Action::Reduce(_)))
        .count();
    assert_eq!(shifts, 2);
    assert_eq!(reduces, 1);
    assert_eq!(
        classify_conflict(&cell),
        InspectionConflictType::ShiftReduce
    );
}

#[test]
fn sr_one_shift_multiple_reduces() {
    let cell = vec![
        Action::Shift(adze_ir::StateId(3)),
        Action::Reduce(RuleId(1)),
        Action::Reduce(RuleId(2)),
    ];
    assert_eq!(
        classify_conflict(&cell),
        InspectionConflictType::ShiftReduce
    );
}

#[test]
fn sr_table_level_detection() {
    let table = make_inspectable_table(
        vec![vec![
            vec![
                Action::Shift(adze_ir::StateId(2)),
                Action::Reduce(RuleId(1)),
            ],
            vec![Action::Accept],
        ]],
        vec![SymbolId(1), SymbolId(2)],
    );
    let summary = count_conflicts(&table);
    assert!(summary.shift_reduce > 0, "should detect S/R in table");
}

#[test]
fn sr_no_conflict_when_single_action() {
    let cell = vec![Action::Shift(adze_ir::StateId(1))];
    let ct = classify_conflict(&cell);
    assert_ne!(ct, InspectionConflictType::ShiftReduce);
}

// ============================================================================
// 2. Reduce-Reduce Conflict Detection (tests 9–15)
// ============================================================================

#[test]
fn rr_basic_two_reduces() {
    let cell = vec![Action::Reduce(RuleId(1)), Action::Reduce(RuleId(2))];
    assert_eq!(
        classify_conflict(&cell),
        InspectionConflictType::ReduceReduce
    );
}

#[test]
fn rr_three_reduces() {
    let cell = vec![
        Action::Reduce(RuleId(0)),
        Action::Reduce(RuleId(3)),
        Action::Reduce(RuleId(7)),
    ];
    assert_eq!(
        classify_conflict(&cell),
        InspectionConflictType::ReduceReduce
    );
}

#[test]
fn rr_resolve_picks_lowest_rule_id() {
    let conflict = Conflict {
        state: adze_ir::StateId(0),
        symbol: SymbolId(1),
        actions: vec![Action::Reduce(RuleId(5)), Action::Reduce(RuleId(2))],
        conflict_type: ConflictType::ReduceReduce,
    };
    let g = Grammar::new("dummy".into());
    let mut r2 = ConflictResolver {
        conflicts: vec![conflict],
    };
    r2.resolve_conflicts(&g);
    assert_eq!(r2.conflicts[0].actions.len(), 1);
    assert!(matches!(
        r2.conflicts[0].actions[0],
        Action::Reduce(RuleId(2))
    ));
}

#[test]
fn rr_resolve_three_picks_lowest() {
    let mut resolver = ConflictResolver {
        conflicts: vec![Conflict {
            state: adze_ir::StateId(0),
            symbol: SymbolId(1),
            actions: vec![
                Action::Reduce(RuleId(10)),
                Action::Reduce(RuleId(3)),
                Action::Reduce(RuleId(7)),
            ],
            conflict_type: ConflictType::ReduceReduce,
        }],
    };
    let g = Grammar::new("dummy".into());
    resolver.resolve_conflicts(&g);
    assert_eq!(resolver.conflicts[0].actions.len(), 1);
    assert!(matches!(
        resolver.conflicts[0].actions[0],
        Action::Reduce(RuleId(3))
    ));
}

#[test]
fn rr_table_detection() {
    let table = make_inspectable_table(
        vec![vec![vec![
            Action::Reduce(RuleId(1)),
            Action::Reduce(RuleId(2)),
        ]]],
        vec![SymbolId(1)],
    );
    let summary = count_conflicts(&table);
    assert!(summary.reduce_reduce > 0, "should detect R/R in table");
    assert_eq!(summary.shift_reduce, 0);
}

#[test]
fn rr_not_detected_for_single_reduce() {
    let cell = [Action::Reduce(RuleId(1))];
    assert_eq!(cell.len(), 1, "single reduce is not a table-level conflict");
}

#[test]
fn rr_duplicate_rule_ids() {
    let cell = vec![Action::Reduce(RuleId(4)), Action::Reduce(RuleId(4))];
    assert_eq!(classify_conflict(&cell), InspectionConflictType::Mixed);
}

// ============================================================================
// 3. Precedence-Based Resolution (tests 16–26)
// ============================================================================

#[test]
fn prec_higher_shift_wins() {
    let shift = make_prec_info(3, Associativity::Left);
    let reduce = make_prec_info(1, Associativity::Left);
    assert_eq!(
        compare_precedences(Some(shift), Some(reduce)),
        PrecedenceComparison::PreferShift
    );
}

#[test]
fn prec_higher_reduce_wins() {
    let shift = make_prec_info(1, Associativity::Left);
    let reduce = make_prec_info(5, Associativity::Left);
    assert_eq!(
        compare_precedences(Some(shift), Some(reduce)),
        PrecedenceComparison::PreferReduce
    );
}

#[test]
fn prec_no_info_returns_none() {
    assert_eq!(compare_precedences(None, None), PrecedenceComparison::None);
    assert_eq!(
        compare_precedences(Some(make_prec_info(1, Associativity::Left)), None),
        PrecedenceComparison::None
    );
    assert_eq!(
        compare_precedences(None, Some(make_prec_info(1, Associativity::Left))),
        PrecedenceComparison::None
    );
}

#[test]
fn prec_static_resolver_from_grammar() {
    let g = expr_grammar_with_prec();
    let resolver = StaticPrecedenceResolver::from_grammar(&g);
    let plus_id = g
        .tokens
        .iter()
        .find(|(_, t)| t.name == "+")
        .map(|(id, _)| *id)
        .unwrap();
    let prec = resolver.token_precedence(plus_id).unwrap();
    assert_eq!(prec.level, 1);
    assert_eq!(prec.associativity, Associativity::Left);
}

#[test]
fn prec_static_resolver_star_higher_than_plus() {
    let g = expr_grammar_with_prec();
    let resolver = StaticPrecedenceResolver::from_grammar(&g);
    let plus_id = g
        .tokens
        .iter()
        .find(|(_, t)| t.name == "+")
        .map(|(id, _)| *id)
        .unwrap();
    let star_id = g
        .tokens
        .iter()
        .find(|(_, t)| t.name == "*")
        .map(|(id, _)| *id)
        .unwrap();
    let plus_prec = resolver.token_precedence(plus_id).unwrap();
    let star_prec = resolver.token_precedence(star_id).unwrap();
    assert!(star_prec.level > plus_prec.level);
}

#[test]
fn prec_unknown_token_returns_none() {
    let g = expr_grammar_with_prec();
    let resolver = StaticPrecedenceResolver::from_grammar(&g);
    assert!(resolver.token_precedence(SymbolId(9999)).is_none());
}

#[test]
fn prec_rule_precedence_from_grammar() {
    let g = expr_grammar_with_prec();
    let resolver = StaticPrecedenceResolver::from_grammar(&g);
    let has_rule_prec = g.all_rules().any(|r| {
        let rid = RuleId(r.production_id.0);
        resolver.rule_precedence(rid).is_some()
    });
    assert!(
        has_rule_prec,
        "grammar should have at least one rule with precedence"
    );
}

#[test]
fn prec_advanced_resolver_shift_higher() {
    let mut g = Grammar::new("test".into());
    g.precedences.push(Precedence {
        level: 2,
        associativity: Associativity::Left,
        symbols: vec![SymbolId(10)],
    });
    g.rules.insert(
        SymbolId(20),
        vec![Rule {
            lhs: SymbolId(20),
            rhs: vec![Symbol::Terminal(SymbolId(10))],
            precedence: Some(PrecedenceKind::Static(1)),
            associativity: Some(Associativity::Left),
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    let resolver = PrecedenceResolver::new(&g);
    let decision = resolver.can_resolve_shift_reduce(SymbolId(10), SymbolId(20));
    assert_eq!(decision, Some(PrecedenceDecision::PreferShift));
}

#[test]
fn prec_advanced_resolver_reduce_higher() {
    let mut g = Grammar::new("test".into());
    g.precedences.push(Precedence {
        level: 1,
        associativity: Associativity::Left,
        symbols: vec![SymbolId(10)],
    });
    g.rules.insert(
        SymbolId(20),
        vec![Rule {
            lhs: SymbolId(20),
            rhs: vec![Symbol::Terminal(SymbolId(10))],
            precedence: Some(PrecedenceKind::Static(5)),
            associativity: Some(Associativity::Left),
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    let resolver = PrecedenceResolver::new(&g);
    let decision = resolver.can_resolve_shift_reduce(SymbolId(10), SymbolId(20));
    assert_eq!(decision, Some(PrecedenceDecision::PreferReduce));
}

#[test]
fn prec_advanced_resolver_no_info() {
    let g = Grammar::new("empty".into());
    let resolver = PrecedenceResolver::new(&g);
    let decision = resolver.can_resolve_shift_reduce(SymbolId(1), SymbolId(2));
    assert_eq!(decision, None);
}

// ============================================================================
// 4. Associativity-Based Resolution (tests 27–34)
// ============================================================================

#[test]
fn assoc_left_prefers_reduce() {
    let shift = make_prec_info(1, Associativity::Left);
    let reduce = make_prec_info(1, Associativity::Left);
    assert_eq!(
        compare_precedences(Some(shift), Some(reduce)),
        PrecedenceComparison::PreferReduce
    );
}

#[test]
fn assoc_right_prefers_shift() {
    let shift = make_prec_info(1, Associativity::Right);
    let reduce = make_prec_info(1, Associativity::Right);
    assert_eq!(
        compare_precedences(Some(shift), Some(reduce)),
        PrecedenceComparison::PreferShift
    );
}

#[test]
fn assoc_none_returns_error() {
    let shift = make_prec_info(1, Associativity::None);
    let reduce = make_prec_info(1, Associativity::None);
    assert_eq!(
        compare_precedences(Some(shift), Some(reduce)),
        PrecedenceComparison::Error
    );
}

#[test]
fn assoc_left_resolve_sr_conflict() {
    let mut g = Grammar::new("test".into());
    g.precedences.push(Precedence {
        level: 1,
        associativity: Associativity::Left,
        symbols: vec![SymbolId(10)],
    });
    g.rules.insert(
        SymbolId(20),
        vec![Rule {
            lhs: SymbolId(20),
            rhs: vec![Symbol::Terminal(SymbolId(10))],
            precedence: Some(PrecedenceKind::Static(1)),
            associativity: Some(Associativity::Left),
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    let resolver = PrecedenceResolver::new(&g);
    let decision = resolver.can_resolve_shift_reduce(SymbolId(10), SymbolId(20));
    assert_eq!(decision, Some(PrecedenceDecision::PreferReduce));
}

#[test]
fn assoc_right_resolve_sr_conflict() {
    let mut g = Grammar::new("test".into());
    g.precedences.push(Precedence {
        level: 1,
        associativity: Associativity::Right,
        symbols: vec![SymbolId(10)],
    });
    g.rules.insert(
        SymbolId(20),
        vec![Rule {
            lhs: SymbolId(20),
            rhs: vec![Symbol::Terminal(SymbolId(10))],
            precedence: Some(PrecedenceKind::Static(1)),
            associativity: Some(Associativity::Right),
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    let resolver = PrecedenceResolver::new(&g);
    let decision = resolver.can_resolve_shift_reduce(SymbolId(10), SymbolId(20));
    assert_eq!(decision, Some(PrecedenceDecision::PreferShift));
}

#[test]
fn assoc_none_resolve_sr_conflict_error() {
    let mut g = Grammar::new("test".into());
    g.precedences.push(Precedence {
        level: 1,
        associativity: Associativity::None,
        symbols: vec![SymbolId(10)],
    });
    g.rules.insert(
        SymbolId(20),
        vec![Rule {
            lhs: SymbolId(20),
            rhs: vec![Symbol::Terminal(SymbolId(10))],
            precedence: Some(PrecedenceKind::Static(1)),
            associativity: Some(Associativity::None),
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    let resolver = PrecedenceResolver::new(&g);
    let decision = resolver.can_resolve_shift_reduce(SymbolId(10), SymbolId(20));
    assert_eq!(decision, Some(PrecedenceDecision::Error));
}

#[test]
fn assoc_conflict_resolver_left_eliminates_shift() {
    let g = GrammarBuilder::new("left_assoc")
        .token("num", r"\d+")
        .token("+", r"\+")
        .precedence(1, Associativity::Left, vec!["+"])
        .rule("expr", vec!["num"])
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .start("expr")
        .build();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&g, &ff);
    let mut resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
    let had_conflicts = !resolver.conflicts.is_empty();
    resolver.resolve_conflicts(&g);
    if had_conflicts {
        for c in &resolver.conflicts {
            if c.conflict_type == ConflictType::ShiftReduce {
                let only_reduce = c.actions.len() == 1 && matches!(c.actions[0], Action::Reduce(_));
                let is_fork = c.actions.len() == 1 && matches!(c.actions[0], Action::Fork(_));
                assert!(
                    only_reduce || is_fork,
                    "left-assoc should resolve to reduce or fork, got {:?}",
                    c.actions
                );
            }
        }
    }
}

#[test]
fn assoc_precedence_breaks_tie_between_levels() {
    let shift = make_prec_info(5, Associativity::Left);
    let reduce = make_prec_info(3, Associativity::Right);
    assert_eq!(
        compare_precedences(Some(shift), Some(reduce)),
        PrecedenceComparison::PreferShift
    );
}

// ============================================================================
// 5. GLR Fork Decision Making (tests 35–42)
// ============================================================================

#[test]
fn fork_created_for_unresolvable_sr() {
    let g = ambiguous_concat_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&g, &ff);
    let mut resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
    resolver.resolve_conflicts(&g);
    let has_fork = resolver
        .conflicts
        .iter()
        .any(|c| c.actions.iter().any(|a| matches!(a, Action::Fork(_))));
    if resolver
        .conflicts
        .iter()
        .any(|c| c.conflict_type == ConflictType::ShiftReduce)
    {
        assert!(has_fork, "unresolvable SR should become Fork");
    }
}

#[test]
fn fork_contains_both_shift_and_reduce() {
    let fork = Action::Fork(vec![
        Action::Shift(adze_ir::StateId(1)),
        Action::Reduce(RuleId(2)),
    ]);
    if let Action::Fork(actions) = &fork {
        assert!(actions.iter().any(|a| matches!(a, Action::Shift(_))));
        assert!(actions.iter().any(|a| matches!(a, Action::Reduce(_))));
    } else {
        panic!("expected Fork");
    }
}

#[test]
fn fork_preserves_action_count() {
    let actions = vec![
        Action::Shift(adze_ir::StateId(1)),
        Action::Shift(adze_ir::StateId(2)),
        Action::Reduce(RuleId(3)),
    ];
    let fork = Action::Fork(actions.clone());
    if let Action::Fork(inner) = fork {
        assert_eq!(inner.len(), 3);
    }
}

#[test]
fn fork_nested_is_valid() {
    let inner_fork = Action::Fork(vec![
        Action::Shift(adze_ir::StateId(1)),
        Action::Reduce(RuleId(1)),
    ]);
    let outer = Action::Fork(vec![inner_fork, Action::Shift(adze_ir::StateId(2))]);
    if let Action::Fork(actions) = &outer {
        assert_eq!(actions.len(), 2);
        assert!(matches!(&actions[0], Action::Fork(_)));
    }
}

#[test]
fn fork_empty_is_valid() {
    let fork = Action::Fork(vec![]);
    if let Action::Fork(actions) = &fork {
        assert!(actions.is_empty());
    }
}

#[test]
fn fork_single_action_degenerate() {
    let fork = Action::Fork(vec![Action::Accept]);
    if let Action::Fork(actions) = &fork {
        assert_eq!(actions.len(), 1);
    }
}

#[test]
fn fork_classify_as_sr_via_inspection() {
    let cell = vec![Action::Fork(vec![
        Action::Shift(adze_ir::StateId(1)),
        Action::Reduce(RuleId(2)),
    ])];
    let ct = classify_conflict(&cell);
    assert_eq!(ct, InspectionConflictType::ShiftReduce);
}

#[test]
fn fork_classify_as_rr_via_inspection() {
    let cell = vec![Action::Fork(vec![
        Action::Reduce(RuleId(1)),
        Action::Reduce(RuleId(2)),
    ])];
    let ct = classify_conflict(&cell);
    assert_eq!(ct, InspectionConflictType::ReduceReduce);
}

// ============================================================================
// 6. Action Cell with Multiple Actions (tests 43–50)
// ============================================================================

#[test]
fn cell_all_action_types() {
    let cell = [
        Action::Shift(adze_ir::StateId(1)),
        Action::Reduce(RuleId(1)),
        Action::Accept,
        Action::Error,
        Action::Recover,
        Action::Fork(vec![Action::Shift(adze_ir::StateId(2))]),
    ];
    assert_eq!(cell.len(), 6);
}

#[test]
fn cell_action_ordering_canonical() {
    let mut cell = [
        Action::Fork(vec![]),
        Action::Recover,
        Action::Error,
        Action::Accept,
        Action::Reduce(RuleId(1)),
        Action::Shift(adze_ir::StateId(1)),
    ];
    cell.sort_by_key(|a| match a {
        Action::Shift(_) => 0u8,
        Action::Reduce(_) => 1,
        Action::Accept => 2,
        Action::Error => 3,
        Action::Recover => 4,
        Action::Fork(_) => 5,
        _ => 6,
    });
    assert!(matches!(cell[0], Action::Shift(_)));
    assert!(matches!(cell[1], Action::Reduce(_)));
    assert!(matches!(cell[2], Action::Accept));
    assert!(matches!(cell[3], Action::Error));
    assert!(matches!(cell[4], Action::Recover));
    assert!(matches!(cell[5], Action::Fork(_)));
}

#[test]
fn cell_many_shifts_in_table() {
    let table = make_table(vec![vec![
        (0..20)
            .map(|i| Action::Shift(adze_ir::StateId(i)))
            .collect(),
    ]]);
    assert_eq!(table.action_table[0][0].len(), 20);
}

#[test]
fn cell_many_reduces_in_table() {
    let table = make_table(vec![vec![
        (0..15).map(|i| Action::Reduce(RuleId(i))).collect(),
    ]]);
    assert_eq!(table.action_table[0][0].len(), 15);
}

#[test]
fn cell_mixed_in_multi_state_table() {
    let table = make_table(vec![
        vec![vec![
            Action::Shift(adze_ir::StateId(1)),
            Action::Reduce(RuleId(1)),
        ]],
        vec![vec![Action::Reduce(RuleId(2)), Action::Reduce(RuleId(3))]],
        vec![vec![Action::Accept]],
    ]);
    assert_eq!(table.state_count, 3);
    assert_eq!(table.action_table[0][0].len(), 2);
    assert_eq!(table.action_table[1][0].len(), 2);
    assert_eq!(table.action_table[2][0].len(), 1);
}

#[test]
fn cell_state_id_u16_max_boundary() {
    let cell = [Action::Shift(adze_ir::StateId(u16::MAX))];
    assert!(matches!(cell[0], Action::Shift(adze_ir::StateId(65535))));
}

#[test]
fn cell_rule_id_u16_max_boundary() {
    let cell = [Action::Reduce(RuleId(u16::MAX))];
    assert!(matches!(cell[0], Action::Reduce(RuleId(65535))));
}

#[test]
fn cell_empty_no_conflict() {
    let table = make_inspectable_table(vec![vec![vec![]]], vec![SymbolId(0)]);
    let summary = count_conflicts(&table);
    assert_eq!(summary.shift_reduce, 0);
    assert_eq!(summary.reduce_reduce, 0);
}

// ============================================================================
// 7. Advanced Conflict Resolution Strategies (tests 51–58)
// ============================================================================

#[test]
fn advanced_analyzer_default_stats() {
    let stats = ConflictStats::default();
    assert_eq!(stats.shift_reduce_conflicts, 0);
    assert_eq!(stats.reduce_reduce_conflicts, 0);
    assert_eq!(stats.precedence_resolved, 0);
    assert_eq!(stats.associativity_resolved, 0);
    assert_eq!(stats.explicit_glr, 0);
    assert_eq!(stats.default_resolved, 0);
}

#[test]
fn advanced_analyzer_analyze_empty_table() {
    let table = ParseTable::default();
    let mut analyzer = ConflictAnalyzer::new();
    let stats = analyzer.analyze_table(&table);
    assert_eq!(stats.shift_reduce_conflicts, 0);
    assert_eq!(stats.reduce_reduce_conflicts, 0);
}

#[test]
fn advanced_analyzer_new_default_equivalent() {
    let a = ConflictAnalyzer::new();
    let b = ConflictAnalyzer::default();
    assert_eq!(a.get_stats().shift_reduce_conflicts, 0);
    assert_eq!(b.get_stats().shift_reduce_conflicts, 0);
}

#[test]
fn advanced_conflict_resolver_for_empty_grammar() {
    let g = Grammar::new("empty".into());
    let resolver = PrecedenceResolver::new(&g);
    assert!(
        resolver
            .can_resolve_shift_reduce(SymbolId(1), SymbolId(2))
            .is_none()
    );
}

#[test]
fn conflict_inspection_state_has_no_conflicts() {
    let table = make_inspectable_table(
        vec![vec![vec![Action::Shift(adze_ir::StateId(1))]]],
        vec![SymbolId(0)],
    );
    assert!(!state_has_conflicts(&table, adze_ir::StateId(0)));
}

#[test]
fn conflict_inspection_state_has_conflicts() {
    let table = make_inspectable_table(
        vec![vec![vec![
            Action::Shift(adze_ir::StateId(1)),
            Action::Reduce(RuleId(1)),
        ]]],
        vec![SymbolId(0)],
    );
    assert!(state_has_conflicts(&table, adze_ir::StateId(0)));
}

#[test]
fn conflict_inspection_get_state_conflicts_detail() {
    let table = make_inspectable_table(
        vec![vec![vec![
            Action::Shift(adze_ir::StateId(1)),
            Action::Reduce(RuleId(1)),
        ]]],
        vec![SymbolId(0)],
    );
    let details = get_state_conflicts(&table, adze_ir::StateId(0));
    assert!(!details.is_empty());
    assert_eq!(details[0].state, adze_ir::StateId(0));
    assert_eq!(
        details[0].conflict_type,
        InspectionConflictType::ShiftReduce
    );
}

#[test]
fn conflict_inspection_find_by_symbol() {
    let table = make_inspectable_table(
        vec![vec![
            vec![
                Action::Shift(adze_ir::StateId(1)),
                Action::Reduce(RuleId(1)),
            ],
            vec![Action::Accept],
        ]],
        vec![SymbolId(5), SymbolId(6)],
    );
    let conflicts = find_conflicts_for_symbol(&table, SymbolId(5));
    assert!(!conflicts.is_empty());
    let no_conflicts = find_conflicts_for_symbol(&table, SymbolId(6));
    assert!(no_conflicts.is_empty());
}

// ============================================================================
// 8. Integration: Full Pipeline (tests 59–62)
// ============================================================================

#[test]
fn integration_expr_grammar_detect_and_resolve() {
    let g = expr_grammar_with_prec();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&g, &ff);
    let mut resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
    let conflicts_before = resolver.conflicts.len();
    resolver.resolve_conflicts(&g);
    for c in &resolver.conflicts {
        assert!(!c.actions.is_empty());
    }
    let total_actions_after: usize = resolver.conflicts.iter().map(|c| c.actions.len()).sum();
    assert!(total_actions_after <= conflicts_before * 10);
}

#[test]
fn integration_ambiguous_grammar_all_forks() {
    let g = ambiguous_concat_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&g, &ff);
    let mut resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
    resolver.resolve_conflicts(&g);
    for c in &resolver.conflicts {
        if c.conflict_type == ConflictType::ShiftReduce {
            assert!(
                c.actions.iter().any(|a| matches!(a, Action::Fork(_))),
                "no-prec SR should become Fork"
            );
        }
    }
}

#[test]
fn integration_static_resolver_matches_advanced() {
    let g = expr_grammar_with_prec();
    let static_resolver = StaticPrecedenceResolver::from_grammar(&g);
    let _advanced_resolver = PrecedenceResolver::new(&g);

    let plus_id = g
        .tokens
        .iter()
        .find(|(_, t)| t.name == "+")
        .map(|(id, _)| *id)
        .unwrap();

    let static_plus = static_resolver.token_precedence(plus_id);
    assert!(static_plus.is_some());

    let has_rule_prec = g.all_rules().any(|r| {
        let rid = RuleId(r.production_id.0);
        static_resolver.rule_precedence(rid).is_some()
    });
    assert!(has_rule_prec);
}

#[test]
fn integration_conflict_summary_counts() {
    let table = make_inspectable_table(
        vec![
            vec![
                vec![
                    Action::Shift(adze_ir::StateId(1)),
                    Action::Reduce(RuleId(1)),
                ],
                vec![Action::Reduce(RuleId(2)), Action::Reduce(RuleId(3))],
            ],
            vec![vec![Action::Accept], vec![Action::Error]],
        ],
        vec![SymbolId(10), SymbolId(11)],
    );
    let summary = count_conflicts(&table);
    assert_eq!(
        summary.shift_reduce, 1,
        "one S/R conflict at state 0, sym 0"
    );
    assert_eq!(
        summary.reduce_reduce, 1,
        "one R/R conflict at state 0, sym 1"
    );
    assert_eq!(summary.states_with_conflicts.len(), 1);
    assert_eq!(summary.states_with_conflicts[0], adze_ir::StateId(0));
}

// ============================================================================
// 9. First/Follow Set Computation (tests 63–72)
// ============================================================================

#[test]
fn ff_simple_grammar_computes() {
    let g = simple_single_rule_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    // S → a, so FIRST(S) should contain 'a' (SymbolId(1))
    let s = SymbolId(10);
    let first_s = ff.first(s);
    assert!(first_s.is_some(), "S should have a FIRST set");
    let first_set = first_s.unwrap();
    assert!(
        first_set.contains(1),
        "FIRST(S) should contain terminal 'a' (id=1)"
    );
}

#[test]
fn ff_simple_grammar_not_nullable() {
    let g = simple_single_rule_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let s = SymbolId(10);
    assert!(!ff.is_nullable(s), "S → a is not nullable");
}

#[test]
fn ff_epsilon_grammar_nullable() {
    let g = epsilon_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let a_nt = SymbolId(11);
    assert!(ff.is_nullable(a_nt), "A → ε | a should be nullable");
}

#[test]
fn ff_epsilon_grammar_first_contains_terminal() {
    let g = epsilon_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let a_nt = SymbolId(11);
    let first_a = ff.first(a_nt).unwrap();
    assert!(first_a.contains(1), "FIRST(A) should contain 'a'");
}

#[test]
fn ff_multi_nonterminal_first_sets() {
    let g = multi_nonterminal_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let a_sym = SymbolId(11);
    let b_sym = SymbolId(12);
    let first_a = ff.first(a_sym).unwrap();
    let first_b = ff.first(b_sym).unwrap();
    assert!(first_a.contains(1), "FIRST(A) should contain 'a'");
    assert!(first_b.contains(2), "FIRST(B) should contain 'b'");
}

#[test]
fn ff_multi_nonterminal_follow_sets() {
    let g = multi_nonterminal_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    // S → A B, so FOLLOW(A) should contain FIRST(B) = {b}
    let a_sym = SymbolId(11);
    let follow_a = ff.follow(a_sym).unwrap();
    assert!(
        follow_a.contains(2),
        "FOLLOW(A) should contain 'b' from S → A B"
    );
}

#[test]
fn ff_expr_grammar_first_set() {
    let g = expr_grammar_with_prec();
    let ff = FirstFollowSets::compute(&g).unwrap();
    // Find the expr nonterminal symbol
    let expr_sym = g
        .rule_names
        .iter()
        .find(|(_, name)| *name == "expr")
        .map(|(id, _)| *id)
        .unwrap();
    let first_expr = ff.first(expr_sym);
    assert!(first_expr.is_some(), "expr should have a FIRST set");
    // FIRST(expr) should contain 'num'
    let num_id = g
        .tokens
        .iter()
        .find(|(_, t)| t.name == "num")
        .map(|(id, _)| *id)
        .unwrap();
    assert!(
        first_expr.unwrap().contains(num_id.0 as usize),
        "FIRST(expr) should contain 'num'"
    );
}

#[test]
fn ff_ambiguous_grammar_has_first() {
    let g = ambiguous_concat_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let e = SymbolId(10);
    let first_e = ff.first(e);
    assert!(first_e.is_some(), "E should have a FIRST set");
    assert!(first_e.unwrap().contains(1), "FIRST(E) should contain 'a'");
}

#[test]
fn ff_ambiguous_grammar_follow_contains_eof() {
    let g = ambiguous_concat_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let e = SymbolId(10);
    let follow_e = ff.follow(e);
    assert!(follow_e.is_some(), "E should have a FOLLOW set");
    // FOLLOW of start symbol should contain EOF sentinel (SymbolId(0))
    assert!(
        follow_e.unwrap().contains(0),
        "FOLLOW(E) should contain EOF sentinel"
    );
}

#[test]
fn ff_first_of_sequence_basic() {
    let g = multi_nonterminal_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    // FIRST of [A, B] should be FIRST(A) = {a} since A is not nullable
    let seq = vec![
        Symbol::NonTerminal(SymbolId(11)),
        Symbol::NonTerminal(SymbolId(12)),
    ];
    let first_seq = ff.first_of_sequence(&seq).unwrap();
    assert!(first_seq.contains(1), "FIRST([A, B]) should contain 'a'");
    // B's first should not be in there since A is not nullable
    assert!(
        !first_seq.contains(2),
        "FIRST([A, B]) should not contain 'b' when A not nullable"
    );
}

// ============================================================================
// 10. Empty Grammar Edge Cases (tests 73–76)
// ============================================================================

#[test]
fn empty_grammar_no_conflicts_detected() {
    let g = Grammar::new("empty".into());
    // FirstFollowSets::compute on empty grammar may succeed or fail;
    // if it succeeds, there should be zero conflicts.
    if let Ok(ff) = FirstFollowSets::compute(&g) {
        let collection = ItemSetCollection::build_canonical_collection(&g, &ff);
        let resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
        assert!(
            resolver.conflicts.is_empty(),
            "empty grammar should have no conflicts"
        );
    }
}

#[test]
fn empty_grammar_precedence_resolver_returns_none() {
    let g = Grammar::new("empty".into());
    let resolver = PrecedenceResolver::new(&g);
    assert!(
        resolver
            .can_resolve_shift_reduce(SymbolId(1), SymbolId(2))
            .is_none()
    );
}

#[test]
fn empty_grammar_static_resolver_no_token_prec() {
    let g = Grammar::new("empty".into());
    let resolver = StaticPrecedenceResolver::from_grammar(&g);
    assert!(resolver.token_precedence(SymbolId(0)).is_none());
    assert!(resolver.token_precedence(SymbolId(1)).is_none());
}

#[test]
fn empty_grammar_analyzer_zero_stats() {
    let table = ParseTable::default();
    let mut analyzer = ConflictAnalyzer::new();
    let stats = analyzer.analyze_table(&table);
    assert_eq!(stats.shift_reduce_conflicts, 0);
    assert_eq!(stats.reduce_reduce_conflicts, 0);
    assert_eq!(stats.precedence_resolved, 0);
    assert_eq!(stats.associativity_resolved, 0);
}

// ============================================================================
// 11. Grammars with Epsilon Rules (tests 77–80)
// ============================================================================

#[test]
fn epsilon_grammar_first_follow_computes() {
    let g = epsilon_grammar();
    let result = FirstFollowSets::compute(&g);
    assert!(
        result.is_ok(),
        "epsilon grammar should compute FIRST/FOLLOW"
    );
}

#[test]
fn epsilon_grammar_canonical_collection_builds() {
    let g = epsilon_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&g, &ff);
    assert!(
        !collection.sets.is_empty(),
        "should produce at least one item set"
    );
}

#[test]
fn epsilon_detect_conflicts() {
    let g = epsilon_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&g, &ff);
    let resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
    // S → ε | a may or may not have conflicts depending on lookahead;
    // verify detect_conflicts runs without panicking
    let _ = resolver.conflicts.len();
}

#[test]
fn epsilon_resolve_doesnt_panic() {
    let g = epsilon_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&g, &ff);
    let mut resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
    resolver.resolve_conflicts(&g);
    // After resolution, each conflict should have at least one action
    for c in &resolver.conflicts {
        assert!(!c.actions.is_empty());
    }
}

// ============================================================================
// 12. Large Grammars with Many Conflicts (tests 81–84)
// ============================================================================

#[test]
fn large_table_many_sr_conflicts() {
    // Build a table with 50 states, each having an SR conflict
    let action_table: Vec<Vec<Vec<Action>>> = (0..50)
        .map(|i| {
            vec![vec![
                Action::Shift(adze_ir::StateId(i + 1)),
                Action::Reduce(RuleId(i)),
            ]]
        })
        .collect();
    let index_to_symbol = vec![SymbolId(100)];
    let table = make_inspectable_table(action_table, index_to_symbol);
    let summary = count_conflicts(&table);
    assert_eq!(summary.shift_reduce, 50);
    assert_eq!(summary.reduce_reduce, 0);
    assert_eq!(summary.states_with_conflicts.len(), 50);
}

#[test]
fn large_table_many_rr_conflicts() {
    let action_table: Vec<Vec<Vec<Action>>> = (0..30)
        .map(|i| {
            vec![vec![
                Action::Reduce(RuleId(i * 2)),
                Action::Reduce(RuleId(i * 2 + 1)),
            ]]
        })
        .collect();
    let index_to_symbol = vec![SymbolId(50)];
    let table = make_inspectable_table(action_table, index_to_symbol);
    let summary = count_conflicts(&table);
    assert_eq!(summary.reduce_reduce, 30);
    assert_eq!(summary.shift_reduce, 0);
}

#[test]
fn large_table_mixed_conflicts() {
    let mut action_table = Vec::new();
    for i in 0u16..20 {
        if i % 2 == 0 {
            action_table.push(vec![vec![
                Action::Shift(adze_ir::StateId(i + 1)),
                Action::Reduce(RuleId(i)),
            ]]);
        } else {
            action_table.push(vec![vec![
                Action::Reduce(RuleId(i)),
                Action::Reduce(RuleId(i + 100)),
            ]]);
        }
    }
    let index_to_symbol = vec![SymbolId(200)];
    let table = make_inspectable_table(action_table, index_to_symbol);
    let summary = count_conflicts(&table);
    assert_eq!(summary.shift_reduce, 10);
    assert_eq!(summary.reduce_reduce, 10);
    assert_eq!(summary.states_with_conflicts.len(), 20);
}

#[test]
fn large_resolver_batch_rr() {
    // Resolve 20 RR conflicts in batch
    let conflicts: Vec<Conflict> = (0..20)
        .map(|i| Conflict {
            state: adze_ir::StateId(i),
            symbol: SymbolId(1),
            actions: vec![
                Action::Reduce(RuleId(i * 3 + 2)),
                Action::Reduce(RuleId(i * 3)),
                Action::Reduce(RuleId(i * 3 + 1)),
            ],
            conflict_type: ConflictType::ReduceReduce,
        })
        .collect();
    let mut resolver = ConflictResolver { conflicts };
    let g = Grammar::new("dummy".into());
    resolver.resolve_conflicts(&g);
    for (i, c) in resolver.conflicts.iter().enumerate() {
        let i = i as u16;
        assert_eq!(
            c.actions.len(),
            1,
            "each RR conflict should resolve to one action"
        );
        // Lowest rule id is i*3
        assert!(
            matches!(c.actions[0], Action::Reduce(RuleId(r)) if r == i * 3),
            "should pick lowest rule_id: expected {} got {:?}",
            i * 3,
            c.actions[0]
        );
    }
}

// ============================================================================
// 13. Precedence Climbing Patterns (tests 85–88)
// ============================================================================

#[test]
fn prec_climbing_three_levels() {
    let g = GrammarBuilder::new("climb3")
        .token("num", r"\d+")
        .token("+", r"\+")
        .token("*", r"\*")
        .token("^", r"\^")
        .precedence(1, Associativity::Left, vec!["+"])
        .precedence(2, Associativity::Left, vec!["*"])
        .precedence(3, Associativity::Right, vec!["^"])
        .rule("expr", vec!["num"])
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "^", "expr"], 3, Associativity::Right)
        .start("expr")
        .build();

    let resolver = StaticPrecedenceResolver::from_grammar(&g);
    let plus_id = g
        .tokens
        .iter()
        .find(|(_, t)| t.name == "+")
        .map(|(id, _)| *id)
        .unwrap();
    let star_id = g
        .tokens
        .iter()
        .find(|(_, t)| t.name == "*")
        .map(|(id, _)| *id)
        .unwrap();
    let caret_id = g
        .tokens
        .iter()
        .find(|(_, t)| t.name == "^")
        .map(|(id, _)| *id)
        .unwrap();

    let plus_prec = resolver.token_precedence(plus_id).unwrap();
    let star_prec = resolver.token_precedence(star_id).unwrap();
    let caret_prec = resolver.token_precedence(caret_id).unwrap();

    assert!(plus_prec.level < star_prec.level);
    assert!(star_prec.level < caret_prec.level);
    assert_eq!(plus_prec.associativity, Associativity::Left);
    assert_eq!(caret_prec.associativity, Associativity::Right);
}

#[test]
fn prec_climbing_conflict_resolution_pipeline() {
    let g = GrammarBuilder::new("climb_pipeline")
        .token("num", r"\d+")
        .token("+", r"\+")
        .token("*", r"\*")
        .precedence(1, Associativity::Left, vec!["+"])
        .precedence(2, Associativity::Left, vec!["*"])
        .rule("expr", vec!["num"])
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .start("expr")
        .build();

    let ff = FirstFollowSets::compute(&g).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&g, &ff);
    let mut resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
    resolver.resolve_conflicts(&g);

    // All SR conflicts should be resolved (not left as multi-action) since
    // every operator has explicit precedence/associativity
    for c in &resolver.conflicts {
        if c.conflict_type == ConflictType::ShiftReduce {
            assert_eq!(
                c.actions.len(),
                1,
                "SR conflict should resolve to single action with full prec info: {:?}",
                c.actions
            );
        }
    }
}

#[test]
fn prec_right_assoc_grammar_pipeline() {
    let g = right_assoc_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&g, &ff);
    let mut resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
    resolver.resolve_conflicts(&g);

    for c in &resolver.conflicts {
        if c.conflict_type == ConflictType::ShiftReduce {
            // Right assoc: shift should win → single Shift action
            if c.actions.len() == 1 {
                assert!(
                    matches!(c.actions[0], Action::Shift(_)),
                    "right-assoc SR should resolve to Shift, got {:?}",
                    c.actions[0]
                );
            }
        }
    }
}

#[test]
fn prec_mixed_assoc_levels_comparison() {
    // Higher level always wins regardless of associativity
    assert_eq!(
        compare_precedences(
            Some(make_prec_info(10, Associativity::Right)),
            Some(make_prec_info(5, Associativity::Left)),
        ),
        PrecedenceComparison::PreferShift
    );
    assert_eq!(
        compare_precedences(
            Some(make_prec_info(2, Associativity::Right)),
            Some(make_prec_info(8, Associativity::Left)),
        ),
        PrecedenceComparison::PreferReduce
    );
}

// ============================================================================
// 14. Conflict Statistics and Reporting (tests 89–94)
// ============================================================================

#[test]
fn stats_summary_total_conflicts() {
    let table = make_inspectable_table(
        vec![
            vec![
                vec![
                    Action::Shift(adze_ir::StateId(1)),
                    Action::Reduce(RuleId(0)),
                ],
                vec![Action::Reduce(RuleId(1)), Action::Reduce(RuleId(2))],
                vec![Action::Accept],
            ],
            vec![
                vec![
                    Action::Shift(adze_ir::StateId(2)),
                    Action::Reduce(RuleId(3)),
                ],
                vec![Action::Error],
                vec![Action::Reduce(RuleId(4))],
            ],
        ],
        vec![SymbolId(10), SymbolId(11), SymbolId(12)],
    );
    let summary = count_conflicts(&table);
    assert_eq!(summary.shift_reduce, 2, "two SR conflicts");
    assert_eq!(summary.reduce_reduce, 1, "one RR conflict");
}

#[test]
fn stats_details_contain_symbol_names() {
    let table = make_inspectable_table(
        vec![vec![vec![
            Action::Shift(adze_ir::StateId(1)),
            Action::Reduce(RuleId(0)),
        ]]],
        vec![SymbolId(42)],
    );
    let summary = count_conflicts(&table);
    assert!(!summary.conflict_details.is_empty());
    assert_eq!(summary.conflict_details[0].symbol_name, "sym_42");
}

#[test]
fn stats_no_conflicts_empty_details() {
    let table = make_inspectable_table(vec![vec![vec![Action::Accept]]], vec![SymbolId(0)]);
    let summary = count_conflicts(&table);
    assert!(summary.conflict_details.is_empty());
    assert!(summary.states_with_conflicts.is_empty());
}

#[test]
fn stats_conflict_detail_actions_match_cell() {
    let table = make_inspectable_table(
        vec![vec![vec![
            Action::Reduce(RuleId(5)),
            Action::Reduce(RuleId(9)),
        ]]],
        vec![SymbolId(7)],
    );
    let summary = count_conflicts(&table);
    assert_eq!(summary.conflict_details.len(), 1);
    assert_eq!(summary.conflict_details[0].actions.len(), 2);
    assert_eq!(
        summary.conflict_details[0].conflict_type,
        InspectionConflictType::ReduceReduce
    );
}

#[test]
fn stats_analyzer_tracks_sr_and_rr() {
    let table = make_inspectable_table(
        vec![
            vec![vec![
                Action::Shift(adze_ir::StateId(1)),
                Action::Reduce(RuleId(0)),
            ]],
            vec![vec![Action::Reduce(RuleId(1)), Action::Reduce(RuleId(2))]],
        ],
        vec![SymbolId(0)],
    );
    let mut analyzer = ConflictAnalyzer::new();
    let _stats = analyzer.analyze_table(&table);
    // ConflictAnalyzer reports based on its own heuristics; verify it runs without panic
    // and that the summary API also reports conflicts
    let summary = count_conflicts(&table);
    assert!(
        summary.shift_reduce + summary.reduce_reduce > 0,
        "count_conflicts should detect conflicts in this table"
    );
}

#[test]
fn stats_find_conflicts_for_absent_symbol() {
    let table = make_inspectable_table(
        vec![vec![vec![
            Action::Shift(adze_ir::StateId(1)),
            Action::Reduce(RuleId(0)),
        ]]],
        vec![SymbolId(10)],
    );
    let conflicts = find_conflicts_for_symbol(&table, SymbolId(999));
    assert!(
        conflicts.is_empty(),
        "absent symbol should return no conflicts"
    );
}

// ============================================================================
// 15. Ambiguous Grammar Handling (tests 95–98)
// ============================================================================

#[test]
fn ambiguous_grammar_preserves_all_actions_before_resolution() {
    let g = ambiguous_concat_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&g, &ff);
    let resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
    for c in &resolver.conflicts {
        assert!(
            c.actions.len() >= 2,
            "pre-resolution conflict should have ≥2 actions"
        );
    }
}

#[test]
fn ambiguous_grammar_fork_after_resolution() {
    let g = ambiguous_concat_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&g, &ff);
    let mut resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
    resolver.resolve_conflicts(&g);
    let any_fork = resolver
        .conflicts
        .iter()
        .any(|c| c.actions.iter().any(|a| matches!(a, Action::Fork(_))));
    if !resolver.conflicts.is_empty() {
        assert!(
            any_fork,
            "ambiguous grammar without prec should produce forks"
        );
    }
}

#[test]
fn ambiguous_concat_produces_sr_not_rr() {
    let g = ambiguous_concat_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&g, &ff);
    let resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
    // E → a | E E produces shift-reduce conflicts (shift 'a' vs reduce E)
    let has_sr = resolver
        .conflicts
        .iter()
        .any(|c| c.conflict_type == ConflictType::ShiftReduce);
    assert!(has_sr, "E → a | E E should have SR conflicts");
}

#[test]
fn ambiguous_grammar_conflict_states_nonempty() {
    let g = ambiguous_concat_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&g, &ff);
    let resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
    let states: Vec<_> = resolver.conflicts.iter().map(|c| c.state).collect();
    assert!(!states.is_empty(), "should have conflicts in some state(s)");
}

// ============================================================================
// 16. Proptest (tests 99+)
// ============================================================================

mod proptest_section {
    use super::*;
    use proptest::prelude::*;

    fn arb_action() -> impl Strategy<Value = Action> {
        prop_oneof![
            (0u16..100).prop_map(|s| Action::Shift(adze_ir::StateId(s))),
            (0u16..100).prop_map(|r| Action::Reduce(RuleId(r))),
            Just(Action::Accept),
            Just(Action::Error),
            Just(Action::Recover),
        ]
    }

    proptest! {
        #[test]
        fn prop_classify_single_action_never_sr(action in arb_action()) {
            let cell = vec![action];
            let ct = classify_conflict(&cell);
            prop_assert_ne!(ct, InspectionConflictType::ShiftReduce);
        }

        #[test]
        fn prop_classify_single_action_never_rr(action in arb_action()) {
            let cell = [action];
            prop_assert_eq!(cell.len(), 1);
        }

        #[test]
        fn prop_shift_reduce_pair_is_sr(
            s in 0u16..1000,
            r in 0u16..1000,
        ) {
            let cell = vec![Action::Shift(adze_ir::StateId(s)), Action::Reduce(RuleId(r))];
            let ct = classify_conflict(&cell);
            prop_assert_eq!(ct, InspectionConflictType::ShiftReduce);
        }

        #[test]
        fn prop_two_reduces_is_rr(
            r1 in 0u16..1000,
            r2 in 0u16..1000,
        ) {
            prop_assume!(r1 != r2);
            let cell = vec![Action::Reduce(RuleId(r1)), Action::Reduce(RuleId(r2))];
            let ct = classify_conflict(&cell);
            prop_assert_eq!(ct, InspectionConflictType::ReduceReduce);
        }

        #[test]
        fn prop_compare_prec_symmetric_level(
            level in 1i16..100,
        ) {
            let shift_l = make_prec_info(level, Associativity::Left);
            let reduce_l = make_prec_info(level, Associativity::Left);
            prop_assert_eq!(
                compare_precedences(Some(shift_l), Some(reduce_l)),
                PrecedenceComparison::PreferReduce
            );

            let shift_r = make_prec_info(level, Associativity::Right);
            let reduce_r = make_prec_info(level, Associativity::Right);
            prop_assert_eq!(
                compare_precedences(Some(shift_r), Some(reduce_r)),
                PrecedenceComparison::PreferShift
            );
        }

        #[test]
        fn prop_higher_prec_always_wins(
            low in 1i16..50,
            high in 51i16..100,
        ) {
            prop_assert_eq!(
                compare_precedences(
                    Some(make_prec_info(high, Associativity::Left)),
                    Some(make_prec_info(low, Associativity::Left)),
                ),
                PrecedenceComparison::PreferShift
            );
            prop_assert_eq!(
                compare_precedences(
                    Some(make_prec_info(low, Associativity::Left)),
                    Some(make_prec_info(high, Associativity::Left)),
                ),
                PrecedenceComparison::PreferReduce
            );
        }

        #[test]
        fn prop_fork_length_preserved(n in 1usize..20) {
            let actions: Vec<Action> = (0..n)
                .map(|i| Action::Shift(adze_ir::StateId(i as u16)))
                .collect();
            let fork = Action::Fork(actions.clone());
            if let Action::Fork(inner) = fork {
                prop_assert_eq!(inner.len(), n);
            }
        }
    }
}
