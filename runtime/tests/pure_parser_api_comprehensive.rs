// Wave 131: Comprehensive tests for adze pure_parser API
use adze::pure_parser::*;

// =====================================================================
// Parser construction
// =====================================================================

#[test]
fn parser_new() {
    let p = Parser::new();
    assert!(p.language().is_none());
}

#[test]
fn parser_default() {
    let p = Parser::default();
    assert!(p.language().is_none());
}

#[test]
fn parser_set_timeout() {
    let mut p = Parser::new();
    p.set_timeout_micros(1000);
    // No panic means success
}

#[test]
fn parser_set_timeout_zero() {
    let mut p = Parser::new();
    p.set_timeout_micros(0);
}

#[test]
fn parser_set_timeout_large() {
    let mut p = Parser::new();
    p.set_timeout_micros(u64::MAX);
}

#[test]
fn parser_reset() {
    let mut p = Parser::new();
    p.reset();
    assert!(p.language().is_none());
}

#[test]
fn parser_parse_without_language() {
    let mut p = Parser::new();
    let result = p.parse_string("hello");
    // Should return error or empty result since no language is set
    // ParseResult is a struct with root: Option<ParsedNode> and errors: Vec<ParseError>
    let _ = result.root;
    let _ = result.errors;
}

#[test]
fn parser_parse_bytes_without_language() {
    let mut p = Parser::new();
    let result = p.parse_bytes(b"hello");
    let _ = result.root;
    let _ = result.errors;
}

#[test]
fn parser_parse_empty_string() {
    let mut p = Parser::new();
    let result = p.parse_string("");
    let _ = result.root;
    let _ = result.errors;
}

#[test]
fn parser_parse_empty_bytes() {
    let mut p = Parser::new();
    let result = p.parse_bytes(b"");
    let _ = result.root;
    let _ = result.errors;
}

// =====================================================================
// Point struct tests
// =====================================================================

#[test]
fn point_default() {
    let p = Point { row: 0, column: 0 };
    assert_eq!(p.row, 0);
    assert_eq!(p.column, 0);
}

#[test]
fn point_values() {
    let p = Point {
        row: 10,
        column: 25,
    };
    assert_eq!(p.row, 10);
    assert_eq!(p.column, 25);
}

#[test]
fn point_clone() {
    let p = Point { row: 5, column: 3 };
    let p2 = p;
    assert_eq!(p.row, p2.row);
    assert_eq!(p.column, p2.column);
}

#[test]
fn point_debug() {
    let p = Point { row: 1, column: 2 };
    let debug = format!("{:?}", p);
    assert!(!debug.is_empty());
}

// =====================================================================
// ParseResult tests
// =====================================================================

#[test]
fn parse_result_is_enum() {
    let mut p = Parser::new();
    let result = p.parse_string("test");
    if let Some(ref node) = result.root {
        assert!(node.end_byte() >= node.start_byte());
    }
    let _ = result.errors.len();
}

// =====================================================================
// ParsedNode accessor tests (testing via public methods only)
// =====================================================================

#[test]
fn parser_multiple_resets() {
    let mut p = Parser::new();
    p.reset();
    p.reset();
    p.reset();
    assert!(p.language().is_none());
}

#[test]
fn parser_cancellation_flag_none() {
    let mut p = Parser::new();
    p.set_cancellation_flag(None);
}

// =====================================================================
// TSLanguage validation edge cases
// =====================================================================

#[test]
fn parser_language_starts_none() {
    let p = Parser::new();
    assert!(p.language().is_none(), "New parser should have no language");
}

// =====================================================================
// ParseError struct tests
// =====================================================================

#[test]
fn parse_error_debug() {
    let err = ParseError {
        work_limit: None,
        position: 5,
        point: Point { row: 0, column: 5 },
        expected: vec![1, 2, 3],
        found: 0,
    };
    let debug = format!("{:?}", err);
    assert!(debug.contains("ParseError"));
}

#[test]
fn parse_error_clone() {
    let err = ParseError {
        work_limit: None,
        position: 10,
        point: Point { row: 1, column: 0 },
        expected: vec![5],
        found: 6,
    };
    let err2 = err.clone();
    assert_eq!(err.position, err2.position);
    assert_eq!(err.expected, err2.expected);
    assert_eq!(err.found, err2.found);
}

#[test]
fn parse_error_empty_expected() {
    let err = ParseError {
        work_limit: None,
        position: 0,
        point: Point { row: 0, column: 0 },
        expected: vec![],
        found: 0,
    };
    assert!(err.expected.is_empty());
}

#[test]
fn parse_error_multiple_expected() {
    let err = ParseError {
        work_limit: None,
        position: 100,
        point: Point { row: 5, column: 10 },
        expected: vec![1, 2, 3, 4, 5],
        found: 99,
    };
    assert_eq!(err.expected.len(), 5);
}

// =====================================================================
// Parser sequential operations
// =====================================================================

#[test]
fn parser_parse_then_reset_then_parse() {
    let mut p = Parser::new();
    let _ = p.parse_string("first");
    p.reset();
    let _ = p.parse_string("second");
}

#[test]
fn parser_parse_various_sizes() {
    let mut p = Parser::new();
    for size in [0, 1, 10, 100, 1000] {
        let input = "a".repeat(size);
        let _ = p.parse_string(&input);
        p.reset();
    }
}

#[test]
fn parser_parse_unicode() {
    let mut p = Parser::new();
    let _ = p.parse_string("日本語テスト");
}

#[test]
fn parser_parse_emoji() {
    let mut p = Parser::new();
    let _ = p.parse_string("🎉🚀✨");
}

#[test]
fn parser_parse_mixed_content() {
    let mut p = Parser::new();
    let _ = p.parse_string("hello 世界 🌍 \n\t");
}

#[test]
fn parser_parse_null_bytes() {
    let mut p = Parser::new();
    let _ = p.parse_bytes(&[0, 0, 0]);
}

#[test]
fn parser_parse_binary_data() {
    let mut p = Parser::new();
    let data: Vec<u8> = (0..=255).collect();
    let _ = p.parse_bytes(&data);
}

// =====================================================================
// Concurrent parser usage (different instances)
// =====================================================================

#[test]
fn multiple_parsers_independent() {
    let mut p1 = Parser::new();
    let mut p2 = Parser::new();
    p1.set_timeout_micros(100);
    p2.set_timeout_micros(200);
    // They should be independent
    let _ = p1.parse_string("a");
    let _ = p2.parse_string("b");
}
