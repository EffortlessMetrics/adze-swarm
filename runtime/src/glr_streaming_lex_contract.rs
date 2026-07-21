//! Contract surface for stack-aware GLR streaming lexing (#857).
//!
//! PR1 defines the adapter obligations and audit helpers used by failing fixtures.
//! Runtime integration lands in later ladder slices.

#![cfg(all(feature = "glr", feature = "pure-rust"))]

use adze_glr_core::{LexMode, ParseTable};
use adze_ir::StateId;
use core::ffi::c_void;
use std::collections::BTreeSet;

use crate::pure_parser::{TSLanguage, TSLexState};

/// A single contract obligation violated by the fixed state-0 pretokenization bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamingLexContractViolation {
    pub kind: StreamingLexContractViolationKind,
    pub detail: String,
}

/// Categories of streaming-lexer contract failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingLexContractViolationKind {
    FixedStateZeroPretokenization,
    HardCodedAsciiWhitespaceSkip,
    IgnoredDistinctLexModes,
    MissedGrammarExtra,
    ExternalScannerMaskNotUnioned,
}

/// Distinct internal lex states present in a generated parse table.
pub fn distinct_internal_lex_states(parse_table: &ParseTable) -> BTreeSet<u16> {
    parse_table
        .lex_modes
        .iter()
        .map(|mode| mode.lex_state)
        .collect()
}

/// Lex modes required by the supplied active parser stacks.
pub fn required_lex_modes_for_active_stacks(
    parse_table: &ParseTable,
    stack_states: &[StateId],
) -> Vec<LexMode> {
    let mut modes = Vec::new();
    for state in stack_states {
        let mode = parse_table.lex_mode(*state);
        if !modes.contains(&mode) {
            modes.push(mode);
        }
    }
    modes
}

/// Whether the fixed-mode bridge always selects parser-state-0 lex mode.
pub fn fixed_mode_bridge_uses_only_state_zero_lex_mode(language: &TSLanguage) -> bool {
    if language.lex_modes.is_null() || language.state_count <= 1 {
        return true;
    }

    // SAFETY: generated languages expose one lex mode per parser state.
    let first = unsafe { *language.lex_modes };
    (1..language.state_count).all(|state| {
        // SAFETY: `state < state_count` and lex_modes has one entry per state.
        let mode = unsafe { *language.lex_modes.add(state as usize) };
        mode.lex_state == first.lex_state && mode.external_lex_state == first.external_lex_state
    })
}

/// Whether the fixed-mode bridge skips ASCII whitespace before invoking the lexer.
pub fn fixed_mode_bridge_skips_ascii_whitespace() -> bool {
    true
}

/// Audit the current pretokenization bridge against the streaming contract.
pub fn audit_fixed_mode_pretokenization_bridge(
    language: &TSLanguage,
    parse_table: &ParseTable,
    source: &[u8],
) -> Vec<StreamingLexContractViolation> {
    let mut violations = Vec::new();

    let distinct = distinct_internal_lex_states(parse_table);
    if distinct.len() >= 2 && fixed_mode_bridge_uses_only_state_zero_lex_mode(language) {
        violations.push(StreamingLexContractViolation {
            kind: StreamingLexContractViolationKind::FixedStateZeroPretokenization,
            detail: format!(
                "parse table exposes {} distinct internal lex states but the bridge always uses state-0 mode",
                distinct.len()
            ),
        });
    }

    if fixed_mode_bridge_skips_ascii_whitespace()
        && source
            .iter()
            .any(|byte| matches!(*byte, b' ' | b'\t' | b'\n' | b'\r'))
    {
        violations.push(StreamingLexContractViolation {
            kind: StreamingLexContractViolationKind::HardCodedAsciiWhitespaceSkip,
            detail: "bridge unconditionally skips ASCII whitespace before generated lexing"
                .to_string(),
        });
    }

    violations
}

/// Union external-scanner validity masks across active stacks.
pub fn union_external_scanner_mask(parse_table: &ParseTable, stack_states: &[StateId]) -> u16 {
    stack_states.iter().fold(0u16, |mask, state| {
        mask | parse_table.lex_mode(*state).external_lex_state
    })
}

/// Whether every active stack's external mask is reflected in the union mask.
pub fn external_scanner_mask_union_covers_active_stacks(
    parse_table: &ParseTable,
    stack_states: &[StateId],
) -> bool {
    let union = union_external_scanner_mask(parse_table, stack_states);
    stack_states.iter().all(|state| {
        let mode_mask = parse_table.lex_mode(*state).external_lex_state;
        mode_mask == 0 || (union & mode_mask) == mode_mask
    })
}

/// Lex mode selected by the current fixed-mode pretokenization bridge.
pub fn fixed_mode_bridge_lex_mode(language: &TSLanguage) -> TSLexState {
    if language.lex_modes.is_null() {
        return TSLexState {
            lex_state: 0,
            external_lex_state: 0,
        };
    }

    // SAFETY: generated languages provide one lex mode per parser state. The bridge
    // currently always uses parser state 0.
    unsafe { *language.lex_modes }
}

/// Tokenize with the current fixed-mode pretokenization bridge.
pub fn tokenize_with_fixed_mode_bridge(
    language: &'static TSLanguage,
    lex_fn: unsafe extern "C" fn(*mut c_void, TSLexState) -> bool,
    source: &[u8],
) -> Result<Vec<crate::glr_lexer::TokenWithPosition>, Vec<crate::errors::ParseError>> {
    crate::__private::lex_with_language_fn(language, lex_fn, source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_glr_core::{Action, ParseRule};
    use adze_ir::{Grammar, RuleId, SymbolId};

    fn tiny_table(lex_modes: Vec<LexMode>) -> ParseTable {
        let mut table = ParseTable::new(Grammar::new("contract".to_string()));
        table.state_count = lex_modes.len();
        table.lex_modes = lex_modes;
        table.action_table = vec![
            vec![vec![], vec![Action::Shift(StateId(1))], vec![]],
            vec![vec![Action::Reduce(RuleId(0))], vec![], vec![]],
            vec![vec![Action::Accept], vec![], vec![]],
        ];
        table.goto_table = vec![
            vec![StateId(u16::MAX); 3],
            vec![StateId(u16::MAX); 3],
            vec![StateId(u16::MAX); 3],
        ];
        table.rules = vec![ParseRule {
            lhs: SymbolId(2),
            rhs_len: 1,
        }];
        table
    }

    #[test]
    fn required_lex_modes_deduplicates_active_stacks() {
        let table = tiny_table(vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            },
            LexMode {
                lex_state: 1,
                external_lex_state: 0,
            },
            LexMode {
                lex_state: 1,
                external_lex_state: 0,
            },
        ]);

        let modes =
            required_lex_modes_for_active_stacks(&table, &[StateId(0), StateId(1), StateId(2)]);
        assert_eq!(modes.len(), 2);
        assert_eq!(modes[0].lex_state, 0);
        assert_eq!(modes[1].lex_state, 1);
    }

    #[test]
    fn external_scanner_mask_union_covers_each_active_stack() {
        let table = tiny_table(vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0b001,
            },
            LexMode {
                lex_state: 1,
                external_lex_state: 0b010,
            },
        ]);

        assert!(external_scanner_mask_union_covers_active_stacks(
            &table,
            &[StateId(0), StateId(1)],
        ));
        assert_eq!(
            union_external_scanner_mask(&table, &[StateId(0), StateId(1)]),
            0b011
        );
    }
}
