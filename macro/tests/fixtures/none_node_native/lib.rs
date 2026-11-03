#[adze::grammar("fallible_struct_none")]
pub mod fallible_struct {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub struct Root {
        #[adze::leaf(pattern = r"\d+", try_transform = |text| text.parse::<u8>())]
        pub value: u8,
    }
}
#[adze::grammar("fallible_enum_none")]
pub mod fallible_enum {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Root {
        Count(#[adze::leaf(pattern = r"\d+", try_transform = |text| text.parse::<u8>())] u8),
    }
}
#[adze::grammar("infallible_struct_none")]
pub mod infallible_struct {
    #[adze::language]
    pub struct Root {
        #[adze::leaf(pattern = r"\d+", transform = |text| text.to_owned())]
        pub value: String,
    }
}
#[adze::grammar("infallible_enum_none")]
pub mod infallible_enum {
    #[adze::language]
    pub enum Root {
        Count(#[adze::leaf(pattern = r"\d+", transform = |text| text.to_owned())] String),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use adze::{
        Extract,
        errors::{ParseError, ParseErrorReason},
    };
    const SOURCE: &[u8] = b"prefix 17";
    const OFFSET: usize = 7;
    fn assert_missing(error: ParseError, owner: &str) {
        assert!(
            matches!(error.reason, ParseErrorReason::MissingToken(message) if message == format!("Extract called with None node for {owner} Root"))
        );
        assert_eq!(error.start..error.end, OFFSET..OFFSET);
        assert!(error.expected.is_empty());
    }
    #[test]
    fn test_generated_struct_none_returns_exact_missing_token_without_unwind() {
        let result = <fallible_struct::Root as Extract<_>>::try_extract(None, SOURCE, OFFSET, None);
        assert_missing(result.unwrap_err(), "struct");
    }
    #[test]
    fn test_generated_enum_none_returns_exact_missing_token_without_unwind() {
        let result = <fallible_enum::Root as Extract<_>>::try_extract(None, SOURCE, OFFSET, None);
        assert_missing(result.unwrap_err(), "enum");
    }
    #[test]
    fn test_box_forwards_none_error_and_exact_nonzero_position() {
        let result =
            <Box<fallible_struct::Root> as Extract<_>>::try_extract(None, SOURCE, OFFSET, None);
        assert_missing(result.unwrap_err(), "struct");
        let result =
            <Box<fallible_enum::Root> as Extract<_>>::try_extract(None, SOURCE, OFFSET, None);
        assert_missing(result.unwrap_err(), "enum");
    }
    #[test]
    fn test_generated_some_nodes_retain_successful_typed_values() {
        assert_eq!(
            fallible_struct::parse("17").unwrap(),
            fallible_struct::Root { value: 17 }
        );
        assert_eq!(
            fallible_enum::parse("17").unwrap(),
            fallible_enum::Root::Count(17)
        );
    }
    #[test]
    fn test_some_leaf_conversion_failure_remains_distinct_from_missing_token() {
        for errors in [
            fallible_struct::parse("256").unwrap_err(),
            fallible_enum::parse("256").unwrap_err(),
        ] {
            assert_eq!(errors.len(), 1);
            assert!(matches!(
                errors[0].reason,
                ParseErrorReason::ConversionFailed(_)
            ));
            assert_eq!(errors[0].start..errors[0].end, 0..3);
        }
    }
    #[test]
    fn test_infallible_none_boundary_remains_explicitly_separate() {
        assert!(
            std::panic::catch_unwind(|| <infallible_struct::Root as Extract<_>>::extract(
                None, SOURCE, OFFSET, None
            ))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| <infallible_enum::Root as Extract<_>>::extract(
                None, SOURCE, OFFSET, None
            ))
            .is_err()
        );
    }
    #[test]
    fn test_optional_none_remains_legitimate_absence() {
        let value =
            <Option<fallible_struct::Root> as Extract<_>>::try_extract(None, SOURCE, OFFSET, None)
                .unwrap();
        assert_eq!(value, None);
        let value =
            <Option<fallible_enum::Root> as Extract<_>>::try_extract(None, SOURCE, OFFSET, None)
                .unwrap();
        assert_eq!(value, None);
    }
}
