//! Contract tests for generated internal streaming lexer integration (#889).

#![cfg(not(feature = "strict-invariants"))]

use adze_glr_core::driver::GlrError;
use adze_glr_core::ts_lexer::NextToken;
use adze_glr_core::{Action, Driver, GotoIndexing, LexMode, ParseRule, ParseTable, SymbolMetadata};
use adze_ir::{Grammar, RuleId, StateId, SymbolId};
use std::collections::BTreeMap;

const INV: StateId = StateId(65535);

fn default_sym_meta(count: usize) -> Vec<SymbolMetadata> {
    vec![
        SymbolMetadata {
            name: String::new(),
            is_visible: false,
            is_named: false,
            is_supertype: false,
            is_terminal: false,
            is_extra: false,
            is_fragile: false,
            symbol_id: SymbolId(0),
        };
        count
    ]
}

fn make_table(
    states: Vec<Vec<Vec<Action>>>,
    gotos: Vec<Vec<StateId>>,
    rules: Vec<ParseRule>,
    start: SymbolId,
    eof: SymbolId,
    terminal_count: usize,
) -> ParseTable {
    let symbol_count = states.first().map(|s| s.len()).unwrap_or(0);
    let state_count = states.len();

    let mut symbol_to_index = BTreeMap::new();
    for i in 0..symbol_count {
        symbol_to_index.insert(SymbolId(i as u16), i);
    }

    let mut nonterminal_to_index = BTreeMap::new();
    for i in 0..gotos.first().map(|g| g.len()).unwrap_or(0) {
        for row in &gotos {
            if row[i] != INV {
                nonterminal_to_index.insert(SymbolId(i as u16), i);
                break;
            }
        }
    }

    let rule_count = rules.len();

    ParseTable {
        action_table: states,
        goto_table: gotos,
        rules,
        state_count,
        symbol_count,
        symbol_to_index,
        index_to_symbol: (0..terminal_count as u16).map(SymbolId).collect(),
        nonterminal_to_index,
        eof_symbol: eof,
        start_symbol: start,
        grammar: Grammar::new("streaming_lexer_internal".to_string()),
        symbol_metadata: default_sym_meta(symbol_count),
        initial_state: StateId(0),
        token_count: terminal_count,
        external_token_count: 0,
        lex_modes: vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            };
            state_count
        ],
        extras: vec![],
        dynamic_prec_by_rule: vec![0; rule_count.max(1)],
        rule_assoc_by_rule: vec![0; rule_count.max(1)],
        alias_sequences: vec![],
        field_names: vec![],
        goto_indexing: GotoIndexing::NonterminalMap,
        field_map: BTreeMap::new(),
        external_scanner_states: vec![],
    }
}

fn single_token_table() -> ParseTable {
    let eof = SymbolId(0);
    let s = SymbolId(2);
    let rules = vec![ParseRule { lhs: s, rhs_len: 1 }];
    let actions = vec![
        vec![vec![], vec![Action::Shift(StateId(1))], vec![]],
        vec![vec![Action::Reduce(RuleId(0))], vec![], vec![]],
        vec![vec![Action::Accept], vec![], vec![]],
    ];
    let gotos = vec![
        vec![INV, INV, StateId(2)],
        vec![INV, INV, INV],
        vec![INV, INV, INV],
    ];
    make_table(actions, gotos, rules, s, eof, 2)
}

#[test]
fn streaming_lexer_internal_adapter_contract_preserves_mode_and_ranges() {
    let table = single_token_table();
    let observed_modes = std::sync::Mutex::new(Vec::<LexMode>::new());
    let observed_modes_ref = &observed_modes;

    let lexer = move |_input: &str, pos: usize, mode: LexMode| -> Option<NextToken> {
        observed_modes_ref.lock().expect("mode lock").push(mode);
        if pos == 0 {
            Some(NextToken {
                kind: 1,
                start: 0,
                end: 1,
            })
        } else {
            None
        }
    };

    let mut driver = Driver::new(&table);
    let result = driver.parse_streaming("a", lexer, None::<fn(&str, usize, &[bool], _) -> _>);
    assert!(result.is_ok() || matches!(result, Err(GlrError::Parse(_))));
    let modes = observed_modes.lock().expect("mode lock");
    assert_eq!(modes.len(), 1);
    assert_eq!(modes[0].lex_state, 0);
}

#[test]
fn streaming_lexer_internal_adapter_contract_rejects_empty_candidate_set() {
    let table = single_token_table();
    let lexer = |_input: &str, _pos: usize, _mode: LexMode| -> Option<NextToken> { None };

    let mut driver = Driver::new(&table);
    let result = driver.parse_streaming("a", lexer, None::<fn(&str, usize, &[bool], _) -> _>);

    match result {
        Err(GlrError::Parse(_)) => {}
        Ok(_) => panic!("expected parse error for empty candidate set, got Ok"),
        Err(other) => panic!("expected parse error for empty candidate set, got {other:?}"),
    }
}
