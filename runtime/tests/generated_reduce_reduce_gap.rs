//! Generated reduce/reduce gap receipt.
//!
//! The product GLR matrix has generated shift/reduce proof and hand-built
//! reduce/reduce driver proof. This canary keeps the generated reduce/reduce
//! gap explicit: the generated table currently resolves/collapses the
//! reduce/reduce cell, and typed extraction for the selected leaf-shaped tree
//! still panics instead of returning a structured parse error.

#![cfg(all(test, feature = "pure-rust", feature = "glr"))]

use adze::decoder;
use adze_glr_core::conflict_inspection::count_conflicts;

#[test]
fn generated_reduce_reduce_gap_is_explicit() {
    let table = decoder::decode_parse_table(adze_example::reduce_reduce::grammar::language());
    let summary = count_conflicts(&table);

    assert_eq!(
        summary.reduce_reduce, 0,
        "generated reduce/reduce cells are not currently preserved as product proof; \
         update this gap canary, support tiers, and product audit when they are"
    );

    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let parsed = std::panic::catch_unwind(|| adze_example::reduce_reduce::grammar::parse("x"));
    std::panic::set_hook(previous_hook);
    assert!(
        parsed.is_err(),
        "generated reduce/reduce typed extraction is expected to remain outside the \
         product contract until it returns a deterministic AST or structured error"
    );
}
