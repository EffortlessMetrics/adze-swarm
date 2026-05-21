use adze_ir::{ProductionId, Rule, RuleId, Symbol, SymbolId};
use indexmap::IndexMap;

use crate::pure_parser::TSLanguage;

use super::{decode_rules, symbol_for_index};

pub(super) fn decode_fallback_rules(
    lang: &TSLanguage,
    rules: &mut IndexMap<SymbolId, Vec<Rule>>,
) -> IndexMap<RuleId, ProductionId> {
    let mut production_ids = IndexMap::new();

    if lang.rules.is_null() {
        decode_rules_from_parsed(lang, rules, &mut production_ids);
    } else {
        decode_production_ids_from_rule_count(lang, &mut production_ids);
    }

    production_ids
}

fn decode_rules_from_parsed(
    lang: &TSLanguage,
    rules: &mut IndexMap<SymbolId, Vec<Rule>>,
    production_ids: &mut IndexMap<RuleId, ProductionId>,
) {
    let parsed_rules = decode_rules(lang);
    let has_alias_data = has_alias_data(lang);

    for (i, pr) in parsed_rules.into_iter().enumerate() {
        let rhs = decode_rhs_for_rule(lang, i, pr.rhs_len as usize, has_alias_data);
        let production_id = ProductionId(i as u16);

        rules.entry(pr.lhs).or_default().push(Rule {
            lhs: pr.lhs,
            rhs,
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id,
        });
        production_ids.insert(RuleId(i as u16), production_id);
    }
}

fn decode_production_ids_from_rule_count(
    lang: &TSLanguage,
    production_ids: &mut IndexMap<RuleId, ProductionId>,
) {
    for i in 0..lang.rule_count as usize {
        production_ids.insert(RuleId(i as u16), ProductionId(i as u16));
    }
}

fn decode_rhs_for_rule(
    lang: &TSLanguage,
    rule_index: usize,
    rhs_len: usize,
    has_alias_data: bool,
) -> Vec<Symbol> {
    if has_alias_data && rhs_len <= 1000 {
        decode_fallback_rhs(lang, rule_index, rhs_len).unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn has_alias_data(lang: &TSLanguage) -> bool {
    !lang.alias_map.is_null() && !lang.alias_sequences.is_null()
}

fn decode_fallback_rhs(
    lang: &TSLanguage,
    rule_index: usize,
    rhs_len: usize,
) -> Option<Vec<Symbol>> {
    let alias_map_size = (lang.production_count as usize).max(rule_index + 1);
    if alias_map_size == 0 {
        return Some(Vec::new());
    }

    // SAFETY: `lang.alias_map` is non-null (caller checks alias data).
    // `alias_map_size` is bounded to at least `rule_index + 1` elements, which
    // is the minimum needed for indexing below.
    let alias_map_slice = unsafe { std::slice::from_raw_parts(lang.alias_map, alias_map_size) };
    let offset = *alias_map_slice.get(rule_index)? as usize;
    let total_sequences_needed = offset.saturating_add(rhs_len);
    if total_sequences_needed > usize::MAX / 2 {
        return Some(Vec::new());
    }

    Some(read_rhs_symbols(
        lang,
        offset,
        rhs_len,
        total_sequences_needed,
    ))
}

fn read_rhs_symbols(
    lang: &TSLanguage,
    offset: usize,
    rhs_len: usize,
    sequence_len: usize,
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
        }
    }

    rhs
}
