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

        let rhs = decode_rule_rhs(lang, i, rhs_len, rule_count).unwrap_or_else(|| {
            (0..rhs_len)
                .map(|_| Symbol::NonTerminal(SymbolId(0)))
                .collect()
        });
        let precedence = decode_dynamic_precedence(lang, i);
        let fields = fields::decode_rule_fields(lang, i);

        rules.entry(lhs).or_default().push(Rule {
            lhs,
            rhs,
            precedence,
            associativity: None,
            fields,
            production_id: ProductionId(i as u16),
        });
    }
}

fn decode_rule_rhs(
    lang: &TSLanguage,
    rule_index: usize,
    rhs_len: usize,
    rule_count: usize,
) -> Option<Vec<Symbol>> {
    if lang.alias_map.is_null() || lang.alias_sequences.is_null() {
        return Some(placeholder_rhs(rhs_len));
    }

    // SAFETY: `lang.alias_map` is non-null (branch guard above). `rule_count`
    // matches the alias_map array length per TSLanguage contract.
    let alias_map_slice = unsafe { std::slice::from_raw_parts(lang.alias_map, rule_count) };
    let offset = *alias_map_slice.get(rule_index)? as usize;
    let max_sequences_needed = offset.saturating_add(rhs_len);
    if max_sequences_needed > usize::MAX / 2 {
        return Some(placeholder_rhs(rhs_len));
    }

    Some(read_rhs_symbols(
        lang,
        offset,
        rhs_len,
        max_sequences_needed,
        true,
    ))
}

fn read_rhs_symbols(
    lang: &TSLanguage,
    offset: usize,
    rhs_len: usize,
    sequence_len: usize,
    fill_missing_with_placeholder: bool,
) -> Vec<Symbol> {
    // TODO(safety): `sequence_len` is derived from alias_map data which may not
    // reflect the true allocation size of `lang.alias_sequences`.
    let alias_sequences_slice =
        unsafe { std::slice::from_raw_parts(lang.alias_sequences, sequence_len) };
    let mut rhs = Vec::with_capacity(rhs_len);

    for j in 0..rhs_len {
        let seq_idx = offset + j;
        if let Some(&sym_idx) = alias_sequences_slice.get(seq_idx) {
            rhs.push(symbol_for_index(lang, sym_idx));
        } else if fill_missing_with_placeholder {
            rhs.push(Symbol::NonTerminal(SymbolId(0)));
        }
    }

    rhs
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
