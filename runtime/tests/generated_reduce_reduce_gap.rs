//! Generated reduce/reduce product proof.
//!
//! The product GLR matrix has generated shift/reduce proof and hand-built
//! reduce/reduce driver proof. This canary proves generated reduce/reduce
//! wrapper variants keep distinct reductions through grammar expansion,
//! table generation, typed extraction, and document ambiguity summaries.

#![cfg(all(test, feature = "pure-rust", feature = "glr"))]

use adze::decoder;
use adze_glr_core::conflict_inspection::count_conflicts;

#[test]
fn generated_reduce_reduce_preserves_conflict_and_extracts_selected_tree() {
    let table = decoder::decode_parse_table(adze_example::reduce_reduce::grammar::language());
    let summary = count_conflicts(&table);

    assert!(
        summary.reduce_reduce > 0,
        "generated reduce/reduce cells should survive grammar expansion and table generation"
    );

    assert!(
        matches!(
            adze_example::reduce_reduce::grammar::parse("x")
                .expect("generated reduce/reduce selected tree should extract typed AST"),
            adze_example::reduce_reduce::grammar::Choice::FromA(_)
                | adze_example::reduce_reduce::grammar::Choice::FromB(_)
        ),
        "selected generated reduce/reduce tree should extract one deterministic typed AST"
    );

    let document = adze_example::reduce_reduce::grammar::parse_document("x")
        .expect("generated reduce/reduce parse_document should succeed");
    assert!(
        document.diagnostics().is_empty(),
        "valid generated reduce/reduce input should not emit diagnostics: {:?}",
        document.diagnostics()
    );
    assert!(
        !document.ambiguities().is_empty(),
        "generated reduce/reduce document should expose an ambiguity summary"
    );
}
