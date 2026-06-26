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
