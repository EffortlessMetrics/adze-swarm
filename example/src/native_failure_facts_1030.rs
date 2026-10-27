//! Failure-facts fixture for #1030: a streaming-routed conflicted grammar
//! whose numeric leaf is fallible (`try_transform`) with a process-visible
//! invocation counter.
//!
//! Shape mirrors `ambiguous_expr` (single binary variant, no precedence) so
//! the table keeps genuine shift/reduce conflicts and routes through the
//! stack-aware streaming driver. The only intentional difference is the
//! `Number` leaf: it converts through [`counted_i32_parse`] so tests can
//! prove typed extraction runs exactly once per `parse()` / document-AST
//! projection instead of being re-executed by an LR-substitution reparse.

use std::sync::atomic::AtomicUsize;

/// Counts `try_transform` invocations for the fixture leaf.
///
/// Tests reset this with `store(0, ...)` before each phase and assert the
/// exact call count afterwards. It lives here (not in the test) because the
/// generated `try_transform` closure must reference a `'static` item.
pub static NATIVE_FAILURE_FACTS_1030_CALLS: AtomicUsize = AtomicUsize::new(0);

/// Fallible leaf conversion that records its invocation before delegating to
/// `i32` parsing. Overflowing digit runs (e.g. `"2147483648"`) fail with the
/// standard library message while still counting the call.
pub fn counted_i32_parse(text: &str) -> Result<i32, String> {
    NATIVE_FAILURE_FACTS_1030_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    text.parse::<i32>().map_err(|error| error.to_string())
}

// For pure-rust: Include and re-export the generated parser symbols
#[cfg(feature = "pure-rust")]
pub mod generated {
    include!(concat!(
        env!("OUT_DIR"),
        "/grammar_native_failure_facts_1030/parser_native_failure_facts_1030.rs"
    ));
}

// Re-export the key symbols for tests
#[cfg(feature = "pure-rust")]
pub use generated::{LANGUAGE, SMALL_PARSE_TABLE, SMALL_PARSE_TABLE_MAP};

#[adze::grammar("native_failure_facts_1030")]
pub mod grammar {
    /// Expression: intentionally ambiguous binary operations (no precedence).
    #[adze::language]
    #[derive(PartialEq, Eq, Debug, Clone)]
    pub enum Expr {
        /// Single binary operation variant over every operator, exactly like
        /// `ambiguous_expr`, so the conflicted shape (and streaming route)
        /// is preserved.
        Binary(
            Box<Expr>,
            #[adze::leaf(pattern = r"[-+*/]")] String,
            Box<Expr>,
        ),

        /// Number terminal with a counted fallible conversion.
        Number(
            #[adze::leaf(pattern = r"\d+", try_transform = |s| super::counted_i32_parse(s))] i32,
        ),
    }

    /// Whitespace handling
    #[adze::extra]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s")]
        _whitespace: (),
    }
}
