//! Parse-table propagation tests for compiler-identity metadata (#862 PR4).

#![cfg(feature = "serialization")]

use adze_glr_core::{FirstFollowSets, ParseTable, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;

fn build_table(grammar: &adze_ir::Grammar) -> adze_glr_core::ParseTable {
    let first_follow = FirstFollowSets::compute(grammar).expect("first/follow");
    build_lr1_automaton(grammar, &first_follow).expect("build automaton")
}

#[test]
fn parse_table_start_symbol_matches_explicit_metadata() {
    let grammar = GrammarBuilder::new("root9")
        .token("x", "x")
        .rule("Root9", vec!["x"])
        .rule("source_file", vec!["x"])
        .start("Root9")
        .build();

    let root_id = grammar.explicit_start_symbol().expect("explicit start");
    let table = build_table(&grammar);

    assert_eq!(table.start_symbol(), root_id);
    assert_eq!(table.grammar.explicit_start_symbol(), Some(root_id));
}

#[test]
fn parse_table_serialization_roundtrip_preserves_compiler_identity() {
    let grammar = GrammarBuilder::new("wrappers")
        .token("identifier", r"[a-z]+")
        .rule("Identifier", vec!["identifier"])
        .wrapper_token("Identifier", "identifier")
        .start("Identifier")
        .build();

    let wrapper_id = grammar.find_symbol_by_name("Identifier").unwrap();
    let token_id = grammar.wrapper_token_for(wrapper_id).unwrap();
    let table = build_table(&grammar);

    let bytes = table.to_bytes().expect("serialize table");
    let restored = ParseTable::from_bytes(&bytes).expect("deserialize table");

    assert_eq!(restored.start_symbol(), table.start_symbol());
    assert_eq!(
        restored.grammar.explicit_start_symbol(),
        table.grammar.explicit_start_symbol()
    );
    assert_eq!(
        restored.grammar.wrapper_token_for(wrapper_id),
        Some(token_id)
    );
}
