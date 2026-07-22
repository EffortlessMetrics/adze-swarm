//! Comprehensive error message quality tests.
//!
//! These tests verify that error messages produced by public error types
//! are helpful, well-formatted, and consistent.

use adze::error_recovery::{
    ErrorNode, ErrorRecoveryConfig, ErrorRecoveryConfigBuilder, ErrorRecoveryState,
    RecoveryStrategy,
};
use adze::error_reporting::{ErrorReporter, ErrorReportingExt, ParseError as ReportingParseError};
use adze::errors::{ParseError, ParseErrorReason};
use adze::glr_validation::{ErrorKind, ErrorLocation, RelatedInfo, ValidationError};
use adze::{SpanError, SpanErrorReason};

// ============================================================================
// 1. Parse error messages include byte position
// ============================================================================

#[test]
fn parse_error_includes_byte_positions() {
    let err = ParseError {
        reason: ParseErrorReason::UnexpectedToken("@".to_string()),
        start: 10,
        end: 11,
        expected: vec![],
    };
    let dbg = format!("{:?}", err);
    assert!(dbg.contains("10"), "Debug should include start byte: {dbg}");
    assert!(dbg.contains("11"), "Debug should include end byte: {dbg}");
}

#[test]
fn parse_error_display_includes_reason_and_byte_span() {
    let err = ParseError {
        reason: ParseErrorReason::UnexpectedToken("@".to_string()),
        start: 10,
        end: 11,
        expected: vec![],
    };
    let display = format!("{err}");
    assert!(
        display.contains("unexpected token \"@\""),
        "Display should include reason: {display}"
    );
    assert!(
        display.contains("bytes 10..11"),
        "Display should include byte span: {display}"
    );
}

#[test]
fn parse_error_source_span_is_one_indexed() {
    let source = "let x\n  @bad";
    let err = ParseError {
        reason: ParseErrorReason::UnexpectedToken("@".to_string()),
        start: 8,
        end: 9,
        expected: vec![],
    };

    let span = err.source_span(source.as_bytes());

    assert_eq!(span.start.line, 2);
    assert_eq!(span.start.column, 3);
    assert_eq!(span.end.line, 2);
    assert_eq!(span.end.column, 4);
}

#[test]
fn parse_error_display_with_source_includes_line_column_and_excerpt() {
    let source = "let x\n  @bad";
    let err = ParseError {
        reason: ParseErrorReason::UnexpectedToken("@".to_string()),
        start: 8,
        end: 9,
        expected: vec![],
    };

    let display = format!("{}", err.display_with_source(source));

    assert!(
        display.contains("at 2:3"),
        "Display should include line:column: {display}"
    );
    assert!(
        display.contains("bytes 8..9"),
        "Display should include byte span: {display}"
    );
    assert!(
        display.contains("  @bad"),
        "Display should include source excerpt: {display}"
    );
    assert!(
        display.contains("  ^"),
        "Display should include caret marker: {display}"
    );
}

#[test]
fn parse_error_display_with_source_marks_zero_width_spans() {
    let source = "abc";
    let err = ParseError {
        reason: ParseErrorReason::MissingToken(";".to_string()),
        start: 3,
        end: 3,
        expected: vec![],
    };

    let display = format!("{}", err.display_with_source(source));

    assert!(
        display.contains("missing token \";\""),
        "Display should include missing token reason: {display}"
    );
    assert!(
        display.contains("   ^"),
        "Display should mark the insertion point: {display}"
    );
}

#[test]
fn reporting_parse_error_includes_position_in_display() {
    let err = ReportingParseError {
        line: 5,
        column: 12,
        unexpected_token: Some("@".to_string()),
        expected: vec!["number".to_string()],
        context: String::new(),
    };
    let display = format!("{err}");
    assert!(
        display.contains("5:12"),
        "Display should include line:column position: {display}"
    );
}

#[test]
fn span_error_includes_byte_positions() {
    let err = SpanError {
        span: (15, 20),
        source_len: 10,
        reason: SpanErrorReason::EndOutOfBounds,
    };
    let display = format!("{err}");
    assert!(display.contains("15"), "Should contain start: {display}");
    assert!(display.contains("20"), "Should contain end: {display}");
    assert!(
        display.contains("10"),
        "Should contain source_len: {display}"
    );
}

// ============================================================================
// 2. Parse error messages include the problematic input token
// ============================================================================

#[test]
fn unexpected_token_error_includes_token_text() {
    let err = ParseError {
        reason: ParseErrorReason::UnexpectedToken("foobar".to_string()),
        start: 0,
        end: 6,
        expected: vec![],
    };
    let dbg = format!("{:?}", err);
    assert!(
        dbg.contains("foobar"),
        "Debug should include token text: {dbg}"
    );
}

#[test]
fn reporting_error_displays_unexpected_token() {
    let err = ReportingParseError {
        line: 1,
        column: 1,
        unexpected_token: Some("???".to_string()),
        expected: vec![],
        context: String::new(),
    };
    let display = format!("{err}");
    assert!(
        display.contains("???"),
        "Display should include unexpected token: {display}"
    );
}

#[test]
fn reporting_error_displays_end_of_input_when_no_token() {
    let err = ReportingParseError {
        line: 1,
        column: 1,
        unexpected_token: None,
        expected: vec![],
        context: String::new(),
    };
    let display = format!("{err}");
    assert!(
        display.contains("end of input"),
        "Should mention end of input when no token: {display}"
    );
}

// ============================================================================
// 3. Parse error messages include expected tokens (if available)
// ============================================================================

#[test]
fn reporting_error_lists_expected_tokens() {
    let err = ReportingParseError {
        line: 1,
        column: 1,
        unexpected_token: Some("@".to_string()),
        expected: vec![
            "identifier".to_string(),
            "number".to_string(),
            "string".to_string(),
        ],
        context: String::new(),
    };
    let display = format!("{err}");
    assert!(
        display.contains("identifier"),
        "Should list expected token: {display}"
    );
    assert!(
        display.contains("number"),
        "Should list expected token: {display}"
    );
    assert!(
        display.contains("string"),
        "Should list expected token: {display}"
    );
    assert!(
        display.contains("expected"),
        "Should contain 'expected' keyword: {display}"
    );
}

#[test]
fn reporting_error_omits_expected_when_empty() {
    let err = ReportingParseError {
        line: 1,
        column: 1,
        unexpected_token: Some("@".to_string()),
        expected: vec![],
        context: String::new(),
    };
    let display = format!("{err}");
    assert!(
        !display.contains("expected one of"),
        "Should not show expected section when empty: {display}"
    );
}

#[test]
fn missing_token_error_names_the_token() {
    let err = ParseError {
        reason: ParseErrorReason::MissingToken("semicolon".to_string()),
        start: 5,
        end: 5,
        expected: vec![],
    };
    let dbg = format!("{:?}", err);
    assert!(
        dbg.contains("semicolon"),
        "Should name the missing token: {dbg}"
    );
}

// ============================================================================
// 4. Language set error message is informative
// ============================================================================

#[test]
fn validation_error_for_empty_grammar_is_informative() {
    let err = ValidationError {
        kind: ErrorKind::EmptyGrammar,
        message: "Grammar has no rules".to_string(),
        location: ErrorLocation {
            symbol: None,
            rule_index: None,
            position: None,
            description: "grammar definition".to_string(),
        },
        suggestion: Some("Add at least one production rule".to_string()),
        related: vec![],
    };
    let display = format!("{err}");
    assert!(
        display.contains("no rules"),
        "Should describe the problem: {display}"
    );
    assert!(
        display.contains("grammar definition"),
        "Should include location: {display}"
    );
}

#[test]
fn validation_error_includes_suggestion() {
    let err = ValidationError {
        kind: ErrorKind::UndefinedSymbol,
        message: "Symbol 'expr' is referenced but not defined".to_string(),
        location: ErrorLocation {
            symbol: Some(adze_ir::SymbolId(42)),
            rule_index: None,
            position: None,
            description: "rule reference".to_string(),
        },
        suggestion: Some("Define a rule for 'expr'".to_string()),
        related: vec![],
    };
    let display = format!("{err}");
    assert!(
        display.contains("Suggestion"),
        "Should show suggestion: {display}"
    );
    assert!(
        display.contains("Define a rule"),
        "Should include the suggestion text: {display}"
    );
}

// ============================================================================
// 5. Null language error message is clear
// ============================================================================

#[test]
fn validation_error_for_no_start_symbol_is_clear() {
    let err = ValidationError {
        kind: ErrorKind::NoStartSymbol,
        message: "Grammar has no start symbol defined".to_string(),
        location: ErrorLocation {
            symbol: None,
            rule_index: None,
            position: None,
            description: "grammar root".to_string(),
        },
        suggestion: Some("Set a start symbol with grammar.start_symbol".to_string()),
        related: vec![],
    };
    let display = format!("{err}");
    assert!(
        display.contains("no start symbol"),
        "Should be clear about missing start symbol: {display}"
    );
}

#[test]
fn span_error_for_empty_source_is_clear() {
    let err = SpanError {
        span: (0, 1),
        source_len: 0,
        reason: SpanErrorReason::EndOutOfBounds,
    };
    let display = format!("{err}");
    assert!(
        display.contains("source length (0)"),
        "Should clearly show source was empty: {display}"
    );
}

// ============================================================================
// 6. Error display trait implementation is human-readable
// ============================================================================

#[test]
fn span_error_display_is_human_readable() {
    let err = SpanError {
        span: (5, 3),
        source_len: 100,
        reason: SpanErrorReason::StartGreaterThanEnd,
    };
    let display = format!("{err}");
    // Should read like a sentence, not dump raw data
    assert!(
        display.contains("Invalid span"),
        "Should have human-readable prefix: {display}"
    );
    assert!(
        display.contains("start") && display.contains("end"),
        "Should explain the constraint: {display}"
    );
}

#[test]
fn reporting_error_display_reads_as_sentence() {
    let err = ReportingParseError {
        line: 3,
        column: 7,
        unexpected_token: Some("++".to_string()),
        expected: vec!["expression".to_string()],
        context: "after operator".to_string(),
    };
    let display = format!("{err}");
    assert!(
        display.starts_with("Parse error"),
        "Should start with 'Parse error': {display}"
    );
    assert!(
        display.contains("unexpected token"),
        "Should describe what was unexpected: {display}"
    );
}

#[test]
fn validation_error_display_structured_output() {
    let err = ValidationError {
        kind: ErrorKind::LeftRecursion,
        message: "Left recursion detected in rule 'expr'".to_string(),
        location: ErrorLocation {
            symbol: Some(adze_ir::SymbolId(10)),
            rule_index: Some(0),
            position: None,
            description: "rule 'expr' at index 0".to_string(),
        },
        suggestion: Some("Refactor using right recursion".to_string()),
        related: vec![RelatedInfo {
            location: "rule 'term'".to_string(),
            message: "also participates in the cycle".to_string(),
        }],
    };
    let display = format!("{err}");
    assert!(
        display.contains("Error:"),
        "Should have Error label: {display}"
    );
    assert!(
        display.contains("Location:"),
        "Should have Location label: {display}"
    );
    assert!(
        display.contains("Suggestion:"),
        "Should have Suggestion label: {display}"
    );
    assert!(
        display.contains("Related"),
        "Should have Related section: {display}"
    );
}

// ============================================================================
// 7. Error debug trait implementation includes all relevant info
// ============================================================================

#[test]
fn parse_error_debug_includes_reason_and_range() {
    let err = ParseError {
        reason: ParseErrorReason::UnexpectedToken("xyz".to_string()),
        start: 42,
        end: 45,
        expected: vec![],
    };
    let dbg = format!("{:?}", err);
    assert!(
        dbg.contains("UnexpectedToken"),
        "Debug should include reason variant: {dbg}"
    );
    assert!(
        dbg.contains("xyz"),
        "Debug should include token text: {dbg}"
    );
    assert!(dbg.contains("42"), "Debug should include start: {dbg}");
    assert!(dbg.contains("45"), "Debug should include end: {dbg}");
}

#[test]
fn span_error_debug_includes_all_fields() {
    let err = SpanError {
        span: (7, 3),
        source_len: 50,
        reason: SpanErrorReason::StartGreaterThanEnd,
    };
    let dbg = format!("{:?}", err);
    assert!(
        dbg.contains("StartGreaterThanEnd"),
        "Debug should include reason variant: {dbg}"
    );
    assert!(dbg.contains("7"), "Debug should include span start: {dbg}");
    assert!(dbg.contains("3"), "Debug should include span end: {dbg}");
    assert!(dbg.contains("50"), "Debug should include source_len: {dbg}");
}

#[test]
fn failed_node_debug_includes_inner_errors() {
    let inner = ParseError {
        reason: ParseErrorReason::UnexpectedToken("bad".to_string()),
        start: 2,
        end: 5,
        expected: vec![],
    };
    let outer = ParseError {
        reason: ParseErrorReason::FailedNode(vec![inner]),
        start: 0,
        end: 10,
        expected: vec![],
    };
    let dbg = format!("{:?}", outer);
    assert!(
        dbg.contains("FailedNode"),
        "Should include FailedNode variant: {dbg}"
    );
    assert!(
        dbg.contains("bad"),
        "Should include nested error details: {dbg}"
    );
    assert!(
        dbg.contains("UnexpectedToken"),
        "Should include nested reason: {dbg}"
    );
}

#[test]
fn error_node_debug_includes_recovery_strategy() {
    let node = ErrorNode {
        start_byte: 0,
        end_byte: 5,
        start_position: (0, 0),
        end_position: (0, 5),
        expected: vec![1, 2],
        actual: Some(99),
        recovery: RecoveryStrategy::TokenDeletion,
        skipped_tokens: vec![99],
    };
    let dbg = format!("{:?}", node);
    assert!(
        dbg.contains("TokenDeletion"),
        "Should include strategy: {dbg}"
    );
    assert!(dbg.contains("99"), "Should include actual token: {dbg}");
}

// ============================================================================
// 8. Error messages don't leak internal implementation details
// ============================================================================

#[test]
fn span_error_display_no_internal_details() {
    let err = SpanError {
        span: (5, 3),
        source_len: 100,
        reason: SpanErrorReason::StartGreaterThanEnd,
    };
    let display = format!("{err}");
    // Should not contain Rust-internal terms
    assert!(
        !display.contains("unsafe"),
        "Should not leak 'unsafe': {display}"
    );
    assert!(!display.contains("ptr"), "Should not leak 'ptr': {display}");
    assert!(
        !display.contains("alloc"),
        "Should not leak 'alloc': {display}"
    );
    assert!(
        !display.contains("0x"),
        "Should not leak memory addresses: {display}"
    );
}

#[test]
fn reporting_error_display_no_internal_details() {
    let err = ReportingParseError {
        line: 1,
        column: 1,
        unexpected_token: Some("x".to_string()),
        expected: vec!["number".to_string()],
        context: "in expression".to_string(),
    };
    let display = format!("{err}");
    assert!(
        !display.contains("stack"),
        "Should not leak parser internals: {display}"
    );
    assert!(
        !display.contains("state"),
        "Should not leak parser state: {display}"
    );
    assert!(
        !display.contains("SymbolId"),
        "Should not leak SymbolId type: {display}"
    );
}

#[test]
fn validation_error_display_no_raw_ids() {
    let err = ValidationError {
        kind: ErrorKind::UndefinedSymbol,
        message: "Symbol 'foo' is not defined".to_string(),
        location: ErrorLocation {
            symbol: Some(adze_ir::SymbolId(999)),
            rule_index: None,
            position: None,
            description: "in rule 'bar'".to_string(),
        },
        suggestion: None,
        related: vec![],
    };
    let display = format!("{err}");
    // The Display should use the message, not dump raw SymbolId(999)
    assert!(
        !display.contains("SymbolId"),
        "Should not leak SymbolId in Display: {display}"
    );
}

// ============================================================================
// 9. Error messages for unicode input include correct positions
// ============================================================================

#[test]
fn reporting_error_with_unicode_position() {
    let mut reporter = ErrorReporter::new("héllo wörld".to_string());
    // Track character-based position through Unicode text
    reporter.record_token("héllo", 0);
    reporter.record_token(" ", 6);
    reporter.record_token("wörld", 7);
    // Positions should advance by character count
    let err = reporter.error_at_current(
        // We can't create a GLRParser easily, so check the reporter state directly
        &make_dummy_parser(),
        Some("!".to_string()),
    );
    // The reporter tracks column based on chars, not bytes
    assert!(
        err.line == 1,
        "Unicode input should stay on line 1: {}",
        err.line
    );
    assert!(
        err.column > 1,
        "Column should advance past unicode text: {}",
        err.column
    );
}

#[test]
fn parse_error_byte_positions_for_unicode() {
    // "ñ" is 2 bytes in UTF-8. Verify byte positions are used correctly.
    let err = ParseError {
        reason: ParseErrorReason::UnexpectedToken("ñ".to_string()),
        start: 5,
        end: 7, // 2-byte character
        expected: vec![],
    };
    let dbg = format!("{:?}", err);
    assert!(dbg.contains("5"), "Start byte position present: {dbg}");
    assert!(dbg.contains("7"), "End byte position present: {dbg}");
    assert!(
        err.end - err.start == 2,
        "Byte range should cover 2-byte char"
    );
}

// ============================================================================
// 10. Error messages for multi-line input include line/column
// ============================================================================

#[test]
fn reporting_error_tracks_line_across_newlines() {
    let mut reporter = ErrorReporter::new("line1\nline2\nline3".to_string());
    reporter.record_token("line1", 0);
    reporter.record_token("\n", 5);
    reporter.record_token("line2", 6);
    reporter.record_token("\n", 11);
    reporter.record_token("line3", 12);

    let err = reporter.error_at_current(&make_dummy_parser(), Some("!".to_string()));
    assert_eq!(err.line, 3, "Should be on line 3 after two newlines");
    assert!(err.column > 1, "Column should advance on third line");
}

#[test]
fn reporting_error_resets_column_after_newline() {
    let mut reporter = ErrorReporter::new("ab\ncd".to_string());
    reporter.record_token("ab", 0);
    reporter.record_token("\n", 2);
    reporter.record_token("c", 3);

    let err = reporter.error_at_current(&make_dummy_parser(), None);
    assert_eq!(err.line, 2, "Should be on line 2");
    assert_eq!(err.column, 2, "Column should be 2 after 'c' on line 2");
}

#[test]
fn reporting_error_display_shows_line_column() {
    let err = ReportingParseError {
        line: 10,
        column: 25,
        unexpected_token: Some("?".to_string()),
        expected: vec![],
        context: String::new(),
    };
    let display = format!("{err}");
    assert!(
        display.contains("10") && display.contains("25"),
        "Display should include both line and column: {display}"
    );
}

// ============================================================================
// 11. Multiple errors in same input produce distinct messages
// ============================================================================

#[test]
fn multiple_parse_errors_are_distinct() {
    let errors = [
        ParseError {
            reason: ParseErrorReason::UnexpectedToken("@".to_string()),
            start: 0,
            end: 1,
            expected: vec![],
        },
        ParseError {
            reason: ParseErrorReason::MissingToken(";".to_string()),
            start: 5,
            end: 5,
            expected: vec![],
        },
        ParseError {
            reason: ParseErrorReason::UnexpectedToken("#".to_string()),
            start: 10,
            end: 11,
            expected: vec![],
        },
    ];

    let messages: Vec<String> = errors.iter().map(|e| format!("{:?}", e)).collect();
    // Each message should be unique
    for i in 0..messages.len() {
        for j in (i + 1)..messages.len() {
            assert_ne!(
                messages[i], messages[j],
                "Error messages should be distinct"
            );
        }
    }
}

#[test]
fn multiple_reporting_errors_at_different_positions() {
    let e1 = ReportingParseError {
        line: 1,
        column: 5,
        unexpected_token: Some("@".to_string()),
        expected: vec!["id".to_string()],
        context: String::new(),
    };
    let e2 = ReportingParseError {
        line: 3,
        column: 10,
        unexpected_token: Some("$".to_string()),
        expected: vec!["number".to_string()],
        context: String::new(),
    };
    let d1 = format!("{e1}");
    let d2 = format!("{e2}");
    assert_ne!(d1, d2, "Errors at different positions should differ");
    assert!(d1.contains("1:5"), "First error position: {d1}");
    assert!(d2.contains("3:10"), "Second error position: {d2}");
}

// ============================================================================
// 12. Error recovery info in error messages is useful
// ============================================================================

#[test]
fn error_node_carries_expected_and_actual() {
    let node = ErrorNode {
        start_byte: 10,
        end_byte: 15,
        start_position: (2, 3),
        end_position: (2, 8),
        expected: vec![1, 2, 3],
        actual: Some(99),
        recovery: RecoveryStrategy::PanicMode,
        skipped_tokens: vec![99, 100],
    };
    let dbg = format!("{:?}", node);
    assert!(dbg.contains("expected"), "Should include expected: {dbg}");
    assert!(dbg.contains("actual"), "Should include actual: {dbg}");
    assert!(dbg.contains("PanicMode"), "Should include strategy: {dbg}");
}

#[test]
fn recovery_strategy_debug_is_descriptive() {
    let strategies = [
        (RecoveryStrategy::PanicMode, "PanicMode"),
        (RecoveryStrategy::TokenInsertion, "TokenInsertion"),
        (RecoveryStrategy::TokenDeletion, "TokenDeletion"),
        (RecoveryStrategy::TokenSubstitution, "TokenSubstitution"),
        (RecoveryStrategy::PhraseLevel, "PhraseLevel"),
        (RecoveryStrategy::ScopeRecovery, "ScopeRecovery"),
    ];
    for (strategy, expected_name) in strategies {
        let dbg = format!("{:?}", strategy);
        assert_eq!(dbg, expected_name, "Strategy debug should match name");
    }
}

#[test]
fn error_nodes_collected_include_byte_ranges() {
    let config = ErrorRecoveryConfig::default();
    let mut state = ErrorRecoveryState::new(config);
    state.record_error(
        10,
        20,
        (1, 10),
        (1, 20),
        vec![1, 2],
        Some(99),
        RecoveryStrategy::TokenDeletion,
        vec![99],
    );
    let nodes = state.get_error_nodes();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].start_byte, 10);
    assert_eq!(nodes[0].end_byte, 20);
}

// ============================================================================
// 13. Error message formatting is consistent across error types
// ============================================================================

#[test]
fn span_error_all_variants_have_consistent_format() {
    let errors = vec![
        SpanError {
            span: (5, 3),
            source_len: 10,
            reason: SpanErrorReason::StartGreaterThanEnd,
        },
        SpanError {
            span: (15, 20),
            source_len: 10,
            reason: SpanErrorReason::StartOutOfBounds,
        },
        SpanError {
            span: (0, 20),
            source_len: 10,
            reason: SpanErrorReason::EndOutOfBounds,
        },
    ];
    for err in &errors {
        let display = format!("{err}");
        assert!(
            display.starts_with("Invalid span"),
            "All span errors should start with 'Invalid span': {display}"
        );
        // All should mention the span range
        let (start, end) = err.span;
        assert!(
            display.contains(&format!("{start}..{end}")),
            "Should contain span range: {display}"
        );
    }
}

#[test]
fn reporting_errors_with_and_without_context_consistent() {
    let with_ctx = ReportingParseError {
        line: 1,
        column: 1,
        unexpected_token: Some("x".to_string()),
        expected: vec![],
        context: "in function body".to_string(),
    };
    let without_ctx = ReportingParseError {
        line: 1,
        column: 1,
        unexpected_token: Some("x".to_string()),
        expected: vec![],
        context: String::new(),
    };
    let d1 = format!("{with_ctx}");
    let d2 = format!("{without_ctx}");
    // Both should have the common prefix
    assert!(d1.starts_with("Parse error at 1:1"));
    assert!(d2.starts_with("Parse error at 1:1"));
    // Only the one with context should contain the context
    assert!(d1.contains("in function body"));
    assert!(!d2.contains("in function body"));
}

#[test]
fn validation_errors_consistent_structure() {
    let err1 = ValidationError {
        kind: ErrorKind::EmptyGrammar,
        message: "No rules".to_string(),
        location: ErrorLocation {
            symbol: None,
            rule_index: None,
            position: None,
            description: "root".to_string(),
        },
        suggestion: None,
        related: vec![],
    };
    let err2 = ValidationError {
        kind: ErrorKind::LeftRecursion,
        message: "Cycle detected".to_string(),
        location: ErrorLocation {
            symbol: None,
            rule_index: None,
            position: None,
            description: "rule 'expr'".to_string(),
        },
        suggestion: Some("Use iteration".to_string()),
        related: vec![],
    };
    let d1 = format!("{err1}");
    let d2 = format!("{err2}");
    // Both should start with "Error:"
    assert!(
        d1.contains("Error:"),
        "Validation errors should contain 'Error:': {d1}"
    );
    assert!(
        d2.contains("Error:"),
        "Validation errors should contain 'Error:': {d2}"
    );
    // Both should have Location:
    assert!(d1.contains("Location:"), "Should have Location: {d1}");
    assert!(d2.contains("Location:"), "Should have Location: {d2}");
}

// ============================================================================
// 14. Error source chain is correct for wrapped errors
// ============================================================================

#[test]
fn span_error_implements_std_error() {
    let err = SpanError {
        span: (5, 3),
        source_len: 10,
        reason: SpanErrorReason::StartGreaterThanEnd,
    };
    // SpanError implements std::error::Error
    let dyn_err: &dyn std::error::Error = &err;
    assert!(
        dyn_err.source().is_none(),
        "SpanError has no source (it is a leaf error)"
    );
}

#[test]
fn span_error_can_be_used_in_result_chain() {
    fn inner() -> Result<(), SpanError> {
        Err(SpanError {
            span: (5, 3),
            source_len: 10,
            reason: SpanErrorReason::StartGreaterThanEnd,
        })
    }

    fn outer() -> Result<(), Box<dyn std::error::Error>> {
        inner()?;
        Ok(())
    }

    let result = outer();
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("Invalid span"),
        "Boxed error should preserve Display: {}",
        err
    );
}

#[test]
fn failed_node_wraps_inner_errors() {
    let inner = ParseError {
        reason: ParseErrorReason::UnexpectedToken("x".to_string()),
        start: 0,
        end: 1,
        expected: vec![],
    };
    let outer = ParseError {
        reason: ParseErrorReason::FailedNode(vec![inner]),
        start: 0,
        end: 10,
        expected: vec![],
    };
    // FailedNode contains child errors accessible via the reason
    match &outer.reason {
        ParseErrorReason::FailedNode(children) => {
            assert_eq!(children.len(), 1);
            match &children[0].reason {
                ParseErrorReason::UnexpectedToken(tok) => assert_eq!(tok, "x"),
                other => panic!("Expected UnexpectedToken, got {:?}", other),
            }
        }
        other => panic!("Expected FailedNode, got {:?}", other),
    }
}

// ============================================================================
// 15. Error messages work correctly when formatted with Display vs Debug
// ============================================================================

#[test]
fn span_error_display_vs_debug_differ() {
    let err = SpanError {
        span: (5, 3),
        source_len: 100,
        reason: SpanErrorReason::StartGreaterThanEnd,
    };
    let display = format!("{err}");
    let debug = format!("{err:?}");
    // Display should be human-readable
    assert!(
        display.contains("Invalid span"),
        "Display is human-readable: {display}"
    );
    // Debug should include struct/enum names
    assert!(
        debug.contains("SpanError"),
        "Debug includes type name: {debug}"
    );
    assert!(
        debug.contains("StartGreaterThanEnd"),
        "Debug includes variant: {debug}"
    );
    // They should not be identical
    assert_ne!(display, debug, "Display and Debug should differ");
}

#[test]
fn reporting_error_display_is_not_debug() {
    let err = ReportingParseError {
        line: 2,
        column: 10,
        unexpected_token: Some("@".to_string()),
        expected: vec!["number".to_string()],
        context: String::new(),
    };
    let display = format!("{err}");
    let debug = format!("{err:?}");
    // Display should be user-facing
    assert!(
        display.contains("Parse error at"),
        "Display is user-facing: {display}"
    );
    // Debug includes struct field names
    assert!(
        debug.contains("ParseError"),
        "Debug includes struct name: {debug}"
    );
    assert_ne!(display, debug, "Display and Debug should differ");
}

#[test]
fn validation_error_display_is_structured() {
    let err = ValidationError {
        kind: ErrorKind::CyclicDependency,
        message: "Cycle found".to_string(),
        location: ErrorLocation {
            symbol: None,
            rule_index: None,
            position: None,
            description: "grammar".to_string(),
        },
        suggestion: None,
        related: vec![],
    };
    let display = format!("{err}");
    let debug = format!("{err:?}");
    // Display should be structured for humans
    assert!(
        display.contains("Error: Cycle found"),
        "Display structured: {display}"
    );
    // Debug should show Rust struct representation
    assert!(
        debug.contains("ValidationError"),
        "Debug shows struct: {debug}"
    );
    assert!(
        debug.contains("CyclicDependency"),
        "Debug shows variant: {debug}"
    );
}

// ============================================================================
// Additional tests to reach 25+
// ============================================================================

#[test]
fn span_error_start_out_of_bounds_message() {
    let err = SpanError {
        span: (50, 60),
        source_len: 30,
        reason: SpanErrorReason::StartOutOfBounds,
    };
    let display = format!("{err}");
    assert!(
        display.contains("start"),
        "Should mention 'start': {display}"
    );
    assert!(
        display.contains("50"),
        "Should include start value: {display}"
    );
    assert!(
        display.contains("30"),
        "Should include source length: {display}"
    );
}

#[test]
fn reporting_error_context_is_parenthesized() {
    let err = ReportingParseError {
        line: 1,
        column: 1,
        unexpected_token: Some("x".to_string()),
        expected: vec![],
        context: "inside array".to_string(),
    };
    let display = format!("{err}");
    assert!(
        display.contains("(inside array)"),
        "Context should be parenthesized: {display}"
    );
}

#[test]
fn multiple_expected_tokens_joined_with_commas() {
    let err = ReportingParseError {
        line: 1,
        column: 1,
        unexpected_token: Some("x".to_string()),
        expected: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        context: String::new(),
    };
    let display = format!("{err}");
    assert!(
        display.contains("a, b, c"),
        "Expected tokens should be comma-separated: {display}"
    );
}

#[test]
fn error_node_with_no_actual_token() {
    let node = ErrorNode {
        start_byte: 0,
        end_byte: 0,
        start_position: (0, 0),
        end_position: (0, 0),
        expected: vec![1],
        actual: None,
        recovery: RecoveryStrategy::TokenInsertion,
        skipped_tokens: vec![],
    };
    let dbg = format!("{:?}", node);
    assert!(
        dbg.contains("None"),
        "Should show None for missing actual: {dbg}"
    );
    assert!(
        dbg.contains("TokenInsertion"),
        "Should include strategy: {dbg}"
    );
}

#[test]
fn validation_error_with_related_info() {
    let err = ValidationError {
        kind: ErrorKind::CyclicDependency,
        message: "Cycle between A and B".to_string(),
        location: ErrorLocation {
            symbol: None,
            rule_index: None,
            position: None,
            description: "grammar".to_string(),
        },
        suggestion: None,
        related: vec![
            RelatedInfo {
                location: "rule A".to_string(),
                message: "references B".to_string(),
            },
            RelatedInfo {
                location: "rule B".to_string(),
                message: "references A".to_string(),
            },
        ],
    };
    let display = format!("{err}");
    assert!(
        display.contains("references B"),
        "Should include related info: {display}"
    );
    assert!(
        display.contains("references A"),
        "Should include all related info: {display}"
    );
    assert!(
        display.contains("rule A") && display.contains("rule B"),
        "Should include related locations: {display}"
    );
}

#[test]
fn parse_error_reason_missing_token_debug() {
    let reason = ParseErrorReason::MissingToken("closing_brace".to_string());
    let dbg = format!("{:?}", reason);
    assert!(dbg.contains("MissingToken"), "Should show variant: {dbg}");
    assert!(
        dbg.contains("closing_brace"),
        "Should show token name: {dbg}"
    );
}

#[test]
fn parse_error_reason_failed_node_with_empty_children() {
    let reason = ParseErrorReason::FailedNode(vec![]);
    let dbg = format!("{:?}", reason);
    assert!(dbg.contains("FailedNode"), "Should show variant: {dbg}");
}

#[test]
fn span_error_equality_comparison() {
    let err1 = SpanError {
        span: (5, 3),
        source_len: 10,
        reason: SpanErrorReason::StartGreaterThanEnd,
    };
    let err2 = SpanError {
        span: (5, 3),
        source_len: 10,
        reason: SpanErrorReason::StartGreaterThanEnd,
    };
    let err3 = SpanError {
        span: (5, 3),
        source_len: 10,
        reason: SpanErrorReason::EndOutOfBounds,
    };
    assert_eq!(err1, err2, "Same errors should be equal");
    assert_ne!(err1, err3, "Different reasons should differ");
}

#[test]
fn error_recovery_config_builder_produces_informative_config() {
    let config = ErrorRecoveryConfigBuilder::new()
        .max_panic_skip(25)
        .max_consecutive_errors(5)
        .add_sync_token(10)
        .add_insertable_token(20)
        .add_deletable_token(30)
        .add_scope_delimiter(40, 41)
        .build();
    let dbg = format!("{:?}", config);
    assert!(dbg.contains("25"), "Debug should show max_panic_skip");
    assert!(
        dbg.contains("5"),
        "Debug should show max_consecutive_errors"
    );
}

#[test]
fn reporting_error_reporter_populates_expected_tokens() {
    let parser = make_dummy_parser();
    let reporter = ErrorReporter::new(String::new());

    let error = reporter.error_at_current(&parser, Some("@".to_string()));

    assert_eq!(error.unexpected_token.as_deref(), Some("@"));
    assert!(
        error.expected.iter().any(|token| token == "num"),
        "expected token set should include the one-token grammar's token name: {:?}",
        error.expected
    );
}

#[test]
fn reporting_parse_with_errors_preserves_expected_tokens_after_bad_input() {
    let mut parser = make_dummy_parser();

    let errors = parser
        .parse_with_errors(vec![(adze_ir::SymbolId(2), "@".to_string())])
        .expect_err("invalid token should fail to parse");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].line, 1);
    assert_eq!(errors[0].column, 1);
    assert_eq!(errors[0].unexpected_token.as_deref(), Some("@"));
    assert!(
        errors[0].expected.iter().any(|token| token == "num"),
        "expected token set should survive parse failure: {:?}",
        errors[0].expected
    );
}

#[test]
fn reporting_parse_with_errors_includes_source_excerpt_after_bad_input() {
    let mut parser = make_dummy_parser();

    let errors = parser
        .parse_with_errors(vec![(adze_ir::SymbolId(2), "@".to_string())])
        .expect_err("invalid token should fail to parse");

    assert!(
        errors[0].context.contains("@"),
        "source excerpt should include the unexpected input: {:?}",
        errors[0].context
    );
    assert!(
        errors[0].context.contains("^"),
        "source excerpt should include a caret marker: {:?}",
        errors[0].context
    );
}

#[test]
fn reporting_parse_with_errors_tracks_multiline_bad_input_location_and_excerpt() {
    let mut parser = make_dummy_parser();

    let errors = parser
        .parse_with_errors(vec![
            (adze_ir::SymbolId(1), "1\n".to_string()),
            (adze_ir::SymbolId(2), "@".to_string()),
        ])
        .expect_err("invalid token after newline should fail to parse without panicking");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].line, 2);
    assert_eq!(errors[0].column, 1);
    assert_eq!(errors[0].unexpected_token.as_deref(), Some("@"));
    assert!(
        !errors[0].expected.is_empty(),
        "expected token set should survive multiline parse failure: {:?}",
        errors[0].expected
    );
    assert!(
        errors[0].context.contains("@"),
        "source excerpt should include the unexpected line: {:?}",
        errors[0].context
    );
    assert!(
        errors[0].context.contains("^"),
        "source excerpt should include a caret marker: {:?}",
        errors[0].context
    );
}

#[test]
fn reporting_parse_diagnostics_include_byte_span_for_multiline_bad_input() {
    let mut parser = make_dummy_parser();

    let errors = parser
        .parse_with_diagnostics(vec![
            (adze_ir::SymbolId(1), "1\n".to_string()),
            (adze_ir::SymbolId(2), "@".to_string()),
        ])
        .expect_err("invalid token after newline should produce a structured diagnostic");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].line, 2);
    assert_eq!(errors[0].column, 1);
    assert_eq!(errors[0].start_byte, 2);
    assert_eq!(errors[0].end_byte, 3);
    assert_eq!(errors[0].unexpected_token.as_deref(), Some("@"));
    assert!(
        !errors[0].expected.is_empty(),
        "expected token set should survive structured diagnostic failure: {:?}",
        errors[0].expected
    );

    let display = format!("{}", errors[0]);
    assert!(
        display.contains("bytes 2..3"),
        "diagnostic display should include byte span: {display}"
    );
}

#[test]
fn reporting_parse_diagnostics_display_includes_multiline_excerpt() {
    let mut parser = make_dummy_parser();

    let errors = parser
        .parse_with_diagnostics(vec![
            (adze_ir::SymbolId(1), "1\n".to_string()),
            (adze_ir::SymbolId(2), "@".to_string()),
        ])
        .expect_err("invalid token after newline should produce a structured diagnostic");

    let display = format!("{}", errors[0]);
    assert!(
        display.contains("Parse error at 2:1"),
        "diagnostic display should include line/column: {display}"
    );
    assert!(
        display.contains("bytes 2..3"),
        "diagnostic display should include byte span: {display}"
    );
    assert!(
        display.contains("@\n^"),
        "diagnostic display should include the source excerpt and caret: {display}"
    );
}

// ============================================================================
// Helper: create a minimal GLRParser for ErrorReporter tests
// ============================================================================

fn make_dummy_parser() -> adze::glr_parser::GLRParser {
    use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
    use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

    let mut g = Grammar::new("dummy".to_string());
    let num = SymbolId(1);
    g.tokens.insert(
        num,
        Token {
            name: "num".to_string(),
            pattern: TokenPattern::Regex("[0-9]+".to_string()),
            fragile: false,
        },
    );
    let start = SymbolId(10);
    g.rule_names.insert(start, "start".to_string());
    g.rules.entry(start).or_default().push(Rule {
        lhs: start,
        rhs: vec![Symbol::Terminal(num)],
        production_id: ProductionId(0),
        precedence: None,
        associativity: None,
        fields: vec![],
    });
    g.set_start_symbol(start);

    let ff = FirstFollowSets::compute(&g).unwrap();
    let table = build_lr1_automaton(&g, &ff).unwrap();

    adze::glr_parser::GLRParser::new(table, g)
}

// ============================================================================
// 18. Structured expected-tokens field on errors::ParseError
// ============================================================================

#[test]
fn parse_error_expected_field_default_is_empty_vec() {
    let err = ParseError {
        reason: ParseErrorReason::UnexpectedToken("x".to_string()),
        start: 0,
        end: 1,
        expected: vec![],
    };
    assert!(
        err.expected.is_empty(),
        "expected field should default to empty vec"
    );
}

#[test]
fn parse_error_expected_field_holds_token_names() {
    let err = ParseError {
        reason: ParseErrorReason::UnexpectedToken("x".to_string()),
        start: 0,
        end: 1,
        expected: vec!["number".to_string(), "string".to_string()],
    };
    assert_eq!(err.expected, vec!["number", "string"]);
}

#[test]
fn parse_error_expected_field_backward_compatible_display() {
    // When expected is empty, Display should not mention "expected one of:"
    let err = ParseError {
        reason: ParseErrorReason::UnexpectedToken("@".to_string()),
        start: 10,
        end: 11,
        expected: vec![],
    };
    let display = format!("{err}");
    assert!(
        !display.contains("expected one of:"),
        "empty expected should not produce 'expected one of:' in Display: {display}"
    );

    // When expected is populated but the reason string already contains
    // expected info, Display should remain backward compatible
    let err = ParseError {
        reason: ParseErrorReason::UnexpectedToken(
            "unexpected; expected one of: number, string".to_string(),
        ),
        start: 0,
        end: 1,
        expected: vec!["number".to_string(), "string".to_string()],
    };
    let display = format!("{err}");
    assert!(
        display.contains("expected one of: number, string"),
        "Display should include expected tokens from reason: {display}"
    );
}

#[test]
fn parse_error_display_uses_structured_expected_when_reason_has_no_expected() {
    let err = ParseError {
        reason: ParseErrorReason::UnexpectedToken("@".to_string()),
        start: 0,
        end: 1,
        expected: vec!["number".to_string(), "string".to_string()],
    };
    let display = format!("{err}");
    assert!(
        display.contains("expected one of: number, string"),
        "Display should render structured expected tokens when reason has none: {display}"
    );
}

#[test]
fn missing_token_expected_field_contains_token_name() {
    let err = ParseError {
        reason: ParseErrorReason::MissingToken(";".to_string()),
        start: 5,
        end: 5,
        expected: vec![";".to_string()],
    };
    assert_eq!(err.expected, vec![";"]);
    let display = format!("{err}");
    assert!(
        display.contains("missing token \";\""),
        "Display should include missing token: {display}"
    );
}
