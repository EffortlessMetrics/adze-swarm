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
