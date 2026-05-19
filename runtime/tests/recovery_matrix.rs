//! Product recovery matrix for generated parsers, documents, JSON, and
//! Tree-sitter-compatible selected-tree error facts.

#![cfg(all(
    test,
    feature = "pure-rust",
    feature = "glr",
    feature = "serialization",
    feature = "ts-compat"
))]

use adze::{
    document::{ADZE_DOCUMENT_JSON_SCHEMA, PointRange},
    parser::Parser as CoreParser,
    ts_compat::Tree,
};
use std::{ops::Range, sync::Arc};

struct GeneratedRecoveryCase {
    label: &'static str,
    source: &'static str,
    byte_span: Range<usize>,
    expected_token: &'static str,
}

struct ObjectLikeRecoveryCase {
    label: &'static str,
    source: &'static str,
    byte_span: Range<usize>,
    expected_token: &'static str,
}

fn generated_cases() -> Vec<GeneratedRecoveryCase> {
    vec![
        GeneratedRecoveryCase {
            label: "unexpected EOF",
            source: "1 +",
            byte_span: 3..3,
            expected_token: r"/\d+/",
        },
        GeneratedRecoveryCase {
            label: "unexpected EOF after trailing newline",
            source: "1 +\n",
            byte_span: 4..4,
            expected_token: r"/\d+/",
        },
        GeneratedRecoveryCase {
            label: "invalid ASCII token",
            source: "1 + @",
            byte_span: 4..5,
            expected_token: r"/\d+/",
        },
        GeneratedRecoveryCase {
            label: "invalid UTF-8 scalar",
            source: "1 + \u{03bb}",
            byte_span: 4..6,
            expected_token: r"/\d+/",
        },
        GeneratedRecoveryCase {
            label: "multiline invalid token",
            source: "1 +\n@",
            byte_span: 4..5,
            expected_token: r"/\d+/",
        },
    ]
}

fn object_like_cases() -> Vec<ObjectLikeRecoveryCase> {
    vec![
        ObjectLikeRecoveryCase {
            label: "missing colon before value",
            source: "{ name 1 }",
            byte_span: 7..8,
            expected_token: ":",
        },
        ObjectLikeRecoveryCase {
            label: "multibyte invalid identifier continuation before colon",
            source: "{ nam\u{00e9}: 1 }",
            byte_span: 5..7,
            expected_token: ":",
        },
        ObjectLikeRecoveryCase {
            label: "multiline invalid value after colon",
            source: "{\n name: nope\n}",
            byte_span: 9..10,
            expected_token: r"/\d+/",
        },
        ObjectLikeRecoveryCase {
            label: "multiline unexpected EOF after entry",
            source: "{\n name: 1\n",
            byte_span: 11..11,
            expected_token: "}",
        },
    ]
}

#[test]
fn generated_bad_input_matrix_preserves_document_diagnostics_and_json() {
    use adze_example::typed_ast_contract::grammar;

    for case in generated_cases() {
        let parse_errors = match grammar::parse(case.source) {
            Ok(_) => panic!("{} should fail typed AST extraction", case.label),
            Err(errors) => errors,
        };
        let parse_error = parse_errors
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("{} should produce a parse error", case.label));
        let document = grammar::parse_document(case.source).unwrap_or_else(|err| {
            panic!("{} should return partial parse facts: {err:?}", case.label)
        });
        let diagnostic = document
            .diagnostics()
            .first()
            .unwrap_or_else(|| panic!("{} should produce a document diagnostic", case.label));
        let json = document.to_json_value();
        let json_diagnostic = json["diagnostics"]
            .as_array()
            .and_then(|diagnostics| diagnostics.first())
            .unwrap_or_else(|| panic!("{} should serialize a JSON diagnostic", case.label));

        assert_eq!(
            parse_error.byte_span(),
            case.byte_span,
            "{} should keep its generated parser byte-span contract",
            case.label
        );
        assert_eq!(
            diagnostic.byte_span(),
            parse_error.byte_span(),
            "{} document diagnostic should agree with typed parser error",
            case.label
        );
        assert_eq!(
            diagnostic.point_range,
            PointRange::from_byte_range(case.source, diagnostic.byte_span()),
            "{} diagnostic point range should be derived from the byte span",
            case.label
        );
        assert_eq!(
            diagnostic.expected, parse_error.expected,
            "{} document diagnostic should preserve expected tokens",
            case.label
        );
        assert!(
            diagnostic
                .expected
                .iter()
                .any(|token| token == case.expected_token),
            "{} should include the generated expected-token name: {:?}",
            case.label,
            diagnostic.expected
        );
        assert!(
            document.metadata().error_count > 0,
            "{} document metadata should record parser recovery",
            case.label
        );
        assert!(
            document.tree().has_errors(),
            "{} tree should carry errors",
            case.label
        );
        assert_eq!(json["schema"].as_str(), Some(ADZE_DOCUMENT_JSON_SCHEMA));
        assert_eq!(
            json["metadata"]["error_count"].as_u64(),
            Some(document.metadata().error_count as u64),
            "{} JSON metadata should preserve error count",
            case.label
        );
        assert_eq!(
            json_diagnostic["start_byte"].as_u64(),
            Some(diagnostic.start_byte as u64),
            "{} JSON diagnostic start byte should match native diagnostic",
            case.label
        );
        assert_eq!(
            json_diagnostic["end_byte"].as_u64(),
            Some(diagnostic.end_byte as u64),
            "{} JSON diagnostic end byte should match native diagnostic",
            case.label
        );
        assert!(
            json_diagnostic["expected"]
                .as_array()
                .is_some_and(|expected| expected
                    .iter()
                    .any(|token| token.as_str() == Some(case.expected_token))),
            "{} JSON diagnostic should serialize expected-token names: {json_diagnostic:?}",
            case.label
        );
    }
}

#[test]
fn generated_object_like_bad_input_matrix_preserves_document_diagnostics_and_json() {
    use adze_example::object_like_contract::grammar;

    for case in object_like_cases() {
        let parse_errors = match grammar::parse(case.source) {
            Ok(ast) => panic!("{} unexpectedly parsed as {ast:?}", case.label),
            Err(errors) => errors,
        };
        let parse_error = parse_errors
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("{} should produce a parse error", case.label));
        let document = grammar::parse_document(case.source).unwrap_or_else(|err| {
            panic!(
                "{} should return partial object-like parse facts: {err:?}",
                case.label
            )
        });
        let diagnostic = document.diagnostics().first().unwrap_or_else(|| {
            panic!(
                "{} should produce an object-like document diagnostic",
                case.label
            )
        });
        let json = document.to_json_value();
        let json_diagnostic = json["diagnostics"]
            .as_array()
            .and_then(|diagnostics| diagnostics.first())
            .unwrap_or_else(|| {
                panic!(
                    "{} should serialize an object-like JSON diagnostic",
                    case.label
                )
            });

        assert_eq!(
            parse_error.byte_span(),
            case.byte_span,
            "{} should keep its object-like generated parser byte-span contract",
            case.label
        );
        assert_eq!(
            diagnostic.byte_span(),
            parse_error.byte_span(),
            "{} object-like document diagnostic should agree with typed parser error",
            case.label
        );
        assert_eq!(
            diagnostic.point_range,
            PointRange::from_byte_range(case.source, diagnostic.byte_span()),
            "{} object-like diagnostic point range should be derived from the byte span",
            case.label
        );
        assert_eq!(
            diagnostic.expected, parse_error.expected,
            "{} object-like document diagnostic should preserve expected tokens",
            case.label
        );
        assert!(
            diagnostic
                .expected
                .iter()
                .any(|token| token == case.expected_token),
            "{} should include the object-like expected-token name: {:?}",
            case.label,
            diagnostic.expected
        );
        assert!(
            document.metadata().error_count > 0,
            "{} object-like document metadata should record parser recovery",
            case.label
        );
        assert!(
            document.tree().has_errors(),
            "{} object-like tree should carry errors",
            case.label
        );
        assert_eq!(json["schema"].as_str(), Some(ADZE_DOCUMENT_JSON_SCHEMA));
        assert_eq!(
            json["metadata"]["error_count"].as_u64(),
            Some(document.metadata().error_count as u64),
            "{} object-like JSON metadata should preserve error count",
            case.label
        );
        assert_eq!(
            json_diagnostic["start_byte"].as_u64(),
            Some(diagnostic.start_byte as u64),
            "{} object-like JSON diagnostic start byte should match native diagnostic",
            case.label
        );
        assert_eq!(
            json_diagnostic["end_byte"].as_u64(),
            Some(diagnostic.end_byte as u64),
            "{} object-like JSON diagnostic end byte should match native diagnostic",
            case.label
        );
        assert!(
            json_diagnostic["expected"]
                .as_array()
                .is_some_and(|expected| expected
                    .iter()
                    .any(|token| token.as_str() == Some(case.expected_token))),
            "{} object-like JSON diagnostic should serialize expected-token names: {json_diagnostic:?}",
            case.label
        );
        assert!(
            diagnostic
                .message
                .contains(&format!("expected one of: {}", case.expected_token)),
            "{} object-like diagnostic message should name the expected token: {}",
            case.label,
            diagnostic.message
        );
        assert!(
            !diagnostic.message.contains("SymbolId") && !diagnostic.message.contains("symbol "),
            "{} object-like diagnostic message should not expose raw symbol internals: {}",
            case.label,
            diagnostic.message
        );
    }
}

#[test]
fn core_document_recovery_matrix_agrees_with_ts_compat_error_projection() {
    let lang = adze_example::ts_langs::arithmetic();
    let mut parser = CoreParser::new(lang.grammar.clone(), lang.table.clone(), lang.name.clone());
    let source = "1-@";
    let document = parser
        .parse_document(source)
        .expect("core parser should return partial parse facts for bad input");
    let diagnostic = document
        .diagnostics()
        .first()
        .expect("bad input should produce a native document diagnostic");
    let related_node = diagnostic
        .related_nodes
        .first()
        .and_then(|node_id| document.tree().node(*node_id))
        .expect("diagnostic should link to a recovered error node");
    let ts_tree = Tree::from_document(Arc::clone(&lang), &document);

    assert!(document.metadata().error_count > 0);
    assert!(document.tree().root().has_error());
    assert!(related_node.has_error());
    assert_eq!(related_node.byte_range(), diagnostic.byte_span());
    assert_eq!(ts_tree.error_count(), document.metadata().error_count);
    assert!(ts_tree.has_errors());
    assert_eq!(
        ts_tree.root_node().has_error(),
        document.tree().root().has_error()
    );
}

#[test]
fn glr_bad_input_matrix_returns_diagnostic_document_without_panicking() {
    use adze_example::ambiguous_expr::grammar;

    let source = "1 + @";
    let document = grammar::parse_document(source)
        .expect("GLR parse_document should return partial facts for bad input");
    let diagnostic = document
        .diagnostics()
        .first()
        .expect("GLR bad input should produce a document diagnostic");

    assert!(document.metadata().error_count > 0);
    assert!(document.tree().has_errors());
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
}
