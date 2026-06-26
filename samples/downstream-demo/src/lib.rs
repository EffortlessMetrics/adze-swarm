#[adze::grammar("downstream_arithmetic")]
pub mod grammar {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Expr {
        Number(#[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())] i32),

        #[adze::prec_left(1)]
        Add(Box<Expr>, #[adze::leaf(text = "+")] (), Box<Expr>),

        #[adze::prec_left(2)]
        Mul(Box<Expr>, #[adze::leaf(text = "*")] (), Box<Expr>),
    }

    #[adze::extra]
    #[allow(dead_code)]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s+")]
        _ws: (),
    }
}

#[cfg(test)]
mod tests {
    use super::grammar::{self, Expr};

    #[test]
    fn downstream_demo_parses_typed_ast_with_precedence() {
        let expr = grammar::parse("1 + 2 * 3").expect("expression should parse");

        assert_eq!(
            expr,
            Expr::Add(
                Box::new(Expr::Number(1)),
                (),
                Box::new(Expr::Mul(
                    Box::new(Expr::Number(2)),
                    (),
                    Box::new(Expr::Number(3)),
                )),
            )
        );
    }

    #[test]
    fn downstream_demo_reports_expected_tokens_for_bad_input() {
        let source = "1 + @";
        let errors = grammar::parse(source).expect_err("bad input should fail clearly");
        let first = errors
            .first()
            .expect("parse should report at least one error");

        assert_eq!(first.byte_span(), 4..5);
        assert!(
            first.expected.iter().any(|name| name == r"/\d+/"),
            "expected token set should name the number token, got {:?}",
            first.expected
        );

        let rendered = first.display_with_source(source).to_string();
        assert!(rendered.contains("bytes 4..5"));
        assert!(rendered.contains("expected one of:"));
        assert!(rendered.contains("    ^"));
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::grammar;

    #[test]
    fn edge_empty_input() {
        let result = grammar::parse("");
        // Empty input should fail gracefully with a diagnostic, not panic
        assert!(result.is_err(), "empty input should fail, not panic");
        let errors = result.unwrap_err();
        assert!(!errors.is_empty(), "should have at least one error");
    }

    #[test]
    fn edge_only_whitespace() {
        let result = grammar::parse("   ");
        // Only whitespace should fail (no expression found)
        assert!(result.is_err(), "whitespace-only should fail");
    }

    #[test]
    fn edge_deeply_nested() {
        // Deeply nested expression
        let source = "1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10";
        let result = grammar::parse(source);
        assert!(result.is_ok(), "long expression should parse: {:?}", result.err());
    }

    #[test]
    fn edge_unexpected_eof_during_operator() {
        let source = "1 +";
        let errors = grammar::parse(source).expect_err("incomplete should fail");
        let rendered = errors[0].display_with_source(source).to_string();
        // The rendered error should mention what was expected
        assert!(
            rendered.contains("expected") || rendered.contains("EOF") || rendered.contains("end"),
            "error should be informative, got: {}",
            rendered
        );
    }
}

mod acceptance_tests {
    use super::grammar::{self, Expr};

    /// Happy path: valid input → typed AST with correct precedence
    #[test]
    fn acceptance_valid_input_returns_typed_ast() {
        let result = grammar::parse("1 + 2 * 3").expect("valid input should parse");
        assert_eq!(
            result,
            Expr::Add(
                Box::new(Expr::Number(1)),
                (),
                Box::new(Expr::Mul(
                    Box::new(Expr::Number(2)),
                    (),
                    Box::new(Expr::Number(3)),
                )),
            )
        );
    }

    /// Error path: bad input → clear diagnostics with token names and source rendering
    #[test]
    fn acceptance_bad_input_returns_clear_diagnostics() {
        let source = "1 + @";
        let errors = grammar::parse(source).expect_err("bad input should fail");
        let first = &errors[0];

        // Error points at the unexpected token
        assert_eq!(first.byte_span(), 4..5);

        // Expected tokens are human-readable (not opaque IDs)
        assert!(
            first.expected.iter().any(|name| name == r"/\d+/"),
            "expected token names, got {:?}",
            first.expected
        );

        // Source rendering produces a caret-pointer diagnostic
        let rendered = first.display_with_source(source).to_string();
        assert!(rendered.contains("expected one of:"), "rendered: {}", rendered);
    }

    /// parse_document path: returns AdzeDocument for tooling projections
    #[test]
    fn acceptance_parse_document_returns_canonical_document() {
        let doc = grammar::parse_document("1 + 2").expect("document should parse");

        // Document has diagnostics (empty for valid input)
        assert!(doc.diagnostics().is_empty(), "valid input should have no diagnostics");

        // Document can project to JSON (requires serialization feature)
        #[cfg(feature = "serialization")]
        {
            let json = doc.to_json_value();
            assert!(json.is_object(), "JSON projection should be an object");
        }

        // Document can project to typed AST
        let ast: Expr = doc.ast().expect("AST projection should succeed");
        assert_eq!(
            ast,
            Expr::Add(
                Box::new(Expr::Number(1)),
                (),
                Box::new(Expr::Number(2)),
            )
        );

        // Document preserves source text
        assert_eq!(doc.source_text(), "1 + 2");
    }

    /// parse_document on bad input: document with diagnostics, not an error
    #[test]
    fn acceptance_parse_document_on_bad_input_has_diagnostics() {
        let doc = grammar::parse_document("1 +").expect("document should still be created");

        // Document has diagnostics for the incomplete input
        assert!(
            !doc.diagnostics().is_empty(),
            "bad input should produce diagnostics in the document"
        );
    }

    /// Whitespace is an extra (ignored), not part of the AST
    #[test]
    fn acceptance_whitespace_is_ignored() {
        let result = grammar::parse("  1   +   2  ").expect("should parse with whitespace");
        assert_eq!(
            result,
            Expr::Add(
                Box::new(Expr::Number(1)),
                (),
                Box::new(Expr::Number(2)),
            )
        );
    }

    /// UTF-8 input parses correctly
    #[test]
    fn acceptance_multiline_input_parses() {
        let source = "1\n+\n2";
        let result = grammar::parse(source).expect("multiline should parse");
        assert!(matches!(result, Expr::Add(_, _, _)));
    }
}

mod document_projection_tests {
    use super::grammar;
    use super::grammar::Expr;

    /// Test that parse_document returns a document with the correct tree structure
    #[test]
    fn document_tree_has_correct_node_count() {
        let doc = grammar::parse_document("1 + 2").expect("should parse");
        let tree = doc.tree();
        // Root + Number(1) + '+' + Number(2) = at least 4 nodes
        assert!(tree.node_count() >= 3, "tree should have at least 3 nodes, got {}", tree.node_count());
    }

    /// Test that the document preserves source text correctly
    #[test]
    fn document_preserves_source_text() {
        let source = "1 + 2 * 3";
        let doc = grammar::parse_document(source).expect("should parse");
        assert_eq!(doc.source_text(), source);
    }

    /// Test that source_slice extracts substrings by byte range
    #[test]
    fn document_source_slice_works() {
        let doc = grammar::parse_document("1 + 2").expect("should parse");
        // "1" is bytes 0..1
        assert_eq!(doc.source_slice(0..1), Some("1"));
        // "+" is bytes 2..3
        assert_eq!(doc.source_slice(2..3), Some("+"));
    }

    /// Test that AST projection from document matches typed parse
    #[test]
    fn document_ast_projection_matches_typed_parse() {
        let source = "1 + 2 * 3";
        let typed: Expr = grammar::parse(source).expect("typed parse should work");
        let doc = grammar::parse_document(source).expect("document parse should work");
        let from_doc: Expr = doc.ast().expect("AST projection should work");
        assert_eq!(typed, from_doc, "typed parse and document AST projection should agree");
    }

    /// Test that diagnostics are accessible and have correct structure
    #[test]
    fn document_diagnostics_have_structure() {
        let doc = grammar::parse_document("1 +").expect("document should be created even for bad input");
        let diags = doc.diagnostics();
        assert!(!diags.is_empty(), "bad input should have diagnostics");
        let first = &diags[0];
        // Diagnostic should have a byte range
        assert!(first.start_byte <= first.end_byte, "diagnostic range should be valid");
    }
}
