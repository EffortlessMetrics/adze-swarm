use adze_ir::{ProductionId, Rule, RuleId, Symbol, SymbolId};
use indexmap::IndexMap;

use crate::pure_parser::TSLanguage;

use super::{decode_rule_rhs, decode_rules};

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
        if let Some(production_id) = lang.production_for_rule(i) {
            production_ids.insert(RuleId(i as u16), ProductionId(production_id));
        }
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
    // Use the same advertised allocation and per-row bounds as native
    // metadata decoding; a fallback cannot grow raw slices from offsets.
    decode_rule_rhs(lang, rule_index, rhs_len, lang.production_count as usize)
}
