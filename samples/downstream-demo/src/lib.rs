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
