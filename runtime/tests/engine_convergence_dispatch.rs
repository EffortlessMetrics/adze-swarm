//! Engine convergence (#895/#896/#897 + #1030 remainder): one
//! table-derived dispatch consumed by `parse()` and `parse_document()`,
//! no cross-engine retry after a selected-engine failure, and native
//! GLR expected-token names in streaming diagnostics without any
//! LR observer reparse.

#![cfg(all(feature = "pure-rust", feature = "glr"))]

use std::sync::atomic::Ordering;

use adze::__private::{EngineDispatch, engine_dispatch_for_language};
use adze_example::native_failure_facts_1030::NATIVE_FAILURE_FACTS_1030_CALLS;
use adze_example::native_failure_facts_1030::grammar as conflicted;
use adze_example::typed_ast_contract::grammar as deterministic;

/// The fixture converter counter is process-global, so every test that
/// parses through the conflicted grammar serializes on this lock
/// (per-invocation isolation; no global route state is asserted).
static CONFLICTED_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn reset_calls() {
    NATIVE_FAILURE_FACTS_1030_CALLS.store(0, Ordering::SeqCst);
}
fn calls() -> usize {
    NATIVE_FAILURE_FACTS_1030_CALLS.load(Ordering::SeqCst)
}

/// Dispatch selection per table class, derived from the actual decoded
/// table (not the generated conflict hint).
#[test]
fn parser_selection_dispatch_follows_decoded_table_class() {
    assert_eq!(
        engine_dispatch_for_language(deterministic::language()),
        EngineDispatch::DeterministicLr,
        "precedence-resolved arithmetic table must select the deterministic LR engine"
    );
    assert_eq!(
        engine_dispatch_for_language(conflicted::language()),
        EngineDispatch::StreamingGlr,
        "genuinely ambiguous table must select the one streaming GLR engine"
    );
}

/// `parse()` and `parse_document()` consume the same decision: for each
/// table class they agree on success, and on failure they agree on the
/// first diagnostic span.
#[test]
fn parser_selection_parse_and_parse_document_agree() {
    let _guard = CONFLICTED_LOCK.lock().unwrap();
    // Deterministic class.
    assert!(deterministic::parse("1 + 2").is_ok());
    let doc = deterministic::parse_document("1 + 2").expect("document");
    assert!(doc.diagnostics().is_empty());
    let errors = deterministic::parse("1 +").expect_err("truncated must fail");
    let doc = deterministic::parse_document("1 +").expect("document keeps facts");
    assert_eq!(
        errors[0].start..errors[0].end,
        doc.diagnostics()[0].byte_span(),
        "deterministic parse/document must agree on first failure span"
    );

    // Streaming GLR class.
    assert!(conflicted::parse("7 + 2").is_ok());
    let doc = conflicted::parse_document("7 + 2").expect("document");
    assert!(doc.diagnostics().is_empty());
    let errors = conflicted::parse("7 +").expect_err("truncated must fail");
    let doc = conflicted::parse_document("7 +").expect("document keeps facts");
    assert!(!doc.diagnostics().is_empty());
    assert_eq!(
        errors[0].start..errors[0].end,
        doc.diagnostics()[0].byte_span(),
        "streaming parse/document must agree on first failure span: {errors:?} vs {:?}",
        doc.diagnostics()
    );
}

/// Streaming diagnostics carry native GLR expected-token names, and
/// obtaining them runs no converter and no second engine: the counted
/// leaf converter stays at zero on a syntax failure.
#[test]
fn parser_selection_streaming_expected_names_are_native() {
    let _guard = CONFLICTED_LOCK.lock().unwrap();
    reset_calls();
    let errors = conflicted::parse("7 +").expect_err("truncated streaming input must fail");
    assert_eq!(calls(), 0, "syntax failure must not run any converter");
    let first = &errors[0];
    assert!(
        !first.expected.is_empty(),
        "native GLR diagnostic must carry expected-token names, got {first:?}"
    );
    assert!(
        first
            .expected
            .iter()
            .all(|name| !name.contains("SymbolId") && !name.starts_with("symbol ")),
        "expected names must be generated token names, not raw ids: {:?}",
        first.expected
    );
    assert!(
        first.to_string().contains("expected"),
        "rendered native diagnostic must name expected tokens: {first}"
    );

    // Document diagnostics project the same native expected names.
    let doc = conflicted::parse_document("7 +").expect("document keeps facts");
    assert_eq!(
        doc.diagnostics()[0].expected,
        first.expected,
        "document must project the same native expected names as parse()"
    );
    assert_eq!(calls(), 0, "document parse must not run any converter");
}

/// A conversion failure on valid GLR syntax runs extraction exactly
/// once and preserves its exact facts; no second engine re-runs it.
#[test]
fn parser_selection_conversion_failure_runs_exactly_once() {
    let _guard = CONFLICTED_LOCK.lock().unwrap();
    reset_calls();
    let errors = conflicted::parse("2147483648").expect_err("overflow conversion must fail");
    assert_eq!(calls(), 1, "parse() conversion must run exactly once");
    assert!(
        errors[0]
            .to_string()
            .contains("number too large to fit in target type")
            || errors[0].to_string().contains("too large"),
        "exact converter message must be preserved, got: {}",
        errors[0]
    );

    let doc = conflicted::parse_document("2147483648").expect("document");
    reset_calls();
    let doc_errors = doc
        .ast::<conflicted::Expr>()
        .expect_err("document conversion must fail");
    assert_eq!(
        calls(),
        1,
        "document ast() conversion must run exactly once"
    );
    assert_eq!(errors[0].to_string(), doc_errors[0].to_string());
    assert_eq!(
        errors[0].start..errors[0].end,
        doc_errors[0].start..doc_errors[0].end
    );
}

/// Clean -> failure -> clean agrees with a fresh parse: no stale
/// route/error state is retained between invocations.
#[test]
fn parser_selection_clean_failure_clean_has_no_stale_state() {
    let _guard = CONFLICTED_LOCK.lock().unwrap();
    assert!(conflicted::parse("7 + 2").is_ok());
    assert!(conflicted::parse("7 +").is_err());
    assert!(conflicted::parse("7 + 2").is_ok());
    assert!(deterministic::parse("1 + 2").is_ok());
    assert!(deterministic::parse("1 +").is_err());
    assert!(deterministic::parse("1 + 2").is_ok());
}

/// Structural guard (#897/#1030): the generated typed/document routes
/// contain no LR observer reparse. The only `parse_string` calls left
/// in `__private.rs` belong to the selected deterministic engine
/// itself (`parse_with_pure_parser` / the LR document branch).
#[test]
fn parser_selection_no_lr_observer_in_private_routes() {
    let source = include_str!("../src/__private.rs");
    assert!(
        !source.contains("let mut lr_parser"),
        "diagnostic-only LR observer must stay removed (#1030 remainder)"
    );
    assert!(
        !source.contains("parse_with_glr"),
        "duplicate hint-driven typed GLR route must stay retired (#897)"
    );
    assert!(
        source.contains("engine_dispatch_for_table"),
        "the shared table-derived dispatch must remain the routing seam (#896)"
    );
}

/// Mid-input conflicted GLR failures (critic regression): the offending
/// token is away from byte 0 and no missing terminal is inserted at its
/// span, yet both surfaces must carry the native GLR expected-token set
/// derived from the driver failure state's ACTION row — never empty.
#[test]
fn parser_selection_mid_input_expected_names_are_native() {
    let _guard = CONFLICTED_LOCK.lock().unwrap();
    for (source, span) in [
        ("7 + )", 4..5),
        ("7 7", 2..3),
        ("7 + ,", 4..5),
        ("7,", 1..2),
    ] {
        let errors = conflicted::parse(source).expect_err("mid-input failure must fail parse()");
        let first = &errors[0];
        assert_eq!(
            first.start..first.end,
            span,
            "{source}: parse() failure span"
        );
        assert!(
            !first.expected.is_empty(),
            "{source}: parse() must carry native expected tokens, got {first:?}"
        );
        assert!(
            first
                .expected
                .iter()
                .all(|name| !name.contains("SymbolId") && !name.starts_with("symbol ")),
            "{source}: expected names must be generated token names: {:?}",
            first.expected
        );

        let doc = conflicted::parse_document(source).expect("document keeps facts");
        let diagnostic = &doc.diagnostics()[0];
        assert_eq!(
            diagnostic.byte_span(),
            span,
            "{source}: parse_document() failure span"
        );
        assert!(
            !diagnostic.expected.is_empty(),
            "{source}: parse_document() must carry native expected tokens, got {diagnostic:?}"
        );
        assert_eq!(
            diagnostic.expected, first.expected,
            "{source}: parse()/document must agree on the native expected set"
        );
    }
}
