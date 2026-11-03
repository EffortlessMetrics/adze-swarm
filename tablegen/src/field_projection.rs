//! Canonical first-slice selected-node projection programs.
//!
//! Only allocation-time optional origins grant transparent carrier traversal.
//! Unsupported alternatives and composite origins are omitted, never guessed.
use adze_ir::{Grammar, Symbol, SymbolId};

/// Shared projection interpretation revision. Revision 2 certifies direct FIELD
/// observations for admitted single-rule owners with no optional transitions.
pub const FIELD_PROJECTION_REVISION: u32 = 2;

/// One original field occurrence in an unambiguous prepared owner rule.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldOccurrence {
    /// Canonical field name.
    pub name: String,
    /// Physical RHS slot (excluding non-RHS extras).
    pub position: usize,
    /// Original grammar identity of the actual selected target.
    pub target: SymbolId,
    /// Explicit optional carrier, if this occurrence may be absent.
    pub optional: Option<OptionalCarrier>,
}
/// Explicit producer-sanctioned retained or unary-elided optional carrier.
#[derive(Clone, Debug, PartialEq)]
pub struct OptionalCarrier {
    /// Allocated carrier identity.
    pub helper: SymbolId,
    /// Present syntax alternative in the carrier's rule list.
    pub present_alternative: usize,
    /// Empty syntax alternative in that same list.
    pub empty_alternative: usize,
}
/// One admitted single-rule owner.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnerProjection {
    /// Original grammar owner identity.
    pub owner: SymbolId,
    /// Prepared RHS identities, before native unary carrier elision.
    pub rhs: Vec<SymbolId>,
    /// Ordered source field occurrences.
    pub fields: Vec<FieldOccurrence>,
}
fn leaf(symbol: &Symbol) -> Option<SymbolId> {
    match symbol {
        Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => Some(*id),
        _ => None,
    }
}
/// Derive the shared program from retained producer provenance.
///
/// A missing owner is explicitly unsupported by this first slice. In particular,
/// legacy shape/name classification grants facts but never projection capability.
#[must_use]
pub fn for_grammar(grammar: &Grammar) -> Vec<OwnerProjection> {
    let schema = crate::field_schema::field_semantics_for_grammar(grammar);
    let mut owners = Vec::new();
    for (owner, rules) in &grammar.rules {
        if rules.len() != 1 || grammar.optional_origins.contains_key(owner) {
            continue;
        }
        let rule = &rules[0];
        let Some(rhs) = rule.rhs.iter().map(leaf).collect::<Option<Vec<_>>>() else {
            continue;
        };
        let mut bindings = rule.fields.clone();
        for origin in &grammar.optional_field_origins {
            if origin.source_rule.owner == *owner && origin.source_rule.alternative == 0 {
                let binding = (origin.field, origin.lowered_rhs_position);
                if rule.rhs.get(binding.1) == Some(&Symbol::NonTerminal(origin.helper))
                    && !bindings.contains(&binding)
                {
                    bindings.push(binding);
                }
            }
        }
        bindings.sort_by_key(|(_, position)| *position);
        let mut fields = Vec::new();
        let mut supported = true;
        let mut occurrences = std::collections::BTreeSet::new();
        for (field, position) in bindings {
            if !occurrences.insert((field.0, position)) {
                supported = false;
                break;
            }
            let Some(name) = grammar.fields.get(&field) else {
                supported = false;
                break;
            };
            let Some(symbol) = rule.rhs.get(position) else {
                supported = false;
                break;
            };
            let Some(id) = leaf(symbol) else {
                supported = false;
                break;
            };
            let (target, optional) = if let Some(origin) = grammar.optional_origins.get(&id) {
                let Some(target) = leaf(&origin.present) else {
                    supported = false;
                    break;
                };
                if grammar.optional_origins.contains_key(&target) {
                    supported = false;
                    break;
                }
                let Some(carrier_rules) = grammar.rules.get(&id) else {
                    supported = false;
                    break;
                };
                let present_ok = carrier_rules
                    .get(origin.present_alternative)
                    .is_some_and(|rule| rule.rhs == [origin.present.clone()]);
                let empty_ok = carrier_rules
                    .get(origin.empty_alternative)
                    .is_some_and(|rule| rule.rhs.is_empty() || rule.rhs == [Symbol::Epsilon]);
                if !present_ok
                    || !empty_ok
                    || origin.present_alternative == origin.empty_alternative
                {
                    supported = false;
                    break;
                }
                (
                    target,
                    Some(OptionalCarrier {
                        helper: id,
                        present_alternative: origin.present_alternative,
                        empty_alternative: origin.empty_alternative,
                    }),
                )
            } else {
                (id, None)
            };
            // A legacy helper resolved by the fact analyzer is not a direct
            // target and has no producer-proven carrier transition.
            if !schema
                .get(owner)
                .and_then(|fields| fields.get(name))
                .is_some_and(|fact| {
                    fact.targets
                        .iter()
                        .any(|symbol| leaf(symbol) == Some(target))
                })
            {
                supported = false;
                break;
            }
            fields.push(FieldOccurrence {
                name: name.clone(),
                position,
                target,
                optional,
            });
        }
        if supported {
            owners.push(OwnerProjection {
                owner: *owner,
                rhs,
                fields,
            });
        }
    }
    owners
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::{FieldId, ProductionId, Rule};
    fn fixture() -> Grammar {
        let mut grammar = Grammar::new("optional_projection".into());
        grammar.fields.insert(FieldId(0), "value".into());
        grammar.add_rule(Rule {
            lhs: SymbolId(10),
            rhs: vec![Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(1))))],
            fields: vec![(FieldId(0), 0)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(0),
        });
        grammar.normalize();
        grammar
    }
    #[test]
    fn test_projection_uses_explicit_origin_independent_of_helper_name() {
        let mut grammar = fixture();
        let helper = *grammar.optional_origins.keys().next().unwrap();
        grammar
            .rule_names
            .insert(helper, "Not_A_Helper_Name".into());
        let plan = for_grammar(&grammar);
        let owner = plan
            .iter()
            .find(|owner| owner.owner == SymbolId(10))
            .unwrap();
        assert_eq!(owner.fields[0].target, SymbolId(1));
        assert_eq!(owner.fields[0].optional.as_ref().unwrap().helper, helper);
        grammar.optional_origins.clear();
        grammar.rule_names.shift_remove(&helper);
        assert!(
            !for_grammar(&grammar)
                .iter()
                .any(|owner| owner.owner == SymbolId(10)),
            "no name/shape discovery may grant a carrier transition"
        );
    }
    #[test]
    fn test_projection_rejects_stale_origin_and_ambiguous_owner() {
        let mut grammar = fixture();
        let helper = *grammar.optional_origins.keys().next().unwrap();
        grammar
            .optional_origins
            .get_mut(&helper)
            .unwrap()
            .present_alternative = 1;
        assert!(
            !for_grammar(&grammar)
                .iter()
                .any(|owner| owner.owner == SymbolId(10))
        );
        let mut grammar = fixture();
        let rule = grammar.rules[&SymbolId(10)][0].clone();
        grammar.add_rule(rule);
        assert!(
            !for_grammar(&grammar)
                .iter()
                .any(|owner| owner.owner == SymbolId(10))
        );
    }
}
