#![cfg(all(feature = "pure-rust", feature = "glr", feature = "ts-compat"))]

use adze::decoder::decode_parse_table;
use adze::pure_parser::{ExternalScanner, TSLanguage, TSLexState, TSParseAction, TSRule};
use adze_glr_core::Action;
use adze_glr_core::conflict_inspection::{cell_has_conflict, count_conflicts};
use adze_ir::{RuleId, StateId, SymbolId};
use std::ptr;

static SYMBOL_NAME_END: &[u8] = b"end\0";
static SYMBOL_NAME_NUM: &[u8] = b"num\0";
static SYMBOL_NAME_START: &[u8] = b"start\0";
static FIELD_NAME_VALUE: &[u8] = b"value\0";

#[repr(transparent)]
struct SymbolNames([*const u8; 3]);
unsafe impl Sync for SymbolNames {}

static SYMBOL_NAMES: SymbolNames = SymbolNames([
    SYMBOL_NAME_END.as_ptr(),
    SYMBOL_NAME_NUM.as_ptr(),
    SYMBOL_NAME_START.as_ptr(),
]);

#[repr(transparent)]
struct FieldNames([*const u8; 1]);
unsafe impl Sync for FieldNames {}

static FIELD_NAMES: FieldNames = FieldNames([FIELD_NAME_VALUE.as_ptr()]);

static SYMBOL_METADATA: [u8; 3] = [
    0x00, // EOF
    0x03, // num: visible + named terminal
    0x02, // start: named nonterminal
];

// All states use Tree-sitter small-table rows:
// state 0: num shifts to state 1, start goto state 2
// state 1: EOF reduces production 0
// state 2: EOF accepts
static SMALL_PARSE_TABLE: [u16; 8] = [
    1, 1, // symbol 1 => Shift(1)
    2, 2, // symbol 2 => goto StateId(2)
    0, 0x8001, // EOF => Reduce(RuleId(0))
    0, 0xFFFF, // EOF => Accept
];

static SMALL_PARSE_TABLE_MAP: [u32; 4] = [0, 4, 6, 8];
static LEX_MODES: [TSLexState; 3] = [
    TSLexState {
        lex_state: 0,
        external_lex_state: 0,
    },
    TSLexState {
        lex_state: 7,
        external_lex_state: 0,
    },
    TSLexState {
        lex_state: 3,
        external_lex_state: 0,
    },
];
static PUBLIC_SYMBOL_MAP: [u16; 3] = [0, 5, 9];
static PRIMARY_STATE_IDS: [u16; 3] = [0, 1, 2];
static PRODUCTION_ID_MAP: [u16; 1] = [0];
static PRODUCTION_LHS_INDEX: [u16; 1] = [2];
static TS_RULES: [TSRule; 1] = [TSRule {
    lhs: 2,
    rhs_len: 1,
    _pad: 0,
}];
static ALIAS_MAP: [u16; 1] = [0];
static ALIAS_SEQUENCES: [u16; 1] = [1];
static FIELD_MAP_SLICES: [u16; 2] = [0, 1];
static FIELD_MAP_ENTRIES: [u16; 2] = [0, 0];
static PARSE_ACTIONS: [TSParseAction; 1] = [TSParseAction {
    action_type: 3,
    extra: 0,
    child_count: 1,
    dynamic_precedence: 0,
    symbol: 0,
}];

static EXT_SYMBOL_NAME_END: &[u8] = b"end\0";
static EXT_SYMBOL_NAME_NUM: &[u8] = b"num\0";
static EXT_SYMBOL_NAME_INDENT: &[u8] = b"indent\0";
static EXT_SYMBOL_NAME_START: &[u8] = b"start\0";

#[repr(transparent)]
struct ExternalSymbolNames([*const u8; 4]);
unsafe impl Sync for ExternalSymbolNames {}

static EXTERNAL_SYMBOL_NAMES: ExternalSymbolNames = ExternalSymbolNames([
    EXT_SYMBOL_NAME_END.as_ptr(),
    EXT_SYMBOL_NAME_NUM.as_ptr(),
    EXT_SYMBOL_NAME_INDENT.as_ptr(),
    EXT_SYMBOL_NAME_START.as_ptr(),
]);

static EXTERNAL_SYMBOL_METADATA: [u8; 4] = [
    0x00, // EOF
    0x03, // num: visible + named terminal
    0x03, // indent: visible + named external terminal
    0x02, // start: named nonterminal
];
static EXTERNAL_PUBLIC_SYMBOL_MAP: [u16; 4] = [0, 5, 8, 9];
static EXTERNAL_SMALL_PARSE_TABLE: [u16; 1] = [0];
static EXTERNAL_SMALL_PARSE_TABLE_MAP: [u32; 3] = [0, 0, 0];
static EXTERNAL_LEX_MODES: [TSLexState; 2] = [
    TSLexState {
        lex_state: 4,
        external_lex_state: 0,
    },
    TSLexState {
        lex_state: 7,
        external_lex_state: 1,
    },
];
static EXTERNAL_PRIMARY_STATE_IDS: [u16; 2] = [0, 1];
static EXTERNAL_SCANNER_STATES: [bool; 2] = [true, false];
static EXTERNAL_SCANNER_SYMBOL_MAP: [u16; 1] = [2];

// Combined fixture:
// - sparse public symbol IDs
// - one external terminal
// - non-zero lex modes and external scanner state bits
// - alias sequence data
// - field-map data
// - compressed small-table action/goto rows
static COMBINED_SMALL_PARSE_TABLE: [u16; 8] = [
    2, 1, // external indent => Shift(1)
    3, 2, // start => goto StateId(2)
    0, 0x8001, // EOF => Reduce(RuleId(0))
    0, 0xFFFF, // EOF => Accept
];
static COMBINED_SMALL_PARSE_TABLE_MAP: [u32; 4] = [0, 4, 6, 8];
static COMBINED_LEX_MODES: [TSLexState; 3] = [
    TSLexState {
        lex_state: 4,
        external_lex_state: 0,
    },
    TSLexState {
        lex_state: 7,
        external_lex_state: 1,
    },
    TSLexState {
        lex_state: 3,
        external_lex_state: 0,
    },
];
static COMBINED_PRIMARY_STATE_IDS: [u16; 3] = [0, 1, 2];
static COMBINED_PRODUCTION_ID_MAP: [u16; 1] = [0];
static COMBINED_PRODUCTION_LHS_INDEX: [u16; 1] = [3];
static COMBINED_TS_RULES: [TSRule; 1] = [TSRule {
    lhs: 3,
    rhs_len: 1,
    _pad: 0,
}];
static COMBINED_ALIAS_MAP: [u16; 1] = [0];
static COMBINED_ALIAS_SEQUENCES: [u16; 1] = [2];
static COMBINED_FIELD_MAP_SLICES: [u16; 2] = [0, 1];
static COMBINED_FIELD_MAP_ENTRIES: [u16; 2] = [0, 0];
static COMBINED_EXTERNAL_SCANNER_STATES: [bool; 3] = [true, false, true];
static COMBINED_EXTERNAL_SCANNER_SYMBOL_MAP: [u16; 1] = [2];

// Synthetic conflict fixture:
// - state 0 stores two entries for the same terminal symbol;
// - decode must preserve them as one multi-action Shift/Reduce cell;
// - conflict inspection must still see the decoded cell as GLR-relevant.
static CONFLICT_SMALL_PARSE_TABLE: [u16; 6] = [
    1, 1, // num => Shift(1)
    1, 0x8001, // num => Reduce(RuleId(0))
    0, 0xFFFF, // EOF => Accept
];
static CONFLICT_SMALL_PARSE_TABLE_MAP: [u32; 3] = [0, 4, 6];
static CONFLICT_LEX_MODES: [TSLexState; 2] = [
    TSLexState {
        lex_state: 0,
        external_lex_state: 0,
    },
    TSLexState {
        lex_state: 7,
        external_lex_state: 0,
    },
];
static CONFLICT_PRIMARY_STATE_IDS: [u16; 2] = [0, 1];

static LANGUAGE: TSLanguage = TSLanguage {
    version: 15,
    symbol_count: 3,
    alias_count: 1,
    token_count: 2,
    external_token_count: 0,
    state_count: 3,
    large_state_count: 0,
    production_id_count: 1,
    field_count: 1,
    max_alias_sequence_length: 1,
    production_id_map: PRODUCTION_ID_MAP.as_ptr(),
    parse_table: ptr::null(),
    small_parse_table: SMALL_PARSE_TABLE.as_ptr(),
    small_parse_table_map: SMALL_PARSE_TABLE_MAP.as_ptr(),
    parse_actions: PARSE_ACTIONS.as_ptr(),
    symbol_names: SYMBOL_NAMES.0.as_ptr(),
    field_names: FIELD_NAMES.0.as_ptr(),
    field_map_slices: FIELD_MAP_SLICES.as_ptr(),
    field_map_entries: FIELD_MAP_ENTRIES.as_ptr(),
    symbol_metadata: SYMBOL_METADATA.as_ptr(),
    public_symbol_map: PUBLIC_SYMBOL_MAP.as_ptr(),
    alias_map: ALIAS_MAP.as_ptr(),
    alias_sequences: ALIAS_SEQUENCES.as_ptr(),
    lex_modes: LEX_MODES.as_ptr(),
    lex_fn: None,
    keyword_lex_fn: None,
    keyword_capture_token: 0,
    external_scanner: ExternalScanner {
        states: ptr::null(),
        symbol_map: ptr::null(),
        create: None,
        destroy: None,
        scan: None,
        serialize: None,
        deserialize: None,
    },
    primary_state_ids: PRIMARY_STATE_IDS.as_ptr(),
    production_lhs_index: PRODUCTION_LHS_INDEX.as_ptr(),
    production_count: 1,
    eof_symbol: 0,
    rules: TS_RULES.as_ptr(),
    rule_count: 1,
};

static LANGUAGE_WITH_EXTERNAL: TSLanguage = TSLanguage {
    version: 15,
    symbol_count: 4,
    alias_count: 0,
    token_count: 2,
    external_token_count: 1,
    state_count: 2,
    large_state_count: 0,
    production_id_count: 0,
    field_count: 0,
    max_alias_sequence_length: 0,
    production_id_map: ptr::null(),
    parse_table: ptr::null(),
    small_parse_table: EXTERNAL_SMALL_PARSE_TABLE.as_ptr(),
    small_parse_table_map: EXTERNAL_SMALL_PARSE_TABLE_MAP.as_ptr(),
    parse_actions: PARSE_ACTIONS.as_ptr(),
    symbol_names: EXTERNAL_SYMBOL_NAMES.0.as_ptr(),
    field_names: ptr::null(),
    field_map_slices: ptr::null(),
    field_map_entries: ptr::null(),
    symbol_metadata: EXTERNAL_SYMBOL_METADATA.as_ptr(),
    public_symbol_map: EXTERNAL_PUBLIC_SYMBOL_MAP.as_ptr(),
    alias_map: ptr::null(),
    alias_sequences: ptr::null(),
    lex_modes: EXTERNAL_LEX_MODES.as_ptr(),
    lex_fn: None,
    keyword_lex_fn: None,
    keyword_capture_token: 0,
    external_scanner: ExternalScanner {
        states: EXTERNAL_SCANNER_STATES.as_ptr() as *const u8,
        symbol_map: EXTERNAL_SCANNER_SYMBOL_MAP.as_ptr(),
        create: None,
        destroy: None,
        scan: None,
        serialize: None,
        deserialize: None,
    },
    primary_state_ids: EXTERNAL_PRIMARY_STATE_IDS.as_ptr(),
    production_lhs_index: ptr::null(),
    production_count: 0,
    eof_symbol: 0,
    rules: ptr::null(),
    rule_count: 0,
};

static LANGUAGE_COMBINED_ABI: TSLanguage = TSLanguage {
    version: 15,
    symbol_count: 4,
    alias_count: 1,
    token_count: 2,
    external_token_count: 1,
    state_count: 3,
    large_state_count: 0,
    production_id_count: 1,
    field_count: 1,
    max_alias_sequence_length: 1,
    production_id_map: COMBINED_PRODUCTION_ID_MAP.as_ptr(),
    parse_table: ptr::null(),
    small_parse_table: COMBINED_SMALL_PARSE_TABLE.as_ptr(),
    small_parse_table_map: COMBINED_SMALL_PARSE_TABLE_MAP.as_ptr(),
    parse_actions: PARSE_ACTIONS.as_ptr(),
    symbol_names: EXTERNAL_SYMBOL_NAMES.0.as_ptr(),
    field_names: FIELD_NAMES.0.as_ptr(),
    field_map_slices: COMBINED_FIELD_MAP_SLICES.as_ptr(),
    field_map_entries: COMBINED_FIELD_MAP_ENTRIES.as_ptr(),
    symbol_metadata: EXTERNAL_SYMBOL_METADATA.as_ptr(),
    public_symbol_map: EXTERNAL_PUBLIC_SYMBOL_MAP.as_ptr(),
    alias_map: COMBINED_ALIAS_MAP.as_ptr(),
    alias_sequences: COMBINED_ALIAS_SEQUENCES.as_ptr(),
    lex_modes: COMBINED_LEX_MODES.as_ptr(),
    lex_fn: None,
    keyword_lex_fn: None,
    keyword_capture_token: 0,
    external_scanner: ExternalScanner {
        states: COMBINED_EXTERNAL_SCANNER_STATES.as_ptr() as *const u8,
        symbol_map: COMBINED_EXTERNAL_SCANNER_SYMBOL_MAP.as_ptr(),
        create: None,
        destroy: None,
        scan: None,
        serialize: None,
        deserialize: None,
    },
    primary_state_ids: COMBINED_PRIMARY_STATE_IDS.as_ptr(),
    production_lhs_index: COMBINED_PRODUCTION_LHS_INDEX.as_ptr(),
    production_count: 1,
    eof_symbol: 0,
    rules: COMBINED_TS_RULES.as_ptr(),
    rule_count: 1,
};

static LANGUAGE_WITH_CONFLICT_CELL: TSLanguage = TSLanguage {
    version: 15,
    symbol_count: 3,
    alias_count: 0,
    token_count: 2,
    external_token_count: 0,
    state_count: 2,
    large_state_count: 0,
    production_id_count: 1,
    field_count: 0,
    max_alias_sequence_length: 0,
    production_id_map: PRODUCTION_ID_MAP.as_ptr(),
    parse_table: ptr::null(),
    small_parse_table: CONFLICT_SMALL_PARSE_TABLE.as_ptr(),
    small_parse_table_map: CONFLICT_SMALL_PARSE_TABLE_MAP.as_ptr(),
    parse_actions: PARSE_ACTIONS.as_ptr(),
    symbol_names: SYMBOL_NAMES.0.as_ptr(),
    field_names: ptr::null(),
    field_map_slices: ptr::null(),
    field_map_entries: ptr::null(),
    symbol_metadata: SYMBOL_METADATA.as_ptr(),
    public_symbol_map: PUBLIC_SYMBOL_MAP.as_ptr(),
    alias_map: ptr::null(),
    alias_sequences: ptr::null(),
    lex_modes: CONFLICT_LEX_MODES.as_ptr(),
    lex_fn: None,
    keyword_lex_fn: None,
    keyword_capture_token: 0,
    external_scanner: ExternalScanner {
        states: ptr::null(),
        symbol_map: ptr::null(),
        create: None,
        destroy: None,
        scan: None,
        serialize: None,
        deserialize: None,
    },
    primary_state_ids: CONFLICT_PRIMARY_STATE_IDS.as_ptr(),
    production_lhs_index: PRODUCTION_LHS_INDEX.as_ptr(),
    production_count: 1,
    eof_symbol: 0,
    rules: TS_RULES.as_ptr(),
    rule_count: 1,
};

#[test]
fn compressed_tslanguage_decode_preserves_metadata_actions_and_fields() {
    let decoded = decode_parse_table(&LANGUAGE);

    assert_eq!(decoded.symbol_count, LANGUAGE.symbol_count as usize);
    assert_eq!(decoded.token_count, LANGUAGE.token_count as usize);
    assert_eq!(
        decoded.external_token_count,
        LANGUAGE.external_token_count as usize
    );
    assert_eq!(decoded.state_count, LANGUAGE.state_count as usize);
    assert_eq!(decoded.eof_symbol, SymbolId(0));
    assert_eq!(
        decoded.index_to_symbol,
        vec![SymbolId(0), SymbolId(5), SymbolId(9)]
    );
    assert_eq!(decoded.symbol_to_index.get(&SymbolId(9)), Some(&2));

    assert_eq!(decoded.goto_table[0][2], StateId(2));
    assert_eq!(decoded.action_table[0][1], vec![Action::Shift(StateId(1))]);
    assert_eq!(decoded.action_table[1][0], vec![Action::Reduce(RuleId(0))]);
    assert_eq!(decoded.action_table[2][0], vec![Action::Accept]);

    assert_eq!(decoded.rules.len(), 1);
    assert_eq!(decoded.rules[0].lhs, SymbolId(9));
    assert_eq!(decoded.rules[0].rhs_len, 1);
    assert_eq!(decoded.field_names, vec!["value".to_string()]);
    assert_eq!(decoded.field_map.get(&(RuleId(0), 0)), Some(&0));
}

#[test]
fn compressed_tslanguage_decode_preserves_multi_action_conflict_cell() {
    let decoded = decode_parse_table(&LANGUAGE_WITH_CONFLICT_CELL);

    assert_eq!(decoded.state_count, 2);
    assert_eq!(
        decoded.action_table[0][1],
        vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))],
        "duplicate small-table entries for the same symbol should decode as one multi-action cell"
    );
    assert!(
        cell_has_conflict(&decoded.action_table[0][1]),
        "decoded multi-action cell should be visible to GLR conflict inspection"
    );

    let summary = count_conflicts(&decoded);
    assert_eq!(summary.shift_reduce, 1);
    assert_eq!(summary.reduce_reduce, 0);
    assert_eq!(summary.states_with_conflicts, vec![StateId(0)]);
}

#[test]
fn compressed_tslanguage_decode_preserves_public_symbol_map() {
    let decoded = decode_parse_table(&LANGUAGE);

    assert_eq!(
        decoded.index_to_symbol,
        vec![SymbolId(0), SymbolId(5), SymbolId(9)]
    );
    assert_eq!(decoded.symbol_to_index.get(&SymbolId(0)), Some(&0));
    assert_eq!(decoded.symbol_to_index.get(&SymbolId(5)), Some(&1));
    assert_eq!(decoded.symbol_to_index.get(&SymbolId(9)), Some(&2));
    assert_eq!(decoded.symbol_metadata[1].symbol_id, SymbolId(5));
    assert_eq!(decoded.symbol_metadata[2].symbol_id, SymbolId(9));
    assert_eq!(decoded.nonterminal_to_index.get(&SymbolId(9)), Some(&2));
}

#[test]
fn compressed_tslanguage_decode_preserves_alias_sequences() {
    let decoded = decode_parse_table(&LANGUAGE);

    assert_eq!(decoded.alias_sequences, vec![vec![Some(SymbolId(5))]]);
}

#[test]
fn compressed_tslanguage_decode_preserves_lex_modes() {
    let decoded = decode_parse_table(&LANGUAGE);

    assert_eq!(decoded.lex_modes.len(), 3);
    assert_eq!(decoded.lex_modes[0].lex_state, 0);
    assert_eq!(decoded.lex_modes[1].lex_state, 7);
    assert_eq!(decoded.lex_modes[2].lex_state, 3);
    assert_eq!(decoded.lex_modes[1].external_lex_state, 0);
}

#[test]
fn compressed_tslanguage_decode_preserves_external_token_metadata() {
    let decoded = decode_parse_table(&LANGUAGE_WITH_EXTERNAL);

    assert_eq!(decoded.token_count, 2);
    assert_eq!(decoded.external_token_count, 1);
    assert_eq!(
        decoded.index_to_symbol,
        vec![SymbolId(0), SymbolId(5), SymbolId(8), SymbolId(9)]
    );
    assert_eq!(decoded.symbol_metadata[2].name, "indent");
    assert_eq!(decoded.symbol_metadata[2].symbol_id, SymbolId(8));
    assert!(decoded.symbol_metadata[2].is_terminal);
    assert_eq!(
        decoded.external_scanner_states,
        vec![vec![true], vec![false]]
    );
    assert_eq!(decoded.grammar.externals.len(), 1);
    assert_eq!(decoded.grammar.externals[0].name, "indent");
    assert_eq!(decoded.grammar.externals[0].symbol_id, SymbolId(8));
    assert_eq!(decoded.lex_modes.len(), 2);
    assert_eq!(decoded.lex_modes[0].lex_state, 4);
    assert_eq!(decoded.lex_modes[1].lex_state, 7);
    assert_eq!(decoded.lex_modes[1].external_lex_state, 1);
}

#[test]
fn combined_tslanguage_decode_preserves_metadata_fields_aliases_externals_and_lex_modes() {
    let decoded = decode_parse_table(&LANGUAGE_COMBINED_ABI);

    assert_eq!(decoded.symbol_count, 4);
    assert_eq!(decoded.token_count, 2);
    assert_eq!(decoded.external_token_count, 1);
    assert_eq!(decoded.state_count, 3);
    assert_eq!(
        decoded.index_to_symbol,
        vec![SymbolId(0), SymbolId(5), SymbolId(8), SymbolId(9)]
    );
    assert_eq!(decoded.symbol_to_index.get(&SymbolId(8)), Some(&2));
    assert_eq!(decoded.nonterminal_to_index.get(&SymbolId(9)), Some(&3));

    assert_eq!(decoded.action_table[0][2], vec![Action::Shift(StateId(1))]);
    assert_eq!(decoded.goto_table[0][3], StateId(2));
    assert_eq!(decoded.action_table[1][0], vec![Action::Reduce(RuleId(0))]);
    assert_eq!(decoded.action_table[2][0], vec![Action::Accept]);

    assert_eq!(decoded.rules.len(), 1);
    assert_eq!(decoded.rules[0].lhs, SymbolId(9));
    assert_eq!(decoded.rules[0].rhs_len, 1);
    assert_eq!(decoded.field_names, vec!["value".to_string()]);
    assert_eq!(decoded.field_map.get(&(RuleId(0), 0)), Some(&0));
    assert_eq!(decoded.alias_sequences, vec![vec![Some(SymbolId(8))]]);

    assert_eq!(decoded.symbol_metadata[2].name, "indent");
    assert_eq!(decoded.symbol_metadata[2].symbol_id, SymbolId(8));
    assert!(decoded.symbol_metadata[2].is_terminal);
    assert_eq!(decoded.grammar.externals.len(), 1);
    assert_eq!(decoded.grammar.externals[0].name, "indent");
    assert_eq!(decoded.grammar.externals[0].symbol_id, SymbolId(8));
    assert_eq!(
        decoded.external_scanner_states,
        vec![vec![true], vec![false], vec![true]]
    );

    assert_eq!(decoded.lex_modes.len(), 3);
    assert_eq!(decoded.lex_modes[0].lex_state, 4);
    assert_eq!(decoded.lex_modes[1].lex_state, 7);
    assert_eq!(decoded.lex_modes[1].external_lex_state, 1);
    assert_eq!(decoded.lex_modes[2].lex_state, 3);
}
