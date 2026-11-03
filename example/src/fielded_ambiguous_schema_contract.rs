//! Canonical schema transport fixture with real streaming-GLR conflicts.

#[adze::grammar("fielded_ambiguous_schema_contract")]
pub mod grammar {
    /// No precedence: binary expressions retain a shift/reduce conflict.
    #[adze::language]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Expr {
        Number(#[adze::leaf(pattern = r"\d+", transform = |value| value.parse().unwrap())] i32),
        Binary {
            #[adze::field("left")]
            left: Box<Expr>,
            #[adze::field("operator")]
            #[adze::leaf(text = "+")]
            operator: (),
            #[adze::field("right")]
            right: Box<Expr>,
        },
    }
}
