//! Producer-retained origins for the first optional-field projection slice.
//!
//! These records describe lowering decisions, rather than rediscovering helper
//! meaning from generated names or rule shapes. They do not change native rules.

use crate::{FieldId, Symbol, SymbolId};
use serde::{Deserialize, Serialize};

/// Identity of a source syntax alternative before helper allocation.
///
/// The namespace is the original owner and its alternative ordinal. It is
/// independent of shared production metadata and of the flattened, regenerated
/// lowered-rule index. A consumer must still establish the selected alternative;
/// the initial projection slice admits only owners with one alternative.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceRuleId {
    /// Original semantic owner.
    pub owner: SymbolId,
    /// Original alternative ordinal for that owner.
    pub alternative: usize,
}

/// Exact origin recorded when an optional carrier is allocated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OptionalOrigin {
    /// Source symbol admitted by the present branch. Named targets remain opaque.
    pub present: Symbol,
    /// Present alternative ordinal in the carrier's own lowered rule list.
    pub present_alternative: usize,
    /// Empty alternative ordinal in that same syntax-rule list.
    pub empty_alternative: usize,
}

/// Source field occurrence whose physical binding was lowered into a carrier.
///
/// Unlike a runtime edge, this record retains the semantic owner. In particular,
/// ordinary `Option<T>` does not move its native FIELD outside the empty choice.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OptionalFieldOrigin {
    /// Original source syntax alternative.
    pub source_rule: SourceRuleId,
    /// Original RHS occurrence, before normalization.
    pub source_rhs_position: usize,
    /// RHS position in the prepared owner rule.
    pub lowered_rhs_position: usize,
    /// Source field identity.
    pub field: FieldId,
    /// Explicitly allocated optional carrier.
    pub helper: SymbolId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Grammar, ProductionId, Rule};
    fn fixture() -> Grammar {
        let mut grammar = Grammar::new("origin".into());
        grammar.fields.insert(FieldId(1), "value".into());
        grammar.add_rule(Rule {
            lhs: SymbolId(10),
            rhs: vec![Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(1))))],
            fields: vec![(FieldId(1), 0)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(0),
        });
        grammar
    }
    #[test]
    fn test_normalize_records_exact_optional_origin_and_is_idempotent() {
        let mut grammar = fixture();
        grammar.normalize();
        let Symbol::NonTerminal(helper) = grammar.rules[&SymbolId(10)][0].rhs[0] else {
            panic!("carrier");
        };
        assert_eq!(
            grammar.optional_origins[&helper],
            OptionalOrigin {
                present: Symbol::Terminal(SymbolId(1)),
                present_alternative: 0,
                empty_alternative: 1,
            }
        );
        assert_eq!(
            grammar.optional_field_origins[0].source_rule,
            SourceRuleId {
                owner: SymbolId(10),
                alternative: 0
            }
        );
        assert_eq!(grammar.optional_field_origins[0].source_rhs_position, 0);
        assert!(validate(&grammar).is_ok());
        let before = grammar.clone();
        grammar.normalize();
        assert_eq!(grammar, before);
    }
    #[test]
    fn test_source_occurrence_survives_preceding_sequence_expansion() {
        let mut grammar = fixture();
        grammar.rules.get_mut(&SymbolId(10)).unwrap()[0].rhs.insert(
            0,
            Symbol::Sequence(vec![
                Symbol::Terminal(SymbolId(2)),
                Symbol::Terminal(SymbolId(3)),
            ]),
        );
        grammar.rules.get_mut(&SymbolId(10)).unwrap()[0].fields[0].1 = 1;
        grammar.normalize();
        let origin = &grammar.optional_field_origins[0];
        assert_eq!(origin.source_rhs_position, 1);
        assert_eq!(origin.lowered_rhs_position, 2);
        assert_eq!(
            grammar.rules[&SymbolId(10)][0].fields,
            vec![(FieldId(1), 2)]
        );
        assert!(validate(&grammar).is_ok());
        let before = grammar.clone();
        grammar.normalize();
        assert_eq!(before, grammar);
    }
    #[test]
    fn test_stale_optional_provenance_is_rejected_before_transport() {
        let mut grammar = fixture();
        grammar.normalize();
        let helper = grammar.optional_field_origins[0].helper;
        grammar
            .optional_origins
            .get_mut(&helper)
            .unwrap()
            .present_alternative = 1;
        assert_eq!(validate(&grammar), Err(OptionalOriginError { helper }));
    }
    #[test]
    fn test_optional_provenance_survives_token_merge_and_symbol_renumbering() {
        use crate::{Token, TokenPattern};
        let mut grammar = fixture();
        grammar.start_symbol = Some(SymbolId(10));
        grammar
            .rule_names
            .insert(SymbolId(10), "source_file".into());
        for id in [1, 4] {
            grammar.tokens.insert(
                SymbolId(id),
                Token {
                    name: format!("number{id}"),
                    pattern: TokenPattern::Regex(r"\d+".into()),
                    fragile: false,
                },
            );
        }
        grammar.tokens.insert(
            SymbolId(3),
            Token {
                name: "close".into(),
                pattern: TokenPattern::String(">".into()),
                fragile: false,
            },
        );
        let rule = &mut grammar.rules.get_mut(&SymbolId(10)).unwrap()[0];
        rule.rhs = vec![
            Symbol::Terminal(SymbolId(1)),
            Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(4)))),
            Symbol::Terminal(SymbolId(3)),
        ];
        rule.fields[0].1 = 1;
        grammar.normalize();
        crate::GrammarOptimizer::new().optimize(&mut grammar);
        assert!(validate(&grammar).is_ok());
        let occurrence = &grammar.optional_field_origins[0];
        assert_eq!(Some(occurrence.source_rule.owner), grammar.start_symbol);
        assert_eq!(occurrence.source_rhs_position, 1);
        assert_eq!(occurrence.lowered_rhs_position, 1);
        let numeric: Vec<_> = grammar
            .tokens
            .iter()
            .filter(|(_, token)| token.pattern == TokenPattern::Regex(r"\d+".into()))
            .collect();
        assert_eq!(numeric.len(), 1);
        assert_eq!(
            grammar.optional_origins[&occurrence.helper].present,
            Symbol::Terminal(*numeric[0].0)
        );
    }
    #[test]
    fn test_optional_origin_round_trips_without_changing_native_rules() {
        let mut grammar = fixture();
        grammar.normalize();
        let serialized = serde_json::to_string(&grammar).unwrap();
        let roundtrip: Grammar = serde_json::from_str(&serialized).unwrap();
        assert_eq!(roundtrip, grammar);
        let mut legacy = serde_json::to_value(&grammar).unwrap();
        legacy.as_object_mut().unwrap().remove("optional_origins");
        legacy
            .as_object_mut()
            .unwrap()
            .remove("optional_field_origins");
        let legacy: Grammar = serde_json::from_value(legacy).unwrap();
        assert_eq!(legacy.rules, grammar.rules);
        assert!(legacy.optional_origins.is_empty());
    }
}

/// A retained origin no longer matches its producer's lowered syntax.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid optional lowering origin for helper {helper}")]
pub struct OptionalOriginError {
    /// Allocated carrier whose provenance is stale or malformed.
    pub helper: SymbolId,
}
/// Check retained optional origins before co-generating capability metadata.
///
/// This checks explicit references; it never discovers new transparent helpers.
pub fn validate(grammar: &crate::Grammar) -> Result<(), OptionalOriginError> {
    for (helper, origin) in &grammar.optional_origins {
        let valid = grammar.rules.get(helper).is_some_and(|rules| {
            rules.len() == 2
                && origin.present_alternative != origin.empty_alternative
                && rules
                    .get(origin.present_alternative)
                    .is_some_and(|rule| rule.rhs == [origin.present.clone()])
                && rules
                    .get(origin.empty_alternative)
                    .is_some_and(|rule| rule.rhs.is_empty() || rule.rhs == [Symbol::Epsilon])
        });
        if !valid {
            return Err(OptionalOriginError { helper: *helper });
        }
    }
    for origin in &grammar.optional_field_origins {
        let valid = grammar.optional_origins.contains_key(&origin.helper)
            && grammar.fields.contains_key(&origin.field)
            && grammar
                .rules
                .get(&origin.source_rule.owner)
                .and_then(|rules| rules.get(origin.source_rule.alternative))
                .is_some_and(|rule| {
                    rule.rhs.get(origin.lowered_rhs_position)
                        == Some(&Symbol::NonTerminal(origin.helper))
                });
        if !valid {
            return Err(OptionalOriginError {
                helper: origin.helper,
            });
        }
    }
    Ok(())
}
