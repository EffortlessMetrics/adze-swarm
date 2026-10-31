//! A small configuration language that returns ordinary Rust values.

#[adze::grammar("retry_config")]
pub mod grammar {
    /// A fixed-order retry policy; this example is not a general TOML parser.
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub struct Policy {
        #[adze::leaf(text = "retries")]
        _retries: (),
        #[adze::leaf(text = "=")]
        _retries_equals: (),
        #[adze::leaf(pattern = r"[0-9]+", try_transform = |text| text.parse::<u16>())]
        pub retries: u16,
        #[adze::leaf(text = ";")]
        _retries_end: (),
        #[adze::leaf(text = "targets")]
        _targets: (),
        #[adze::leaf(text = "=")]
        _targets_equals: (),
        #[adze::leaf(text = "[")]
        _targets_open: (),
        #[adze::repeat(non_empty = true)]
        #[adze::delimited(#[adze::leaf(text = ",")] ())]
        #[adze::leaf(pattern = r"[a-zA-Z_][a-zA-Z0-9_]*", transform = |text| text.to_owned())]
        pub targets: Vec<String>,
        #[adze::leaf(text = "]")]
        _targets_close: (),
        #[adze::leaf(text = ";")]
        _targets_end: (),
        pub timeout: Option<Timeout>,
    }

    /// Optional timeout in seconds; absence is distinct from a zero timeout.
    #[derive(Debug, PartialEq)]
    pub struct Timeout {
        #[adze::leaf(text = "timeout")]
        _timeout: (),
        #[adze::leaf(text = "=")]
        _equals: (),
        #[adze::leaf(pattern = r"[0-9]+", try_transform = |text| text.parse::<u16>())]
        pub seconds: u16,
        #[adze::leaf(text = ";")]
        _end: (),
    }

    #[adze::extra]
    #[allow(dead_code)]
    struct Whitespace {
        #[adze::leaf(pattern = r"[ \t\r\n]+")]
        _text: (),
    }

    #[adze::extra]
    #[allow(dead_code)]
    struct Comment {
        #[adze::leaf(pattern = r"#[^\r\n]*")]
        _text: (),
    }
}
