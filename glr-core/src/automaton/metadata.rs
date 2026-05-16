use crate::{Action, SymbolMetadata};
use adze_ir::*;
use std::collections::BTreeMap;

pub(super) fn build_symbol_metadata(grammar: &Grammar) -> Vec<SymbolMetadata> {
    let mut symbol_metadata = Vec::new();

    for (symbol_id, token) in &grammar.tokens {
        symbol_metadata.push(SymbolMetadata {
            name: token.name.clone(),
            is_visible: !token.name.starts_with('_'),
            is_named: !matches!(&token.pattern, TokenPattern::String(_)),
            is_supertype: false,
            is_terminal: true,
            is_extra: grammar.extras.contains(symbol_id),
            is_fragile: false,
            symbol_id: *symbol_id,
        });
    }

    for symbol_id in grammar.rules.keys() {
        let is_supertype = grammar.supertypes.contains(symbol_id);
        symbol_metadata.push(SymbolMetadata {
            name: format!("rule_{}", symbol_id.0),
            is_visible: true,
            is_named: true,
            is_supertype,
            is_terminal: false,
            is_extra: false,
            is_fragile: false,
            symbol_id: *symbol_id,
        });
    }

    for external in &grammar.externals {
        symbol_metadata.push(SymbolMetadata {
            name: external.name.clone(),
            is_visible: !external.name.starts_with('_'),
            is_named: true,
            is_supertype: false,
            is_terminal: true,
            is_extra: false,
            is_fragile: false,
            symbol_id: external.symbol_id,
        });
    }

    symbol_metadata
}

pub(super) fn build_external_scanner_states(
    grammar: &Grammar,
    state_count: usize,
    symbol_to_index: &BTreeMap<SymbolId, usize>,
    action_table: &[Vec<Vec<Action>>],
) -> Vec<Vec<bool>> {
    let mut external_scanner_states = vec![vec![false; grammar.externals.len()]; state_count];

    for state_idx in 0..state_count {
        for (external_idx, external) in grammar.externals.iter().enumerate() {
            if let Some(&symbol_idx) = symbol_to_index.get(&external.symbol_id)
                && action_table[state_idx][symbol_idx]
                    .iter()
                    .any(|a| matches!(a, Action::Shift(_)))
            {
                external_scanner_states[state_idx][external_idx] = true;
            }
        }
    }

    external_scanner_states
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(name: &str, pattern: TokenPattern) -> Token {
        Token {
            name: name.to_string(),
            pattern,
            fragile: false,
        }
    }

    fn rule(lhs: SymbolId, production_id: ProductionId) -> Rule {
        Rule {
            lhs,
            rhs: vec![],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id,
        }
    }

    #[test]
    fn build_symbol_metadata_empty_grammar_returns_empty_vec() {
        let grammar = Grammar::new("empty".to_string());
        let meta = build_symbol_metadata(&grammar);
        assert!(meta.is_empty());
    }

    #[test]
    fn build_symbol_metadata_token_visible_when_no_underscore_prefix() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.tokens.insert(
            SymbolId(1),
            token("ident", TokenPattern::Regex("[a-z]+".to_string())),
        );

        let meta = build_symbol_metadata(&grammar);
        assert_eq!(meta.len(), 1);
        let entry = &meta[0];
        assert_eq!(entry.name, "ident");
        assert!(entry.is_visible);
        assert!(entry.is_named); // Regex pattern => is_named = true
        assert!(entry.is_terminal);
        assert!(!entry.is_supertype);
        assert!(!entry.is_extra);
        assert!(!entry.is_fragile);
        assert_eq!(entry.symbol_id, SymbolId(1));
    }

    #[test]
    fn build_symbol_metadata_underscore_token_is_hidden() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.tokens.insert(
            SymbolId(2),
            token("_internal", TokenPattern::Regex("x".to_string())),
        );

        let meta = build_symbol_metadata(&grammar);
        assert_eq!(meta.len(), 1);
        assert!(!meta[0].is_visible);
    }

    #[test]
    fn build_symbol_metadata_string_token_is_anonymous() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.tokens.insert(
            SymbolId(3),
            token("plus", TokenPattern::String("+".to_string())),
        );

        let meta = build_symbol_metadata(&grammar);
        // TokenPattern::String => is_named = false (anonymous literal).
        assert!(!meta[0].is_named);
        assert!(meta[0].is_terminal);
    }

    #[test]
    fn build_symbol_metadata_marks_extras() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.tokens.insert(
            SymbolId(4),
            token("ws", TokenPattern::Regex(r"\s+".to_string())),
        );
        grammar.extras.push(SymbolId(4));

        let meta = build_symbol_metadata(&grammar);
        assert!(meta[0].is_extra);
    }

    #[test]
    fn build_symbol_metadata_rule_entry_has_synthetic_name() {
        let mut grammar = Grammar::new("g".to_string());
        grammar
            .rules
            .insert(SymbolId(7), vec![rule(SymbolId(7), ProductionId(0))]);

        let meta = build_symbol_metadata(&grammar);
        assert_eq!(meta.len(), 1);
        let entry = &meta[0];
        assert_eq!(entry.name, "rule_7");
        assert!(entry.is_visible);
        assert!(entry.is_named);
        assert!(!entry.is_terminal);
        assert!(!entry.is_supertype);
        assert_eq!(entry.symbol_id, SymbolId(7));
    }

    #[test]
    fn build_symbol_metadata_marks_supertype_rules() {
        let mut grammar = Grammar::new("g".to_string());
        grammar
            .rules
            .insert(SymbolId(8), vec![rule(SymbolId(8), ProductionId(0))]);
        grammar.supertypes.push(SymbolId(8));

        let meta = build_symbol_metadata(&grammar);
        assert!(meta[0].is_supertype);
        assert!(!meta[0].is_terminal);
    }

    #[test]
    fn build_symbol_metadata_external_visible_when_no_underscore() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.externals.push(ExternalToken {
            name: "newline".to_string(),
            symbol_id: SymbolId(9),
        });

        let meta = build_symbol_metadata(&grammar);
        assert_eq!(meta.len(), 1);
        let entry = &meta[0];
        assert_eq!(entry.name, "newline");
        assert!(entry.is_visible);
        assert!(entry.is_named);
        assert!(entry.is_terminal);
        assert_eq!(entry.symbol_id, SymbolId(9));
    }

    #[test]
    fn build_symbol_metadata_external_hidden_when_underscore_prefixed() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.externals.push(ExternalToken {
            name: "_indent".to_string(),
            symbol_id: SymbolId(10),
        });

        let meta = build_symbol_metadata(&grammar);
        assert!(!meta[0].is_visible);
    }

    #[test]
    fn build_symbol_metadata_orders_tokens_rules_externals() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.tokens.insert(
            SymbolId(1),
            token("t", TokenPattern::Regex("a".to_string())),
        );
        grammar
            .rules
            .insert(SymbolId(2), vec![rule(SymbolId(2), ProductionId(0))]);
        grammar.externals.push(ExternalToken {
            name: "e".to_string(),
            symbol_id: SymbolId(3),
        });

        let meta = build_symbol_metadata(&grammar);
        assert_eq!(meta.len(), 3);
        // Tokens first, then rules, then externals.
        assert_eq!(meta[0].name, "t");
        assert!(meta[0].is_terminal);
        assert_eq!(meta[1].name, "rule_2");
        assert!(!meta[1].is_terminal);
        assert_eq!(meta[2].name, "e");
        assert!(meta[2].is_terminal);
    }

    #[test]
    fn build_external_scanner_states_zero_states_is_empty() {
        let grammar = Grammar::new("g".to_string());
        let symbol_to_index = BTreeMap::new();
        let result = build_external_scanner_states(&grammar, 0, &symbol_to_index, &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn build_external_scanner_states_no_externals_yields_empty_per_state() {
        let grammar = Grammar::new("g".to_string());
        let symbol_to_index = BTreeMap::new();
        let action_table: Vec<Vec<Vec<Action>>> = vec![vec![], vec![]];
        let result = build_external_scanner_states(&grammar, 2, &symbol_to_index, &action_table);
        assert_eq!(result.len(), 2);
        assert!(result[0].is_empty());
        assert!(result[1].is_empty());
    }

    #[test]
    fn build_external_scanner_states_shift_flag_set() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.externals.push(ExternalToken {
            name: "ext".to_string(),
            symbol_id: SymbolId(5),
        });
        let mut symbol_to_index = BTreeMap::new();
        symbol_to_index.insert(SymbolId(5), 0);

        // state 0 has Shift at column 0; state 1 has Reduce only.
        let action_table: Vec<Vec<Vec<Action>>> = vec![
            vec![vec![Action::Shift(StateId(7))]],
            vec![vec![Action::Reduce(RuleId(0))]],
        ];

        let result = build_external_scanner_states(&grammar, 2, &symbol_to_index, &action_table);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], vec![true]);
        assert_eq!(result[1], vec![false]);
    }

    #[test]
    fn build_external_scanner_states_no_shift_means_false() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.externals.push(ExternalToken {
            name: "ext".to_string(),
            symbol_id: SymbolId(5),
        });
        let mut symbol_to_index = BTreeMap::new();
        symbol_to_index.insert(SymbolId(5), 0);

        let action_table: Vec<Vec<Vec<Action>>> =
            vec![vec![vec![Action::Accept, Action::Reduce(RuleId(1))]]];

        let result = build_external_scanner_states(&grammar, 1, &symbol_to_index, &action_table);
        assert_eq!(result, vec![vec![false]]);
    }

    #[test]
    fn build_external_scanner_states_skips_externals_missing_from_index() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.externals.push(ExternalToken {
            name: "ext".to_string(),
            symbol_id: SymbolId(99),
        });
        // symbol_to_index does NOT contain SymbolId(99).
        let symbol_to_index = BTreeMap::new();
        let action_table: Vec<Vec<Vec<Action>>> = vec![vec![]];

        let result = build_external_scanner_states(&grammar, 1, &symbol_to_index, &action_table);
        assert_eq!(result, vec![vec![false]]);
    }
}
