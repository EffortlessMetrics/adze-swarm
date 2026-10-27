//! Native GLR failure facts (#1030): a streaming-routed conflicted grammar
//! with a counted fallible (`try_transform`) leaf must surface the native
//! selected-tree conversion error exactly once.
//!
//! The fixture grammar (`adze_example::native_failure_facts_1030`) mirrors
//! `ambiguous_expr` so it keeps genuine shift/reduce conflicts and routes
//! through the stack-aware streaming driver; its only difference is a
//! counted `try_transform` number leaf. The single test below proves, in
//! clean -> failure -> clean order:
//!
//! 1. `parse()` and document-AST projection each invoke extraction exactly
//!    once with an identical `ConversionFailed` message and byte span.
//! 2. The route observation proves the streaming driver executed.
//! 3. A clean parse after the failure still succeeds with a single
//!    conversion call (counter/reset hygiene).

#![cfg(all(feature = "pure-rust", feature = "glr", feature = "runtime-e2e"))]

use std::sync::atomic::Ordering;

use adze::errors::ParseErrorReason;
use adze::glr_streaming_runtime::{TrueGlrParseRoute, last_true_glr_parse_route};
use adze_example::native_failure_facts_1030::NATIVE_FAILURE_FACTS_1030_CALLS;
use adze_example::native_failure_facts_1030::grammar;
use adze_example::native_failure_facts_1030::grammar::Expr;

const OVERFLOW_SOURCE: &str = "2147483648";

fn reset_calls() {
    NATIVE_FAILURE_FACTS_1030_CALLS.store(0, Ordering::SeqCst);
}

fn calls() -> usize {
    NATIVE_FAILURE_FACTS_1030_CALLS.load(Ordering::SeqCst)
}

#[test]
fn native_failure_facts_1030_fixture_routes_through_streaming_driver() {
    let language = grammar::language();
    let table = adze::decoder::decode_parse_table(language);
    assert!(
        adze::glr_streaming_runtime::should_route_conflict_table_through_streaming_driver(
            language, &table
        ),
        "native_failure_facts_1030 must stay a streaming-routed conflicted grammar"
    );
}

#[test]
fn native_failure_facts_1030_single_extraction_with_identical_facts() {
    // Clean parse first: one number leaf, exactly one conversion call.
    reset_calls();
    let clean = grammar::parse("42").expect("clean input must parse");
    assert_eq!(clean, Expr::Number(42));
    assert_eq!(
        calls(),
        1,
        "clean parse() must invoke the fallible leaf exactly once"
    );

    // Failure through parse(): extraction must run exactly once, so the
    // native selected-tree ConversionFailed error surfaces unchanged.
    reset_calls();
    let parse_errors =
        grammar::parse(OVERFLOW_SOURCE).expect_err("overflowing digit run must fail conversion");
    assert_eq!(
        calls(),
        1,
        "failing parse() must invoke extraction exactly once (no LR-substitution reparse)"
    );
    assert_eq!(
        last_true_glr_parse_route(),
        Some(TrueGlrParseRoute::StreamingDriver),
        "failing parse() must execute the streaming driver"
    );
    let from_parse = parse_errors
        .first()
        .expect("conversion failure must yield at least one structured error");
    let ParseErrorReason::ConversionFailed(parse_message) = &from_parse.reason else {
        panic!("overflow must be a ConversionFailed error, got: {from_parse:?}");
    };
    assert_eq!(from_parse.byte_span(), 0..OVERFLOW_SOURCE.len());

    // Failure through the document pipeline: parse_document() retains syntax
    // without running typed extraction, then ast() converts exactly once.
    reset_calls();
    let document = grammar::parse_document(OVERFLOW_SOURCE)
        .expect("document parse must retain syntax when AST conversion fails");
    assert_eq!(
        calls(),
        0,
        "parse_document() must not run typed extraction while building the document"
    );
    let document_errors = document
        .ast::<Expr>()
        .expect_err("document AST projection must report the conversion failure");
    assert_eq!(
        calls(),
        1,
        "document ast() must invoke extraction exactly once"
    );
    let from_document = document_errors
        .first()
        .expect("document conversion failure must yield at least one structured error");
    assert_eq!(from_parse.byte_span(), from_document.byte_span());
    assert_eq!(from_parse.to_string(), from_document.to_string());
    assert!(
        from_document.to_string().contains(parse_message.as_str()),
        "document AST must preserve the parse() conversion text: {}",
        from_document.to_string()
    );

    // Clean parse after the failure: counter/reset hygiene, no stuck state.
    reset_calls();
    let clean_again = grammar::parse("42").expect("clean input must parse after a failure");
    assert_eq!(clean_again, Expr::Number(42));
    assert_eq!(
        calls(),
        1,
        "post-failure clean parse() must invoke the fallible leaf exactly once"
    );
}
