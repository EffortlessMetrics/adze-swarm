//! #864: fallible leaf transforms must surface structured conversion
//! diagnostics instead of panicking or silently defaulting.

use adze::errors::{ParseError, ParseErrorReason};

fn first_error(errors: &[ParseError]) -> &ParseError {
    errors
        .first()
        .expect("conversion failure should return at least one structured error")
}

#[test]
fn fallible_transform_valid_and_boundary_values_parse() {
    use adze_example::typed_ast_contract::grammar::Expr;

    assert_eq!(
        adze_example::typed_ast_contract::grammar::parse("42").expect("valid integer"),
        Expr::Number(42)
    );
    assert_eq!(
        adze_example::typed_ast_contract::grammar::parse("2147483647")
            .expect("i32::MAX must remain valid"),
        Expr::Number(i32::MAX)
    );
}

#[test]
fn fallible_transform_overflow_returns_structured_token_range() {
    let source = "1 + 2147483648";
    let errors = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("one past i32::MAX must be a conversion error, not a panic or default");

    let first = first_error(&errors);
    assert!(
        matches!(first.reason, ParseErrorReason::ConversionFailed(_)),
        "overflow should be a structured conversion failure: {first:?}"
    );
    assert_eq!(first.byte_span(), 4..14);
    let span = first.source_span(source.as_bytes());
    assert_eq!((span.start.line, span.start.column), (1, 5));
    assert_eq!((span.end.line, span.end.column), (1, 15));
    assert!(
        first.to_string().contains("too large"),
        "conversion diagnostic should preserve the parse failure text: {first}"
    );
}

#[test]
fn fallible_transform_very_long_digit_token_returns_structured_error() {
    let source = "999999999999999999999999999999999999";
    let errors = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("very long digit token must fail conversion without panicking");

    assert_eq!(first_error(&errors).byte_span(), 0..source.len());
}

#[test]
fn fallible_transform_document_ast_matches_parse_facts() {
    let source = "1 + 2147483648";
    let parse_errors = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("parse must report the conversion failure");
    let document = adze_example::typed_ast_contract::grammar::parse_document(source)
        .expect("document parse should retain syntax even when AST conversion fails");
    let ast_errors = document
        .ast::<adze_example::typed_ast_contract::grammar::Expr>()
        .expect_err("document AST projection must report the same conversion failure");

    let from_parse = first_error(&parse_errors);
    let from_document = first_error(&ast_errors);
    assert_eq!(from_parse.byte_span(), from_document.byte_span());
    assert_eq!(from_parse.to_string(), from_document.to_string());
}

#[test]
fn fallible_transform_struct_field_valid_utf8_label_parses() {
    use adze_example::fallible_transforms::Unit;
    use adze_example::fallible_transforms::grammar::Measurement;

    let parsed = adze_example::fallible_transforms::grammar::parse("λ 12 mm")
        .expect("valid measurement should parse");
    assert_eq!(
        parsed,
        Measurement {
            label: "λ".to_string(),
            value: 12,
            unit: Unit::Millimetres,
        }
    );
}

#[test]
fn fallible_transform_struct_field_overflow_reports_utf8_adjacent_token_range() {
    // λ occupies bytes 0..2, so the digit token starts at byte 3 even
    // though it is the fourth byte-column position in the source line.
    let source = "λ 2147483648 mm";
    let errors = adze_example::fallible_transforms::grammar::parse(source)
        .expect_err("struct-field numeric overflow must be structured");

    let first = first_error(&errors);
    assert!(
        matches!(first.reason, ParseErrorReason::ConversionFailed(_)),
        "overflow should be a conversion failure: {first:?}"
    );
    assert_eq!(first.byte_span(), 3..13);
    let span = first.source_span(source.as_bytes());
    assert_eq!((span.start.line, span.start.column), (1, 4));
    assert_eq!((span.end.line, span.end.column), (1, 14));
}

#[test]
fn fallible_transform_custom_conversion_error_text_is_preserved() {
    let source = "λ 12 zz";
    let errors = adze_example::fallible_transforms::grammar::parse(source)
        .expect_err("unknown unit must fail through the custom conversion");

    let first = first_error(&errors);
    let ParseErrorReason::ConversionFailed(message) = &first.reason else {
        panic!("custom failure should be a conversion failure: {first:?}");
    };
    assert!(
        message.contains("unknown unit `zz`"),
        "custom conversion text should be preserved: {message}"
    );
    assert_eq!(first.byte_span(), 6..8);
}

#[test]
fn fallible_transform_node_targeted_extraction_matches_parse_facts() {
    let source = "1 + 2147483648";
    let parse_errors = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("parse must report the conversion failure");
    let document = adze_example::typed_ast_contract::grammar::parse_document(source)
        .expect("document parse should retain syntax even when AST conversion fails");
    let root_id = document.tree().root().node_id();
    let node_errors = document
        .ast_from_node::<adze_example::typed_ast_contract::grammar::Expr>(root_id)
        .expect_err("node-targeted extraction must report the same conversion failure");

    let from_parse = first_error(&parse_errors);
    let from_node = first_error(&node_errors);
    assert_eq!(from_parse.byte_span(), from_node.byte_span());
    assert_eq!(from_parse.to_string(), from_node.to_string());
}

#[test]
fn with_try_leaf_missing_transform_is_structured_error() {
    use adze::{Extract, WithTryLeaf};

    let error = <WithTryLeaf<i32> as Extract<i32>>::try_extract(None, b"", 7, None)
        .expect_err("missing try_transform must be structured, not a panic");
    assert_eq!(error.byte_span(), 7..7);
    let ParseErrorReason::ConversionFailed(message) = &error.reason else {
        panic!("missing transform should be a conversion failure: {error:?}");
    };
    assert!(
        message.contains("no try_transform function provided"),
        "missing-transform message should name the cause: {message}"
    );
}

#[test]
fn with_try_leaf_long_conversion_message_is_bounded() {
    use adze::{Extract, WithTryLeaf};

    let leaf_fn = |_: &str| -> Result<i32, String> { Err("x".repeat(1000)) };
    let error = <WithTryLeaf<i32> as Extract<i32>>::try_extract(None, b"", 0, Some(&leaf_fn))
        .expect_err("conversion failure should be structured");
    let ParseErrorReason::ConversionFailed(message) = &error.reason else {
        panic!("long failure should be a conversion failure: {error:?}");
    };
    assert!(
        message.len() <= 256,
        "conversion message must be bounded, got {} bytes",
        message.len()
    );
    assert!(message.ends_with("..."), "truncation should be marked");
}

#[test]
fn fallible_transform_arithmetic_overflow_is_not_silently_zero() {
    let errors = adze_example::arithmetic::grammar::parse("2147483648")
        .expect_err("arithmetic overflow must not silently become Number(0)");

    assert_eq!(first_error(&errors).byte_span(), 0..10);
}

#[cfg(feature = "pure-rust")]
#[test]
fn with_try_leaf_invalid_present_node_bytes_fail_without_calling_converter() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use adze::{Extract, WithTryLeaf};
    use adze::errors::ParseErrorReason;

    // Node spans bytes 1..3, which hold 0xFF bytes that are never valid
    // UTF-8, so there is no decodable leaf text for a present node.
    let source = b"a\xff\xffz";
    let node = adze::pure_parser::ParsedNode::builder(1, vec![], 1, 3).build();

    // 'static counter: WithTryLeaf::LeafFn is `dyn Fn...` ('static).
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    CALLS.store(0, Ordering::SeqCst);
    // Empty-accepting converter: succeeds even on "". Before the fix,
    // undecodable bytes were coerced to "" and this converter succeeded.
    let leaf_fn = |_: &str| -> Result<String, String> {
        CALLS.fetch_add(1, Ordering::SeqCst);
        Ok(String::new())
    };

    let error = <WithTryLeaf<String> as Extract<String>>::try_extract(
        Some(&node),
        source,
        0,
        Some(&leaf_fn),
    )
    .expect_err("invalid present-node text must be a structured failure");
    assert_eq!(error.byte_span(), 1..3);
    assert!(
        matches!(error.reason, ParseErrorReason::ConversionFailed(_)),
        "invalid leaf text must be ConversionFailed, got: {error:?}"
    );
    assert_eq!(
        CALLS.load(Ordering::SeqCst),
        0,
        "converter must not be invoked for undecodable leaf text"
    );
}
