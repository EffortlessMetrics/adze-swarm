//! Parse table decoding from Tree-sitter language ABI data.
//!
//! This submodule keeps parse-table reconstruction separate from grammar,
//! field, production, symbol, and external-token decoding.

use adze_glr_core::{Action, LexMode, ParseRule, ParseTable, SymbolMetadata};
use adze_ir::{RuleId, StateId, SymbolId};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::{CStr, c_char};

use crate::pure_parser::{TSLanguage, TSParseAction};
use crate::ts_format::TSActionTag;

use super::decode_grammar;

fn decode_alias_sequences(
    lang: &TSLanguage,
    index_to_symbol: &[SymbolId],
) -> Vec<Vec<Option<SymbolId>>> {
    let production_count = if lang.production_id_count > 0 {
        lang.production_id_count as usize
    } else if lang.production_count > 0 {
        lang.production_count as usize
    } else {
        lang.rule_count as usize
    };
    let stride = lang.max_alias_sequence_length as usize;

    if lang.alias_count == 0
        || production_count == 0
        || stride == 0
        || lang.alias_map.is_null()
        || lang.alias_sequences.is_null()
    {
        return Vec::new();
    }

    let safe_production_count = production_count.min(100_000);
    let Some(alias_cell_count) = safe_production_count.checked_mul(stride) else {
        return Vec::new();
    };
    if alias_cell_count > 10_000_000 {
        return Vec::new();
    }

    // SAFETY: non-null pointers are checked above, and the TSLanguage ABI stores one
    // alias-map entry per production plus a dense alias sequence table.
    let alias_map = unsafe { std::slice::from_raw_parts(lang.alias_map, safe_production_count) };
    // SAFETY: see above; `alias_cell_count` is checked for overflow and capped.
    let alias_cells = unsafe { std::slice::from_raw_parts(lang.alias_sequences, alias_cell_count) };

    alias_map
        .iter()
        .map(|offset| {
            let offset = *offset as usize;
            (0..stride)
                .map(|position| {
                    let raw_symbol = offset
                        .checked_add(position)
                        .and_then(|index| alias_cells.get(index))
                        .copied()
                        .unwrap_or(0);
                    if raw_symbol == 0 {
                        None
                    } else {
                        Some(
                            index_to_symbol
                                .get(raw_symbol as usize)
                                .copied()
                                .unwrap_or(SymbolId(raw_symbol)),
                        )
                    }
                })
                .collect()
        })
        .collect()
}

/// Decode a raw legacy ParseTable.
///
/// A native ABI language has no authoritative start field. This compatibility
/// entrypoint retains historical inference; checked generated entrypoints use
/// `NativeLanguageIdentityV1` and never invoke that inference.
pub fn decode_parse_table(lang: &'static TSLanguage) -> ParseTable {
    decode_parse_table_inner(lang, None)
}

pub(crate) fn decode_parse_table_with_start(
    lang: &'static TSLanguage,
    start: SymbolId,
) -> ParseTable {
    decode_parse_table_inner(lang, Some(start))
}

fn decode_parse_table_inner(
    lang: &'static TSLanguage,
    explicit_start: Option<SymbolId>,
) -> ParseTable {
    let symbol_count = lang.symbol_count as usize;
    let tcols = terminal_column_count(lang);
    let (index_to_symbol, symbol_to_index) = decode_symbol_maps(lang, symbol_count);
    let mut grammar = decode_grammar(lang);
    let rules = decode_parse_rules(lang, &grammar, &index_to_symbol);
    let rid_by_pair = build_reduce_rule_map(&rules);
    let mut goto_table = empty_goto_table(lang, symbol_count);
    let mut extras_set = BTreeSet::new();

    let symbol_metadata = decode_symbol_metadata(lang, &index_to_symbol, &mut extras_set);
    let mut action_table = decode_large_states(
        lang,
        symbol_count,
        tcols,
        &rules,
        &rid_by_pair,
        &index_to_symbol,
        &mut goto_table,
        &mut extras_set,
    );
    decode_small_states(
        lang,
        symbol_count,
        tcols,
        rules.len(),
        &mut goto_table,
        &mut action_table,
    );

    let external_scanner_states = decode_external_scanner_states(lang);
    let nonterminal_to_index = build_nonterminal_to_index(&index_to_symbol, tcols);
    let eof_symbol = decode_eof_symbol(lang, &index_to_symbol);
    let extras: Vec<SymbolId> = extras_set.into_iter().collect();
    let field_map = build_field_map(&grammar);
    let lex_modes = decode_lex_modes(lang);
    let field_names: Vec<String> = grammar.fields.values().cloned().collect();
    grammar.extras = extras.clone();
    let alias_sequences = decode_alias_sequences(lang, &index_to_symbol);
    let start_symbol = explicit_start.unwrap_or_else(|| select_start_symbol(lang, tcols, &rules));
    grammar.set_start_symbol(start_symbol);

    let mut table = ParseTable {
        action_table,
        goto_table,
        symbol_metadata,
        state_count: lang.state_count as usize,
        symbol_count: lang.symbol_count as usize,
        symbol_to_index,
        index_to_symbol,
        external_scanner_states,
        nonterminal_to_index,
        goto_indexing: adze_glr_core::GotoIndexing::NonterminalMap,
        eof_symbol,
        start_symbol,
        rules,
        grammar,
        initial_state: StateId(0),
        token_count: lang.token_count as usize,
        external_token_count: lang.external_token_count as usize,
        lex_modes,
        extras: extras.clone(),
        dynamic_prec_by_rule: Vec::new(),
        rule_assoc_by_rule: Vec::new(),
        alias_sequences,
        field_names,
        field_map,
    };

    table.detect_goto_indexing();
    table
}

fn terminal_column_count(lang: &TSLanguage) -> usize {
    (lang.token_count + lang.external_token_count) as usize
}

fn empty_goto_table(lang: &TSLanguage, symbol_count: usize) -> Vec<Vec<StateId>> {
    vec![vec![StateId(0); symbol_count]; lang.state_count as usize]
}

pub(super) fn decode_symbol_maps(
    lang: &TSLanguage,
    symbol_count: usize,
) -> (Vec<SymbolId>, BTreeMap<SymbolId, usize>) {
    let mut index_to_symbol = dense_index_to_symbol(symbol_count);

    if !lang.public_symbol_map.is_null() {
        let mut saw_symbol = BTreeSet::new();
        let mut public_map_ok = true;
        for (col, slot) in index_to_symbol.iter_mut().enumerate() {
            let public_sym = unsafe { *lang.public_symbol_map.add(col) };
            if !saw_symbol.insert(SymbolId(public_sym)) {
                public_map_ok = false;
                break;
            }
            *slot = SymbolId(public_sym);
        }

        if !public_map_ok {
            index_to_symbol = dense_index_to_symbol(symbol_count);
        }
    }

    let mut symbol_to_index = invert_symbol_map(&index_to_symbol);
    if symbol_to_index.len() != symbol_count {
        index_to_symbol = dense_index_to_symbol(symbol_count);
        symbol_to_index = invert_symbol_map(&index_to_symbol);
    }

    (index_to_symbol, symbol_to_index)
}

fn dense_index_to_symbol(symbol_count: usize) -> Vec<SymbolId> {
    (0..symbol_count).map(|col| SymbolId(col as u16)).collect()
}

fn invert_symbol_map(index_to_symbol: &[SymbolId]) -> BTreeMap<SymbolId, usize> {
    index_to_symbol
        .iter()
        .copied()
        .enumerate()
        .map(|(col, sym)| (sym, col))
        .collect()
}

fn decode_parse_rules(
    lang: &TSLanguage,
    grammar: &adze_ir::Grammar,
    index_to_symbol: &[SymbolId],
) -> Vec<ParseRule> {
    // Syntax metadata is already dense in RuleId order. ProductionId is a
    // field/alias slot and must never select or overwrite a syntax rule.
    let mut rules = if !lang.rules.is_null() {
        // SAFETY: the native ABI advertises rule_count TSRule records.
        unsafe { std::slice::from_raw_parts(lang.rules, lang.rule_count as usize) }
            .iter()
            .map(|rule| ParseRule {
                lhs: SymbolId(rule.lhs),
                rhs_len: rule.rhs_len as u16,
            })
            .collect::<Vec<_>>()
    } else {
        grammar
            .all_rules()
            .map(|rule| ParseRule {
                lhs: rule.lhs,
                rhs_len: rule.rhs.len() as u16,
            })
            .collect::<Vec<_>>()
    };

    // Native TS_RULES are columns; the grammar fallback is already public.
    if !lang.rules.is_null() {
        for rule in &mut rules {
            if (rule.lhs.0 as usize) < index_to_symbol.len() {
                rule.lhs = index_to_symbol[rule.lhs.0 as usize];
            }
        }
    }

    rules
}

fn build_reduce_rule_map(rules: &[ParseRule]) -> HashMap<(u16, u8), u16> {
    let mut rid_by_pair = HashMap::with_capacity(rules.len());
    for (i, r) in rules.iter().enumerate() {
        rid_by_pair.insert((r.lhs.0, r.rhs_len as u8), i as u16);
    }
    rid_by_pair
}

fn decode_symbol_metadata(
    lang: &TSLanguage,
    index_to_symbol: &[SymbolId],
    extras_set: &mut BTreeSet<SymbolId>,
) -> Vec<SymbolMetadata> {
    index_to_symbol
        .iter()
        .copied()
        .enumerate()
        .map(|(i, sym)| {
            let (ts_metadata, name) = decode_symbol_metadata_cell(lang, i);
            if (ts_metadata & 0x04) != 0 {
                extras_set.insert(sym);
            }

            SymbolMetadata {
                name,
                is_visible: (ts_metadata & 0x01) != 0,
                is_named: (ts_metadata & 0x02) != 0,
                is_supertype: (ts_metadata & 0x08) != 0,
                is_terminal: (i as u32) < lang.token_count + lang.external_token_count,
                is_extra: (ts_metadata & 0x04) != 0,
                is_fragile: false,
                symbol_id: sym,
            }
        })
        .collect()
}

fn decode_symbol_metadata_cell(lang: &TSLanguage, index: usize) -> (u8, String) {
    // SAFETY: Pointer arithmetic is valid because callers pass indexes from the
    // symbol table bounds. Both pointers are null-checked before dereferencing.
    unsafe {
        let ts_metadata = if !lang.symbol_metadata.is_null() {
            *lang.symbol_metadata.add(index)
        } else {
            0
        };
        let name_ptr = if !lang.symbol_names.is_null() {
            *lang.symbol_names.add(index)
        } else {
            std::ptr::null()
        };
        let name = if name_ptr.is_null() {
            format!("symbol_{}", index)
        } else {
            CStr::from_ptr(name_ptr as *const c_char)
                .to_string_lossy()
                .into_owned()
        };
        (ts_metadata, name)
    }
}

#[allow(clippy::too_many_arguments)]
fn decode_large_states(
    lang: &TSLanguage,
    symbol_count: usize,
    tcols: usize,
    rules: &[ParseRule],
    rid_by_pair: &HashMap<(u16, u8), u16>,
    index_to_symbol: &[SymbolId],
    goto_table: &mut [Vec<StateId>],
    extras_set: &mut BTreeSet<SymbolId>,
) -> Vec<Vec<Vec<Action>>> {
    let mut action_table = Vec::new();

    for state in 0..lang.large_state_count as usize {
        let mut state_actions = Vec::new();
        for symbol in 0..symbol_count {
            let table_offset = state * lang.symbol_count as usize + symbol;
            // SAFETY: `table_offset` is within the large-state parse table shape.
            let table_value = unsafe { *lang.parse_table.add(table_offset) };
            state_actions.push(decode_large_state_cell(
                lang,
                symbol,
                table_value,
                tcols,
                rules,
                rid_by_pair,
                index_to_symbol,
                &mut goto_table[state],
                extras_set,
            ));
        }
        action_table.push(state_actions);
    }

    action_table
}

#[allow(clippy::too_many_arguments)]
fn decode_large_state_cell(
    lang: &TSLanguage,
    symbol: usize,
    table_value: u16,
    tcols: usize,
    rules: &[ParseRule],
    rid_by_pair: &HashMap<(u16, u8), u16>,
    index_to_symbol: &[SymbolId],
    state_gotos: &mut [StateId],
    extras_set: &mut BTreeSet<SymbolId>,
) -> Vec<Action> {
    if symbol >= tcols {
        if table_value != 0 {
            state_gotos[symbol] = StateId(table_value);
        }
        return Vec::new();
    }

    if table_value == 0 {
        return Vec::new();
    }

    // SAFETY: `table_value` is read from trusted TSLanguage action-table data.
    let action = unsafe {
        let raw = &*lang.parse_actions.add(table_value as usize);
        if raw.extra != 0
            && raw.action_type == TSActionTag::Shift as u8
            && let Some(&sym) = index_to_symbol.get(symbol)
        {
            extras_set.insert(sym);
        }
        decode_action(raw, rules, rid_by_pair)
    };

    if matches!(action, Action::Error) {
        Vec::new()
    } else {
        vec![action]
    }
}

fn decode_small_states(
    lang: &TSLanguage,
    symbol_count: usize,
    tcols: usize,
    rules_len: usize,
    goto_table: &mut [Vec<StateId>],
    action_table: &mut Vec<Vec<Vec<Action>>>,
) {
    if lang.small_parse_table_map.is_null() || lang.small_parse_table.is_null() {
        return;
    }

    for state in lang.large_state_count as usize..lang.state_count as usize {
        let mut state_actions = vec![vec![]; lang.symbol_count as usize];
        let map_index = state - lang.large_state_count as usize;
        // SAFETY: map indexes cover all small states and include a sentinel offset.
        let start_offset = unsafe { *lang.small_parse_table_map.add(map_index) } as usize;
        let end_offset = unsafe { *lang.small_parse_table_map.add(map_index + 1) } as usize;
        decode_small_state_pairs(
            lang,
            state,
            start_offset,
            end_offset,
            symbol_count,
            tcols,
            rules_len,
            goto_table,
            &mut state_actions,
        );
        action_table.push(state_actions);
    }
}

#[allow(clippy::too_many_arguments)]
fn decode_small_state_pairs(
    lang: &TSLanguage,
    state: usize,
    start_offset: usize,
    end_offset: usize,
    symbol_count: usize,
    tcols: usize,
    rules_len: usize,
    goto_table: &mut [Vec<StateId>],
    state_actions: &mut [Vec<Action>],
) {
    let mut offset = start_offset;
    while offset + 1 < end_offset {
        // SAFETY: offsets are bounded by the trusted small-table map range.
        let symbol = unsafe { *lang.small_parse_table.add(offset) } as usize;
        let action_index = unsafe { *lang.small_parse_table.add(offset + 1) } as usize;
        offset += 2;

        if symbol >= symbol_count || action_index == 0 {
            continue;
        }

        if symbol >= tcols {
            goto_table[state][symbol] = StateId(action_index as u16);
        } else if let Some(action) = decode_small_action(lang, action_index, rules_len) {
            state_actions[symbol].push(action);
        }
    }
}

fn decode_small_action(lang: &TSLanguage, action_index: usize, rules_len: usize) -> Option<Action> {
    let action = if action_index == 0xFFFF {
        Action::Accept
    } else if action_index & 0x8000 != 0 {
        let encoded_rule_id = (action_index & 0x7FFF).checked_sub(1)?;
        // The production map holds one entry per rule (`rule_count`), so the
        // access is bounded by the map length, not `production_id_count`:
        // sparse tables (`production_id_count > rule_count`) admit no ids here.
        if encoded_rule_id >= rules_len || encoded_rule_id >= lang.rule_count as usize {
            return None;
        }
        // Validate the relation, but retain syntax identity in reductions.
        lang.production_for_rule(encoded_rule_id)?;
        Action::Reduce(RuleId(encoded_rule_id as u16))
    } else {
        Action::Shift(StateId(action_index as u16))
    };

    (!matches!(action, Action::Error)).then_some(action)
}

fn decode_external_scanner_states(lang: &TSLanguage) -> Vec<Vec<bool>> {
    if lang.external_token_count == 0 || lang.external_scanner.states.is_null() {
        return vec![vec![]; lang.state_count as usize];
    }

    let mut states = Vec::with_capacity(lang.state_count as usize);
    let external_count = lang.external_token_count as usize;

    // SAFETY: external scanner states are a flat state_count * external_count bool table.
    unsafe {
        let states_ptr = lang.external_scanner.states as *const bool;
        for state_idx in 0..lang.state_count as usize {
            let mut state_externals = Vec::with_capacity(external_count);
            for external_idx in 0..external_count {
                let idx = state_idx * external_count + external_idx;
                state_externals.push(*states_ptr.add(idx));
            }
            states.push(state_externals);
        }
    }

    states
}

fn build_nonterminal_to_index(
    index_to_symbol: &[SymbolId],
    tcols: usize,
) -> BTreeMap<SymbolId, usize> {
    let mut nonterminal_to_index = BTreeMap::new();
    for (col, sym) in index_to_symbol.iter().enumerate() {
        if col >= tcols {
            nonterminal_to_index.insert(*sym, col);
        }
    }
    nonterminal_to_index
}

fn decode_eof_symbol(lang: &TSLanguage, index_to_symbol: &[SymbolId]) -> SymbolId {
    index_to_symbol
        .get(lang.eof_symbol as usize)
        .copied()
        .unwrap_or(SymbolId(0))
}

fn build_field_map(grammar: &adze_ir::Grammar) -> BTreeMap<(RuleId, u16), u16> {
    let mut field_map = BTreeMap::new();
    for (rule_index, rule) in grammar.all_rules().enumerate() {
        for (fid, pos) in &rule.fields {
            field_map.insert((RuleId(rule_index as u16), *pos as u16), fid.0);
        }
    }
    field_map
}

fn decode_lex_modes(lang: &TSLanguage) -> Vec<LexMode> {
    if lang.lex_modes.is_null() || lang.state_count == 0 {
        return vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            };
            lang.state_count as usize
        ];
    }

    let state_count = lang.state_count as usize;
    // SAFETY: `lang.lex_modes` is non-null and contains one entry per state.
    let lex_modes_slice = unsafe { std::slice::from_raw_parts(lang.lex_modes, state_count) };
    lex_modes_slice
        .iter()
        .map(|&m| LexMode {
            lex_state: m.lex_state,
            external_lex_state: m.external_lex_state,
        })
        .collect()
}

fn select_start_symbol(lang: &TSLanguage, tcols: usize, rules: &[ParseRule]) -> SymbolId {
    let is_nt = |sym: SymbolId| sym.0 as usize >= tcols;
    let lhs_symbols: BTreeSet<SymbolId> = rules.iter().map(|r| r.lhs).collect();
    let nt_symbols: Vec<_> = lhs_symbols.into_iter().filter(|s| is_nt(*s)).collect();
    let start = if nt_symbols.is_empty() {
        SymbolId((tcols + 1) as u16)
    } else if let Some(source_file) = nt_symbols
        .iter()
        .filter(|s| symbol_name(lang, **s).as_deref() == Some("source_file"))
        .min_by_key(|s| s.0)
        .copied()
    {
        // Generated grammars always name the augmented start rule
        // `source_file`; it is the canonical document root regardless of
        // how the remaining nonterminals are ordered.
        source_file
    } else {
        let fallback = nt_symbols
            .first()
            .copied()
            .unwrap_or(SymbolId((tcols + 1) as u16));
        let meaningful = nt_symbols
            .iter()
            .filter(|s| is_meaningful_start_symbol(lang, **s))
            .min_by_key(|s| s.0)
            .copied();

        meaningful.unwrap_or_else(|| {
            nt_symbols
                .iter()
                .max_by_key(|s| s.0)
                .copied()
                .unwrap_or(fallback)
        })
    };

    debug_assert_ne!(start, SymbolId(0), "start_symbol cannot be ERROR(0)");
    start
}

/// Resolve a rule LHS to its generated name. Decoded rule LHS values are
/// table column indices, and `symbol_names` is indexed by column, so the
/// lookup is direct. (Document roots use public symbol ids instead; the
/// two spaces must not be mixed.)
fn symbol_name(lang: &TSLanguage, symbol: SymbolId) -> Option<String> {
    let column = symbol.0 as usize;
    if column >= lang.symbol_count as usize {
        return None;
    }

    let name_ptr = unsafe { lang.symbol_names.add(column).as_ref() }?;

    // SAFETY: `*name_ptr` is a pointer to a null-terminated C string per TSLanguage contract.
    let name = unsafe { CStr::from_ptr(*name_ptr as *const c_char) };
    name.to_str().ok().map(ToOwned::to_owned)
}

fn is_meaningful_start_symbol(lang: &TSLanguage, symbol: SymbolId) -> bool {
    symbol_name(lang, symbol)
        .map(|name| !name.contains("repeat") && !name.starts_with('_'))
        .unwrap_or(true)
}

/// Decode a TSParseAction into our Action enum
pub(super) fn decode_action(
    action: &TSParseAction,
    rules: &[ParseRule],
    rid_by_pair: &HashMap<(u16, u8), u16>,
) -> Action {
    // Based on Tree-sitter's encoding, action_type determines the action
    // The TSParseAction struct contains different data depending on action type

    // Tree-sitter action types using shared constants
    match action.action_type {
        x if x == TSActionTag::Shift as u8 => {
            // Shift action: move to a new state
            // The symbol field contains the state to shift to
            // extra field indicates if this is an "extra" token (whitespace, etc.)
            Action::Shift(StateId(action.symbol))
        }
        x if x == TSActionTag::Reduce as u8 => {
            // Normalize Reduce action to proper rule index
            let direct = action.symbol as usize;

            // Fast path: symbol already a valid rule index and matches child_count
            let rid: u16 =
                if direct < rules.len() && (rules[direct].rhs_len as u8) == action.child_count {
                    // Using rule ID directly from symbol field
                    action.symbol
                } else {
                    // Fallback: legacy TS encoding (symbol = LHS, child_count = rhs_len)
                    // This happens when symbol is the LHS column index
                    let key = (action.symbol, action.child_count);
                    match rid_by_pair.get(&key) {
                        Some(&rid) => rid,
                        None => {
                            debug_assert!(
                                false,
                                "Reduce mapping failed: no rule for (lhs={}, rhs_len={})",
                                action.symbol, action.child_count
                            );
                            // In release, use a distinct sentinel past rules.len()
                            // so later bounds checks catch it deterministically.
                            u16::MAX
                        }
                    }
                };

            // Short-circuit invalid rule IDs
            if rid == u16::MAX || (rid as usize) >= rules.len() {
                Action::Error // Invalid reduce rule
            } else {
                Action::Reduce(RuleId(rid))
            }
        }
        x if x == TSActionTag::Accept as u8 => {
            // Accept action: parsing complete
            Action::Accept
        }
        x if x == TSActionTag::Recover as u8 => {
            // Recover action: error recovery
            Action::Recover
        }
        x if x == TSActionTag::Error as u8 => {
            // Error action
            Action::Error
        }
        _ => {
            // Unknown action type // Expected: V for Recover
            Action::Error
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pure_parser::{ExternalScanner, TREE_SITTER_LANGUAGE_VERSION, TSRule};
    use adze_ir::ProductionId;

    /// Sparse-production fixture: FEWER rules than production slots
    /// (`production_id_count = 5 > rule_count = 2`). The map stays
    /// rule-count sized (RuleId -> ProductionId); slot VALUES may exceed
    /// the rule count. Mirrors the tablegen truncation-direction pin
    /// (#1060): encode sizes by rule count, so decode must bound by it too.
    static SPARSE_PRODUCTION_ID_MAP: [u16; 2] = [4, 0];
    static SPARSE_RULES: [TSRule; 2] = [
        TSRule {
            lhs: 2,
            rhs_len: 1,
            _pad: 0,
        },
        TSRule {
            lhs: 2,
            rhs_len: 1,
            _pad: 0,
        },
    ];
    static SPARSE_SMALL_TABLE: [u16; 2] = [1, 0x8001];
    static SPARSE_SMALL_TABLE_MAP: [u32; 2] = [0, 2];
    static SPARSE_NAME_ERROR: &[u8] = b"end\0";
    static SPARSE_NAME_TOKEN: &[u8] = b"token\0";
    static SPARSE_NAME_NODE: &[u8] = b"node\0";

    fn sparse_language() -> &'static TSLanguage {
        let symbol_names = Box::leak(Box::new([
            SPARSE_NAME_ERROR.as_ptr(),
            SPARSE_NAME_TOKEN.as_ptr(),
            SPARSE_NAME_NODE.as_ptr(),
        ]));
        Box::leak(Box::new(TSLanguage {
            version: TREE_SITTER_LANGUAGE_VERSION,
            symbol_count: 3,
            alias_count: 0,
            token_count: 2,
            external_token_count: 0,
            state_count: 1,
            large_state_count: 0,
            production_id_count: 5,
            field_count: 0,
            max_alias_sequence_length: 0,
            production_id_map: SPARSE_PRODUCTION_ID_MAP.as_ptr(),
            parse_table: std::ptr::null(),
            small_parse_table: SPARSE_SMALL_TABLE.as_ptr(),
            small_parse_table_map: SPARSE_SMALL_TABLE_MAP.as_ptr(),
            parse_actions: std::ptr::null(),
            symbol_names: symbol_names.as_ptr(),
            field_names: std::ptr::null(),
            field_map_slices: std::ptr::null(),
            field_map_entries: std::ptr::null(),
            symbol_metadata: std::ptr::null(),
            public_symbol_map: std::ptr::null(),
            alias_map: std::ptr::null(),
            alias_sequences: std::ptr::null(),
            lex_modes: std::ptr::null(),
            lex_fn: None,
            keyword_lex_fn: None,
            keyword_capture_token: 0,
            external_scanner: ExternalScanner::default(),
            primary_state_ids: std::ptr::null(),
            production_lhs_index: std::ptr::null(),
            production_count: 5,
            eof_symbol: 0,
            rules: SPARSE_RULES.as_ptr(),
            rule_count: 2,
        }))
    }

    #[test]
    fn test_sparse_production_table_decodes_with_syntax_rule_identity() {
        let table = decode_parse_table(sparse_language());

        assert_eq!(table.rules.len(), 2);
        assert_eq!(
            table.grammar.production_ids.get(&RuleId(0)),
            Some(&ProductionId(4))
        );
        assert_eq!(
            table.grammar.production_ids.get(&RuleId(1)),
            Some(&ProductionId(0))
        );
        // Slot 4 names metadata, never syntax: the reduce keeps RuleId(0).
        match table.action_table[0][1][0] {
            Action::Reduce(RuleId(rule_id)) => assert_eq!(rule_id, 0),
            ref action => panic!("expected mapped reduce action, got {action:?}"),
        }
    }

    #[test]
    fn test_sparse_production_bound_rejects_ids_at_or_above_rule_count() {
        let lang = sparse_language();
        let reduce = |rule_id: u16| decode_small_action(lang, 0x8000 | (rule_id as usize + 1), 2);

        // Valid rule ids decode with syntax identity, however sparse the slots.
        assert!(matches!(reduce(0), Some(Action::Reduce(RuleId(0)))));
        assert!(matches!(reduce(1), Some(Action::Reduce(RuleId(1)))));
        // Rule ids inside `production_id_count` but at/above `rule_count` are
        // rejected: the old production-count bound admitted them and read
        // past the rule-count-sized map.
        assert_eq!(reduce(2), None);
        assert_eq!(reduce(4), None);
        assert_eq!(reduce(5), None);
        // Underflowing encode (rule "minus one") and Accept arm are unaffected.
        assert_eq!(decode_small_action(lang, 0x8000, 2), None);
        assert_eq!(decode_small_action(lang, 0xFFFF, 2), Some(Action::Accept));

        // Executable form of the map-sizing contract: the old
        // `id < production_id_count` bound and the current
        // `id < rule_count` bound agree on every valid rule id and differ
        // exactly on the sparse window `[rule_count, production_id_count)`.
        let (rule_count, production_id_count) =
            (lang.rule_count as usize, lang.production_id_count as usize);
        assert!(
            rule_count < production_id_count,
            "fixture must be sparse: production_id_count > rule_count"
        );
        for id in 0..=production_id_count {
            let old_admits = id < production_id_count;
            let new_admits = id < rule_count;
            if id < rule_count {
                assert!(
                    old_admits && new_admits,
                    "valid rule id {id} admitted by both"
                );
            } else if id < production_id_count {
                assert!(
                    old_admits && !new_admits,
                    "sparse-window id {id}: old bound admits, rule-count bound rejects"
                );
            } else {
                assert!(!old_admits && !new_admits, "id {id} beyond both bounds");
            }
        }
    }
}
