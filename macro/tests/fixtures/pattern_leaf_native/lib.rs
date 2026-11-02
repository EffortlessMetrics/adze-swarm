#[adze::grammar("disjoint")]
pub mod disjoint {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Choice {
        Number(#[adze::leaf(pattern = r"\d+", try_transform = |text| text.parse::<u8>())] u8),
        Word(#[adze::leaf(pattern = r"[a-z]+", transform = |text| text.to_owned())] String),
    }
}

#[adze::grammar("reversed")]
pub mod reversed {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Choice {
        Word(#[adze::leaf(pattern = r"[a-z]+", transform = |text| text.to_owned())] String),
        Number(#[adze::leaf(pattern = r"\d+", try_transform = |text| text.parse::<u8>())] u8),
    }
}

#[adze::grammar("overlapping")]
pub mod overlapping {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Choice {
        Narrow(#[adze::leaf(pattern = r"[a-z]+", transform = |text| text.to_owned())] String),
        Broad(#[adze::leaf(pattern = r"\w+", transform = |text| text.to_owned())] String),
    }
}

#[adze::grammar("selected_error")]
pub mod selected_error {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Choice {
        Broad(
            #[adze::leaf(pattern = r"\w+", try_transform = |_| Err::<String, _>("selected broad rejected"))]
             String,
        ),
        Lower(#[adze::leaf(pattern = r"[a-z]+", transform = |text| text.to_owned())] String),
    }
}

#[adze::grammar("single_pattern")]
pub mod single_pattern {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Choice {
        Number(#[adze::leaf(pattern = r"\d+", try_transform = |text| text.parse::<u8>())] u8),
    }
}

#[adze::grammar("literal")]
pub mod literal {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Choice {
        #[adze::leaf(text = "yes", transform = |_| true)]
        Yes(bool),
        #[adze::leaf(text = "no", transform = |_| false)]
        No(bool),
    }
}

#[adze::grammar("variant_pattern")]
pub mod variant_pattern {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Choice {
        #[adze::leaf(pattern = r"\d+", try_transform = |text| text.parse::<u8>())]
        Number(u8),
        #[adze::leaf(pattern = r"[a-z]+", transform = |text| text.to_owned())]
        Word(String),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze::document::{AdzeDocument, ParseOutcome};
    fn assert_leaf(document: &AdzeDocument, source: &str, kind: &str) {
        assert_eq!(document.outcome(), ParseOutcome::Complete);
        assert!(document.diagnostics().is_empty());
        assert_eq!(document.tree().root().byte_range(), 0..source.len());
        let leaves: Vec<_> = (0..document.tree().node_count())
            .map(|i| {
                document
                    .tree()
                    .node(adze::document::NodeId::new(i))
                    .unwrap()
            })
            .filter(|node| node.flags().is_terminal())
            .collect();
        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0].kind_name(), Some(kind));
        assert_eq!(leaves[0].utf8_text().unwrap(), source);
        assert_eq!(leaves[0].byte_range(), 0..source.len());
    }
    #[test]
    fn test_disjoint_pattern_native_identity_selects_exact_values() {
        assert_leaf(&disjoint::parse_document("17").unwrap(), "17", r"/\d+/");
        assert_leaf(&disjoint::parse_document("x").unwrap(), "x", "/[a-z]+/");
        assert_eq!(disjoint::parse("17").unwrap(), disjoint::Choice::Number(17));
        assert_eq!(
            disjoint::parse("x").unwrap(),
            disjoint::Choice::Word("x".into())
        );
    }
    #[test]
    fn test_reversed_declaration_uses_selected_native_identity() {
        assert_leaf(&reversed::parse_document("17").unwrap(), "17", r"/\d+/");
        assert_leaf(&reversed::parse_document("x").unwrap(), "x", "/[a-z]+/");
        assert_eq!(
            reversed::parse("x").unwrap(),
            reversed::Choice::Word("x".into())
        );
        assert_eq!(reversed::parse("17").unwrap(), reversed::Choice::Number(17));
    }
    #[test]
    fn test_overlapping_patterns_follow_native_maximal_munch_and_tie() {
        assert_leaf(
            &overlapping::parse_document("abc1").unwrap(),
            "abc1",
            r"/\w+/",
        );
        assert_eq!(
            overlapping::parse("abc1").unwrap(),
            overlapping::Choice::Broad("abc1".into())
        );
        assert_leaf(
            &overlapping::parse_document("abc").unwrap(),
            "abc",
            "/[a-z]+/",
        );
        assert_eq!(
            overlapping::parse("abc").unwrap(),
            overlapping::Choice::Narrow("abc".into())
        );
    }
    #[test]
    fn test_selected_overlapping_transform_error_never_tries_next_variant() {
        assert_leaf(&selected_error::parse_document("x").unwrap(), "x", r"/\w+/");
        let errors = selected_error::parse("x").unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].start..errors[0].end, 0..1);
        assert!(
            matches!(&errors[0].reason, adze::errors::ParseErrorReason::ConversionFailed(message) if message == "selected broad rejected")
        );
    }
    #[test]
    fn test_selected_numeric_transform_error_preserves_span() {
        assert_leaf(&disjoint::parse_document("256").unwrap(), "256", r"/\d+/");
        let errors = disjoint::parse("256").unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].start..errors[0].end, 0..3);
        assert!(matches!(
            &errors[0].reason,
            adze::errors::ParseErrorReason::ConversionFailed(_)
        ));
    }
    #[test]
    fn test_single_pattern_value_and_fallible_error_remain_exact() {
        assert_leaf(
            &single_pattern::parse_document("17").unwrap(),
            "17",
            r"/\d+/",
        );
        assert_eq!(
            single_pattern::parse("17").unwrap(),
            single_pattern::Choice::Number(17)
        );
        let errors = single_pattern::parse("256").unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].start..errors[0].end, 0..3);
        assert!(matches!(
            &errors[0].reason,
            adze::errors::ParseErrorReason::ConversionFailed(_)
        ));
    }
    #[test]
    fn test_existing_literal_leaf_identity_dispatch_remains_exact() {
        assert_leaf(&literal::parse_document("yes").unwrap(), "yes", "\"yes\"");
        assert_leaf(&literal::parse_document("no").unwrap(), "no", "\"no\"");
        assert_eq!(literal::parse("yes").unwrap(), literal::Choice::Yes(true));
        assert_eq!(literal::parse("no").unwrap(), literal::Choice::No(false));
    }
    #[test]
    fn test_variant_level_regex_identity_selects_exact_values() {
        assert_leaf(
            &variant_pattern::parse_document("17").unwrap(),
            "17",
            r"/\d+/",
        );
        assert_leaf(
            &variant_pattern::parse_document("x").unwrap(),
            "x",
            "/[a-z]+/",
        );
        assert_eq!(
            variant_pattern::parse("17").unwrap(),
            variant_pattern::Choice::Number(17)
        );
        assert_eq!(
            variant_pattern::parse("x").unwrap(),
            variant_pattern::Choice::Word("x".into())
        );
    }
}
