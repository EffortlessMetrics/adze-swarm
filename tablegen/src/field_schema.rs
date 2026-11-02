//! Canonical field/child semantic schema for grammars.
//!
//! The single authority for field cardinality and target-kind facts derived
//! from a (possibly normalized) grammar. Normalization replaces
//! `Optional`/`Repeat`/`Choice`/`Sequence` wrappers with auxiliary rules, so
//! a field bound to a helper non-terminal must be resolved back to its
//! origin shape; otherwise cardinality collapses to "mandatory singular"
//! and targets name the helper instead of the real children. The same
//! resolution applies to macro-generated `_`-prefixed helpers, which share
//! these shapes.
//!
//! Consumers (node-types JSON, typed CST accessors, document metadata,
//! query validation) must derive field facts through this module rather than
//! reimplementing cardinality analysis, so all projections agree on one
//! contract. Typed CST accessor shapes, document metadata, and query
//! validation migrate in follow-up slices; this module only declares and
//! computes the facts.

use adze_ir::{Grammar, Symbol, SymbolId};
use std::collections::{BTreeMap, HashMap};

/// Cardinality and target kinds for one field of one node kind.
///
/// `targets` holds leaf symbols only (terminals, non-terminals, externals):
/// helper wrappers are resolved away, never reported. Sorted deterministically
/// by (symbol kind, id).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSemantics {
    /// True when every alternative provides the field in a non-optional position.
    pub required: bool,
    /// True when the field may appear more than once (repeat or multi-binding).
    pub multiple: bool,
    /// Deterministic union of admitted leaf target kinds across alternatives.
    pub targets: Vec<Symbol>,
}

/// Origin shape of a helper non-terminal produced by normalization or by
/// macro-generated wrappers.
///
/// Every variant has unambiguous formal semantics; anything else is
/// [`AuxShape::Opaque`] and keeps the legacy treat-as-leaf behavior rather
/// than a guessed decomposition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuxShape {
    /// `aux -> [aux, inner] | [Epsilon]` (either nesting order): zero or more.
    Repeat { inner: Symbol },
    /// `aux -> [aux, inner] | [inner]`: one or more.
    RepeatOne { inner: Symbol },
    /// Every alternative is a single symbol: exactly one of the members is
    /// admitted. An epsilon member makes the whole choice optional without
    /// contributing a target.
    Choice { members: Vec<Symbol> },
    /// Not a recognized helper shape: treat the symbol as an opaque leaf.
    Opaque,
}

/// Classify the rules of one helper candidate.
///
/// Only single-symbol right-hand sides participate; anything else is opaque.
/// `self_id` guards against degenerate self-reference: a helper that can
/// only derive itself carries no observable child.
pub fn aux_shape(grammar: &Grammar, self_id: SymbolId) -> AuxShape {
    let Some(rules) = grammar.rules.get(&self_id) else {
        return AuxShape::Opaque;
    };
    if rules.is_empty() {
        return AuxShape::Opaque;
    }
    // Partition alternatives into single symbols (choice members) and the
    // two-symbol self-recursive form `aux -> [aux, inner] | [inner, aux]`.
    // Anything else (longer right-hand sides, non-self recursion) is not a
    // helper shape.
    let mut singles: Vec<Symbol> = Vec::with_capacity(rules.len());
    let mut recursive_inners: Vec<Symbol> = Vec::new();
    for rule in rules {
        match rule.rhs.as_slice() {
            [single] => singles.push(single.clone()),
            [first, second] => {
                if let Symbol::NonTerminal(id) = first
                    && *id == self_id
                {
                    recursive_inners.push(second.clone());
                    continue;
                }
                if let Symbol::NonTerminal(id) = second
                    && *id == self_id
                {
                    recursive_inners.push(first.clone());
                    continue;
                }
                return AuxShape::Opaque;
            }
            _ => return AuxShape::Opaque,
        }
    }
    if !recursive_inners.is_empty() {
        // Only the two canonical recursive shapes are representable.
        if recursive_inners.len() == 1 && singles.len() == 1 {
            if matches!(singles[0], Symbol::Epsilon) {
                return AuxShape::Repeat {
                    inner: recursive_inners.pop().expect("one inner"),
                };
            }
            let inner = recursive_inners.pop().expect("one inner");
            if singles[0] == inner {
                return AuxShape::RepeatOne { inner };
            }
        }
        return AuxShape::Opaque;
    }
    if singles.iter().any(|member| {
        matches!(
            member,
            Symbol::NonTerminal(id) if *id == self_id
        )
    }) {
        // A lone self-reference with no recursive alternative derives
        // nothing observable.
        return AuxShape::Opaque;
    }
    if singles.len() == 1 && !matches!(singles[0], Symbol::Epsilon) {
        // A single non-epsilon member is a transparent alias only when the
        // helper carries no name of its own; named rules keep their identity
        // and are handled by the caller (opaque here).
        return AuxShape::Opaque;
    }
    // Every remaining shape is a choice over its single-symbol members.
    // Epsilon members stay in the set so the choice itself records
    // optionality; resolving them contributes no targets.
    AuxShape::Choice { members: singles }
}

/// Cardinality and target kinds resolved for one symbol occurrence.
#[derive(Debug, Clone, Default)]
struct TargetSet {
    optional: bool,
    multiple: bool,
    targets: Vec<Symbol>,
}

impl TargetSet {
    fn leaf(symbol: &Symbol) -> Self {
        TargetSet {
            optional: false,
            multiple: false,
            targets: vec![symbol.clone()],
        }
    }

    fn empty_optional() -> Self {
        TargetSet {
            optional: true,
            multiple: false,
            targets: Vec::new(),
        }
    }

    fn union_into(&mut self, other: TargetSet) {
        self.optional |= other.optional;
        self.multiple |= other.multiple;
        for target in other.targets {
            if !self.targets.contains(&target) {
                self.targets.push(target);
            }
        }
    }
}

/// Resolve one field-position symbol to cardinality and leaf targets,
/// seeing through helper wrappers produced by normalization.
///
/// `visiting` guards against helper cycles: re-entering a helper being
/// resolved falls back to opaque rather than recursing forever.
fn resolve_symbol(grammar: &Grammar, symbol: &Symbol, visiting: &mut Vec<SymbolId>) -> TargetSet {
    match symbol {
        Symbol::Epsilon => TargetSet::empty_optional(),
        Symbol::Optional(inner) => {
            let mut inner_set = resolve_symbol(grammar, inner, visiting);
            inner_set.optional = true;
            inner_set
        }
        Symbol::Repeat(inner) => {
            let mut inner_set = resolve_symbol(grammar, inner, visiting);
            if inner_set.targets.is_empty() {
                return TargetSet::empty_optional();
            }
            inner_set.optional = true;
            inner_set.multiple = true;
            inner_set
        }
        Symbol::RepeatOne(inner) => {
            let mut inner_set = resolve_symbol(grammar, inner, visiting);
            if inner_set.targets.is_empty() {
                return TargetSet::empty_optional();
            }
            inner_set.multiple = true;
            inner_set
        }
        Symbol::Choice(choices) => {
            let mut combined = TargetSet {
                optional: false,
                multiple: false,
                targets: Vec::new(),
            };
            let mut any_branch = false;
            for choice in choices {
                any_branch = true;
                combined.union_into(resolve_symbol(grammar, choice, visiting));
            }
            if !any_branch {
                combined.optional = true;
            }
            combined
        }
        Symbol::Sequence(seq) => {
            if seq.is_empty() {
                return TargetSet::empty_optional();
            }
            let mut combined = TargetSet {
                // A sequence is absent only when every member is absent.
                optional: true,
                multiple: false,
                targets: Vec::new(),
            };
            let mut presentable = 0usize;
            for member in seq {
                let cardinality = resolve_symbol(grammar, member, visiting);
                // Epsilon members contribute no child; they do not make a
                // sequence multiple nor affect its target set.
                if cardinality.targets.is_empty() {
                    continue;
                }
                presentable += 1;
                combined.optional &= cardinality.optional;
                combined.multiple |= cardinality.multiple;
                for target in cardinality.targets {
                    if !combined.targets.contains(&target) {
                        combined.targets.push(target);
                    }
                }
            }
            if presentable == 0 {
                return TargetSet::empty_optional();
            }
            if presentable > 1 {
                combined.multiple = true;
            }
            combined
        }
        Symbol::Terminal(_) | Symbol::External(_) => TargetSet::leaf(symbol),
        Symbol::NonTerminal(id) => {
            // Only transparent helpers are resolved through: unnamed rules
            // with a recognized helper shape, or explicitly hidden (`_`-prefixed)
            // rules with one. Named language rules keep their node identity:
            // the runtime tree really contains that node.
            let transparent = match grammar.rule_names.get(id) {
                None => true,
                Some(name) => name.starts_with('_'),
            };
            if !transparent {
                return TargetSet::leaf(symbol);
            }
            if visiting.contains(id) {
                return TargetSet::leaf(symbol);
            }
            match aux_shape(grammar, *id) {
                AuxShape::Opaque => TargetSet::leaf(symbol),
                AuxShape::Repeat { inner } => {
                    visiting.push(*id);
                    let mut resolved = resolve_symbol(grammar, &inner, visiting);
                    visiting.pop();
                    if resolved.targets.is_empty() {
                        return TargetSet::empty_optional();
                    }
                    resolved.optional = true;
                    resolved.multiple = true;
                    resolved
                }
                AuxShape::RepeatOne { inner } => {
                    visiting.push(*id);
                    let mut resolved = resolve_symbol(grammar, &inner, visiting);
                    visiting.pop();
                    if resolved.targets.is_empty() {
                        return TargetSet::empty_optional();
                    }
                    resolved.multiple = true;
                    resolved
                }
                AuxShape::Choice { members } => {
                    if members.is_empty() {
                        return TargetSet::empty_optional();
                    }
                    visiting.push(*id);
                    let mut combined = TargetSet {
                        optional: false,
                        multiple: false,
                        targets: Vec::new(),
                    };
                    for member in &members {
                        combined.union_into(resolve_symbol(grammar, member, visiting));
                    }
                    visiting.pop();
                    combined
                }
            }
        }
    }
}

/// Compute canonical field semantics for every rule left-hand side that
/// carries fields.
///
/// A field is required only when every alternative provides it in a
/// non-optional position; it is multiple when any alternative binds it more
/// than once or through a repetition. Target kinds are the deterministic
/// union across alternatives and choice members. Within one alternative, a
/// mandatory occurrence plus an optional occurrence still guarantees
/// presence (per-alternative AND, not OR).
pub fn field_semantics_for_grammar(
    grammar: &Grammar,
) -> BTreeMap<SymbolId, BTreeMap<String, FieldSemantics>> {
    let mut per_owner: BTreeMap<SymbolId, BTreeMap<String, FieldSemantics>> = BTreeMap::new();

    for (lhs, rules) in &grammar.rules {
        // Skip helper non-terminals without their own rule name: their
        // facts belong to the field that references them, not to a public
        // node type. Named rules keep their identity even when they match
        // a helper shape.
        let is_helper = !grammar.rule_names.contains_key(lhs)
            && !matches!(aux_shape(grammar, *lhs), AuxShape::Opaque);
        if is_helper {
            continue;
        }
        let mut field_facts: HashMap<String, OwnerFieldFacts> = HashMap::new();
        for rule in rules {
            // Count bindings per field within this one alternative so a
            // field bound twice here is multiple even without a repeat.
            let mut bound_in_rule: HashMap<&str, usize> = HashMap::new();
            for (field_id, position) in &rule.fields {
                if rule.rhs.get(*position).is_some()
                    && let Some(field_name) = grammar.fields.get(field_id)
                {
                    *bound_in_rule.entry(field_name.as_str()).or_default() += 1;
                }
            }
            // Combine occurrences within one alternative before combining
            // alternatives: a mandatory plus an optional occurrence in the
            // same rule still guarantees presence.
            let mut per_rule: HashMap<String, Vec<TargetSet>> = HashMap::new();
            for (field_id, position) in &rule.fields {
                let (Some(field_name), Some(symbol)) =
                    (grammar.fields.get(field_id), rule.rhs.get(*position))
                else {
                    continue;
                };
                let mut visiting = Vec::new();
                let resolved = resolve_symbol(grammar, symbol, &mut visiting);
                per_rule
                    .entry(field_name.clone())
                    .or_default()
                    .push(resolved);
            }
            for (field_name, cards) in per_rule {
                let rule_optional = cards.iter().all(|card| card.optional);
                let rule_multiple = cards.len() > 1 || cards.iter().any(|card| card.multiple);
                let facts = field_facts.entry(field_name.clone()).or_default();
                facts.multiple |= rule_multiple;
                facts.optional |= rule_optional;
                for card in &cards {
                    for target in &card.targets {
                        if !facts.targets.contains(target) {
                            facts.targets.push(target.clone());
                        }
                    }
                }
            }
            for field_name in bound_in_rule.into_keys() {
                field_facts
                    .entry(field_name.to_string())
                    .or_default()
                    .present_in_alternatives += 1;
            }
        }
        let fields = field_facts
            .into_iter()
            .map(|(field_name, mut facts)| {
                facts.targets.sort_by_key(target_sort_key);
                let required = facts.present_in_alternatives == rules.len() && !facts.optional;
                (
                    field_name,
                    FieldSemantics {
                        required,
                        multiple: facts.multiple,
                        targets: facts.targets,
                    },
                )
            })
            .collect();
        per_owner.insert(*lhs, fields);
    }

    per_owner
}

/// Deterministic ordering for leaf target symbols.
fn target_sort_key(symbol: &Symbol) -> (u8, u16) {
    match symbol {
        Symbol::Terminal(id) => (0, id.0),
        Symbol::NonTerminal(id) => (1, id.0),
        Symbol::External(id) => (2, id.0),
        _ => (3, u16::MAX),
    }
}

/// Accumulated facts for one field across one owner's alternatives.
#[derive(Debug, Default)]
struct OwnerFieldFacts {
    present_in_alternatives: usize,
    optional: bool,
    multiple: bool,
    targets: Vec<Symbol>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::{FieldId, ProductionId, Rule, Token, TokenPattern};

    fn token(name: &str, pattern: TokenPattern) -> Token {
        Token {
            name: name.to_string(),
            pattern,
            fragile: false,
        }
    }

    fn term(id: u16) -> Symbol {
        Symbol::Terminal(SymbolId(id))
    }

    fn add_rule(
        grammar: &mut Grammar,
        pid: &mut u16,
        lhs: u16,
        rhs: Vec<Symbol>,
        fields: Vec<(FieldId, usize)>,
    ) {
        grammar.add_rule(Rule {
            lhs: SymbolId(lhs),
            rhs,
            precedence: None,
            associativity: None,
            fields,
            production_id: ProductionId(*pid),
        });
        *pid += 1;
    }

    /// Six-shape acceptance grammar. Field ids: 1 = value, 2 = items,
    /// 3 = maybe. Rules: 10 = start, 11 = item, 12 = pair, 13 = opts,
    /// 14 = pick. An alias marks the word alternative of `item` without
    /// changing its field facts.
    fn acceptance_grammar() -> Grammar {
        let mut grammar = Grammar::new("acceptance".to_string());
        for (id, name, pattern) in [
            (1u16, "num", TokenPattern::Regex(r"\d+".to_string())),
            (2, "word", TokenPattern::Regex(r"[a-z]+".to_string())),
            (3, "plus", TokenPattern::String("+".to_string())),
            (4, "semi", TokenPattern::String(";".to_string())),
        ] {
            grammar.tokens.insert(SymbolId(id), token(name, pattern));
        }
        for (id, name) in [
            (10u16, "start"),
            (11, "item"),
            (12, "pair"),
            (13, "opts"),
            (14, "pick"),
        ] {
            grammar.rule_names.insert(SymbolId(id), name.to_string());
        }
        grammar.fields.insert(FieldId(2), "items".to_string());
        grammar.fields.insert(FieldId(3), "maybe".to_string());
        grammar.fields.insert(FieldId(1), "value".to_string());
        let mut pid = 0u16;
        // start -> item (field value on the single child).
        add_rule(
            &mut grammar,
            &mut pid,
            10,
            vec![Symbol::NonTerminal(SymbolId(11))],
            vec![(FieldId(1), 0)],
        );
        // item -> num | word (same-named field, different target kinds).
        add_rule(
            &mut grammar,
            &mut pid,
            11,
            vec![term(1)],
            vec![(FieldId(1), 0)],
        );
        add_rule(
            &mut grammar,
            &mut pid,
            11,
            vec![term(2)],
            vec![(FieldId(1), 0)],
        );
        // pair -> Sequence(item, item?) plus (mandatory + optional same field
        // in one alternative; the trailing plain child is unfielded).
        add_rule(
            &mut grammar,
            &mut pid,
            12,
            vec![
                Symbol::Sequence(vec![
                    Symbol::NonTerminal(SymbolId(11)),
                    Symbol::Optional(Box::new(Symbol::NonTerminal(SymbolId(11)))),
                ]),
                term(3),
            ],
            vec![(FieldId(1), 0)],
        );
        // opts -> word* (zero or more repeated children).
        add_rule(
            &mut grammar,
            &mut pid,
            13,
            vec![Symbol::Repeat(Box::new(term(2)))],
            vec![(FieldId(2), 0)],
        );
        // pick -> (num | word) (choice alternatives).
        add_rule(
            &mut grammar,
            &mut pid,
            14,
            vec![Symbol::Choice(vec![term(1), term(2)])],
            vec![(FieldId(1), 0)],
        );
        // seqfield -> semi word num with a field on the sequence: the field
        // fans out to both sequence members after normalization.
        grammar
            .rule_names
            .insert(SymbolId(15), "seqfield".to_string());
        add_rule(
            &mut grammar,
            &mut pid,
            15,
            vec![term(4), Symbol::Sequence(vec![term(1), term(2)])],
            vec![(FieldId(3), 0), (FieldId(1), 1)],
        );
        // A visible alias distinct from grammar identity on item's word
        // alternative (the second item rule): field facts must be unaffected.
        grammar.alias_sequences.insert(
            ProductionId(2),
            adze_ir::AliasSequence {
                aliases: vec![Some("word_alias".to_string())],
            },
        );
        grammar.set_start_symbol(SymbolId(10));
        grammar
    }

    fn normalized_acceptance_grammar() -> Grammar {
        let mut grammar = acceptance_grammar();
        grammar
            .try_normalize()
            .expect("acceptance grammar normalizes");
        grammar.validate().expect("normalized grammar validates");
        grammar
    }

    fn field<'g>(
        schema: &'g BTreeMap<SymbolId, BTreeMap<String, FieldSemantics>>,
        owner: u16,
        name: &str,
    ) -> &'g FieldSemantics {
        schema
            .get(&SymbolId(owner))
            .unwrap_or_else(|| panic!("owner {owner} must have field facts"))
            .get(name)
            .unwrap_or_else(|| panic!("field {name} must exist on owner {owner}"))
    }

    #[test]
    fn test_resolved_targets_reference_no_helpers() {
        let grammar = normalized_acceptance_grammar();
        // Every field bound to a helper must resolve to a non-opaque shape.
        let schema = field_semantics_for_grammar(&grammar);
        let pair_value = field(&schema, 12, "value");
        assert!(
            pair_value
                .targets
                .iter()
                .all(|target| matches!(target, Symbol::Terminal(_) | Symbol::NonTerminal(_))),
            "resolved targets must be leaf symbols, got {:?}",
            pair_value.targets
        );
        // No field may name a helper rule id as its target kind.
        for (owner, fields) in &schema {
            for (name, semantics) in fields {
                for target in &semantics.targets {
                    if let Symbol::NonTerminal(id) = target {
                        assert!(
                            grammar.rule_names.contains_key(id),
                            "owner {owner:?} field {name}: target {target:?} must be a named rule or leaf, not an anonymous helper"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_normalized_item_field_unions_alternative_targets() {
        let schema = field_semantics_for_grammar(&normalized_acceptance_grammar());
        let value = field(&schema, 11, "value");
        assert!(value.required, "present in every alternative");
        assert!(!value.multiple, "single binding per alternative");
        assert_eq!(value.targets, vec![term(1), term(2)]);
    }

    #[test]
    fn test_normalized_pair_field_stays_present_through_optional() {
        let schema = field_semantics_for_grammar(&normalized_acceptance_grammar());
        let value = field(&schema, 12, "value");
        assert!(
            value.required,
            "mandatory occurrence guarantees presence: {value:?}"
        );
        assert!(value.multiple, "expanded sequence binds more than once");
        // The aux helper resolves to the item rule, not a generated id.
        assert_eq!(value.targets, vec![Symbol::NonTerminal(SymbolId(11))]);
    }

    #[test]
    fn test_normalized_repeat_field_is_optional_and_multiple() {
        let schema = field_semantics_for_grammar(&normalized_acceptance_grammar());
        let items = field(&schema, 13, "items");
        assert!(!items.required, "zero repetitions admit absence");
        assert!(items.multiple, "repetition admits several children");
        assert_eq!(items.targets, vec![term(2)]);
    }

    #[test]
    fn test_normalized_choice_field_covers_all_targets() {
        let schema = field_semantics_for_grammar(&normalized_acceptance_grammar());
        let value = field(&schema, 14, "value");
        assert!(value.required, "every alternative provides the field");
        assert!(!value.multiple, "single selection per parse");
        assert_eq!(value.targets, vec![term(1), term(2)]);
    }

    #[test]
    fn test_normalized_field_on_sequence_fans_out() {
        let schema = field_semantics_for_grammar(&normalized_acceptance_grammar());
        let maybe = field(&schema, 15, "maybe");
        assert!(maybe.required, "plain leading child is always present");
        assert!(!maybe.multiple);
        assert_eq!(maybe.targets, vec![term(4)]);
        let value = field(&schema, 15, "value");
        assert!(value.required, "both expanded members are mandatory");
        assert!(value.multiple, "one binding expanded to two children");
        assert_eq!(value.targets, vec![term(1), term(2)]);
    }

    #[test]
    fn test_alias_does_not_disturb_field_facts() {
        // The alias on item's word alternative must not move field targets.
        let schema = field_semantics_for_grammar(&normalized_acceptance_grammar());
        let value = field(&schema, 11, "value");
        assert_eq!(value.targets, vec![term(1), term(2)]);
        assert!(value.required);
    }

    #[test]
    fn test_helpers_have_no_public_node_entry() {
        // Every owner key in the schema must be a rule the generator would
        // consider: helpers only appear when named. Anonymous helpers are
        // facts for their referencing field, not nodes.
        let grammar = normalized_acceptance_grammar();
        let schema = field_semantics_for_grammar(&grammar);
        for owner in schema.keys() {
            let named = grammar.rule_names.contains_key(owner);
            let resolves = !matches!(aux_shape(&grammar, *owner), AuxShape::Opaque);
            assert!(
                named || !resolves,
                "anonymous helper {owner:?} must not own a schema entry"
            );
        }
    }

    #[test]
    fn test_aux_shape_detects_all_helper_forms() {
        let mut grammar = Grammar::new("shapes".to_string());
        let mut pid = 0u16;
        let mut add = |grammar: &mut Grammar, lhs: u16, rhs: Vec<Symbol>| {
            grammar.add_rule(Rule {
                lhs: SymbolId(lhs),
                rhs,
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(pid),
            });
            pid += 1;
        };
        let t = |id: u16| Symbol::Terminal(SymbolId(id));
        let nt = |id: u16| Symbol::NonTerminal(SymbolId(id));
        // Optional: aux -> [inner] | [Epsilon] (either order).
        add(&mut grammar, 100, vec![t(1)]);
        add(&mut grammar, 100, vec![Symbol::Epsilon]);
        assert_eq!(
            aux_shape(&grammar, SymbolId(100)),
            AuxShape::Choice {
                members: vec![t(1), Symbol::Epsilon]
            }
        );
        add(&mut grammar, 101, vec![Symbol::Epsilon]);
        add(&mut grammar, 101, vec![t(2)]);
        assert_eq!(
            aux_shape(&grammar, SymbolId(101)),
            AuxShape::Choice {
                members: vec![Symbol::Epsilon, t(2)]
            }
        );
        // Repeat: aux -> [aux, inner] | [Epsilon].
        add(&mut grammar, 102, vec![nt(102), t(1)]);
        add(&mut grammar, 102, vec![Symbol::Epsilon]);
        assert_eq!(
            aux_shape(&grammar, SymbolId(102)),
            AuxShape::Repeat { inner: t(1) }
        );
        // RepeatOne: aux -> [aux, inner] | [inner].
        add(&mut grammar, 103, vec![nt(103), t(1)]);
        add(&mut grammar, 103, vec![t(1)]);
        assert_eq!(
            aux_shape(&grammar, SymbolId(103)),
            AuxShape::RepeatOne { inner: t(1) }
        );
        // Plain multi-alternative choice.
        add(&mut grammar, 104, vec![t(1)]);
        add(&mut grammar, 104, vec![t(2)]);
        assert_eq!(
            aux_shape(&grammar, SymbolId(104)),
            AuxShape::Choice {
                members: vec![t(1), t(2)]
            }
        );
        // Opaque cases: multi-symbol alternatives, self-only recursion,
        // single non-epsilon member, unknown id.
        add(&mut grammar, 105, vec![t(1), t(2)]);
        add(&mut grammar, 105, vec![Symbol::Epsilon]);
        assert_eq!(aux_shape(&grammar, SymbolId(105)), AuxShape::Opaque);
        add(&mut grammar, 106, vec![nt(106)]);
        add(&mut grammar, 106, vec![Symbol::Epsilon]);
        assert_eq!(aux_shape(&grammar, SymbolId(106)), AuxShape::Opaque);
        add(&mut grammar, 107, vec![t(1)]);
        assert_eq!(aux_shape(&grammar, SymbolId(107)), AuxShape::Opaque);
        assert_eq!(aux_shape(&grammar, SymbolId(999)), AuxShape::Opaque);
    }

    #[test]
    fn test_node_types_json_reflects_canonical_schema() {
        use crate::node_types::NodeTypesGenerator;

        let grammar = normalized_acceptance_grammar();
        let json = NodeTypesGenerator::new(&grammar)
            .generate()
            .expect("node types generate");
        let parsed: Vec<serde_json::Value> =
            serde_json::from_str(&json).expect("valid JSON output");
        let by_type: std::collections::HashMap<&str, &serde_json::Value> = parsed
            .iter()
            .map(|node| (node["type"].as_str().unwrap_or(""), node))
            .collect();

        // No anonymous helper becomes a public node type.
        for name in by_type.keys() {
            assert!(
                !name.starts_with("rule_"),
                "helper {name} must not appear as a public node type"
            );
        }

        // Repeat: optional + multiple + leaf target (was required/singular/helper).
        let items = &by_type["opts"]["fields"]["items"];
        assert_eq!(items["required"], false);
        assert_eq!(items["multiple"], true);
        assert_eq!(items["types"].as_array().unwrap().len(), 1);
        assert_eq!(items["types"][0]["type"], "word");

        // Choice: required (every alternative provides it) with both targets.
        let pick = &by_type["pick"]["fields"]["value"];
        assert_eq!(pick["required"], true);
        assert_eq!(pick["multiple"], false);
        let mut pick_types: Vec<&str> = pick["types"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["type"].as_str().unwrap())
            .collect();
        pick_types.sort();
        assert_eq!(pick_types, vec!["num", "word"]);

        // Mandatory + optional in one alternative stays required and multiple.
        let pair = &by_type["pair"]["fields"]["value"];
        assert_eq!(pair["required"], true);
        assert_eq!(pair["multiple"], true);
        assert_eq!(pair["types"].as_array().unwrap().len(), 1);
        assert_eq!(pair["types"][0]["type"], "item");

        // Field on a sequence fans out to both member targets.
        let seqfield = &by_type["seqfield"]["fields"]["value"];
        assert_eq!(seqfield["required"], true);
        assert_eq!(seqfield["multiple"], true);
        let mut seq_types: Vec<&str> = seqfield["types"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["type"].as_str().unwrap())
            .collect();
        seq_types.sort();
        assert_eq!(seq_types, vec!["num", "word"]);
    }
}
