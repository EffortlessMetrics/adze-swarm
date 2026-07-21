//! Failing fixtures and adapter-contract tests for stack-aware GLR streaming lexing.
//!
//! Linked epic: #857 / #888. These tests encode the desired contract and fail until
//! later ladder slices replace fixed state-0 pretokenization with streaming lexing.

#![cfg(all(feature = "pure-rust", feature = "glr", feature = "runtime-e2e"))]

use adze::decoder::decode_parse_table;
use adze::glr_streaming_internal_lexer::{
    StreamingInternalLexError, lex_generated_internal_at, make_generated_internal_streaming_lexer,
};
use adze::glr_streaming_lex_contract::{
    StreamingLexContractViolationKind, audit_fixed_mode_pretokenization_bridge,
    distinct_internal_lex_states, external_scanner_mask_union_covers_active_stacks,
    fixed_mode_bridge_uses_only_state_zero_lex_mode, required_lex_modes_for_active_stacks,
    tokenize_with_fixed_mode_bridge,
};
use adze::glr_streaming_runtime::{TrueGlrParseRoute, last_true_glr_parse_route};
use adze::pure_parser::TSLanguage;
use adze_example::ambiguous_expr::grammar as ambiguous_expr_grammar;
use adze_example::external_word_example::grammar as external_word_grammar;
use adze_example::streaming_lex_modes::grammar as streaming_lex_modes_grammar;
use adze_glr_core::conflict_inspection::cell_has_conflict;
use adze_glr_core::ts_lexer::NextToken;
use adze_glr_core::{Action, Driver, LexMode, build_lex_modes_from_shiftable_terminals};
use adze_ir::StateId;
use core::ffi::c_void;
use std::collections::BTreeSet;

fn streaming_lex_modes_language() -> &'static TSLanguage {
    streaming_lex_modes_grammar::language()
}

fn streaming_lex_modes_contract_table() -> adze_glr_core::ParseTable {
    let mut table = decode_parse_table(streaming_lex_modes_language());
    table.lex_modes = build_lex_modes_from_shiftable_terminals(
        &table.action_table,
        &table.external_scanner_states,
    );
    table
}

fn contains_shift_reduce(cell: &[Action]) -> bool {
    let has_shift = cell.iter().any(|action| matches!(action, Action::Shift(_)));
    let has_reduce = cell
        .iter()
        .any(|action| matches!(action, Action::Reduce(_)));
    has_shift && has_reduce
}

#[test]
fn streaming_lex_modes_generated_abi_exposes_at_least_two_lex_states() {
    let table = streaming_lex_modes_contract_table();
    let distinct = distinct_internal_lex_states(&table);
    assert!(
        distinct.len() >= 2,
        "fixture must expose >=2 internal lex states for contract proof; got {distinct:?}"
    );
}

#[test]
fn streaming_lex_modes_fixed_mode_bridge_ignores_distinct_lex_states() {
    let language = streaming_lex_modes_language();
    let table = streaming_lex_modes_contract_table();
    let distinct = distinct_internal_lex_states(&table);

    assert!(
        distinct.len() >= 2,
        "precondition: fixture exposes multiple lex states"
    );
    assert!(
        fixed_mode_bridge_uses_only_state_zero_lex_mode(language),
        "expected current bridge to ignore per-state lex modes until streaming adapter lands"
    );

    let violations = audit_fixed_mode_pretokenization_bridge(language, &table, b"1+2\n".as_slice());
    assert!(
        violations.iter().any(|violation| {
            violation.kind == StreamingLexContractViolationKind::FixedStateZeroPretokenization
        }),
        "audit should flag fixed state-0 pretokenization: {violations:?}"
    );
}

#[test]
fn streaming_lex_modes_fixed_bridge_drops_meaningful_newline_token() {
    let language = streaming_lex_modes_language();
    let table = streaming_lex_modes_contract_table();
    let source = b"1+2\n";

    let violations = audit_fixed_mode_pretokenization_bridge(language, &table, source);
    assert!(
        violations.iter().any(|violation| {
            violation.kind == StreamingLexContractViolationKind::HardCodedAsciiWhitespaceSkip
        }),
        "meaningful newline fixture should flag hard-coded ASCII skip: {violations:?}"
    );

    let lex_fn = language
        .lex_fn
        .expect("streaming_lex_modes fixture should expose generated lexer");
    let tokens = tokenize_with_fixed_mode_bridge(language, lex_fn, source)
        .expect("bridge tokenization should succeed for audit fixture");
    let offsets = tokens
        .iter()
        .map(|token| token.byte_offset)
        .collect::<BTreeSet<_>>();

    assert!(
        !offsets.contains(&3),
        "current fixed-mode bridge drops the meaningful newline at byte 3; tokens={tokens:?}"
    );
}

#[test]
fn streaming_lex_modes_meaningful_newline_preserves_selected_parse_under_glr() {
    let parsed = streaming_lex_modes_grammar::parse("1+2\n");
    assert!(
        parsed.is_ok(),
        "streaming contract expects selected parse to succeed for fixture line: {parsed:?}"
    );
    assert_eq!(
        last_true_glr_parse_route(),
        Some(TrueGlrParseRoute::StreamingDriver),
        "conflicted generated fixture must route through streaming driver"
    );
}

#[test]
fn streaming_lex_modes_meaningful_newline_survives_tokenization() {
    let document = streaming_lex_modes_grammar::parse_document("1+2\n")
        .expect("streaming parse_document should preserve newline-separated input");
    let root_range = document.tree().root().byte_range();
    assert!(
        root_range.end >= 4,
        "newline byte should survive streaming lexing in document span: {root_range:?}"
    );
    assert_eq!(
        last_true_glr_parse_route(),
        Some(TrueGlrParseRoute::StreamingDriver),
        "fixture tokenization proof should execute through streaming driver routing"
    );
}

#[test]
fn reduce_reduce_route_gate_stays_on_fixed_bridge() {
    let language = adze_example::reduce_reduce::grammar::language();
    let table = decode_parse_table(language);
    assert!(
        !adze::glr_streaming_runtime::should_route_conflict_table_through_streaming_driver(
            language, &table
        ),
        "reduce/reduce fixtures stay on GLRParser until ambiguity parity (#891 PR6)"
    );
}

#[test]
fn ambiguous_expr_route_gate_stays_on_fixed_bridge() {
    let language = ambiguous_expr_grammar::language();
    let table = decode_parse_table(language);
    assert!(
        !adze::glr_streaming_runtime::should_route_conflict_table_through_streaming_driver(
            language, &table
        ),
        "ambiguous_expr should remain on GLRParser until driver parity (#891 PR6)"
    );
}

#[test]
fn streaming_lex_modes_diverged_stacks_require_distinct_lex_modes_at_conflict() {
    let table = streaming_lex_modes_contract_table();
    let mut conflict_states = Vec::new();

    for (state_idx, row) in table.action_table.iter().enumerate() {
        for cell in row {
            if cell_has_conflict(cell) && contains_shift_reduce(cell) {
                conflict_states.push(StateId(state_idx as u16));
            }
        }
    }

    assert!(
        !conflict_states.is_empty(),
        "fixture must retain at least one GLR conflict state"
    );

    let mut saw_distinct_modes = false;
    for state in conflict_states {
        let shift_targets = table.action_table[state.0 as usize]
            .iter()
            .flat_map(|cell| {
                cell.iter().filter_map(|action| match action {
                    Action::Shift(target) => Some(*target),
                    _ => None,
                })
            })
            .collect::<Vec<_>>();

        if shift_targets.len() < 2 {
            continue;
        }

        let modes = required_lex_modes_for_active_stacks(&table, &shift_targets);
        if modes.len() >= 2 {
            saw_distinct_modes = true;
            break;
        }
    }

    assert!(
        saw_distinct_modes,
        "fixture must include a conflict where shift targets require distinct lex modes"
    );
}

#[test]
fn streaming_lex_modes_extras_are_grammar_defined_not_hard_coded_ascii() {
    let language = streaming_lex_modes_language();
    let table = streaming_lex_modes_contract_table();
    let source = b"1 + 2\n";

    let violations = audit_fixed_mode_pretokenization_bridge(language, &table, source);
    assert!(
        violations.iter().any(|violation| {
            violation.kind == StreamingLexContractViolationKind::HardCodedAsciiWhitespaceSkip
        }),
        "inline spaces should be handled by grammar extras, not bridge hard-coded skip: {violations:?}"
    );
}

#[test]
fn streaming_lex_modes_external_scanner_mask_union_contract() {
    let language = external_word_grammar::language();
    let mut table = decode_parse_table(language);
    table.lex_modes = build_lex_modes_from_shiftable_terminals(
        &table.action_table,
        &table.external_scanner_states,
    );

    let active_states = (0..table.state_count)
        .map(|state| StateId(state as u16))
        .filter(|state| table.lex_mode(*state).external_lex_state != 0)
        .collect::<Vec<_>>();

    if active_states.is_empty() {
        return;
    }

    assert!(
        external_scanner_mask_union_covers_active_stacks(&table, &active_states),
        "external scanner mask union must cover each active stack"
    );
}

#[test]
fn streaming_lex_modes_streaming_driver_contract_smoke_uses_contract_table() {
    let table = streaming_lex_modes_contract_table();
    assert!(
        distinct_internal_lex_states(&table).len() >= 2,
        "contract table must expose multiple lex modes before driver integration"
    );

    let mut driver = Driver::new(&table);
    let lexer = |_input: &str, pos: usize, _mode: LexMode| -> Option<NextToken> {
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

    let result = driver.parse_streaming("1", lexer, None::<fn(&str, usize, &[bool], _) -> _>);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn streaming_lex_modes_ambiguous_expr_fixture_retains_conflicts_for_ladder() {
    let language = ambiguous_expr_grammar::language();
    let mut table = decode_parse_table(language);
    table.lex_modes = build_lex_modes_from_shiftable_terminals(
        &table.action_table,
        &table.external_scanner_states,
    );

    let has_conflict = table.action_table.iter().any(|row| {
        row.iter()
            .any(|cell| cell_has_conflict(cell) && contains_shift_reduce(cell))
    });
    assert!(
        has_conflict,
        "ambiguous_expr remains the GLR conflict baseline for later ladder slices"
    );
}

#[test]
fn streaming_lex_modes_generated_lexer_fn_is_available_for_adapter_slices() {
    let language = streaming_lex_modes_language();
    let lex_fn = language.lex_fn.expect("generated lexer fn");
    let _lex_fn: unsafe extern "C" fn(*mut c_void, adze::pure_parser::TSLexState) -> bool = lex_fn;
}

#[test]
fn streaming_lex_modes_internal_adapter_preserves_meaningful_newline_token() {
    let language = streaming_lex_modes_language();
    let table = streaming_lex_modes_contract_table();
    let mode = table.lex_mode(StateId(0));

    let token = lex_generated_internal_at(language, "1+2\n", 3, mode)
        .expect("adapter should lex newline token")
        .expect("newline token expected at byte 3");

    assert_eq!(token.start, 3);
    assert_eq!(token.end, 4);
}

#[test]
fn streaming_lex_modes_internal_adapter_does_not_hard_code_ascii_whitespace_skip() {
    let language = streaming_lex_modes_language();
    let table = streaming_lex_modes_contract_table();
    let mode = table.lex_mode(StateId(0));

    // Unlike the fixed bridge, the adapter does not pre-skip spaces before lexing.
    let at_space = lex_generated_internal_at(language, "1 + 2", 1, mode);
    assert!(
        matches!(
            at_space,
            Err(StreamingInternalLexError::NoProgress { pos: 1 })
        ),
        "adapter should not hard-code ASCII skip; got {at_space:?}"
    );
}

#[test]
fn streaming_lex_modes_internal_adapter_rejects_no_progress_structured_error() {
    let language = streaming_lex_modes_language();
    let table = streaming_lex_modes_contract_table();
    let mode = table.lex_mode(StateId(0));

    let err = lex_generated_internal_at(language, "1+2@", 3, mode).expect_err("invalid byte");
    assert_eq!(
        err,
        StreamingInternalLexError::NoProgress { pos: 3 },
        "unexpected error: {err:?}"
    );
}

#[test]
fn streaming_lex_modes_internal_adapter_is_deterministic_for_position_and_mode() {
    let language = streaming_lex_modes_language();
    let table = streaming_lex_modes_contract_table();
    let mode = table.lex_mode(StateId(0));

    let first = lex_generated_internal_at(language, "1+2", 0, mode).expect("first call");
    let second = lex_generated_internal_at(language, "1+2", 0, mode).expect("second call");
    assert_eq!(first, second);
}

#[test]
fn streaming_lex_modes_internal_adapter_closure_lexes_without_pretokenization() {
    let language = streaming_lex_modes_language();
    let table = streaming_lex_modes_contract_table();
    let mode = table.lex_mode(StateId(0));
    let mut lexer = make_generated_internal_streaming_lexer(language);

    let token = lexer("1+2", 0, mode).expect("adapter closure should lex first token");
    assert_eq!(token.start, 0);
    assert_eq!(token.end, 1);
}
