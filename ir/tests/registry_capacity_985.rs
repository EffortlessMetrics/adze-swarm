//! #985: `Grammar` registry construction fails atomically with a typed
//! capacity error instead of panicking or aliasing EOF.
//!
//! Red history: pre-fix, both tests below panicked at
//! `ir/src/symbol_registry.rs:64` (`attempt to add with overflow`) — the
//! unit-level witness `symbol_registry::tests::try_register_hands_out_
//! last_slot_then_fails_typed` and this file's over-domain case failed
//! identically before the checked `try_register`/`try_build_registry`
//! paths landed.

use adze_ir::{ExternalToken, Grammar, GrammarError, SymbolId};

fn external_grammar(count: usize) -> Grammar {
    let mut grammar = Grammar::new("registry_capacity_985".to_string());
    grammar.externals = (0..count)
        .map(|i| ExternalToken {
            name: format!("ext_{i}"),
            symbol_id: SymbolId(1),
        })
        .collect();
    grammar
}

#[test]
fn try_build_registry_rejects_exhaustion_atomically() {
    // 70_000 distinct external names exceed the SymbolId(u16) domain
    // (EOF + 65_535 further symbols).
    let grammar = external_grammar(70_000);
    let err = grammar
        .try_build_registry()
        .expect_err("over-domain registry build must fail as Err, not panic");
    assert!(
        matches!(err, GrammarError::SymbolSpaceExhausted),
        "expected SymbolSpaceExhausted, got {err:?}"
    );
    // Atomicity: the failure caches nothing on the grammar.
    assert!(
        grammar.symbol_registry.is_none(),
        "failed build must not cache a partial registry"
    );
}

#[test]
fn try_build_registry_succeeds_at_highest_supported_count() {
    // Falsifier 10 (#985): deterministic result at the exact highest
    // supported count — EOF plus 65_535 distinct names fills the domain.
    let grammar = external_grammar(65_535);
    let first = grammar
        .try_build_registry()
        .expect("exact-domain registry build must succeed");
    let second = grammar
        .try_build_registry()
        .expect("exact-domain registry build must succeed");
    assert_eq!(first, second, "identical valid input registers identically");
    assert_eq!(first.len(), 65_536);
    assert_eq!(first.get_id("end"), Some(SymbolId(0)));
    let mut seen = std::collections::HashSet::new();
    for (name, info) in first.iter() {
        if name != "end" {
            assert_ne!(info.id, SymbolId(0), "no symbol may alias EOF");
        }
        assert!(seen.insert(info.id), "ids must be distinct");
    }
}

#[test]
fn try_build_registry_small_grammar_ok() {
    let grammar = external_grammar(3);
    let registry = grammar
        .try_build_registry()
        .expect("small registry build must succeed");
    assert_eq!(registry.len(), 4); // end + 3 externals
    assert_eq!(registry.get_id("end"), Some(SymbolId(0)));
}
