#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]
//! Property-based tests for advanced conflict resolution in adze-glr-core.
//!
//! Run with: `cargo test -p adze-glr-core --test advanced_conflict_proptest`

use adze_glr_core::advanced_conflict::{
    ConflictAnalyzer, ConflictStats, PrecedenceDecision, PrecedenceResolver,
};
use adze_glr_core::conflict_inspection::{ConflictType, classify_conflict, count_conflicts};
use adze_glr_core::{
    Action, ConflictResolver, ConflictType as CrConflictType, GotoIndexing, ParseTable,
};
use adze_ir::{
    Associativity, Grammar, Precedence, PrecedenceKind, ProductionId, Rule, RuleId, StateId,
    Symbol, SymbolId, Token, TokenPattern,
};
use proptest::prelude::*;
use std::collections::BTreeMap;

// ============================================================================
// Strategies
// ============================================================================

fn arb_symbol_id() -> impl Strategy<Value = SymbolId> {
    (1u16..200).prop_map(SymbolId)
}

fn arb_state_id() -> impl Strategy<Value = StateId> {
    (0u16..500).prop_map(StateId)
}

fn arb_rule_id() -> impl Strategy<Value = RuleId> {
    (0u16..500).prop_map(RuleId)
}

fn arb_associativity() -> impl Strategy<Value = Associativity> {
    prop_oneof![
        Just(Associativity::Left),
        Just(Associativity::Right),
        Just(Associativity::None),
    ]
}

fn arb_prec_level() -> impl Strategy<Value = i16> {
    1i16..=20
}

// ============================================================================
// Helpers
// ============================================================================

/// Build a grammar with one token precedence and one rule precedence.
fn grammar_with_prec(
    token_id: SymbolId,
    token_level: i16,
    token_assoc: Associativity,
    rule_sym: SymbolId,
    rule_level: i16,
    rule_assoc: Associativity,
) -> Grammar {
    let mut g = Grammar::new("prec_test".to_string());
    g.tokens.insert(
        token_id,
        Token {
            name: format!("t{}", token_id.0),
            pattern: TokenPattern::String("x".into()),
            fragile: false,
        },
    );
    g.precedences.push(Precedence {
        level: token_level,
        associativity: token_assoc,
        symbols: vec![token_id],
    });
    g.rule_names.insert(rule_sym, "R".into());
    g.rules.insert(
        rule_sym,
        vec![Rule {
            lhs: rule_sym,
            rhs: vec![Symbol::Terminal(token_id)],
            precedence: Some(PrecedenceKind::Static(rule_level)),
            associativity: Some(rule_assoc),
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    g
}

/// Create a minimal ParseTable for unit-level conflict testing.
fn make_test_table(action_table: Vec<Vec<Vec<Action>>>) -> ParseTable {
    let state_count = action_table.len();
    ParseTable {
        action_table,
        goto_table: vec![],
        symbol_metadata: vec![],
        state_count,
        symbol_count: 1,
        symbol_to_index: BTreeMap::new(),
        index_to_symbol: vec![],
        external_scanner_states: vec![],
        rules: vec![],
        nonterminal_to_index: BTreeMap::new(),
        goto_indexing: GotoIndexing::NonterminalMap,
        eof_symbol: SymbolId(0),
        start_symbol: SymbolId(0),
        grammar: Grammar::new("test".to_string()),
        initial_state: StateId(0),
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

/// Build an expression grammar: E → E op E | num
fn expr_grammar(prec: Option<PrecedenceKind>, assoc: Option<Associativity>) -> Grammar {
    let mut g = Grammar::new("expr".to_string());
    let num = SymbolId(1);
    let op = SymbolId(2);
    let e = SymbolId(10);

    g.tokens.insert(
        num,
        Token {
            name: "num".into(),
            pattern: TokenPattern::Regex(r"\d+".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        op,
        Token {
            name: "+".into(),
            pattern: TokenPattern::String("+".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(e, "E".into());
    g.rules.insert(
        e,
        vec![
            Rule {
                lhs: e,
                rhs: vec![
                    Symbol::NonTerminal(e),
                    Symbol::Terminal(op),
                    Symbol::NonTerminal(e),
                ],
                precedence: prec,
                associativity: assoc,
                fields: vec![],
                production_id: ProductionId(0),
            },
            Rule {
                lhs: e,
                rhs: vec![Symbol::Terminal(num)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(1),
            },
        ],
    );
    g
}

/// Build a reduce/reduce grammar: S → A | B; A → a; B → a
fn rr_grammar() -> Grammar {
    let mut g = Grammar::new("rr".to_string());
    let a_tok = SymbolId(1);
    let s = SymbolId(10);
    let a_nt = SymbolId(11);
    let b_nt = SymbolId(12);

    g.tokens.insert(
        a_tok,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(s, "S".into());
    g.rule_names.insert(a_nt, "A".into());
    g.rule_names.insert(b_nt, "B".into());

    g.rules.insert(
        s,
        vec![
            Rule {
                lhs: s,
                rhs: vec![Symbol::NonTerminal(a_nt)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            },
            Rule {
                lhs: s,
                rhs: vec![Symbol::NonTerminal(b_nt)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(1),
            },
        ],
    );
    g.rules.insert(
        a_nt,
        vec![Rule {
            lhs: a_nt,
            rhs: vec![Symbol::Terminal(a_tok)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(2),
        }],
    );
    g.rules.insert(
        b_nt,
        vec![Rule {
            lhs: b_nt,
            rhs: vec![Symbol::Terminal(a_tok)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(3),
        }],
    );
    g
}

// ============================================================================
// 1. Shift-reduce conflict detection via classify_conflict
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn shift_reduce_detected_by_classify(s in arb_state_id(), r in arb_rule_id()) {
        let actions = vec![Action::Shift(s), Action::Reduce(r)];
        prop_assert_eq!(classify_conflict(&actions), ConflictType::ShiftReduce);
    }
}

// ============================================================================
// 2. Reduce-reduce conflict detection via classify_conflict
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn reduce_reduce_detected_by_classify(r1 in arb_rule_id(), r2 in arb_rule_id()) {
        prop_assume!(r1 != r2);
        let actions = vec![Action::Reduce(r1), Action::Reduce(r2)];
        prop_assert_eq!(classify_conflict(&actions), ConflictType::ReduceReduce);
    }
}

// ============================================================================
// 3. Precedence resolution: higher shift wins
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn prec_higher_shift_prefers_shift(
        shift_level in 2i16..=20,
        reduce_level in 1i16..=19,
        assoc in arb_associativity(),
    ) {
        prop_assume!(shift_level > reduce_level);
        let tok = SymbolId(1);
        let rule_sym = SymbolId(10);
        let g = grammar_with_prec(tok, shift_level, assoc, rule_sym, reduce_level, Associativity::Left);
        let resolver = PrecedenceResolver::new(&g);
        let decision = resolver.can_resolve_shift_reduce(tok, rule_sym);
        prop_assert_eq!(decision, Some(PrecedenceDecision::PreferShift));
    }
}

// ============================================================================
// 4. Precedence resolution: higher reduce wins
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn prec_higher_reduce_prefers_reduce(
        shift_level in 1i16..=19,
        reduce_level in 2i16..=20,
        assoc in arb_associativity(),
    ) {
        prop_assume!(reduce_level > shift_level);
        let tok = SymbolId(1);
        let rule_sym = SymbolId(10);
        let g = grammar_with_prec(tok, shift_level, assoc, rule_sym, reduce_level, Associativity::Left);
        let resolver = PrecedenceResolver::new(&g);
        let decision = resolver.can_resolve_shift_reduce(tok, rule_sym);
        prop_assert_eq!(decision, Some(PrecedenceDecision::PreferReduce));
    }
}

// ============================================================================
// 5. Left-associative same precedence prefers reduce
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn left_assoc_same_prec_prefers_reduce(level in arb_prec_level()) {
        let tok = SymbolId(1);
        let rule_sym = SymbolId(10);
        let g = grammar_with_prec(tok, level, Associativity::Left, rule_sym, level, Associativity::Left);
        let resolver = PrecedenceResolver::new(&g);
        let decision = resolver.can_resolve_shift_reduce(tok, rule_sym);
        prop_assert_eq!(decision, Some(PrecedenceDecision::PreferReduce));
    }
}

// ============================================================================
// 6. Right-associative same precedence prefers shift
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn right_assoc_same_prec_prefers_shift(level in arb_prec_level()) {
        let tok = SymbolId(1);
        let rule_sym = SymbolId(10);
        let g = grammar_with_prec(tok, level, Associativity::Right, rule_sym, level, Associativity::Right);
        let resolver = PrecedenceResolver::new(&g);
        let decision = resolver.can_resolve_shift_reduce(tok, rule_sym);
        prop_assert_eq!(decision, Some(PrecedenceDecision::PreferShift));
    }
}

// ============================================================================
// 7. Non-associative same precedence returns Error
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn none_assoc_same_prec_returns_error(level in arb_prec_level()) {
        let tok = SymbolId(1);
        let rule_sym = SymbolId(10);
        let g = grammar_with_prec(tok, level, Associativity::None, rule_sym, level, Associativity::None);
        let resolver = PrecedenceResolver::new(&g);
        let decision = resolver.can_resolve_shift_reduce(tok, rule_sym);
        prop_assert_eq!(decision, Some(PrecedenceDecision::Error));
    }
}

// ============================================================================
// 8. Unknown symbols return None from PrecedenceResolver
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn unknown_symbols_return_none(s in 100u16..200, r in 200u16..300) {
        let g = Grammar::new("empty".to_string());
        let resolver = PrecedenceResolver::new(&g);
        let decision = resolver.can_resolve_shift_reduce(SymbolId(s), SymbolId(r));
        prop_assert_eq!(decision, None);
    }
}

// ============================================================================
// 9. ConflictAnalyzer starts with zero stats
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn analyzer_fresh_zero_stats(_dummy in 0u8..1) {
        let analyzer = ConflictAnalyzer::new();
        let stats = analyzer.get_stats();
        prop_assert_eq!(stats.shift_reduce_conflicts, 0);
        prop_assert_eq!(stats.reduce_reduce_conflicts, 0);
        prop_assert_eq!(stats.precedence_resolved, 0);
        prop_assert_eq!(stats.associativity_resolved, 0);
        prop_assert_eq!(stats.explicit_glr, 0);
        prop_assert_eq!(stats.default_resolved, 0);
    }
}

// ============================================================================
// 10. ConflictStats::default is all zeros
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn conflict_stats_default_all_zero(_dummy in 0u8..1) {
        let stats = ConflictStats::default();
        let total = stats.shift_reduce_conflicts
            + stats.reduce_reduce_conflicts
            + stats.precedence_resolved
            + stats.associativity_resolved
            + stats.explicit_glr
            + stats.default_resolved;
        prop_assert_eq!(total, 0);
    }
}

// ============================================================================
// 11. Conflict-free table: count_conflicts returns zero
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn conflict_free_table_zero_counts(n_states in 1usize..5) {
        // Each state has one symbol column with exactly one action
        let table = make_test_table(
            (0..n_states)
                .map(|i| vec![vec![Action::Shift(StateId(i as u16 + 1))]])
                .collect(),
        );
        let summary = count_conflicts(&table);
        prop_assert_eq!(summary.shift_reduce, 0);
        prop_assert_eq!(summary.reduce_reduce, 0);
        prop_assert!(summary.states_with_conflicts.is_empty());
    }
}

// ============================================================================
// 12. Single SR conflict counted correctly
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn single_sr_conflict_counted(s in arb_state_id(), r in arb_rule_id()) {
        let table = make_test_table(vec![vec![vec![Action::Shift(s), Action::Reduce(r)]]]);
        let summary = count_conflicts(&table);
        prop_assert_eq!(summary.shift_reduce, 1);
        prop_assert_eq!(summary.reduce_reduce, 0);
    }
}

// ============================================================================
// 13. Single RR conflict counted correctly
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn single_rr_conflict_counted(r1 in arb_rule_id(), r2 in arb_rule_id()) {
        prop_assume!(r1 != r2);
        let table = make_test_table(vec![vec![vec![Action::Reduce(r1), Action::Reduce(r2)]]]);
        let summary = count_conflicts(&table);
        prop_assert_eq!(summary.shift_reduce, 0);
        prop_assert_eq!(summary.reduce_reduce, 1);
    }
}

// ============================================================================
// 14. Multiple conflicts in one state accumulate
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn multiple_conflicts_same_state(n in 1usize..5) {
        // n symbol columns, each with a SR conflict
        let cells: Vec<Vec<Action>> = (0..n)
            .map(|i| vec![Action::Shift(StateId(i as u16)), Action::Reduce(RuleId(i as u16))])
            .collect();
        let table = make_test_table(vec![cells]);
        let summary = count_conflicts(&table);
        prop_assert_eq!(summary.shift_reduce, n);
        prop_assert_eq!(summary.states_with_conflicts.len(), 1);
    }
}

// ============================================================================
// 15. Conflict details length matches SR + RR totals
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn details_len_matches_totals(
        sr_count in 0usize..3,
        rr_count in 0usize..3,
    ) {
        let mut cells: Vec<Vec<Action>> = Vec::new();
        for i in 0..sr_count {
            cells.push(vec![
                Action::Shift(StateId(i as u16)),
                Action::Reduce(RuleId(i as u16)),
            ]);
        }
        for i in 0..rr_count {
            cells.push(vec![
                Action::Reduce(RuleId(100 + i as u16)),
                Action::Reduce(RuleId(200 + i as u16)),
            ]);
        }
        if cells.is_empty() {
            cells.push(vec![Action::Shift(StateId(0))]);
        }
        let table = make_test_table(vec![cells]);
        let summary = count_conflicts(&table);
        prop_assert_eq!(summary.conflict_details.len(), sr_count + rr_count);
    }
}

// ============================================================================
// 16. ConflictAnalyzer.analyze_table returns zero for simple table
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn analyzer_simple_table_no_conflicts(n_states in 1usize..4) {
        let table = make_test_table(
            (0..n_states)
                .map(|i| vec![vec![Action::Shift(StateId(i as u16 + 1))]])
                .collect(),
        );
        let mut analyzer = ConflictAnalyzer::new();
        let stats = analyzer.analyze_table(&table);
        prop_assert_eq!(stats.shift_reduce_conflicts, 0);
        prop_assert_eq!(stats.reduce_reduce_conflicts, 0);
    }
}

// ============================================================================
// 17. PrecedenceDecision equality is reflexive
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn precedence_decision_eq_reflexive(idx in 0usize..3) {
        let variants = [
            PrecedenceDecision::PreferShift,
            PrecedenceDecision::PreferReduce,
            PrecedenceDecision::Error,
        ];
        let v = &variants[idx];
        prop_assert_eq!(v, v);
    }
}

// ============================================================================
// 18. Shift-reduce conflict in fork detected
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn fork_with_sr_classified_as_sr(s in arb_state_id(), r in arb_rule_id()) {
        let actions = vec![Action::Fork(vec![Action::Shift(s), Action::Reduce(r)])];
        prop_assert_eq!(classify_conflict(&actions), ConflictType::ShiftReduce);
    }
}

// ============================================================================
// 19. Reduce-reduce conflict in fork detected
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn fork_with_rr_classified_as_rr(r1 in arb_rule_id(), r2 in arb_rule_id()) {
        prop_assume!(r1 != r2);
        let actions = vec![Action::Fork(vec![Action::Reduce(r1), Action::Reduce(r2)])];
        prop_assert_eq!(classify_conflict(&actions), ConflictType::ReduceReduce);
    }
}

// ============================================================================
// 20. Multiple precedence levels: symmetry test
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn prec_symmetry_shift_vs_reduce(
        level_a in arb_prec_level(),
        level_b in arb_prec_level(),
    ) {
        prop_assume!(level_a != level_b);
        let tok = SymbolId(1);
        let rule_sym = SymbolId(10);
        let g = grammar_with_prec(tok, level_a, Associativity::Left, rule_sym, level_b, Associativity::Left);
        let resolver = PrecedenceResolver::new(&g);
        let decision = resolver.can_resolve_shift_reduce(tok, rule_sym);
        if level_a > level_b {
            prop_assert_eq!(decision, Some(PrecedenceDecision::PreferShift));
        } else {
            prop_assert_eq!(decision, Some(PrecedenceDecision::PreferReduce));
        }
    }
}

// ============================================================================
// 21. GLR fork on unresolved SR: expr grammar without precedence
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn glr_fork_on_unresolved_sr(_dummy in 0u8..1) {
        let g = expr_grammar(None, None);
        let ff = adze_glr_core::FirstFollowSets::compute(&g).unwrap();
        let collection = adze_glr_core::ItemSetCollection::build_canonical_collection(&g, &ff);
        let mut resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
        // Without precedence, we expect conflicts
        let has_sr = resolver.conflicts.iter().any(|c| c.conflict_type == CrConflictType::ShiftReduce);
        prop_assert!(has_sr, "E → E + E | num should have shift/reduce conflict");

        // After resolve, SR conflicts get Fork wrappers
        resolver.resolve_conflicts(&g);
        for conflict in &resolver.conflicts {
            if conflict.conflict_type == CrConflictType::ShiftReduce {
                // Resolved: either a single action or a Fork
                let has_fork = conflict.actions.iter().any(|a| matches!(a, Action::Fork(_)));
                let is_single = conflict.actions.len() == 1;
                prop_assert!(has_fork || is_single,
                    "After resolve, SR conflict should be Fork or single action");
            }
        }
    }
}

// ============================================================================
// 22. GLR fork on unresolved RR: reduce/reduce grammar
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn rr_grammar_has_reduce_reduce(_dummy in 0u8..1) {
        let g = rr_grammar();
        let ff = adze_glr_core::FirstFollowSets::compute(&g).unwrap();
        let collection = adze_glr_core::ItemSetCollection::build_canonical_collection(&g, &ff);
        let resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
        let has_rr = resolver.conflicts.iter().any(|c| c.conflict_type == CrConflictType::ReduceReduce);
        prop_assert!(has_rr, "S → A | B; A → a; B → a should have R/R conflict");
    }
}

// ============================================================================
// 23. Conflict resolution reduces action count for RR conflicts
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn rr_resolution_picks_lowest_rule(_dummy in 0u8..1) {
        let g = rr_grammar();
        let ff = adze_glr_core::FirstFollowSets::compute(&g).unwrap();
        let collection = adze_glr_core::ItemSetCollection::build_canonical_collection(&g, &ff);
        let mut resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
        resolver.resolve_conflicts(&g);
        for conflict in &resolver.conflicts {
            if conflict.conflict_type == CrConflictType::ReduceReduce {
                let has_reduce = conflict.actions.iter().any(|a| matches!(a, Action::Reduce(_)));
                if has_reduce {
                    // If actual Reduce actions exist, resolution picks exactly one
                    let reduce_count = conflict.actions.iter()
                        .filter(|a| matches!(a, Action::Reduce(_)))
                        .count();
                    prop_assert_eq!(reduce_count, 1,
                        "R/R resolution should pick exactly one reduce action");
                }
            }
        }
    }
}

// ============================================================================
// 24. Conflict counting: states_with_conflicts has unique entries
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn conflict_states_are_unique(n in 1usize..4) {
        let mut rows = Vec::new();
        for i in 0..n {
            rows.push(vec![vec![
                Action::Shift(StateId(i as u16)),
                Action::Reduce(RuleId(i as u16)),
            ]]);
        }
        let table = make_test_table(rows);
        let summary = count_conflicts(&table);
        let mut seen = std::collections::HashSet::new();
        for s in &summary.states_with_conflicts {
            prop_assert!(seen.insert(s.0), "duplicate state in states_with_conflicts");
        }
    }
}

// ============================================================================
// 25. Precedence resolver with multiple token precedences
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn multiple_token_precs_independent(
        level_a in arb_prec_level(),
        level_b in arb_prec_level(),
    ) {
        let tok_a = SymbolId(1);
        let tok_b = SymbolId(2);
        let rule_sym = SymbolId(10);
        let mut g = Grammar::new("multi_prec".to_string());
        g.tokens.insert(tok_a, Token { name: "a".into(), pattern: TokenPattern::String("a".into()), fragile: false });
        g.tokens.insert(tok_b, Token { name: "b".into(), pattern: TokenPattern::String("b".into()), fragile: false });
        g.precedences.push(Precedence { level: level_a, associativity: Associativity::Left, symbols: vec![tok_a] });
        g.precedences.push(Precedence { level: level_b, associativity: Associativity::Left, symbols: vec![tok_b] });
        g.rule_names.insert(rule_sym, "R".into());
        g.rules.insert(rule_sym, vec![Rule {
            lhs: rule_sym,
            rhs: vec![Symbol::Terminal(tok_a)],
            precedence: Some(PrecedenceKind::Static(level_a)),
            associativity: Some(Associativity::Left),
            fields: vec![],
            production_id: ProductionId(0),
        }]);

        let resolver = PrecedenceResolver::new(&g);
        // tok_a vs rule_sym: same level → Left → PreferReduce
        let dec_a = resolver.can_resolve_shift_reduce(tok_a, rule_sym);
        prop_assert_eq!(dec_a, Some(PrecedenceDecision::PreferReduce));
        // tok_b vs rule_sym: level_b vs level_a
        let dec_b = resolver.can_resolve_shift_reduce(tok_b, rule_sym);
        if level_b > level_a {
            prop_assert_eq!(dec_b, Some(PrecedenceDecision::PreferShift));
        } else if level_b < level_a {
            prop_assert_eq!(dec_b, Some(PrecedenceDecision::PreferReduce));
        } else {
            prop_assert_eq!(dec_b, Some(PrecedenceDecision::PreferReduce));
        }
    }
}

// ============================================================================
// 26. Empty grammar: PrecedenceResolver returns None for everything
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn empty_grammar_resolver_always_none(a in arb_symbol_id(), b in arb_symbol_id()) {
        let g = Grammar::new("empty".to_string());
        let resolver = PrecedenceResolver::new(&g);
        prop_assert_eq!(resolver.can_resolve_shift_reduce(a, b), None);
    }
}

// ============================================================================
// 27. ConflictAnalyzer analyze_table is idempotent
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn analyzer_idempotent(_dummy in 0u8..1) {
        let table = make_test_table(vec![vec![vec![Action::Shift(StateId(1))]]]);
        let mut analyzer = ConflictAnalyzer::new();
        let stats1 = analyzer.analyze_table(&table);
        let stats2 = analyzer.analyze_table(&table);
        prop_assert_eq!(stats1.shift_reduce_conflicts, stats2.shift_reduce_conflicts);
        prop_assert_eq!(stats1.reduce_reduce_conflicts, stats2.reduce_reduce_conflicts);
    }
}

// ============================================================================
// 28. Associativity variations: all three variants produce deterministic outcome
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn associativity_deterministic(level in arb_prec_level(), assoc in arb_associativity()) {
        let tok = SymbolId(1);
        let rule_sym = SymbolId(10);
        let g = grammar_with_prec(tok, level, assoc, rule_sym, level, assoc);
        let resolver = PrecedenceResolver::new(&g);
        let decision = resolver.can_resolve_shift_reduce(tok, rule_sym);
        match assoc {
            Associativity::Left => prop_assert_eq!(decision, Some(PrecedenceDecision::PreferReduce)),
            Associativity::Right => prop_assert_eq!(decision, Some(PrecedenceDecision::PreferShift)),
            Associativity::None => prop_assert_eq!(decision, Some(PrecedenceDecision::Error)),
        }
    }
}

// ============================================================================
// 29. Table with only Accept actions has no conflicts
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn accept_only_no_conflicts(n in 1usize..5) {
        let table = make_test_table(
            (0..n).map(|_| vec![vec![Action::Accept]]).collect(),
        );
        let summary = count_conflicts(&table);
        prop_assert_eq!(summary.shift_reduce + summary.reduce_reduce, 0);
    }
}

// ============================================================================
// 30. Table with only Error actions has no conflicts
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn error_only_no_conflicts(n in 1usize..5) {
        let table = make_test_table(
            (0..n).map(|_| vec![vec![Action::Error]]).collect(),
        );
        let summary = count_conflicts(&table);
        prop_assert_eq!(summary.shift_reduce + summary.reduce_reduce, 0);
    }
}

// ============================================================================
// 31. Conflict detail types match summary counts
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn detail_types_match_counts(
        n_sr in 0usize..3,
        n_rr in 0usize..3,
    ) {
        let mut cells = Vec::new();
        for i in 0..n_sr {
            cells.push(vec![
                Action::Shift(StateId(i as u16)),
                Action::Reduce(RuleId(i as u16)),
            ]);
        }
        for i in 0..n_rr {
            cells.push(vec![
                Action::Reduce(RuleId(100 + i as u16)),
                Action::Reduce(RuleId(200 + i as u16)),
            ]);
        }
        if cells.is_empty() {
            cells.push(vec![Action::Shift(StateId(0))]);
        }
        let table = make_test_table(vec![cells]);
        let summary = count_conflicts(&table);

        let detail_sr = summary.conflict_details.iter()
            .filter(|d| d.conflict_type == ConflictType::ShiftReduce)
            .count();
        let detail_rr = summary.conflict_details.iter()
            .filter(|d| d.conflict_type == ConflictType::ReduceReduce)
            .count();
        prop_assert_eq!(detail_sr, n_sr);
        prop_assert_eq!(detail_rr, n_rr);
    }
}

// ============================================================================
// 32. ConflictResolver detect_conflicts on conflict-free grammar
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn conflict_free_grammar_no_detect(_dummy in 0u8..1) {
        // S → a  (trivially unambiguous)
        let mut g = Grammar::new("simple".to_string());
        let a = SymbolId(1);
        let s = SymbolId(10);
        g.tokens.insert(a, Token { name: "a".into(), pattern: TokenPattern::String("a".into()), fragile: false });
        g.rule_names.insert(s, "S".into());
        g.rules.insert(s, vec![Rule {
            lhs: s,
            rhs: vec![Symbol::Terminal(a)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }]);
        let ff = adze_glr_core::FirstFollowSets::compute(&g).unwrap();
        let collection = adze_glr_core::ItemSetCollection::build_canonical_collection(&g, &ff);
        let resolver = ConflictResolver::detect_conflicts(&collection, &g, &ff);
        prop_assert!(resolver.conflicts.is_empty(), "S → a should be conflict-free");
    }
}

// ============================================================================
// 33. Precedence with dynamic kind still stored correctly in grammar
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn dynamic_prec_not_used_by_resolver(level in arb_prec_level()) {
        let tok = SymbolId(1);
        let rule_sym = SymbolId(10);
        let mut g = Grammar::new("dyn_prec".to_string());
        g.tokens.insert(tok, Token { name: "t".into(), pattern: TokenPattern::String("x".into()), fragile: false });
        g.precedences.push(Precedence { level, associativity: Associativity::Left, symbols: vec![tok] });
        g.rule_names.insert(rule_sym, "R".into());
        // Rule uses Dynamic precedence — PrecedenceResolver only stores Static
        g.rules.insert(rule_sym, vec![Rule {
            lhs: rule_sym,
            rhs: vec![Symbol::Terminal(tok)],
            precedence: Some(PrecedenceKind::Dynamic(level)),
            associativity: Some(Associativity::Left),
            fields: vec![],
            production_id: ProductionId(0),
        }]);

        let resolver = PrecedenceResolver::new(&g);
        // Dynamic prec should NOT be stored → returns None
        let decision = resolver.can_resolve_shift_reduce(tok, rule_sym);
        prop_assert_eq!(decision, None);
    }
}
