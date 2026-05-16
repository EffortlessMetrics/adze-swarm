use adze::errors::ParseErrorReason;
use std::collections::HashSet;
use std::ops::Range;

#[test]
fn generated_typed_parser_bad_token_reports_source_span() {
    let source = "1 + @";
    let errors = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("invalid token must fail through the generated typed parser");

    let first = errors
        .first()
        .expect("generated parser should return at least one parse error");

    assert_eq!(
        first.byte_span(),
        4..5,
        "bad token span should point at the invalid `@` byte"
    );

    let span = first.source_span(source.as_bytes());
    assert_eq!(span.start.line, 1);
    assert_eq!(span.start.column, 5);
    assert_eq!(span.end.line, 1);
    assert_eq!(span.end.column, 6);

    assert!(
        matches!(first.reason, ParseErrorReason::UnexpectedToken(_)),
        "bad generated-parser input should report an unexpected token: {:?}",
        first
    );

    let rendered = first.display_with_source(source).to_string();
    assert!(
        rendered.contains("bytes 4..5"),
        "rendered diagnostic should include byte span: {rendered}"
    );
    assert!(
        rendered.contains(source),
        "rendered diagnostic should include source excerpt: {rendered}"
    );
    assert!(
        rendered.contains("    ^"),
        "rendered diagnostic should place a caret under the invalid token: {rendered}"
    );
}

#[test]
fn generated_typed_parser_multibyte_bad_token_reports_utf8_byte_span() {
    let source = "1 + λ";
    let errors = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("invalid multibyte token must fail through the generated typed parser");

    let first = errors
        .first()
        .expect("generated parser should return at least one parse error");

    assert_eq!(
        first.byte_span(),
        4..6,
        "bad token span should cover the full UTF-8 byte width"
    );

    let span = first.source_span(source.as_bytes());
    assert_eq!(span.start.line, 1);
    assert_eq!(span.start.column, 5);
    assert_eq!(span.end.line, 1);
    assert_eq!(span.end.column, 7);

    let rendered = first.display_with_source(source).to_string();
    assert!(
        rendered.contains("bytes 4..6"),
        "rendered diagnostic should include full UTF-8 byte span: {rendered}"
    );
}

#[test]
fn generated_typed_parser_unexpected_eof_reports_zero_width_source_span() {
    let source = "1 +";
    let errors = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("truncated expression must fail through the generated typed parser");

    let first = errors
        .first()
        .expect("generated parser should return at least one parse error");

    assert_eq!(
        first.byte_span(),
        source.len()..source.len(),
        "unexpected EOF should point at the end-of-input insertion point"
    );

    let span = first.source_span(source.as_bytes());
    assert_eq!(span.start.line, 1);
    assert_eq!(span.start.column, source.len() + 1);
    assert_eq!(span.end.line, 1);
    assert_eq!(span.end.column, source.len() + 1);

    let rendered = first.display_with_source(source).to_string();
    assert!(
        rendered.contains("bytes 3..3"),
        "rendered diagnostic should include zero-width byte span: {rendered}"
    );
    assert!(
        rendered.contains("   ^"),
        "rendered diagnostic should place a caret at EOF: {rendered}"
    );
}

#[test]
fn generated_typed_parser_unexpected_eof_after_newline_reports_file_boundary_location() {
    let source = "1 +\n";
    let errors = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("truncated expression at file boundary must fail");

    let first = errors
        .first()
        .expect("generated parser should return at least one parse error");

    assert_eq!(
        first.byte_span(),
        source.len()..source.len(),
        "unexpected EOF after a trailing newline should point at end-of-input"
    );

    let span = first.source_span(source.as_bytes());
    assert_eq!(span.start.line, 2);
    assert_eq!(span.start.column, 1);
    assert_eq!(span.end.line, 2);
    assert_eq!(span.end.column, 1);

    let rendered = first.display_with_source(source).to_string();
    assert!(
        rendered.contains("at 2:1 (bytes 4..4)"),
        "rendered diagnostic should include file-boundary line/column and byte span: {rendered}"
    );
}

#[test]
fn generated_typed_parser_unexpected_eof_lists_expected_tokens() {
    let source = "1 +";
    let errors = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("truncated expression must fail through the generated typed parser");

    let first = errors
        .first()
        .expect("generated parser should return at least one parse error");

    let ParseErrorReason::UnexpectedToken(message) = &first.reason else {
        panic!(
            "truncated generated-parser input should report an unexpected token: {:?}",
            first.reason
        );
    };

    assert!(
        message.contains("expected one of:"),
        "unexpected-token detail should include normalized expected tokens: {message}"
    );
    assert!(
        message.contains(r"/\d+/"),
        "expected-token detail should use generated token names, not raw ids: {message}"
    );
    assert!(
        !message.contains("SymbolId") && !message.contains("symbol ") && !message.contains("_4"),
        "expected-token detail should not expose raw symbol ids or extra-token internals: {message}"
    );

    let rendered = first.display_with_source(source).to_string();
    assert!(
        rendered.contains("expected one of:"),
        "rendered diagnostic should include expected-token context: {rendered}"
    );
    assert!(
        rendered.contains(r"/\d+/"),
        "rendered diagnostic should include the expected token name: {rendered}"
    );
}

#[test]
fn generated_typed_parser_multiline_bad_token_reports_line_column_and_excerpt() {
    let source = "1 +\n@";
    let errors = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("multiline invalid token must fail through the generated typed parser");

    let first = errors
        .first()
        .expect("generated parser should return at least one parse error");

    assert_eq!(
        first.byte_span(),
        4..5,
        "bad token span should point at the invalid token on the second line"
    );

    let span = first.source_span(source.as_bytes());
    assert_eq!(span.start.line, 2);
    assert_eq!(span.start.column, 1);
    assert_eq!(span.end.line, 2);
    assert_eq!(span.end.column, 2);

    let rendered = first.display_with_source(source).to_string();
    assert!(
        rendered.contains("at 2:1 (bytes 4..5)"),
        "rendered diagnostic should include second-line location and byte span: {rendered}"
    );
    assert!(
        rendered.contains("@\n^"),
        "rendered diagnostic should include the second source line and caret: {rendered}"
    );
}

/// Canary: the public diagnostic contract for generated typed-parser errors
/// should not change when the GLR feature is enabled for the runtime crate.
///
/// The product-proof lane runs this exact test under both `pure-rust` and
/// `pure-rust,glr`. Keeping fixed byte spans, line/column positions, expected
/// token names, and rendered byte ranges here gives us a narrow LR/GLR
/// feature-parity receipt without claiming full parse-error stabilization.
#[test]
fn generated_typed_parser_error_contract_is_feature_stable() {
    struct Case {
        label: &'static str,
        source: &'static str,
        byte_span: Range<usize>,
        start_line: usize,
        start_column: usize,
        end_line: usize,
        end_column: usize,
    }

    let cases = [
        Case {
            label: "unexpected EOF",
            source: "1 +",
            byte_span: 3..3,
            start_line: 1,
            start_column: 4,
            end_line: 1,
            end_column: 4,
        },
        Case {
            label: "invalid ASCII token",
            source: "1 + @",
            byte_span: 4..5,
            start_line: 1,
            start_column: 5,
            end_line: 1,
            end_column: 6,
        },
        Case {
            label: "invalid UTF-8 scalar",
            source: "1 + λ",
            byte_span: 4..6,
            start_line: 1,
            start_column: 5,
            end_line: 1,
            end_column: 7,
        },
    ];

    for case in cases {
        let errors = match adze_example::typed_ast_contract::grammar::parse(case.source) {
            Ok(ast) => panic!("{} unexpectedly parsed as {ast:?}", case.label),
            Err(errors) => errors,
        };

        let first = errors
            .first()
            .unwrap_or_else(|| panic!("{} should produce at least one parse error", case.label));

        assert_eq!(
            first.byte_span(),
            case.byte_span,
            "{} should keep its public byte-span contract",
            case.label
        );

        let span = first.source_span(case.source.as_bytes());
        assert_eq!(span.start.line, case.start_line, "{}", case.label);
        assert_eq!(span.start.column, case.start_column, "{}", case.label);
        assert_eq!(span.end.line, case.end_line, "{}", case.label);
        assert_eq!(span.end.column, case.end_column, "{}", case.label);

        assert!(
            !first.expected.is_empty(),
            "{} should expose structured expected-token names",
            case.label
        );
        assert!(
            first.expected.iter().any(|token| token == r"/\d+/"),
            "{} should keep the arithmetic digit token in expected names: {:?}",
            case.label,
            first.expected
        );
        for token in &first.expected {
            assert!(
                !token.contains("SymbolId")
                    && !token.contains("symbol ")
                    && !token.starts_with('_'),
                "{} should expose human-readable expected names, got {token}",
                case.label
            );
        }

        let rendered = first.display_with_source(case.source).to_string();
        let expected_byte_range = format!("bytes {}..{}", case.byte_span.start, case.byte_span.end);
        assert!(
            rendered.contains(&expected_byte_range),
            "{} should render byte range {expected_byte_range}: {rendered}",
            case.label
        );
    }
}

/// Canary: the public diagnostic contract should also hold for the generated
/// precedence arithmetic example, not only the smaller `typed_ast_contract`
/// grammar used by the stable README canary.
///
/// The product-proof lane runs this exact test under both `pure-rust` and
/// `pure-rust,glr`, giving us a second generated arithmetic grammar shape before
/// promoting structured parse errors beyond Stabilizing.
#[test]
fn generated_precedence_arithmetic_parser_error_contract_is_feature_stable() {
    struct Case {
        label: &'static str,
        source: &'static str,
        byte_span: Range<usize>,
        start_line: usize,
        start_column: usize,
        end_line: usize,
        end_column: usize,
    }

    let cases = [
        Case {
            label: "unexpected EOF after precedence operator",
            source: "1 -",
            byte_span: 3..3,
            start_line: 1,
            start_column: 4,
            end_line: 1,
            end_column: 4,
        },
        Case {
            label: "invalid ASCII token after precedence operator",
            source: "1 - @",
            byte_span: 4..5,
            start_line: 1,
            start_column: 5,
            end_line: 1,
            end_column: 6,
        },
        Case {
            label: "invalid UTF-8 scalar after precedence operator",
            source: "1 - λ",
            byte_span: 4..6,
            start_line: 1,
            start_column: 5,
            end_line: 1,
            end_column: 7,
        },
    ];

    for case in cases {
        let errors = match adze_example::arithmetic::grammar::parse(case.source) {
            Ok(ast) => panic!("{} unexpectedly parsed as {ast:?}", case.label),
            Err(errors) => errors,
        };

        let first = errors
            .first()
            .unwrap_or_else(|| panic!("{} should produce at least one parse error", case.label));

        assert_eq!(
            first.byte_span(),
            case.byte_span,
            "{} should keep its public byte-span contract",
            case.label
        );

        let span = first.source_span(case.source.as_bytes());
        assert_eq!(span.start.line, case.start_line, "{}", case.label);
        assert_eq!(span.start.column, case.start_column, "{}", case.label);
        assert_eq!(span.end.line, case.end_line, "{}", case.label);
        assert_eq!(span.end.column, case.end_column, "{}", case.label);

        assert!(
            !first.expected.is_empty(),
            "{} should expose structured expected-token names",
            case.label
        );
        assert!(
            first.expected.iter().any(|token| token == r"/\d+/"),
            "{} should keep the arithmetic digit token in expected names: {:?}",
            case.label,
            first.expected
        );
        for token in &first.expected {
            assert!(
                !token.contains("SymbolId")
                    && !token.contains("symbol ")
                    && !token.starts_with('_'),
                "{} should expose human-readable expected names, got {token}",
                case.label
            );
        }

        let rendered = first.display_with_source(case.source).to_string();
        let expected_byte_range = format!("bytes {}..{}", case.byte_span.start, case.byte_span.end);
        assert!(
            rendered.contains(&expected_byte_range),
            "{} should render byte range {expected_byte_range}: {rendered}",
            case.label
        );
    }
}

/// Canary: generated-parser diagnostics should stay useful for the fielded
/// precedence grammar that backs the typed CST and native document edge
/// canaries.
///
/// This catches regressions where precedence operator inlining or FIELD metadata
/// preservation keeps the successful parse path working but loses human-readable
/// expected-token diagnostics on bad input. The product-proof lane runs this
/// exact test under both `pure-rust` and `pure-rust,glr`.
#[test]
fn generated_fielded_precedence_parser_error_contract_is_feature_stable() {
    use adze_example::fielded_precedence_typed_cst_contract::grammar;

    grammar::parse("1+2*3").expect("fielded precedence grammar should accept valid input");

    struct Case {
        label: &'static str,
        source: &'static str,
        byte_span: Range<usize>,
        start_line: usize,
        start_column: usize,
        end_line: usize,
        end_column: usize,
        expected: &'static [&'static str],
    }

    let cases = [
        Case {
            label: "unexpected EOF after fielded add operator",
            source: "1+",
            byte_span: 2..2,
            start_line: 1,
            start_column: 3,
            end_line: 1,
            end_column: 3,
            expected: &[r"/\d+/"],
        },
        Case {
            label: "invalid ASCII token after fielded add operator",
            source: "1+@",
            byte_span: 2..3,
            start_line: 1,
            start_column: 3,
            end_line: 1,
            end_column: 4,
            expected: &[r"/\d+/"],
        },
        Case {
            label: "invalid UTF-8 scalar after fielded multiply operator",
            source: "1*λ",
            byte_span: 2..4,
            start_line: 1,
            start_column: 3,
            end_line: 1,
            end_column: 5,
            expected: &[r"/\d+/"],
        },
        Case {
            label: "multiline invalid token after fielded add operator",
            source: "1+\n@",
            byte_span: 3..4,
            start_line: 2,
            start_column: 1,
            end_line: 2,
            end_column: 2,
            expected: &[r"/\d+/"],
        },
    ];

    for case in cases {
        let errors = match grammar::parse(case.source) {
            Ok(ast) => panic!("{} unexpectedly parsed as {ast:?}", case.label),
            Err(errors) => errors,
        };

        let first = errors
            .first()
            .unwrap_or_else(|| panic!("{} should produce at least one parse error", case.label));

        assert_eq!(
            first.byte_span(),
            case.byte_span,
            "{} should keep its public byte-span contract",
            case.label
        );

        let span = first.source_span(case.source.as_bytes());
        assert_eq!(span.start.line, case.start_line, "{}", case.label);
        assert_eq!(span.start.column, case.start_column, "{}", case.label);
        assert_eq!(span.end.line, case.end_line, "{}", case.label);
        assert_eq!(span.end.column, case.end_column, "{}", case.label);

        assert_eq!(
            first.expected,
            case.expected
                .iter()
                .map(|token| (*token).to_string())
                .collect::<Vec<_>>(),
            "{} should expose human-readable expectations by name",
            case.label
        );
        for token in &first.expected {
            assert!(
                !token.contains("SymbolId")
                    && !token.contains("symbol ")
                    && !token.starts_with('_'),
                "{} should expose human-readable expected names, got {token}",
                case.label
            );
        }

        let rendered = first.display_with_source(case.source).to_string();
        let expected_byte_range = format!("bytes {}..{}", case.byte_span.start, case.byte_span.end);
        assert!(
            rendered.contains(&expected_byte_range),
            "{} should render byte range {expected_byte_range}: {rendered}",
            case.label
        );
        assert!(
            rendered.contains(&format!("expected one of: {}", case.expected.join(", "))),
            "{} should render the expected-token set: {rendered}",
            case.label
        );
    }
}

// ============================================================================
// Structured expected-token field tests
// ============================================================================

#[test]
fn generated_typed_parser_unexpected_eof_expected_field_is_populated() {
    let source = "1 +";
    let errors = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("truncated expression must fail through the generated typed parser");

    let first = errors
        .first()
        .expect("generated parser should return at least one parse error");

    // The structured `expected` field should contain meaningful token names
    assert!(
        !first.expected.is_empty(),
        "expected field should be populated for unexpected EOF, got: {:?}",
        first.expected
    );

    // Token names should be human-readable, not raw IDs
    for name in &first.expected {
        assert!(
            !name.contains("SymbolId"),
            "expected token names should not contain raw SymbolId, got: {name}"
        );
        assert!(
            !name.contains("symbol "),
            "expected token names should not contain 'symbol ' prefix, got: {name}"
        );
    }
}

#[test]
fn generated_typed_parser_unexpected_eof_expected_field_sorted_and_deduped() {
    let source = "1 +";
    let errors = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("truncated expression must fail through the generated typed parser");

    let first = errors
        .first()
        .expect("generated parser should return at least one parse error");

    // The expected list should be sorted
    let mut sorted = first.expected.clone();
    sorted.sort();
    assert_eq!(
        first.expected, sorted,
        "expected field should be sorted: {:?}",
        first.expected
    );

    // The expected list should be deduplicated
    let mut deduped = first.expected.clone();
    deduped.dedup();
    assert_eq!(
        first.expected.len(),
        deduped.len(),
        "expected field should not contain duplicates: {:?}",
        first.expected
    );
}

#[test]
fn generated_typed_parser_bad_token_expected_field_is_populated() {
    let source = "1 + @";
    let errors = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("invalid token must fail through the generated typed parser");

    let first = errors
        .first()
        .expect("generated parser should return at least one parse error");

    // Even for bad tokens, the expected field should be populated with what
    // the parser expected at that position.
    assert!(
        !first.expected.is_empty(),
        "expected field should be populated for bad token, got: {:?}",
        first.expected
    );

    // Token names should be human-readable
    for name in &first.expected {
        assert!(
            !name.contains("SymbolId"),
            "expected token names should not contain raw SymbolId, got: {name}"
        );
    }
}

#[test]
fn generated_typed_parser_expected_field_contains_digit_pattern() {
    let source = "1 +";
    let errors = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("truncated expression must fail through the generated typed parser");

    let first = errors
        .first()
        .expect("generated parser should return at least one parse error");

    // For the arithmetic grammar, EOF at this position should expect a number.
    assert!(
        first.expected.iter().any(|t| t == r"/\d+/"),
        "expected tokens should include a digit pattern for arithmetic expression: {:?}",
        first.expected
    );
}

#[test]
fn generated_typed_parser_bad_inputs_return_errors_without_panicking() {
    let cases = [
        ("empty input", ""),
        ("whitespace only", "   "),
        ("trailing operator", "1 +"),
        ("invalid ascii token", "1 + @"),
        ("invalid utf8 scalar", "1 + λ"),
        ("multiline invalid token", "1 +\n@"),
    ];

    for (label, source) in cases {
        let parsed =
            std::panic::catch_unwind(|| adze_example::typed_ast_contract::grammar::parse(source));

        let errors = match parsed {
            Ok(Err(errors)) => errors,
            Ok(Ok(ast)) => panic!("generated parser unexpectedly accepted {label}: {ast:?}"),
            Err(_) => panic!("generated parser panicked for {label}"),
        };

        assert!(
            !errors.is_empty(),
            "generated parser should return at least one structured error for {label}"
        );
    }
}

#[test]
fn generated_parser_multi_error_diagnostics_are_ordered() {
    struct Case {
        label: &'static str,
        source: &'static str,
        parse: fn(&str) -> Result<(), Vec<adze::errors::ParseError>>,
    }

    let cases = [
        Case {
            label: "typed AST contract repeated bad tokens",
            source: "1 + @ @",
            parse: |source| {
                adze_example::typed_ast_contract::grammar::parse(source)
                    .map(|_: adze_example::typed_ast_contract::grammar::Expr| ())
            },
        },
        Case {
            label: "fielded precedence repeated bad tokens",
            source: "1+@+@",
            parse: |source| {
                adze_example::fielded_precedence_typed_cst_contract::grammar::parse(source)
                    .map(|_: adze_example::fielded_precedence_typed_cst_contract::grammar::Expr| ())
            },
        },
        Case {
            label: "object-like repeated structural errors",
            source: "{ name 1\n other nope",
            parse: |source| {
                adze_example::object_like_contract::grammar::parse(source)
                    .map(|_: adze_example::object_like_contract::grammar::Object| ())
            },
        },
    ];

    for case in cases {
        let errors = match (case.parse)(case.source) {
            Ok(()) => panic!("{} should fail", case.label),
            Err(errors) => errors,
        };
        assert!(
            errors.len() > 1,
            "{} should exercise a multi-error diagnostic vector, got {:?}",
            case.label,
            errors
        );

        for pair in errors.windows(2) {
            let previous = &pair[0];
            let next = &pair[1];
            assert!(
                (previous.start, previous.end) <= (next.start, next.end),
                "{} diagnostics should be ordered by byte span, got {:?} before {:?}",
                case.label,
                previous.byte_span(),
                next.byte_span()
            );
        }
    }
}

#[test]
fn generated_parser_multi_error_diagnostics_are_not_duplicated() {
    struct Case {
        label: &'static str,
        source: &'static str,
        parse: fn(&str) -> Result<(), Vec<adze::errors::ParseError>>,
    }

    let cases = [
        Case {
            label: "typed AST contract repeated bad tokens",
            source: "@ + @",
            parse: |source| {
                adze_example::typed_ast_contract::grammar::parse(source)
                    .map(|_: adze_example::typed_ast_contract::grammar::Expr| ())
            },
        },
        Case {
            label: "CSV repeated delimiter errors",
            source: ", ,",
            parse: |source| {
                adze_example::csv_list::grammar::parse(source)
                    .map(|_: adze_example::csv_list::grammar::CsvList| ())
            },
        },
        Case {
            label: "object-like repeated structural errors",
            source: "{ name 1 bad }",
            parse: |source| {
                adze_example::object_like_contract::grammar::parse(source)
                    .map(|_: adze_example::object_like_contract::grammar::Object| ())
            },
        },
    ];

    for case in cases {
        let errors = match (case.parse)(case.source) {
            Ok(()) => panic!("{} should fail", case.label),
            Err(errors) => errors,
        };
        assert!(
            errors.len() > 1,
            "{} should exercise a multi-error diagnostic vector, got {:?}",
            case.label,
            errors
        );

        let mut seen = HashSet::new();
        for error in errors {
            let key = (
                error.start,
                error.end,
                format!("{:?}", error.reason),
                error.expected,
            );
            assert!(
                seen.insert(key),
                "{} should not report duplicate diagnostics at the same span with the same reason and expectations",
                case.label
            );
        }
    }
}

/// Canary: prove that generated parser errors expose structured expected-token
/// names (not opaque IDs) end-to-end.
#[test]
fn expected_token_sets_are_reported() {
    // Use a bare operator — the grammar expects a number first.
    let source = "+";
    let errors = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("bare operator must fail");

    let first = errors
        .first()
        .expect("should produce at least one parse error");

    // The `expected` vec must be non-empty and contain human-readable names.
    assert!(
        !first.expected.is_empty(),
        "expected field must be populated, got: {:?}",
        first.expected
    );

    // Every entry must be a readable token name, not an opaque internal ID.
    for name in &first.expected {
        assert!(
            !name.contains("SymbolId") && !name.contains('_'),
            "expected token should be a human-readable name, not an opaque ID: {name}"
        );
    }

    // For the arithmetic grammar, at least one expected token must reference
    // the digit pattern — the only terminal that can start an expression.
    assert!(
        first.expected.iter().any(|t| t.contains("d")),
        "expected tokens should include the digit pattern for the arithmetic grammar: {:?}",
        first.expected
    );
}

/// Canary: generated-parser diagnostic contracts should hold for at least one
/// non-arithmetic grammar shape before structured parse errors can graduate.
///
/// The `words` grammar gives us a small keyword/word grammar that is independent
/// of the arithmetic expression canaries above. The product-proof lane runs this
/// exact test under both `pure-rust` and `pure-rust,glr`.
#[test]
fn generated_words_parser_error_contract_is_feature_stable() {
    struct Case {
        label: &'static str,
        source: &'static str,
        byte_span: Range<usize>,
        start_line: usize,
        start_column: usize,
        end_line: usize,
        end_column: usize,
        expected: &'static [&'static str],
    }

    let cases = [
        Case {
            label: "unexpected EOF before keyword",
            source: "",
            byte_span: 0..0,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
            expected: &["if"],
        },
        Case {
            label: "invalid ASCII token before keyword",
            source: "123",
            byte_span: 0..1,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 2,
            expected: &["if"],
        },
        Case {
            label: "invalid UTF-8 scalar before keyword",
            source: "λ",
            byte_span: 0..2,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 3,
            expected: &["if"],
        },
        Case {
            label: "invalid uppercase word continuation",
            source: "if HELLO",
            byte_span: 3..4,
            start_line: 1,
            start_column: 4,
            end_line: 1,
            end_column: 5,
            expected: &[r"/[a-z_]+/"],
        },
    ];

    for case in cases {
        let errors = match adze_example::words::grammar::parse(case.source) {
            Ok(ast) => panic!("{} unexpectedly parsed as {ast:?}", case.label),
            Err(errors) => errors,
        };

        let first = errors
            .first()
            .unwrap_or_else(|| panic!("{} should produce at least one parse error", case.label));

        assert_eq!(
            first.byte_span(),
            case.byte_span,
            "{} should keep its public byte-span contract",
            case.label
        );

        let span = first.source_span(case.source.as_bytes());
        assert_eq!(span.start.line, case.start_line, "{}", case.label);
        assert_eq!(span.start.column, case.start_column, "{}", case.label);
        assert_eq!(span.end.line, case.end_line, "{}", case.label);
        assert_eq!(span.end.column, case.end_column, "{}", case.label);

        assert_eq!(
            first.expected,
            case.expected
                .iter()
                .map(|token| (*token).to_string())
                .collect::<Vec<_>>(),
            "{} should expose human-readable expectations by name",
            case.label
        );
        for token in &first.expected {
            assert!(
                !token.contains("SymbolId")
                    && !token.contains("symbol ")
                    && !token.starts_with('_'),
                "{} should expose human-readable expected names, got {token}",
                case.label
            );
        }

        let rendered = first.display_with_source(case.source).to_string();
        let expected_byte_range = format!("bytes {}..{}", case.byte_span.start, case.byte_span.end);
        assert!(
            rendered.contains(&expected_byte_range),
            "{} should render byte range {expected_byte_range}: {rendered}",
            case.label
        );
        assert!(
            rendered.contains(&format!("expected one of: {}", case.expected.join(", "))),
            "{} should render the expected-token set: {rendered}",
            case.label
        );
    }
}

/// Canary: generated-parser diagnostic contracts should hold for a delimited
/// non-empty repeated field, not only single-token and expression grammars.
///
/// The CSV grammar exercises bad inputs around delimiter placement and keeps
/// the expected identifier pattern human-readable through the generated parser.
/// The product-proof lane runs this exact test under both `pure-rust` and
/// `pure-rust,glr`.
#[test]
fn generated_csv_list_parser_error_contract_is_feature_stable() {
    const IDENT_PATTERN: &str = r"/[a-zA-Z_][a-zA-Z0-9_]*/";

    struct Case {
        label: &'static str,
        source: &'static str,
        byte_span: Range<usize>,
        start_line: usize,
        start_column: usize,
        end_line: usize,
        end_column: usize,
        expected: &'static [&'static str],
    }

    let cases = [
        Case {
            label: "unexpected EOF before first list item",
            source: "",
            byte_span: 0..0,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
            expected: &[IDENT_PATTERN],
        },
        Case {
            label: "leading delimiter before first list item",
            source: ", alpha",
            byte_span: 0..1,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 2,
            expected: &[IDENT_PATTERN],
        },
        Case {
            label: "invalid ASCII token before first list item",
            source: "123",
            byte_span: 0..1,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 2,
            expected: &[IDENT_PATTERN],
        },
        Case {
            label: "trailing delimiter after complete list",
            source: "alpha,",
            byte_span: 5..6,
            start_line: 1,
            start_column: 6,
            end_line: 1,
            end_column: 7,
            expected: &["end"],
        },
    ];

    for case in cases {
        let errors = match adze_example::csv_list::grammar::parse(case.source) {
            Ok(ast) => panic!("{} unexpectedly parsed as {ast:?}", case.label),
            Err(errors) => errors,
        };

        let first = errors
            .first()
            .unwrap_or_else(|| panic!("{} should produce at least one parse error", case.label));

        assert_eq!(
            first.byte_span(),
            case.byte_span,
            "{} should keep its public byte-span contract",
            case.label
        );

        let span = first.source_span(case.source.as_bytes());
        assert_eq!(span.start.line, case.start_line, "{}", case.label);
        assert_eq!(span.start.column, case.start_column, "{}", case.label);
        assert_eq!(span.end.line, case.end_line, "{}", case.label);
        assert_eq!(span.end.column, case.end_column, "{}", case.label);

        assert_eq!(
            first.expected,
            case.expected
                .iter()
                .map(|token| (*token).to_string())
                .collect::<Vec<_>>(),
            "{} should expose human-readable expectations by name",
            case.label
        );
        for token in &first.expected {
            assert!(
                !token.contains("SymbolId")
                    && !token.contains("symbol ")
                    && !token.starts_with('_'),
                "{} should expose human-readable expected names, got {token}",
                case.label
            );
        }

        let rendered = first.display_with_source(case.source).to_string();
        let expected_byte_range = format!("bytes {}..{}", case.byte_span.start, case.byte_span.end);
        assert!(
            rendered.contains(&expected_byte_range),
            "{} should render byte range {expected_byte_range}: {rendered}",
            case.label
        );
        assert!(
            rendered.contains(&format!("expected one of: {}", case.expected.join(", "))),
            "{} should render the expected-token set: {rendered}",
            case.label
        );
    }
}

/// Canary: generated-parser diagnostic contracts should hold for an object-like
/// grammar with braces, delimited entries, colon separators, and typed values.
///
/// This keeps structured parse errors honest for a common record/object shape
/// before promoting generated parse diagnostics beyond Stabilizing. The
/// product-proof lane runs this exact test under both `pure-rust` and
/// `pure-rust,glr`.
#[test]
fn generated_object_like_parser_error_contract_is_feature_stable() {
    adze_example::object_like_contract::grammar::parse("{ name: 42 }")
        .expect("object-like contract fixture should accept a valid object");

    struct Case {
        label: &'static str,
        source: &'static str,
        byte_span: Range<usize>,
        start_line: usize,
        start_column: usize,
        end_line: usize,
        end_column: usize,
        expected: &'static [&'static str],
    }

    let cases = [
        Case {
            label: "unexpected EOF before object opener",
            source: "",
            byte_span: 0..0,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
            expected: &["{"],
        },
        Case {
            label: "missing object opener before entry",
            source: "name: 1",
            byte_span: 0..1,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 2,
            expected: &["{"],
        },
        Case {
            label: "missing colon before value",
            source: "{ name 1 }",
            byte_span: 7..8,
            start_line: 1,
            start_column: 8,
            end_line: 1,
            end_column: 9,
            expected: &[":"],
        },
        Case {
            label: "multiline missing colon before value",
            source: "{\n name 1\n}",
            byte_span: 8..9,
            start_line: 2,
            start_column: 7,
            end_line: 2,
            end_column: 8,
            expected: &[":"],
        },
        Case {
            label: "invalid identifier continuation before colon",
            source: "{ name$: 1 }",
            byte_span: 6..7,
            start_line: 1,
            start_column: 7,
            end_line: 1,
            end_column: 8,
            expected: &[":"],
        },
        Case {
            label: "multibyte invalid identifier continuation before colon",
            source: "{ namé: 1 }",
            byte_span: 5..7,
            start_line: 1,
            start_column: 6,
            end_line: 1,
            end_column: 8,
            expected: &[":"],
        },
        Case {
            label: "invalid value after colon",
            source: "{ name: nope }",
            byte_span: 8..9,
            start_line: 1,
            start_column: 9,
            end_line: 1,
            end_column: 10,
            expected: &[r"/\d+/"],
        },
        Case {
            label: "multiline invalid value after colon",
            source: "{\n name: nope\n}",
            byte_span: 9..10,
            start_line: 2,
            start_column: 8,
            end_line: 2,
            end_column: 9,
            expected: &[r"/\d+/"],
        },
        Case {
            label: "multiline unexpected EOF after entry",
            source: "{\n name: 1\n",
            byte_span: 11..11,
            start_line: 3,
            start_column: 1,
            end_line: 3,
            end_column: 1,
            expected: &["}"],
        },
    ];

    for case in cases {
        let errors = match adze_example::object_like_contract::grammar::parse(case.source) {
            Ok(ast) => panic!("{} unexpectedly parsed as {ast:?}", case.label),
            Err(errors) => errors,
        };

        let first = errors
            .first()
            .unwrap_or_else(|| panic!("{} should produce at least one parse error", case.label));

        assert_eq!(
            first.byte_span(),
            case.byte_span,
            "{} should keep its public byte-span contract",
            case.label
        );

        let span = first.source_span(case.source.as_bytes());
        assert_eq!(span.start.line, case.start_line, "{}", case.label);
        assert_eq!(span.start.column, case.start_column, "{}", case.label);
        assert_eq!(span.end.line, case.end_line, "{}", case.label);
        assert_eq!(span.end.column, case.end_column, "{}", case.label);

        assert_eq!(
            first.expected,
            case.expected
                .iter()
                .map(|token| (*token).to_string())
                .collect::<Vec<_>>(),
            "{} should expose human-readable expectations by name",
            case.label
        );
        for token in &first.expected {
            assert!(
                !token.contains("SymbolId")
                    && !token.contains("symbol ")
                    && !token.starts_with('_'),
                "{} should expose human-readable expected names, got {token}",
                case.label
            );
        }

        let rendered = first.display_with_source(case.source).to_string();
        let expected_byte_range = format!("bytes {}..{}", case.byte_span.start, case.byte_span.end);
        assert!(
            rendered.contains(&expected_byte_range),
            "{} should render byte range {expected_byte_range}: {rendered}",
            case.label
        );
        assert!(
            rendered.contains(&format!("expected one of: {}", case.expected.join(", "))),
            "{} should render the expected-token set: {rendered}",
            case.label
        );
    }
}

#[test]
fn generated_object_like_parser_counts_mixed_ascii_multibyte_lines() {
    let source = "{\n namé: 1 }";
    let errors = adze_example::object_like_contract::grammar::parse(source)
        .expect_err("multibyte invalid identifier continuation on second line should fail");

    let first = errors
        .first()
        .expect("generated parser should return at least one parse error");

    assert_eq!(
        first.byte_span(),
        6..8,
        "diagnostic should cover the full UTF-8 byte width of the invalid scalar"
    );

    let span = first.source_span(source.as_bytes());
    assert_eq!(span.start.line, 2);
    assert_eq!(span.start.column, 5);
    assert_eq!(span.end.line, 2);
    assert_eq!(span.end.column, 7);

    let rendered = first.display_with_source(source).to_string();
    assert!(
        rendered.contains("at 2:5 (bytes 6..8)"),
        "rendered diagnostic should count ASCII and multibyte columns on the second line: {rendered}"
    );
    assert!(
        rendered.contains(" namé: 1 }\n    ^^"),
        "rendered diagnostic should mark the multibyte scalar on the second line: {rendered}"
    );
}
