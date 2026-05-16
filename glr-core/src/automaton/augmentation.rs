use crate::{GLRError, GrammarError};
use adze_ir::*;

pub(super) struct AugmentedGrammar {
    pub(super) grammar: Grammar,
    pub(super) original_start: SymbolId,
    pub(super) augmented_start: SymbolId,
}

pub(super) fn augment_grammar(
    grammar: &Grammar,
    max_symbol: u16,
) -> Result<AugmentedGrammar, GLRError> {
    let mut augmented_grammar = grammar.clone();

    let original_start =
        grammar
            .start_symbol()
            .ok_or(GLRError::GrammarError(GrammarError::UnresolvedSymbol(
                SymbolId(0),
            )))?;

    let augmented_start_id = max_symbol.checked_add(2).ok_or_else(|| {
        GLRError::StateMachine(
            "Augmented start symbol would overflow u16: grammar has too many symbols".into(),
        )
    })?;
    let augmented_start = SymbolId(augmented_start_id);

    let max_production_id = grammar
        .all_rules()
        .map(|r| r.production_id.0)
        .max()
        .unwrap_or(0);
    let augmented_production_id = max_production_id
        .checked_add(1)
        .ok_or_else(|| GLRError::StateMachine("Production ID overflow".into()))?;

    let augmented_rule = Rule {
        lhs: augmented_start,
        rhs: vec![Symbol::NonTerminal(original_start)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(augmented_production_id),
    };
    augmented_grammar
        .rules
        .insert(augmented_start, vec![augmented_rule]);
    augmented_grammar
        .rule_names
        .insert(augmented_start, "$start".to_string());

    Ok(AugmentedGrammar {
        grammar: augmented_grammar,
        original_start,
        augmented_start,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal grammar that has exactly one rule LHS so `start_symbol()`
    /// falls through to the "first rules key" branch.
    fn make_minimal_grammar(start: SymbolId, production_id: ProductionId) -> Grammar {
        let mut grammar = Grammar::new("test".to_string());
        let rule = Rule {
            lhs: start,
            rhs: vec![],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id,
        };
        grammar.rules.insert(start, vec![rule]);
        grammar.rule_names.insert(start, "start_rule".to_string());
        grammar
    }

    #[test]
    fn augment_grammar_happy_path_sets_augmented_start_and_rule() {
        let original_start = SymbolId(3);
        let grammar = make_minimal_grammar(original_start, ProductionId(0));

        let augmented = augment_grammar(&grammar, 10).expect("augmentation should succeed");

        // augmented_start.0 == max_symbol + 2
        assert_eq!(augmented.augmented_start, SymbolId(12));
        // original_start is propagated.
        assert_eq!(augmented.original_start, original_start);

        // The augmented grammar has one rule under the new start.
        let new_rules = augmented
            .grammar
            .rules
            .get(&augmented.augmented_start)
            .expect("augmented start must have rules");
        assert_eq!(new_rules.len(), 1);
        let new_rule = &new_rules[0];
        assert_eq!(new_rule.lhs, augmented.augmented_start);
        assert_eq!(new_rule.rhs, vec![Symbol::NonTerminal(original_start)]);
        assert!(new_rule.precedence.is_none());
        assert!(new_rule.associativity.is_none());
        assert!(new_rule.fields.is_empty());
    }

    #[test]
    fn augment_grammar_assigns_dollar_start_rule_name() {
        let original_start = SymbolId(1);
        let grammar = make_minimal_grammar(original_start, ProductionId(0));

        let augmented = augment_grammar(&grammar, 5).expect("augmentation should succeed");

        assert_eq!(
            augmented.grammar.rule_names.get(&augmented.augmented_start),
            Some(&"$start".to_string())
        );
    }

    #[test]
    fn augment_grammar_assigns_next_production_id() {
        let original_start = SymbolId(1);
        // Existing rule uses production_id 7; augmented rule should bump to 8.
        let grammar = make_minimal_grammar(original_start, ProductionId(7));

        let augmented = augment_grammar(&grammar, 3).expect("augmentation should succeed");

        let new_rule = &augmented
            .grammar
            .rules
            .get(&augmented.augmented_start)
            .expect("augmented start must have rules")[0];
        assert_eq!(new_rule.production_id, ProductionId(8));
    }

    #[test]
    fn augment_grammar_preserves_original_rules() {
        let original_start = SymbolId(2);
        let grammar = make_minimal_grammar(original_start, ProductionId(0));

        let augmented = augment_grammar(&grammar, 4).expect("augmentation should succeed");

        // Original rule still present under the original start symbol.
        assert!(augmented.grammar.rules.contains_key(&original_start));
        assert_eq!(
            augmented
                .grammar
                .rules
                .get(&original_start)
                .map(|r| r.len()),
            Some(1)
        );
    }

    #[test]
    fn augment_grammar_returns_unresolved_symbol_when_no_start() {
        // Empty grammar => start_symbol() returns None.
        let grammar = Grammar::new("empty".to_string());

        match augment_grammar(&grammar, 5) {
            Err(GLRError::GrammarError(GrammarError::UnresolvedSymbol(sym))) => {
                assert_eq!(sym, SymbolId(0));
            }
            Err(other) => panic!("unexpected error variant: {other:?}"),
            Ok(_) => panic!("augmentation should fail without a start symbol"),
        }
    }

    #[test]
    fn augment_grammar_rejects_max_symbol_overflow() {
        let original_start = SymbolId(1);
        let grammar = make_minimal_grammar(original_start, ProductionId(0));

        // max_symbol.checked_add(2) overflows when max_symbol == u16::MAX - 1.
        match augment_grammar(&grammar, u16::MAX - 1) {
            Err(GLRError::StateMachine(msg)) => {
                assert!(
                    msg.contains("overflow"),
                    "expected overflow message, got: {msg}"
                );
            }
            Err(other) => panic!("unexpected error variant: {other:?}"),
            Ok(_) => panic!("augmentation should fail on u16 overflow"),
        }
    }

    #[test]
    fn augment_grammar_rejects_production_id_overflow() {
        let original_start = SymbolId(1);
        // Existing rule uses the max ProductionId; the +1 must fail.
        let grammar = make_minimal_grammar(original_start, ProductionId(u16::MAX));

        match augment_grammar(&grammar, 5) {
            Err(GLRError::StateMachine(msg)) => {
                assert!(
                    msg.contains("Production ID overflow"),
                    "expected production-id overflow message, got: {msg}"
                );
            }
            Err(other) => panic!("unexpected error variant: {other:?}"),
            Ok(_) => panic!("augmentation should fail on production-id overflow"),
        }
    }
}
