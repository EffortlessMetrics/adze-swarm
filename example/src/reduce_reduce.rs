// For pure-rust: Include and re-export the generated parser symbols.
#[cfg(feature = "pure-rust")]
pub mod generated {
    include!(concat!(
        env!("OUT_DIR"),
        "/grammar_reduce_reduce/parser_reduce_reduce.rs"
    ));
}

#[cfg(feature = "pure-rust")]
pub use generated::{LANGUAGE, SMALL_PARSE_TABLE, SMALL_PARSE_TABLE_MAP};

// Generated reduce/reduce conflict fixture.
//
// Grammar shape:
//   Choice -> FromA | FromB
//   FromA  -> "x"
//   FromB  -> "x"
//
// After reading "x", both FromA and FromB can reduce before Choice reduces.
// This fixture exists to keep generated parser/tablegen evidence honest for
// reduce/reduce-shaped ambiguity, not only shift/reduce expression grammars.
#[adze::grammar("reduce_reduce")]
pub mod grammar {
    #[adze::language]
    #[derive(PartialEq, Eq, Debug, Clone)]
    pub enum Choice {
        FromA(FromA),
        FromB(FromB),
    }

    #[adze::language]
    #[derive(PartialEq, Eq, Debug, Clone)]
    pub struct FromA(#[adze::leaf(text = "x")] ());

    #[adze::language]
    #[derive(PartialEq, Eq, Debug, Clone)]
    pub struct FromB(#[adze::leaf(text = "x")] ());
}
