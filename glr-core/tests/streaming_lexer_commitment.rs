//! Adversarial and contract tests for GLR lexical commitment (#928).

#![cfg(not(feature = "strict-invariants"))]

use adze_glr_core::lexical_commitment::{
    CandidateOrigin, GLOBAL_LEXICAL_COMMITMENT_POLICY, LexicalSelectionReason,
    MAX_LEX_CANDIDATES_PER_POSITION, TokenCandidate, compatible_stack_indices,
    select_global_lexical_candidate,
};
use adze_glr_core::ts_lexer::NextToken;
use adze_glr_core::{Action, Driver, GotoIndexing, LexMode, ParseRule, ParseTable, SymbolMetadata};
use adze_ir::{Grammar, StateId, SymbolId};
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

fn divergent_mode_table() -> ParseTable {
    let eof = SymbolId(0);
    let start = SymbolId(3);

    let rules = vec![
        ParseRule {
            lhs: start,
            rhs_len: 2,
        },
        ParseRule {
            lhs: start,
            rhs_len: 2,
        },
    ];

    // State 0 forks to stack tops 1 (mode 0, accepts X) and 2 (mode 1, accepts Y).
    let actions = vec![
        vec![
            vec![],
            vec![Action::Shift(StateId(1)), Action::Shift(StateId(2))],
            vec![],
            vec![],
        ],
        vec![vec![], vec![Action::Shift(StateId(3))], vec![], vec![]],
        vec![vec![], vec![], vec![Action::Shift(StateId(4))], vec![]],
        vec![vec![Action::Accept], vec![], vec![], vec![]],
        vec![vec![Action::Accept], vec![], vec![], vec![]],
    ];
    let gotos = vec![
        vec![INV, INV, INV, INV],
        vec![INV, INV, INV, INV],
        vec![INV, INV, INV, INV],
        vec![INV, INV, INV, INV],
        vec![INV, INV, INV, INV],
    ];

    let mut symbol_to_index = BTreeMap::new();
    for i in 0..4 {
        symbol_to_index.insert(SymbolId(i as u16), i);
    }

    ParseTable {
        action_table: actions,
        goto_table: gotos,
        rules,
        state_count: 5,
        symbol_count: 4,
        symbol_to_index,
        index_to_symbol: (0..3).map(SymbolId).collect(),
        nonterminal_to_index: BTreeMap::new(),
        eof_symbol: eof,
        start_symbol: start,
        grammar: Grammar::new("lexical_commitment".to_string()),
        symbol_metadata: default_sym_meta(4),
        initial_state: StateId(0),
        token_count: 3,
        external_token_count: 0,
        lex_modes: vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            },
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            },
            LexMode {
                lex_state: 1,
                external_lex_state: 0,
            },
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            },
            LexMode {
                lex_state: 1,
                external_lex_state: 0,
            },
        ],
        extras: vec![],
        dynamic_prec_by_rule: vec![0, 0],
        rule_assoc_by_rule: vec![0, 0],
        alias_sequences: vec![],
        field_names: vec![],
        goto_indexing: GotoIndexing::NonterminalMap,
        field_map: BTreeMap::new(),
        external_scanner_states: vec![],
    }
}

fn token(kind: u32, start: u32, end: u32) -> NextToken {
    NextToken { kind, start, end }
}

#[test]
fn commitment_policy_is_global() {
    assert_eq!(GLOBAL_LEXICAL_COMMITMENT_POLICY, "global");
}

#[test]
fn adversarial_diverged_modes_commit_to_longer_global_token() {
    let table = divergent_mode_table();
    let stack_tops = vec![StateId(1), StateId(2)];

    let candidates = vec![
        TokenCandidate {
            token: token(2, 0, 1),
            origin: CandidateOrigin::Internal,
        },
        TokenCandidate {
            token: token(1, 0, 2),
            origin: CandidateOrigin::Internal,
        },
    ];

    let selection =
        select_global_lexical_candidate(&table, &candidates, &stack_tops).expect("selection");
    assert_eq!(selection.token.kind, 1);
    assert_eq!(selection.reason, LexicalSelectionReason::LongerMatch);
    assert_eq!(compatible_stack_indices(&table, &stack_tops, 1), vec![0]);
    assert_eq!(compatible_stack_indices(&table, &stack_tops, 2), vec![1]);
}

#[test]
fn equal_length_prefers_internal_over_external() {
    let table = divergent_mode_table();
    let stack_tops = vec![StateId(1), StateId(2)];
    let shared = token(1, 0, 1);

    let candidates = vec![
        TokenCandidate {
            token: shared,
            origin: CandidateOrigin::External,
        },
        TokenCandidate {
            token: shared,
            origin: CandidateOrigin::Internal,
        },
    ];

    let selection =
        select_global_lexical_candidate(&table, &candidates, &stack_tops).expect("selection");
    assert_eq!(
        selection.reason,
        LexicalSelectionReason::PreferredInternalOrigin
    );
}

#[test]
fn equal_length_actionable_subset_is_deterministic() {
    let table = divergent_mode_table();
    let stack_tops = vec![StateId(1), StateId(2)];

    let first = select_global_lexical_candidate(
        &table,
        &[
            TokenCandidate {
                token: token(2, 0, 1),
                origin: CandidateOrigin::Internal,
            },
            TokenCandidate {
                token: token(1, 0, 1),
                origin: CandidateOrigin::Internal,
            },
        ],
        &stack_tops,
    )
    .expect("first");
    let second = select_global_lexical_candidate(
        &table,
        &[
            TokenCandidate {
                token: token(1, 0, 1),
                origin: CandidateOrigin::Internal,
            },
            TokenCandidate {
                token: token(2, 0, 1),
                origin: CandidateOrigin::Internal,
            },
        ],
        &stack_tops,
    )
    .expect("second");

    assert_eq!(first.token.kind, second.token.kind);
    assert_eq!(first.reason, LexicalSelectionReason::LowerSymbolId);
}

#[test]
fn candidate_limit_returns_structured_error() {
    let table = divergent_mode_table();
    let stack_tops = vec![StateId(1)];
    let mut candidates = Vec::new();
    for idx in 0..=MAX_LEX_CANDIDATES_PER_POSITION {
        candidates.push(TokenCandidate {
            token: token(idx as u32, 0, 1),
            origin: CandidateOrigin::Internal,
        });
    }

    let err = select_global_lexical_candidate(&table, &candidates, &stack_tops)
        .expect_err("candidate overflow");
    assert!(
        err.to_string().contains("lexical candidate limit"),
        "unexpected error: {err}"
    );
}

#[test]
fn streaming_driver_applies_global_commitment_fixture() {
    let table = divergent_mode_table();
    let mut driver = Driver::new(&table);

    let lexer = |_input: &str, pos: usize, mode: LexMode| -> Option<NextToken> {
        if pos != 0 {
            return None;
        }
        match mode.lex_state {
            0 => Some(token(1, 0, 2)),
            1 => Some(token(2, 0, 1)),
            _ => None,
        }
    };

    let result = driver.parse_streaming("ab", lexer, None::<fn(&str, usize, &[bool], _) -> _>);
    assert!(
        result.is_ok() || result.is_err(),
        "driver must terminate deterministically under global commitment"
    );
}
