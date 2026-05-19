//! Runnable diagnostics and recovery example.
//!
//! This example keeps diagnostics on the `AdzeDocument` path. It shows
//! generated-parser bad input, multibyte spans, GLR bad input, and the
//! experimental document JSON projection without claiming stable JSON schema
//! status.

use adze::document::ADZE_DOCUMENT_JSON_SCHEMA;

fn main() {
    generated_bad_input();
    multibyte_bad_input();
    glr_bad_input();
}

fn generated_bad_input() {
    use adze_example::typed_ast_contract::grammar;

    let source = "1 +";
    let document =
        grammar::parse_document(source).expect("recoverable EOF should produce document facts");
    let diagnostic = document
        .diagnostics()
        .first()
        .expect("EOF should produce a diagnostic");
    assert_eq!(diagnostic.byte_span(), 3..3);
    assert!(
        diagnostic.expected.iter().any(|token| token == r"/\d+/"),
        "diagnostic should preserve generated expected-token names: {:?}",
        diagnostic.expected
    );

    let json = document.to_json_value();
    assert_eq!(json["schema"].as_str(), Some(ADZE_DOCUMENT_JSON_SCHEMA));
    assert_eq!(json["diagnostics"][0]["start_byte"].as_u64(), Some(3));
    assert_eq!(json["diagnostics"][0]["end_byte"].as_u64(), Some(3));

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
    assert_eq!(diagnostic.byte_span(), 4..6);
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
    assert!(document.tree().has_errors());
    assert!(document.ambiguities().is_empty());
    assert_eq!(diagnostic.byte_span(), 4..5);

    println!("GLR bad input:");
    println!("{}", diagnostic.display_with_source(source));
}
