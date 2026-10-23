//! Grammar fixture for #864 fallible leaf transforms: a struct root whose
//! fields exercise `try_transform` on a numeric leaf and on a custom
//! domain type, next to a multibyte label token.

/// Custom domain type produced by a fallible leaf conversion.
#[derive(Debug, PartialEq, Eq)]
pub enum Unit {
    /// The `mm` unit token.
    Millimetres,
    /// The `cm` unit token.
    Centimetres,
}

fn parse_unit(text: &str) -> Result<Unit, String> {
    match text {
        "mm" => Ok(Unit::Millimetres),
        "cm" => Ok(Unit::Centimetres),
        other => Err(format!("unknown unit `{other}`")),
    }
}

#[adze::grammar("fallible_transforms")]
pub mod grammar {
    /// A labelled measurement such as `λ 12 mm`.
    #[derive(Debug, PartialEq, Eq)]
    #[adze::language]
    pub struct Measurement {
        /// Multibyte label token preceding the converted fields.
        #[adze::leaf(text = "λ")]
        pub label: String,
        /// Numeric amount; conversion overflows `i32` on long digit runs.
        #[adze::leaf(pattern = r"\d+", try_transform = |s| s.parse::<i32>())]
        pub value: i32,
        /// Unit converted through a custom fallible conversion.
        #[adze::leaf(pattern = r"[a-z]+", try_transform = |s| super::parse_unit(s))]
        pub unit: super::Unit,
    }

    #[adze::extra]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s")]
        _ws: (),
    }
}
