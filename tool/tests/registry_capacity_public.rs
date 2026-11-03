//! Public fallible builder boundary for registry-capacity refusal.
use adze_ir::{ExternalToken, Grammar, GrammarError, SymbolId};
use adze_tool::{BuildOptions, build_parser};

#[test]
fn test_public_builder_returns_registry_capacity_error_without_unwinding() {
    let mut grammar = Grammar::new("public_capacity".into());
    // Public imported IR may present more distinct names than the registry can
    // represent. The fallible builder must refuse before table construction.
    grammar.externals = (0..=u16::MAX)
        .map(|index| ExternalToken {
            name: format!("external_{index:05}"),
            symbol_id: SymbolId(index),
        })
        .collect();
    let output = tempfile::tempdir().unwrap();
    let result = build_parser(
        grammar,
        BuildOptions {
            out_dir: output.path().to_string_lossy().into_owned(),
            ..BuildOptions::default()
        },
    );
    let error = result.expect_err("EOF plus 65536 distinct imported names exceeds capacity");
    assert!(
        matches!(
            error.downcast_ref::<GrammarError>(),
            Some(GrammarError::SymbolSpaceExhausted)
        ),
        "expected typed registry capacity cause, got {error:#}"
    );
    assert_eq!(std::fs::read_dir(output.path()).unwrap().count(), 0);
}
