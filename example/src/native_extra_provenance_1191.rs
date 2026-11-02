//! Generated comment/whitespace fixture for native source provenance.
#[adze::grammar("native_extra_provenance_1191")]
pub mod grammar {
    #[adze::language]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Number {
        #[adze::leaf(pattern = r"\d+", try_transform = |text| text.parse::<u16>())]
        pub value: u16,
    }

    #[allow(dead_code)]
    #[adze::extra]
    struct Comment {
        #[adze::leaf(pattern = r"#[^\r\n]*")]
        _comment: (),
    }

    #[allow(dead_code)]
    #[adze::extra]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s")]
        _whitespace: (),
    }
}
