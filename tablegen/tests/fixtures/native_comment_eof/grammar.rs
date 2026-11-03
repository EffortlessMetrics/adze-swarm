#[adze::grammar("number_comment_eof")]
pub mod grammar {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub struct Number {
        #[adze::leaf(pattern = r"\d+", try_transform = |text| text.parse::<u16>())]
        pub value: u16,
    }
    #[adze::extra]
    #[allow(dead_code)]
    struct Comment {
        #[adze::leaf(pattern = r"#[^\r\n]*")]
        _text: (),
    }
}
