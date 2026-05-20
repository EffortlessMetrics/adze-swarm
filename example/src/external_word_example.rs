//! Example demonstrating external scanner and word token attributes

#[allow(dead_code)]
#[adze::grammar("python_like")]
mod grammar {
    #[adze::language]
    pub enum Statement {
        If(IfStatement),
        Expression(Expression),
    }

    pub struct IfStatement {
        #[adze::leaf(text = "if")]
        _if: (),
        condition: Expression,
        #[adze::leaf(text = ":")]
        _colon: (),
        body: Vec<Statement>,
    }

    pub enum Expression {
        Identifier(Identifier),
        Number(#[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())] i32),
    }

    // Word token - helps distinguish keywords from identifiers
    #[adze::word]
    pub struct Identifier {
        #[adze::leaf(pattern = r"[a-zA-Z_]\w*")]
        name: String,
    }

    // External scanner tokens for indentation-based parsing
    #[adze::external]
    pub struct Indent;

    #[adze::external]
    pub struct Dedent;

    #[adze::external]
    pub struct Newline;

    // Regular extras
    #[adze::extra]
    pub struct Whitespace {
        #[adze::leaf(pattern = r"[ \t]+")]
        _ws: (),
    }
}

// Note: In a real implementation, you would need to provide an external scanner
// implementation that handles the Indent/Dedent/Newline tokens based on
// indentation levels, similar to how Python parsers work.

#[cfg(test)]
mod tests {

    #[test]
    fn test_word_token() {
        // This test would work once the full parser is generated
        // The word token helps ensure "if" is parsed as a keyword, not an identifier
    }

    #[test]
    fn generated_external_grammar_bad_input_returns_diagnostic_document() {
        let source = "@";
        let document = super::grammar::parse_document(source)
            .expect("generated external-token grammar should return a diagnostic document");
        let diagnostic = document
            .diagnostics()
            .first()
            .expect("bad generated external-token input should produce a document diagnostic");

        assert!(document.metadata().error_count > 0);
        assert!(document.tree().has_errors());
        assert!(diagnostic.start_byte <= diagnostic.end_byte);
        assert!(diagnostic.end_byte <= source.len());
        assert_eq!(
            diagnostic.point_range,
            adze::document::PointRange::from_byte_range(source, diagnostic.byte_span())
        );
    }
}
