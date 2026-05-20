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

    struct ExternalRecoveryCase {
        label: &'static str,
        source: &'static str,
    }

    #[test]
    fn generated_external_grammar_bad_input_matrix_returns_diagnostic_document() {
        let cases = [
            ExternalRecoveryCase {
                label: "empty source",
                source: "",
            },
            ExternalRecoveryCase {
                label: "whitespace-only source",
                source: "   ",
            },
            ExternalRecoveryCase {
                label: "keyword without condition",
                source: "if",
            },
            ExternalRecoveryCase {
                label: "invalid root token",
                source: "@",
            },
            ExternalRecoveryCase {
                label: "invalid expression after keyword",
                source: "if @:",
            },
            ExternalRecoveryCase {
                label: "missing colon after condition",
                source: "if 1",
            },
            ExternalRecoveryCase {
                label: "trailing invalid token after expression",
                source: "name @",
            },
            ExternalRecoveryCase {
                label: "multibyte invalid expression after keyword",
                source: "if \u{03bb}:",
            },
            ExternalRecoveryCase {
                label: "invalid body token after colon",
                source: "if 1: @",
            },
            ExternalRecoveryCase {
                label: "multibyte invalid body token after colon",
                source: "if 1: \u{03bb}",
            },
            ExternalRecoveryCase {
                label: "invalid body token after external newline boundary",
                source: "if 1:\n@",
            },
            ExternalRecoveryCase {
                label: "invalid body token after crlf external newline boundary",
                source: "if 1:\r\n@",
            },
            ExternalRecoveryCase {
                label: "invalid expression in nested body",
                source: "if 1: if @:",
            },
            ExternalRecoveryCase {
                label: "missing nested condition after external newline",
                source: "if 1:\nif",
            },
            ExternalRecoveryCase {
                label: "invalid nested expression after external newline",
                source: "if 1:\nif @:",
            },
            ExternalRecoveryCase {
                label: "missing nested colon after external newline",
                source: "if 1:\nif 2",
            },
            ExternalRecoveryCase {
                label: "trailing invalid token after nested body expression",
                source: "if 1:\n2 @",
            },
            ExternalRecoveryCase {
                label: "multibyte invalid nested body after crlf boundary",
                source: "if 1:\r\nif 2: \u{03bb}",
            },
        ];

        for case in cases {
            let parse_errors = match super::grammar::parse(case.source) {
                Ok(_) => panic!("{} unexpectedly parsed successfully", case.label),
                Err(errors) => errors,
            };
            let parse_error = parse_errors
                .first()
                .unwrap_or_else(|| panic!("{} should produce a parse error", case.label));
            let document = super::grammar::parse_document(case.source).unwrap_or_else(|err| {
                panic!(
                    "{} should return a generated external-token diagnostic document: {err:?}",
                    case.label
                )
            });
            let diagnostic = document.diagnostics().first().unwrap_or_else(|| {
                panic!(
                    "{} should produce a generated external-token document diagnostic",
                    case.label
                )
            });

            assert_eq!(
                diagnostic.byte_span(),
                parse_error.byte_span(),
                "{} document diagnostic should agree with generated parser error span",
                case.label
            );
            assert_eq!(
                diagnostic.expected, parse_error.expected,
                "{} document diagnostic should preserve expected-token names",
                case.label
            );
            assert!(
                document.metadata().error_count > 0,
                "{} should record parser recovery in document metadata",
                case.label
            );
            assert!(
                document.tree().has_errors(),
                "{} should retain error facts on the selected document tree",
                case.label
            );
            assert!(
                diagnostic.start_byte <= diagnostic.end_byte,
                "{} diagnostic byte span should be ordered",
                case.label
            );
            assert!(
                diagnostic.end_byte <= case.source.len(),
                "{} diagnostic byte span should stay within the source",
                case.label
            );
            assert_eq!(
                diagnostic.point_range,
                adze::document::PointRange::from_byte_range(case.source, diagnostic.byte_span()),
                "{} diagnostic point range should agree with its byte span",
                case.label
            );
            assert!(
                diagnostic
                    .expected
                    .iter()
                    .all(|token| !token.contains("SymbolId") && !token.contains("symbol ")),
                "{} should expose public expected-token names: {:?}",
                case.label,
                diagnostic.expected
            );
            assert!(
                !diagnostic.message.contains("SymbolId") && !diagnostic.message.contains("symbol "),
                "{} should expose a public diagnostic message: {}",
                case.label,
                diagnostic.message
            );

            let rendered = diagnostic.display_with_source(case.source).to_string();
            assert!(
                rendered.contains(&diagnostic.message),
                "{} source-rendered diagnostic should include the diagnostic message",
                case.label
            );
        }
    }
}
