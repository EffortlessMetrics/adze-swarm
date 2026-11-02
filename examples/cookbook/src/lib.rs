//! Adze cookbook: small runnable recipes tied to the generated
//! parser/document architecture (issue #882, parent #853).
//!
//! Each recipe is one complete grammar plus tests that compile and run it.
//! The book index lives at `book/src/cookbook.md` and the task-oriented
//! reference at `docs/reference/parser-cookbook.md`; code lives only here
//! so snippets cannot drift from the checked-in proof.

// Recipe 1: arithmetic with typed values and precedence.
// Prerequisites: first-hour path (#874) / book quickstart for project setup.
// Support boundary: typed extraction (Stable); JSON projection (Experimental,
// pinned schema id only). Expected output: exact typed AST, evaluated value,
// and one AdzeDocument backing AST, ranges, and diagnostics. Limitation:
// i64 arithmetic only, no overflow diagnostics (see Recipe 4).
// Proof: cargo test --manifest-path examples/cookbook/Cargo.toml arithmetic -- --nocapture
#[adze::grammar("cookbook_arithmetic")]
pub mod arithmetic {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Expr {
        Number(#[adze::leaf(pattern = r"\d+", try_transform = |v: &str| v.parse::<i64>())] i64),

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

// Recipe 2: CSV-like delimited repetition.
// Prerequisites: Recipe 1 (grammar module shape, extras).
// Support boundary: typed extraction (Stable). Expected output: exact
// multi-item Vec values. Limitation: identifier-shaped cells only (the
// generated lexer subset); no quoting/escaping.
// Proof: cargo test --manifest-path examples/cookbook/Cargo.toml csv -- --nocapture
#[adze::grammar("cookbook_csv")]
pub mod csv {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub struct Row {
        #[adze::repeat(non_empty = true)]
        #[adze::delimited(
            #[adze::leaf(text = ",")]
            ()
        )]
        pub cells: Vec<Cell>,
    }

    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub struct Cell {
        // NOTE: the generated lexer lowers only a fixed pattern subset
        // (`\s+`, `\d+`, the identifier shape below, and text literals);
        // other regex shapes produce no lexer code and never match.
        #[adze::leaf(pattern = r"[a-zA-Z_][a-zA-Z0-9_]*", transform = |v| v.to_string())]
        pub value: String,
    }

    #[adze::extra]
    #[allow(dead_code)]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s+")]
        _ws: (),
    }
}

// Recipe 3: object-like grammar with named fields, repeated syntax,
// keywords versus identifiers, and whitespace extras. Prerequisites:
// Recipes 1-2. Support boundary: typed extraction (Stable). Expected output:
// exact Config values; identifiers containing keyword text stay whole.
// Limitations:
// fixed key order, all clauses required, non-empty tag lists; optional
// clauses and `#` comments are tracked follow-ups (see cookbook index).
// Proof: cargo test --manifest-path examples/cookbook/Cargo.toml config -- --nocapture
#[adze::grammar("cookbook_config")]
pub mod config {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub struct Config {
        #[adze::leaf(text = "{")]
        _open: (),
        pub host: HostField,
        #[adze::leaf(text = ",")]
        _comma: (),
        pub port: PortField,
        #[adze::leaf(text = ",")]
        _comma2: (),
        pub debug: DebugFlag,
        pub tags: TagsClause,
        #[adze::leaf(text = "}")]
        _close: (),
    }

    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub struct HostField {
        #[adze::leaf(text = "host")]
        _host: (),
        #[adze::leaf(text = ":")]
        _colon: (),
        pub name: Identifier,
    }

    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub struct PortField {
        #[adze::leaf(text = "port")]
        _port: (),
        #[adze::leaf(text = ":")]
        _colon: (),
        pub value: PortNumber,
    }

    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub struct PortNumber {
        #[adze::leaf(pattern = r"\d+", try_transform = |v: &str| v.parse::<i64>())]
        pub value: i64,
    }

    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub struct DebugFlag {
        #[adze::leaf(text = "debug")]
        _debug: (),
        #[adze::leaf(text = ":")]
        _colon: (),
        pub enabled: Bool,
        #[adze::leaf(text = ",")]
        _comma: (),
    }

    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub struct TagsClause {
        #[adze::leaf(text = "tags")]
        _tags: (),
        #[adze::leaf(text = ":")]
        _colon: (),
        #[adze::leaf(text = "[")]
        _open: (),
        #[adze::repeat(non_empty = true)]
        #[adze::delimited(
            #[adze::leaf(text = ",")]
            ()
        )]
        pub tags: Vec<Identifier>,
        #[adze::leaf(text = "]")]
        _close: (),
    }

    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Bool {
        True(TrueKw),
        False(FalseKw),
    }

    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub struct TrueKw {
        #[adze::leaf(text = "true")]
        _t: (),
    }

    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub struct FalseKw {
        #[adze::leaf(text = "false")]
        _f: (),
    }

    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub struct Identifier {
        #[adze::word]
        #[adze::leaf(pattern = r"[a-zA-Z_][a-zA-Z0-9_]*", transform = |v| v.to_string())]
        pub name: String,
    }

    #[adze::extra]
    #[allow(dead_code)]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s+")]
        _ws: (),
    }
    // NOTE: no comment extra: comment shapes need character classes outside
    // the generated lexer's fixed pattern subset (see Recipe 2), so `#`
    // comments are a tracked follow-up, not part of this recipe.
}

// Recipe 4: fallible numeric conversion with structured failure (no unwrap
// on user input). Prerequisites: Recipe 1. Support boundary: typed
// extraction (Stable); conversion failure is a structured diagnostic, not a
// panic. Expected output: valid values keep exact shape; `256` fails as
// ConversionFailed over bytes 0..3. Limitation: single u8 leaf grammar.
// Proof: cargo test --manifest-path examples/cookbook/Cargo.toml fallible -- --nocapture
#[adze::grammar("cookbook_fallible")]
pub mod fallible {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Expr {
        Number(#[adze::leaf(pattern = r"\d+", try_transform = |v: &str| v.parse::<u8>())] u8),

        #[adze::prec_left(1)]
        Add(Box<Expr>, #[adze::leaf(text = "+")] (), Box<Expr>),
    }

    #[adze::extra]
    #[allow(dead_code)]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s+")]
        _ws: (),
    }
}

#[cfg(test)]
mod arithmetic_tests {
    use super::arithmetic::{self, Expr};
    use adze::document::ParseOutcome;

    fn eval(expr: &Expr) -> i64 {
        match expr {
            Expr::Number(n) => *n,
            Expr::Add(l, _, r) => eval(l) + eval(r),
            Expr::Mul(l, _, r) => eval(l) * eval(r),
        }
    }

    #[test]
    fn cookbook_arithmetic_precedence_is_exact() {
        assert_eq!(
            arithmetic::parse("1 + 2 * 3").expect("should parse"),
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
    fn cookbook_arithmetic_evaluates_typed_values() {
        let expr = arithmetic::parse("1 + 2 * 3").expect("should parse");
        assert_eq!(eval(&expr), 7);
    }

    #[test]
    fn cookbook_arithmetic_parse_and_document_agree() {
        let source = "1 + 2 * 3";
        let typed: Expr = arithmetic::parse(source).expect("typed parse works");
        let doc = arithmetic::parse_document(source).expect("document parses");
        assert_eq!(doc.outcome(), ParseOutcome::Complete);
        assert!(doc.diagnostics().is_empty());
        assert_eq!(doc.source_text(), source);
        assert_eq!(doc.source_slice(0..1), Some("1"));
        assert_eq!(doc.source_slice(4..5), Some("2"));
        assert_eq!(doc.ast::<Expr>().expect("document AST works"), typed);
    }

    #[test]
    fn cookbook_arithmetic_bad_input_reports_span() {
        let source = "1 + @";
        let errors = arithmetic::parse(source).expect_err("bad input fails");
        assert_eq!(errors[0].byte_span(), 4..5);
        let rendered = errors[0].display_with_source(source).to_string();
        assert!(rendered.contains("bytes 4..5"));
    }

    #[cfg(feature = "serialization")]
    #[test]
    fn cookbook_arithmetic_document_json_has_pinned_schema() {
        let doc = arithmetic::parse_document("1 + 2").expect("document parses");
        let json = doc.to_json_value().to_string();
        assert!(json.contains(adze::document::ADZE_DOCUMENT_JSON_SCHEMA));
        assert!(json.contains("\"complete\""));
    }
}

#[cfg(test)]
mod csv_tests {
    use super::csv::{self, Cell, Row};

    fn values(row: &Row) -> Vec<&str> {
        row.cells.iter().map(|c| c.value.as_str()).collect()
    }

    #[test]
    fn cookbook_csv_multi_item_row_is_exact() {
        assert_eq!(
            csv::parse("a,b,c").expect("should parse"),
            Row {
                cells: vec![
                    Cell {
                        value: "a".to_string()
                    },
                    Cell {
                        value: "b".to_string()
                    },
                    Cell {
                        value: "c".to_string()
                    },
                ]
            }
        );
    }

    #[test]
    fn cookbook_csv_whitespace_extra_is_ignored() {
        let row = csv::parse("a, b ,c").expect("should parse");
        assert_eq!(values(&row), vec!["a", "b", "c"]);
    }

    #[test]
    fn cookbook_csv_single_item_and_empty_input() {
        assert_eq!(
            values(&csv::parse("solo").expect("single item")),
            vec!["solo"]
        );
        assert!(csv::parse("").is_err(), "empty input fails");
    }
}

#[cfg(test)]
mod config_tests {
    use super::config::{self, Bool, DebugFlag};

    #[test]
    fn cookbook_config_full_object_is_exact() {
        let parsed = config::parse("{ host: web, port: 8080, debug: true, tags: [a, b] }")
            .expect("should parse");
        // Token leaves (`{`, `,`, keywords) are private parser detail; the
        // exact-value assertions below read the public semantic fields.
        assert_eq!(parsed.host.name.name, "web");
        assert_eq!(parsed.port.value.value, 8080);
        assert!(
            matches!(
                parsed.debug,
                DebugFlag {
                    enabled: Bool::True(_),
                    ..
                }
            ),
            "debug flag should be present and true"
        );
        let tags: Vec<&str> = parsed.tags.tags.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(tags, vec!["a", "b"]);
    }

    #[test]
    fn cookbook_config_multiline_whitespace_is_ignored() {
        let source = "{ host: web,\nport: 80,\ndebug: true,\ntags: [a] }";
        let parsed = config::parse(source).expect("should parse");
        assert_eq!(parsed.port.value.value, 80);
        let tags: Vec<&str> = parsed.tags.tags.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(tags, vec!["a"]);
    }

    #[test]
    fn cookbook_config_keywords_survive_as_identifiers() {
        // Identifier values may start with or contain keyword text; the
        // generated lexer keeps the longer identifier match (`hostname`,
        // `truex`). Bare keyword text as an identifier value (`debug`, `host`)
        // still lexes as the keyword, so that shape is a tracked follow-up.
        let source = "{ host: hostname, port: 1, debug: true, tags: [truex, hostname] }";
        let parsed = config::parse(source).expect("should parse");
        assert_eq!(parsed.host.name.name, "hostname");
        let tags: Vec<&str> = parsed.tags.tags.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(tags, vec!["truex", "hostname"]);
    }

    #[test]
    fn cookbook_config_bad_value_fails() {
        assert!(config::parse("{ host: web, port: eighty, tags: [] }").is_err());
    }

    #[test]
    fn cookbook_config_false_keyword_parses() {
        let parsed =
            config::parse("{ host: web, port: 1, debug: false, tags: [a] }").expect("should parse");
        assert!(matches!(
            parsed.debug,
            DebugFlag {
                enabled: Bool::False(_),
                ..
            }
        ));
    }
}

#[cfg(test)]
mod fallible_tests {
    use super::fallible::{self, Expr};
    use adze::document::ParseOutcome;
    use adze::errors::ParseErrorReason;

    #[test]
    fn cookbook_fallible_valid_values_keep_exact_shape() {
        assert_eq!(
            fallible::parse("200").expect("should parse"),
            Expr::Number(200)
        );
        assert_eq!(
            fallible::parse("200 + 55").expect("should parse"),
            Expr::Add(Box::new(Expr::Number(200)), (), Box::new(Expr::Number(55)))
        );
    }

    #[test]
    fn cookbook_fallible_overflow_is_structured_not_panic() {
        let source = "256";
        let errors = fallible::parse(source).expect_err("u8 overflow fails");
        assert!(
            matches!(errors[0].reason, ParseErrorReason::ConversionFailed(_)),
            "got: {:?}",
            errors[0].reason
        );
        assert_eq!(errors[0].byte_span(), 0..3);
        let rendered = errors[0].display_with_source(source).to_string();
        assert!(rendered.contains("leaf conversion failed"));
        assert!(rendered.contains("bytes 0..3"));
    }

    #[test]
    fn cookbook_fallible_document_keeps_syntax_outcome_but_ast_fails() {
        let doc = fallible::parse_document("256").expect("document is still built");
        assert_eq!(doc.outcome(), ParseOutcome::Complete);
        let errors = doc.ast::<Expr>().expect_err("AST conversion fails");
        assert!(
            errors
                .iter()
                .any(|e| matches!(e.reason, ParseErrorReason::ConversionFailed(_))),
            "got: {:?}",
            errors.iter().map(|e| &e.reason).collect::<Vec<_>>()
        );
    }
}
