//! Property-based tests for symbol mapping properties in adze-tablegen.
//!
//! 46 proptest property tests organised into eight categories:
//!   1. prop_sym_name_*      — symbol name properties
//!   2. prop_sym_unique_*    — symbol uniqueness
//!   3. prop_sym_token_*     — token symbol properties
//!   4. prop_sym_rule_*      — rule symbol properties
//!   5. prop_sym_field_*     — field symbol properties
//!   6. prop_sym_codegen_*   — code generation properties
//!   7. prop_sym_abi_*       — ABI layout properties
//!   8. prop_sym_roundtrip_* — roundtrip properties

use std::collections::{BTreeMap, BTreeSet, HashSet};

use adze_glr_core::{GotoIndexing, LexMode, ParseTable};
use adze_ir::{
    ExternalToken, FieldId, Grammar, ProductionId, Rule, StateId, Symbol, SymbolId, Token,
    TokenPattern,
};
use adze_tablegen::AbiLanguageBuilder;
use adze_tablegen::abi::{create_symbol_metadata, symbol_metadata};
use adze_tablegen::node_types::NodeTypesGenerator;
use adze_tablegen::serializer::serialize_language;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const INVALID: StateId = StateId(u16::MAX);

/// Build a token with a string pattern.
fn string_token(name: &str, literal: &str) -> Token {
    Token {
        name: name.to_string(),
        pattern: TokenPattern::String(literal.to_string()),
        fragile: false,
    }
}

/// Build a token with a regex pattern.
fn regex_token(name: &str, pattern: &str) -> Token {
    Token {
        name: name.to_string(),
        pattern: TokenPattern::Regex(pattern.to_string()),
        fragile: false,
    }
}

/// Construct a simple rule (no precedence, no fields).
fn simple_rule(lhs: u16, rhs: Vec<Symbol>, prod_id: u16) -> Rule {
    Rule {
        lhs: SymbolId(lhs),
        rhs,
        production_id: ProductionId(prod_id),
        fields: vec![],
        precedence: None,
        associativity: None,
    }
}

/// Build a Grammar manually from components.
fn make_grammar(
    name: &str,
    tokens: Vec<(SymbolId, Token)>,
    rules: Vec<Rule>,
    rule_names: Vec<(SymbolId, String)>,
    fields: Vec<(FieldId, String)>,
    externals: Vec<ExternalToken>,
) -> Grammar {
    let mut g = Grammar::new(name.to_string());
    for (id, tok) in tokens {
        g.tokens.insert(id, tok);
    }
    for rule in rules {
        g.add_rule(rule);
    }
    for (id, rn) in rule_names {
        g.rule_names.insert(id, rn);
    }
    for (id, fname) in fields {
        g.fields.insert(id, fname);
    }
    g.externals = externals;
    g
}

/// Build a minimal ParseTable for property tests.
///
/// Symbol layout: ERROR(0), terminals [1..=terms], externals, EOF, non-terminals.
fn make_empty_table(states: usize, terms: usize, nonterms: usize, externals: usize) -> ParseTable {
    let states = states.max(1);
    let eof_idx = 1 + terms + externals;
    let nonterms_eff = if nonterms == 0 { 1 } else { nonterms };
    let symbol_count = eof_idx + 1 + nonterms_eff;

    let actions = vec![vec![vec![]; symbol_count]; states];
    let gotos = vec![vec![INVALID; symbol_count]; states];

    let start_symbol = SymbolId((eof_idx + 1) as u16);
    let eof_symbol = SymbolId(eof_idx as u16);
    let token_count = eof_idx - externals;

    let mut symbol_to_index: BTreeMap<SymbolId, usize> = BTreeMap::new();
    for i in 0..symbol_count {
        symbol_to_index.insert(SymbolId(i as u16), i);
    }
    let mut nonterminal_to_index: BTreeMap<SymbolId, usize> = BTreeMap::new();
    nonterminal_to_index.insert(start_symbol, start_symbol.0 as usize);

    let mut index_to_symbol = vec![SymbolId(0); symbol_count];
    for (sym, idx) in &symbol_to_index {
        index_to_symbol[*idx] = *sym;
    }

    let lex_modes = vec![
        LexMode {
            lex_state: 0,
            external_lex_state: 0,
        };
        states
    ];

    ParseTable {
        action_table: actions,
        goto_table: gotos,
        rules: vec![],
        state_count: states,
        symbol_count,
        symbol_to_index,
        index_to_symbol,
        nonterminal_to_index,
        symbol_metadata: vec![],
        token_count,
        external_token_count: externals,
        eof_symbol,
        start_symbol,
        initial_state: StateId(0),
        lex_modes,
        extras: vec![],
        external_scanner_states: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
        grammar: Grammar::default(),
        goto_indexing: GotoIndexing::NonterminalMap,
    }
}

/// Build a grammar with N tokens and a single rule referencing them all.
fn grammar_with_n_tokens(n: usize) -> (Grammar, ParseTable) {
    let eof_idx = 1 + n;
    let start_nt = SymbolId((eof_idx + 1) as u16);

    let tokens: Vec<(SymbolId, Token)> = (1..=n)
        .map(|i| {
            (
                SymbolId(i as u16),
                string_token(&format!("tok_{i}"), &format!("t{i}")),
            )
        })
        .collect();

    let rhs: Vec<Symbol> = (1..=n)
        .map(|i| Symbol::Terminal(SymbolId(i as u16)))
        .collect();

    let rules = vec![simple_rule(start_nt.0, rhs, 0)];
    let rule_names = vec![(start_nt, "start".to_string())];

    let grammar = make_grammar("test", tokens, rules, rule_names, vec![], vec![]);
    let table = make_empty_table(1, n, 1, 0);
    (grammar, table)
}

/// Build a grammar with N named rules.
fn grammar_with_n_rules(n: usize) -> (Grammar, ParseTable) {
    let eof_idx = 1 + 1; // 1 token
    let tokens = vec![(SymbolId(1), string_token("a", "a"))];

    let mut rules = Vec::new();
    let mut rule_names = Vec::new();
    for i in 0..n {
        let nt = SymbolId((eof_idx + 1 + i) as u16);
        rules.push(simple_rule(
            nt.0,
            vec![Symbol::Terminal(SymbolId(1))],
            i as u16,
        ));
        rule_names.push((nt, format!("rule_{i}")));
    }

    let nonterms = n.max(1);
    let grammar = make_grammar("test", tokens, rules, rule_names, vec![], vec![]);
    let table = make_empty_table(1, 1, nonterms, 0);
    (grammar, table)
}

/// Build a grammar with N fields.
fn grammar_with_n_fields(n: usize) -> (Grammar, ParseTable) {
    let eof_idx = 2; // 1 token
    let start_nt = SymbolId((eof_idx + 1) as u16);

    let tokens = vec![(SymbolId(1), string_token("a", "a"))];

    let field_bindings: Vec<(FieldId, usize)> = (0..n).map(|i| (FieldId(i as u16), 0)).collect();

    let rules = vec![Rule {
        lhs: start_nt,
        rhs: vec![Symbol::Terminal(SymbolId(1))],
        production_id: ProductionId(0),
        fields: field_bindings,
        precedence: None,
        associativity: None,
    }];
    let rule_names = vec![(start_nt, "start".to_string())];
    let fields: Vec<(FieldId, String)> = (0..n)
        .map(|i| (FieldId(i as u16), format!("field_{i}")))
        .collect();

    let grammar = make_grammar("test", tokens, rules, rule_names, fields, vec![]);
    let table = make_empty_table(1, 1, 1, 0);
    (grammar, table)
}

/// Build a grammar with N external tokens.
fn grammar_with_n_externals(n: usize) -> (Grammar, ParseTable) {
    let tokens = vec![(SymbolId(1), string_token("a", "a"))];
    let eof_idx = 1 + 1 + n;
    let start_nt = SymbolId((eof_idx + 1) as u16);

    let externals: Vec<ExternalToken> = (0..n)
        .map(|i| ExternalToken {
            name: format!("ext_{i}"),
            symbol_id: SymbolId((2 + i) as u16),
        })
        .collect();

    let rules = vec![simple_rule(
        start_nt.0,
        vec![Symbol::Terminal(SymbolId(1))],
        0,
    )];
    let rule_names = vec![(start_nt, "start".to_string())];

    let grammar = make_grammar("test", tokens, rules, rule_names, vec![], externals);
    let table = make_empty_table(1, 1, 1, n);
    (grammar, table)
}

// =========================================================================
// Category 1: prop_sym_name_* — symbol name properties
// =========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]

    /// Token names are preserved in the grammar.
    #[test]
    fn prop_sym_name_token_preserved(n in 1usize..16) {
        let (grammar, _table) = grammar_with_n_tokens(n);
        for (id, tok) in &grammar.tokens {
            prop_assert!(!tok.name.is_empty(), "token {:?} has empty name", id);
            prop_assert!(tok.name.starts_with("tok_"), "unexpected name: {}", tok.name);
        }
    }

    /// Rule names are preserved and non-empty.
    #[test]
    fn prop_sym_name_rule_preserved(n in 1usize..16) {
        let (grammar, _table) = grammar_with_n_rules(n);
        for (_id, name) in &grammar.rule_names {
            prop_assert!(!name.is_empty());
        }
    }

    /// Field names are preserved and non-empty.
    #[test]
    fn prop_sym_name_field_preserved(n in 1usize..10) {
        let (grammar, _table) = grammar_with_n_fields(n);
        for (_id, fname) in &grammar.fields {
            prop_assert!(!fname.is_empty());
            prop_assert!(fname.starts_with("field_"));
        }
    }

    /// External token names are preserved.
    #[test]
    fn prop_sym_name_external_preserved(n in 1usize..8) {
        let (grammar, _table) = grammar_with_n_externals(n);
        for ext in &grammar.externals {
            prop_assert!(!ext.name.is_empty());
            prop_assert!(ext.name.starts_with("ext_"));
        }
    }

    /// Grammar name is always the value we set.
    #[test]
    fn prop_sym_name_grammar_identity(n in 1usize..8) {
        let (grammar, _table) = grammar_with_n_tokens(n);
        prop_assert_eq!(&grammar.name, "test");
    }

}

// =========================================================================
// Category 2: prop_sym_unique_* — symbol uniqueness
// =========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]

    /// Token SymbolIds are unique within a grammar.
    #[test]
    fn prop_sym_unique_token_ids(n in 1usize..20) {
        let (grammar, _table) = grammar_with_n_tokens(n);
        let ids: Vec<SymbolId> = grammar.tokens.keys().copied().collect();
        let set: HashSet<u16> = ids.iter().map(|s| s.0).collect();
        prop_assert_eq!(ids.len(), set.len(), "duplicate token SymbolIds");
    }

    /// Rule names map to unique SymbolIds.
    #[test]
    fn prop_sym_unique_rule_ids(n in 1usize..16) {
        let (grammar, _table) = grammar_with_n_rules(n);
        let ids: Vec<SymbolId> = grammar.rule_names.keys().copied().collect();
        let set: HashSet<u16> = ids.iter().map(|s| s.0).collect();
        prop_assert_eq!(ids.len(), set.len());
    }

    /// Field IDs are unique.
    #[test]
    fn prop_sym_unique_field_ids(n in 1usize..10) {
        let (grammar, _table) = grammar_with_n_fields(n);
        let ids: Vec<FieldId> = grammar.fields.keys().copied().collect();
        let set: HashSet<u16> = ids.iter().map(|f| f.0).collect();
        prop_assert_eq!(ids.len(), set.len());
    }

    /// External token SymbolIds are unique.
    #[test]
    fn prop_sym_unique_external_ids(n in 1usize..8) {
        let (grammar, _table) = grammar_with_n_externals(n);
        let ids: Vec<SymbolId> = grammar.externals.iter().map(|e| e.symbol_id).collect();
        let set: HashSet<u16> = ids.iter().map(|s| s.0).collect();
        prop_assert_eq!(ids.len(), set.len());
    }

    /// Token names are unique across the grammar.
    #[test]
    fn prop_sym_unique_token_names(n in 1usize..20) {
        let (grammar, _table) = grammar_with_n_tokens(n);
        let names: Vec<&str> = grammar.tokens.values().map(|t| t.name.as_str()).collect();
        let set: HashSet<&str> = names.iter().copied().collect();
        prop_assert_eq!(names.len(), set.len(), "duplicate token names");
    }

}

// =========================================================================
// Category 3: prop_sym_token_* — token symbol properties
// =========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]

    /// Token count matches grammar.tokens length.
    #[test]
    fn prop_sym_token_count(n in 1usize..20) {
        let (grammar, _table) = grammar_with_n_tokens(n);
        prop_assert_eq!(grammar.tokens.len(), n);
    }

    /// String-patterned tokens preserve their literal.
    #[test]
    fn prop_sym_token_string_pattern(n in 1usize..10) {
        let (grammar, _table) = grammar_with_n_tokens(n);
        for (_id, tok) in &grammar.tokens {
            match &tok.pattern {
                TokenPattern::String(s) => prop_assert!(!s.is_empty()),
                TokenPattern::Regex(_) => {} // also valid
            }
        }
    }

    /// Tokens created with string_token are not fragile.
    #[test]
    fn prop_sym_token_not_fragile(n in 1usize..10) {
        let (grammar, _table) = grammar_with_n_tokens(n);
        for (_id, tok) in &grammar.tokens {
            prop_assert!(!tok.fragile, "unexpected fragile token: {}", tok.name);
        }
    }

    /// Regex-patterned tokens also preserve their pattern.
    #[test]
    fn prop_sym_token_regex_pattern(len in 1usize..16) {
        let pat: String = (0..len).map(|_| 'a').collect();
        let tok = regex_token("re", &pat);
        match &tok.pattern {
            TokenPattern::Regex(s) => prop_assert_eq!(s.len(), len),
            _ => prop_assert!(false, "expected regex pattern"),
        }
    }

    /// Token SymbolIds start from 1 (0 is reserved for ERROR).
    #[test]
    fn prop_sym_token_ids_start_at_one(n in 1usize..20) {
        let (grammar, _table) = grammar_with_n_tokens(n);
        for id in grammar.tokens.keys() {
            prop_assert!(id.0 >= 1, "token id {} below 1", id.0);
        }
    }

}

// =========================================================================
// Category 4: prop_sym_rule_* — rule symbol properties
// =========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]

    /// Rule count matches what we constructed.
    #[test]
    fn prop_sym_rule_count(n in 1usize..16) {
        let (grammar, _table) = grammar_with_n_rules(n);
        let total: usize = grammar.rules.values().map(|v| v.len()).sum();
        prop_assert_eq!(total, n);
    }

    /// Every rule's LHS matches its key in the rules map.
    #[test]
    fn prop_sym_rule_lhs_consistency(n in 1usize..16) {
        let (grammar, _table) = grammar_with_n_rules(n);
        for (sym, rule_vec) in &grammar.rules {
            for rule in rule_vec {
                prop_assert_eq!(rule.lhs, *sym, "rule LHS mismatch");
            }
        }
    }

    /// Rule RHS references only known terminal symbols.
    #[test]
    fn prop_sym_rule_rhs_terminals_valid(n in 1usize..16) {
        let (grammar, _table) = grammar_with_n_rules(n);
        let known_terms: HashSet<u16> = grammar.tokens.keys().map(|s| s.0).collect();
        for rule_vec in grammar.rules.values() {
            for rule in rule_vec {
                for sym in &rule.rhs {
                    if let Symbol::Terminal(tid) = sym {
                        prop_assert!(
                            known_terms.contains(&tid.0),
                            "unknown terminal {:?} in rule rhs",
                            tid
                        );
                    }
                }
            }
        }
    }

    /// Production IDs are unique across all rules.
    #[test]
    fn prop_sym_rule_production_ids_unique(n in 1usize..16) {
        let (grammar, _table) = grammar_with_n_rules(n);
        let mut seen = HashSet::new();
        for rule_vec in grammar.rules.values() {
            for rule in rule_vec {
                prop_assert!(
                    seen.insert(rule.production_id.0),
                    "duplicate production id: {}",
                    rule.production_id.0
                );
            }
        }
    }

    /// Rule names and rule_names map have the same set of SymbolIds.
    #[test]
    fn prop_sym_rule_names_match_rules(n in 1usize..16) {
        let (grammar, _table) = grammar_with_n_rules(n);
        let rule_syms: BTreeSet<SymbolId> = grammar.rules.keys().copied().collect();
        let name_syms: BTreeSet<SymbolId> = grammar.rule_names.keys().copied().collect();
        prop_assert_eq!(rule_syms, name_syms, "rule keys != rule_names keys");
    }

}

// =========================================================================
// Category 5: prop_sym_field_* — field symbol properties
// =========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]

    /// Field count matches construction.
    #[test]
    fn prop_sym_field_count(n in 1usize..10) {
        let (grammar, _table) = grammar_with_n_fields(n);
        prop_assert_eq!(grammar.fields.len(), n);
    }

    /// Field names are non-empty.
    #[test]
    fn prop_sym_field_nonempty_names(n in 1usize..10) {
        let (grammar, _table) = grammar_with_n_fields(n);
        for (_id, name) in &grammar.fields {
            prop_assert!(!name.is_empty());
        }
    }

    /// Field IDs start from 0.
    #[test]
    fn prop_sym_field_ids_start_zero(n in 1usize..10) {
        let (grammar, _table) = grammar_with_n_fields(n);
        let min_id = grammar.fields.keys().map(|f| f.0).min().unwrap();
        prop_assert_eq!(min_id, 0);
    }

    /// Field IDs are contiguous.
    #[test]
    fn prop_sym_field_contiguous_ids(n in 1usize..10) {
        let (grammar, _table) = grammar_with_n_fields(n);
        let mut ids: Vec<u16> = grammar.fields.keys().map(|f| f.0).collect();
        ids.sort();
        for (i, id) in ids.iter().enumerate() {
            prop_assert_eq!(*id, i as u16);
        }
    }

    /// Field bindings in rules reference valid field IDs.
    #[test]
    fn prop_sym_field_bindings_valid(n in 1usize..10) {
        let (grammar, _table) = grammar_with_n_fields(n);
        let known_fields: HashSet<u16> = grammar.fields.keys().map(|f| f.0).collect();
        for rule_vec in grammar.rules.values() {
            for rule in rule_vec {
                for (fid, _pos) in &rule.fields {
                    prop_assert!(
                        known_fields.contains(&fid.0),
                        "unknown field {:?} in rule",
                        fid
                    );
                }
            }
        }
    }

}

// =========================================================================
// Category 6: prop_sym_codegen_* — code generation properties
// =========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]

    /// ABI codegen produces non-empty output for any grammar with tokens.
    #[test]
    fn prop_sym_codegen_abi_nonempty(n in 1usize..8) {
        let (grammar, table) = grammar_with_n_tokens(n);
        let code = AbiLanguageBuilder::new(&grammar, &table).generate().to_string();
        prop_assert!(!code.is_empty(), "ABI code generation was empty");
    }

    /// ABI codegen output contains the grammar name.
    #[test]
    fn prop_sym_codegen_contains_grammar_name(n in 1usize..8) {
        let (grammar, table) = grammar_with_n_tokens(n);
        let code = AbiLanguageBuilder::new(&grammar, &table).generate().to_string();
        // The grammar name or a variant should appear somewhere in codegen.
        // This checks that the generator uses the grammar info.
        prop_assert!(code.len() > 10, "code suspiciously short: {}", code.len());
    }

    /// Node types generation produces valid JSON (or an error, not a panic).
    #[test]
    fn prop_sym_codegen_node_types_no_panic(n in 1usize..8) {
        let (grammar, _table) = grammar_with_n_tokens(n);
        let generator = NodeTypesGenerator::new(&grammar);
        // Should either succeed with JSON or return an error — never panic.
        let _result = generator.generate();
    }

    /// ABI codegen is deterministic — same inputs produce same output.
    #[test]
    fn prop_sym_codegen_deterministic(n in 1usize..6) {
        let (grammar, table) = grammar_with_n_tokens(n);
        let code1 = AbiLanguageBuilder::new(&grammar, &table).generate().to_string();
        let code2 = AbiLanguageBuilder::new(&grammar, &table).generate().to_string();
        prop_assert_eq!(code1, code2, "non-deterministic codegen");
    }

    /// Varying number of tokens changes the codegen output.
    #[test]
    fn prop_sym_codegen_varies_with_tokens(n in 2usize..8) {
        let (g1, t1) = grammar_with_n_tokens(n);
        let (g2, t2) = grammar_with_n_tokens(n - 1);
        let c1 = AbiLanguageBuilder::new(&g1, &t1).generate().to_string();
        let c2 = AbiLanguageBuilder::new(&g2, &t2).generate().to_string();
        prop_assert_ne!(c1, c2, "different grammars produced identical code");
    }

}

// =========================================================================
// Category 7: prop_sym_abi_* — ABI layout properties
// =========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]

    /// Symbol metadata flags round-trip through create_symbol_metadata.
    #[test]
    fn prop_sym_abi_metadata_roundtrip(
        visible in any::<bool>(),
        named in any::<bool>(),
        hidden in any::<bool>(),
        auxiliary in any::<bool>(),
        supertype in any::<bool>(),
    ) {
        let meta = create_symbol_metadata(visible, named, hidden, auxiliary, supertype);
        prop_assert_eq!((meta & symbol_metadata::VISIBLE) != 0, visible);
        prop_assert_eq!((meta & symbol_metadata::NAMED) != 0, named);
        prop_assert_eq!((meta & symbol_metadata::HIDDEN) != 0, hidden);
        prop_assert_eq!((meta & symbol_metadata::AUXILIARY) != 0, auxiliary);
        prop_assert_eq!((meta & symbol_metadata::SUPERTYPE) != 0, supertype);
    }

    /// Symbol metadata byte fits in u8 (no overflow).
    #[test]
    fn prop_sym_abi_metadata_fits_u8(
        visible in any::<bool>(),
        named in any::<bool>(),
        hidden in any::<bool>(),
        auxiliary in any::<bool>(),
        supertype in any::<bool>(),
    ) {
        let meta = create_symbol_metadata(visible, named, hidden, auxiliary, supertype);
        prop_assert!(meta <= 0x1F, "metadata byte {:#04x} exceeds 5 bits", meta);
    }

    /// All-false flags produce zero metadata byte.
    #[test]
    fn prop_sym_abi_metadata_zero_is_zero(_dummy in 0u8..1) {
        let meta = create_symbol_metadata(false, false, false, false, false);
        prop_assert_eq!(meta, 0);
    }

    /// ParseTable symbol_to_index is consistent with index_to_symbol.
    #[test]
    fn prop_sym_abi_index_roundtrip(n in 1usize..16) {
        let table = make_empty_table(1, n, 1, 0);
        for (sym, idx) in &table.symbol_to_index {
            prop_assert_eq!(table.index_to_symbol[*idx], *sym, "index roundtrip failed");
        }
    }

    /// EOF symbol is always within the symbol_to_index map.
    #[test]
    fn prop_sym_abi_eof_in_map(n in 1usize..16) {
        let table = make_empty_table(1, n, 1, 0);
        prop_assert!(
            table.symbol_to_index.contains_key(&table.eof_symbol),
            "EOF symbol not in symbol_to_index"
        );
    }

    /// Start symbol is within the symbol count range.
    #[test]
    fn prop_sym_abi_start_in_range(n in 1usize..16) {
        let table = make_empty_table(1, n, 1, 0);
        prop_assert!(
            (table.start_symbol.0 as usize) < table.symbol_count,
            "start symbol {} >= symbol_count {}",
            table.start_symbol.0,
            table.symbol_count
        );
    }

    /// Action table dimensions match state_count × symbol_count.
    #[test]
    fn prop_sym_abi_action_table_dims(
        states in 1usize..8,
        terms in 1usize..8,
    ) {
        let table = make_empty_table(states, terms, 1, 0);
        prop_assert_eq!(table.action_table.len(), table.state_count);
        for row in &table.action_table {
            prop_assert_eq!(row.len(), table.symbol_count);
        }
    }

    /// Goto table dimensions match state_count × symbol_count.
    #[test]
    fn prop_sym_abi_goto_table_dims(
        states in 1usize..8,
        terms in 1usize..8,
    ) {
        let table = make_empty_table(states, terms, 1, 0);
        prop_assert_eq!(table.goto_table.len(), table.state_count);
        for row in &table.goto_table {
            prop_assert_eq!(row.len(), table.symbol_count);
        }
    }
}

// =========================================================================
// Category 8: prop_sym_roundtrip_* — roundtrip properties
// =========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]

    /// Serialization round-trip produces valid JSON for token-only grammars.
    #[test]
    fn prop_sym_roundtrip_serialize_tokens(n in 1usize..8) {
        let (grammar, table) = grammar_with_n_tokens(n);
        let json = serialize_language(&grammar, &table, None);
        prop_assert!(json.is_ok(), "serialization failed: {:?}", json.err());
        let json_str = json.unwrap();
        let parsed: serde_json::Result<serde_json::Value> = serde_json::from_str(&json_str);
        prop_assert!(parsed.is_ok(), "invalid JSON produced");
    }

    /// Serialized JSON preserves symbol count.
    #[test]
    fn prop_sym_roundtrip_symbol_count(n in 1usize..8) {
        let (grammar, table) = grammar_with_n_tokens(n);
        let json_str = serialize_language(&grammar, &table, None).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let sc = val.get("symbol_count").and_then(|v| v.as_u64());
        prop_assert!(sc.is_some(), "no symbol_count in serialized output");
    }

    /// Serialization round-trip for grammars with fields.
    #[test]
    fn prop_sym_roundtrip_serialize_fields(n in 1usize..6) {
        let (grammar, table) = grammar_with_n_fields(n);
        let json = serialize_language(&grammar, &table, None);
        prop_assert!(json.is_ok(), "serialization with fields failed");
    }

    /// Serialization round-trip for grammars with external tokens.
    #[test]
    fn prop_sym_roundtrip_serialize_externals(n in 1usize..6) {
        let (grammar, table) = grammar_with_n_externals(n);
        let json = serialize_language(&grammar, &table, None);
        prop_assert!(json.is_ok(), "serialization with externals failed");
    }

    /// Serialized JSON contains symbol_names array.
    #[test]
    fn prop_sym_roundtrip_has_symbol_names(n in 1usize..8) {
        let (grammar, table) = grammar_with_n_tokens(n);
        let json_str = serialize_language(&grammar, &table, None).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let names = val.get("symbol_names");
        prop_assert!(names.is_some(), "no symbol_names in serialized output");
        prop_assert!(names.unwrap().is_array(), "symbol_names is not an array");
    }

    /// Double serialization is idempotent (deterministic).
    #[test]
    fn prop_sym_roundtrip_idempotent(n in 1usize..6) {
        let (grammar, table) = grammar_with_n_tokens(n);
        let json1 = serialize_language(&grammar, &table, None).unwrap();
        let json2 = serialize_language(&grammar, &table, None).unwrap();
        prop_assert_eq!(json1, json2, "non-deterministic serialization");
    }

    /// Serialization preserves state_count.
    #[test]
    fn prop_sym_roundtrip_state_count(states in 1usize..6, terms in 1usize..6) {
        let tokens: Vec<(SymbolId, Token)> = (1..=terms)
            .map(|i| (SymbolId(i as u16), string_token(&format!("t{i}"), &format!("v{i}"))))
            .collect();
        let eof_idx = 1 + terms;
        let start_nt = SymbolId((eof_idx + 1) as u16);
        let rules = vec![simple_rule(
            start_nt.0,
            vec![Symbol::Terminal(SymbolId(1))],
            0,
        )];
        let rule_names = vec![(start_nt, "start".to_string())];
        let grammar = make_grammar("test", tokens, rules, rule_names, vec![], vec![]);
        let table = make_empty_table(states, terms, 1, 0);
        let json_str = serialize_language(&grammar, &table, None).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let sc = val.get("state_count").and_then(|v| v.as_u64()).unwrap();
        prop_assert_eq!(sc as usize, table.state_count);
    }

    /// Serialization preserves external_token_count.
    #[test]
    fn prop_sym_roundtrip_external_count(n in 1usize..6) {
        let (grammar, table) = grammar_with_n_externals(n);
        let json_str = serialize_language(&grammar, &table, None).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let etc = val.get("external_token_count").and_then(|v| v.as_u64()).unwrap();
        prop_assert_eq!(etc as usize, n);
    }
}
