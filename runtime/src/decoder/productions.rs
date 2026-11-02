use adze_ir::{PrecedenceKind, ProductionId, Rule, RuleId, Symbol, SymbolId};
use indexmap::IndexMap;

use crate::pure_parser::TSLanguage;

use super::decode_rules;
use super::fields;

mod fallback;

pub(super) fn decode_fallback_rules(
    lang: &TSLanguage,
    rules: &mut IndexMap<SymbolId, Vec<Rule>>,
) -> IndexMap<RuleId, ProductionId> {
    fallback::decode_fallback_rules(lang, rules)
}

pub(super) fn decode_metadata_rules(lang: &TSLanguage, rules: &mut IndexMap<SymbolId, Vec<Rule>>) {
    if lang.rules.is_null() || lang.rule_count == 0 {
        return;
    }

    let rule_count = lang.rule_count as usize;
    // SAFETY: `lang.rules` is non-null and `rule_count > 0` (branch guard).
    // TSLanguage contract guarantees the array has `rule_count` elements.
    let rules_slice = unsafe { std::slice::from_raw_parts(lang.rules, rule_count) };

    for (i, &ts_rule) in rules_slice.iter().enumerate() {
        let lhs = SymbolId(ts_rule.lhs);
        let rhs_len = ts_rule.rhs_len as usize;
        if rhs_len > 10_000 {
            continue;
        }

        let Some(production_id) = lang.production_for_rule(i) else {
            continue;
        };
        let rhs = decode_rule_rhs(
            lang,
            production_id as usize,
            rhs_len,
            lang.production_id_count as usize,
        )
        .unwrap_or_else(|| placeholder_rhs(rhs_len));
        let precedence = decode_dynamic_precedence(lang, production_id as usize);
        let fields = fields::decode_rule_fields(lang, production_id as usize);

        rules.entry(lhs).or_default().push(Rule {
            lhs,
            rhs,
            precedence,
            associativity: None,
            fields,
            production_id: ProductionId(production_id),
        });
    }
}

fn decode_rule_rhs(
    lang: &TSLanguage,
    production_index: usize,
    rhs_len: usize,
    production_count: usize,
) -> Option<Vec<Symbol>> {
    if lang.alias_map.is_null() || lang.alias_sequences.is_null() {
        return Some(placeholder_rhs(rhs_len));
    }

    let stride = lang.max_alias_sequence_length as usize;
    let sequence_len = production_count.checked_mul(stride)?;
    // The native generator allocates exactly count * stride cells. Neither
    // an alias offset nor an unrelated syntax RHS length can enlarge it.
    if production_count == 0
        || production_count > 100_000
        || stride == 0
        || sequence_len > 10_000_000
        || production_index >= production_count
    {
        return None;
    }
    // SAFETY: the non-null alias map has production_count entries under the
    // native ABI contract, independently of the number of syntax rules.
    let alias_map_slice = unsafe { std::slice::from_raw_parts(lang.alias_map, production_count) };
    let offset = *alias_map_slice.get(production_index)? as usize;
    let row_end = offset.checked_add(stride)?;
    // Offsets name row starts. Zero is the valid first row, not a missing
    // alias sentinel; absent aliases are zero-filled cells in that row.
    if !offset.is_multiple_of(stride) || row_end > sequence_len {
        return None;
    }
    // SAFETY: the native ABI advertises a dense count * stride allocation.
    // Both factors and the selected complete row have been checked above.
    let cells = unsafe { std::slice::from_raw_parts(lang.alias_sequences, sequence_len) };
    let row = cells.get(offset..row_end)?;
    Some(
        (0..rhs_len)
            .map(|position| {
                row.get(position)
                    .map(|&symbol| symbol_for_index(lang, symbol))
                    .unwrap_or(Symbol::NonTerminal(SymbolId(0)))
            })
            .collect(),
    )
}

fn symbol_for_index(lang: &TSLanguage, sym_idx: u16) -> Symbol {
    let sym_id = SymbolId(sym_idx);
    if (sym_idx as u32) < lang.token_count + lang.external_token_count {
        Symbol::Terminal(sym_id)
    } else {
        Symbol::NonTerminal(sym_id)
    }
}

fn placeholder_rhs(rhs_len: usize) -> Vec<Symbol> {
    (0..rhs_len)
        .map(|_| Symbol::NonTerminal(SymbolId(0)))
        .collect()
}

fn decode_dynamic_precedence(lang: &TSLanguage, rule_index: usize) -> Option<PrecedenceKind> {
    if lang.parse_actions.is_null() || (rule_index as u32) >= lang.production_id_count {
        return None;
    }

    let production_count = lang.production_id_count as usize;
    if rule_index >= production_count {
        return None;
    }

    // SAFETY: `lang.parse_actions` is non-null (branch guard) and
    // `production_count` is derived from `lang.production_id_count`.
    // TSLanguage contract guarantees the array has at least this many entries.
    let parse_actions_slice =
        unsafe { std::slice::from_raw_parts(lang.parse_actions, production_count) };
    let action = parse_actions_slice[rule_index];
    (action.dynamic_precedence != 0)
        .then_some(PrecedenceKind::Dynamic(action.dynamic_precedence as i16))
}
