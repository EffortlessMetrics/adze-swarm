//! Contract fixture for stack-aware GLR streaming lexing (#857 / #888).
//!
//! Wraps an intentionally ambiguous expression with a meaningful newline terminator
//! so the fixed-mode bridge cannot silently discard separator bytes.

#[cfg(feature = "pure-rust")]
pub mod generated {
    include!(concat!(
        env!("OUT_DIR"),
        "/grammar_streaming_lex_modes/parser_streaming_lex_modes.rs"
    ));
}

#[cfg(feature = "pure-rust")]
pub use generated::{LANGUAGE, SMALL_PARSE_TABLE, SMALL_PARSE_TABLE_MAP};

#[adze::grammar("streaming_lex_modes")]
pub mod grammar {
    /// Ambiguous expression followed by a meaningful newline separator.
    #[adze::language]
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub enum Document {
        Line(Box<Expr>, #[adze::leaf(text = "\n")] ()),
    }

    /// Ambiguous expression branch used to create GLR fork points.
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub enum Expr {
        Binary(
            Box<Expr>,
            #[adze::leaf(pattern = r"[-+*/]")] String,
            Box<Expr>,
        ),
        Number(#[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())] i32),
    }

    /// Inline whitespace is a grammar extra, not a hard-coded bridge skip.
    #[adze::extra]
    struct InlineWhitespace {
        #[adze::leaf(pattern = r"[ \t]+")]
        _ws: (),
    }
}

#[cfg(test)]
mod tests {
    use super::grammar;

    #[test]
    fn streaming_lex_modes_fixture_parses_ambiguous_line() {
        let parsed = grammar::parse("1+2\n");
        assert!(parsed.is_ok(), "expected fixture line to parse: {parsed:?}");
    }
}
