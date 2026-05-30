//! Runnable diagnostics and recovery example.
//!
//! This example keeps diagnostics on the `AdzeDocument` path. It shows typed
//! parser errors, generated-parser bad input, multibyte spans, GLR bad input,
//! and the experimental document JSON projection without claiming stable JSON
//! schema or frozen diagnostic wording.

use adze::document::{ADZE_DOCUMENT_JSON_SCHEMA, PointRange};

fn main() {
    generated_bad_input();
    multibyte_bad_input();
    glr_bad_input();
}

fn generated_bad_input() {
    use adze_example::typed_ast_contract::grammar;

    let source = "1 +";
    let parse_error = grammar::parse(source)
        .expect_err("recoverable EOF should fail typed AST extraction")
        .into_iter()
        .next()
        .expect("typed parser should return a structured parse error");
    let document =
        grammar::parse_document(source).expect("recoverable EOF should produce document facts");
    let diagnostic = document
        .diagnostics()
        .first()
        .expect("EOF should produce a diagnostic");
    assert!(document.tree().has_errors());
    assert!(document.metadata().error_count > 0);
    assert_eq!(parse_error.byte_span(), 3..3);
    assert_eq!(diagnostic.byte_span(), 3..3);
    assert_eq!(diagnostic.byte_span(), parse_error.byte_span());
    assert_eq!(diagnostic.expected, parse_error.expected);
    assert_eq!(
        diagnostic.point_range,
        PointRange::from_byte_range(source, diagnostic.byte_span())
    );
    assert!(
        diagnostic.expected.iter().any(|token| token == r"/\d+/"),
        "diagnostic should preserve generated expected-token names: {:?}",
        diagnostic.expected
    );

    let json = document.to_json_value();
    let json_diagnostic = &json["diagnostics"][0];
    assert_eq!(json["schema"].as_str(), Some(ADZE_DOCUMENT_JSON_SCHEMA));
    assert_eq!(
        json["metadata"]["error_count"].as_u64(),
        Some(document.metadata().error_count as u64)
    );
    assert_eq!(json_diagnostic["start_byte"].as_u64(), Some(3));
    assert_eq!(json_diagnostic["end_byte"].as_u64(), Some(3));
    assert!(
        json_diagnostic["expected"]
            .as_array()
            .is_some_and(|expected| expected
                .iter()
                .any(|token| token.as_str() == Some(r"/\d+/"))),
        "JSON diagnostic should preserve public expected-token names: {json_diagnostic:?}"
    );

    println!("generated bad input:");
    println!("{}", diagnostic.display_with_source(source));
    println!(
        "  json diagnostic bytes: {}..{}",
        json["diagnostics"][0]["start_byte"], json["diagnostics"][0]["end_byte"]
    );
}

fn multibyte_bad_input() {
    use adze_example::typed_ast_contract::grammar;

    let source = "1 + \u{03bb}";
    let document = grammar::parse_document(source)
        .expect("recoverable multibyte bad token should produce document facts");
    let diagnostic = document
        .diagnostics()
        .first()
        .expect("multibyte bad token should produce a diagnostic");
    assert!(document.tree().has_errors());
    assert_eq!(diagnostic.byte_span(), 4..6);
    assert_eq!(
        diagnostic.point_range,
        PointRange::from_byte_range(source, diagnostic.byte_span())
    );
    assert_eq!(
        document.source_slice(diagnostic.byte_span()),
        Some("\u{03bb}"),
        "diagnostic span should stay aligned to UTF-8 scalar boundaries"
    );

    println!("multibyte bad input:");
    println!("{}", diagnostic.display_with_source(source));
}

fn glr_bad_input() {
    use adze_example::ambiguous_expr::grammar;

    let source = "1 + @";
    let document =
        grammar::parse_document(source).expect("GLR bad input should produce document facts");
    let diagnostic = document
        .diagnostics()
        .first()
        .expect("GLR bad input should produce a diagnostic");
    assert!(document.metadata().error_count > 0);
    assert!(document.tree().has_errors());
    assert!(document.ambiguities().is_empty());
    assert_eq!(diagnostic.byte_span(), 4..5);
    assert_eq!(
        diagnostic.point_range,
        PointRange::from_byte_range(source, diagnostic.byte_span())
    );
    assert!(
        diagnostic
            .expected
            .iter()
            .all(|token| !token.contains("SymbolId") && !token.contains("symbol ")),
        "GLR diagnostics should expose public token names: {:?}",
        diagnostic.expected
    );

    println!("GLR bad input:");
    println!("{}", diagnostic.display_with_source(source));
}
