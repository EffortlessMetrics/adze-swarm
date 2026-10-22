//! #981: normalization must preserve the semantic referent of positional
//! field metadata when it flattens `Sequence` symbols. A field that named
//! source child `c` must still name `c` afterwards, not whichever symbol
//! happens to land on the old index.

use adze_ir::{FieldId, Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

fn token(name: &str) -> Token {
    Token {
        name: name.to_string(),
        pattern: TokenPattern::String(name.to_string()),
        fragile: false,
    }
}

fn rule(lhs: u16, rhs: Vec<Symbol>, fields: Vec<(FieldId, usize)>) -> Rule {
    Rule {
        lhs: SymbolId(lhs),
        rhs,
        precedence: None,
        associativity: None,
        fields,
        production_id: ProductionId(7),
    }
}

fn terminal(id: u16) -> Symbol {
    Symbol::Terminal(SymbolId(id))
}

/// Resolve every field on the single rule for `lhs` to the symbol IDs it
/// designates after normalization.
fn field_targets(grammar: &Grammar, lhs: u16) -> Vec<(FieldId, SymbolId)> {
    let rules = &grammar.rules[&SymbolId(lhs)];
    let rule = rules.first().expect("rule present");
    rule.fields
        .iter()
        .map(|(field_id, position)| {
            let symbol = &rule.rhs[*position];
            let id = match symbol {
                Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => *id,
                other => panic!("field target should be a leaf symbol, got {other:?}"),
            };
            (*field_id, id)
        })
        .collect()
}

fn base_grammar(rhs: Vec<Symbol>, fields: Vec<(FieldId, usize)>) -> Grammar {
    let mut grammar = Grammar::new("field_origins".to_string());
    grammar.start_symbol = Some(SymbolId(1));
    for (id, name) in [(2u16, "a"), (3, "b"), (4, "c"), (5, "d")] {
        grammar.tokens.insert(SymbolId(id), token(name));
    }
    grammar.fields.insert(FieldId(0), "chosen".to_string());
    grammar.fields.insert(FieldId(1), "other".to_string());
    grammar.add_rule(rule(1, rhs, fields));
    grammar
}

#[test]
fn normalize_field_after_sequence_still_targets_original_child() {
    // The audited #981 witness: field 0 names `c` at source position 1.
    let mut grammar = base_grammar(
        vec![
            Symbol::Sequence(vec![terminal(2), terminal(3)]),
            terminal(4),
        ],
        vec![(FieldId(0), 1)],
    );

    grammar.try_normalize().expect("normalization succeeds");

    assert_eq!(
        field_targets(&grammar, 1),
        vec![(FieldId(0), SymbolId(4))],
        "field must still designate c after flattening"
    );
    grammar.validate().expect("normalized grammar validates");
}

#[test]
fn normalize_field_after_two_sequences_shifts_cumulatively() {
    let mut grammar = base_grammar(
        vec![
            Symbol::Sequence(vec![terminal(2), terminal(3)]),
            Symbol::Sequence(vec![terminal(4)]),
            terminal(5),
        ],
        vec![(FieldId(0), 2)],
    );

    grammar.try_normalize().expect("normalization succeeds");

    assert_eq!(
        field_targets(&grammar, 1),
        vec![(FieldId(0), SymbolId(5))],
        "field must still designate d after two expansions"
    );
}

#[test]
fn normalize_field_after_nested_sequence_still_targets_original_child() {
    let mut grammar = base_grammar(
        vec![
            Symbol::Sequence(vec![
                Symbol::Sequence(vec![terminal(2), terminal(3)]),
                terminal(4),
            ]),
            terminal(5),
        ],
        vec![(FieldId(0), 1)],
    );

    grammar.try_normalize().expect("normalization succeeds");

    assert_eq!(
        field_targets(&grammar, 1),
        vec![(FieldId(0), SymbolId(5))],
        "field must still designate d after nested flattening"
    );
}

#[test]
fn normalize_field_after_empty_sequence_targets_next_child() {
    let mut grammar = base_grammar(
        vec![Symbol::Sequence(vec![]), terminal(2)],
        vec![(FieldId(0), 1)],
    );

    grammar.try_normalize().expect("normalization succeeds");

    assert_eq!(
        field_targets(&grammar, 1),
        vec![(FieldId(0), SymbolId(2))],
        "empty sequence must not leave the field on a wrong position"
    );
}

#[test]
fn normalize_fields_before_and_after_sequence_keep_referents() {
    let mut grammar = base_grammar(
        vec![
            terminal(2),
            Symbol::Sequence(vec![terminal(3), terminal(4)]),
            terminal(5),
        ],
        vec![(FieldId(0), 0), (FieldId(1), 2)],
    );

    grammar.try_normalize().expect("normalization succeeds");

    assert_eq!(
        field_targets(&grammar, 1),
        vec![(FieldId(0), SymbolId(2)), (FieldId(1), SymbolId(5))],
        "fields on both sides of the expansion keep their children"
    );
}

#[test]
fn normalize_field_on_multi_element_sequence_covers_each_child() {
    // The converter lowers `field(name, seq(a, b))` to a field positioned
    // on the sequence. After flattening, the field designates each child
    // of the sequence rather than silently picking one.
    let mut grammar = base_grammar(
        vec![Symbol::Sequence(vec![terminal(2), terminal(3)])],
        vec![(FieldId(0), 0)],
    );

    grammar.try_normalize().expect("normalization succeeds");

    assert_eq!(
        field_targets(&grammar, 1),
        vec![(FieldId(0), SymbolId(2)), (FieldId(0), SymbolId(3))],
        "field on a sequence covers every flattened child"
    );
}

#[test]
fn normalize_field_on_one_element_sequence_maps_to_the_element() {
    let mut grammar = base_grammar(
        vec![Symbol::Sequence(vec![terminal(2)]), terminal(3)],
        vec![(FieldId(0), 0)],
    );

    grammar.try_normalize().expect("normalization succeeds");

    assert_eq!(
        field_targets(&grammar, 1),
        vec![(FieldId(0), SymbolId(2))],
        "field on a one-element sequence designates that element"
    );
}

#[test]
fn try_normalize_field_on_empty_sequence_is_typed_error() {
    let mut grammar = base_grammar(vec![Symbol::Sequence(vec![])], vec![(FieldId(0), 0)]);

    let error = grammar
        .try_normalize()
        .expect_err("a field on an empty sequence has no child to designate");
    assert!(
        error.to_string().contains("empty sequence"),
        "typed error should name the empty-sequence cause: {error}"
    );
}

#[test]
fn normalize_field_mapping_is_idempotent() {
    let mut grammar = base_grammar(
        vec![
            Symbol::Sequence(vec![terminal(2), terminal(3)]),
            terminal(4),
        ],
        vec![(FieldId(0), 1)],
    );

    grammar.try_normalize().expect("first normalization");
    let after_first = field_targets(&grammar, 1);
    grammar.try_normalize().expect("second normalization");
    assert_eq!(field_targets(&grammar, 1), after_first);
    assert_eq!(after_first, vec![(FieldId(0), SymbolId(4))]);
}
