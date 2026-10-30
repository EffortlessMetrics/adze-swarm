#[adze::grammar("repetitions")]
pub mod grammar {
    use adze::Spanned;

    #[adze::language]
    #[derive(Debug)]
    #[allow(dead_code)]
    pub struct NumberList {
        #[adze::repeat(non_empty = true)]
        #[adze::delimited(
            #[adze::leaf(text = ",")]
            ()
        )]
        #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
        pub numbers: Spanned<Vec<Spanned<i32>>>,
    }

    #[adze::extra]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s")]
        _whitespace: (),
    }
}

#[adze::grammar("repetitions_without_delim")]
pub mod grammar2 {
    use adze::Spanned;

    #[adze::language]
    #[derive(Debug)]
    #[allow(dead_code)]
    pub struct NumberList {
        #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
        pub numbers: Spanned<Vec<Spanned<i32>>>,
    }

    #[adze::extra]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s")]
        _whitespace: (),
    }
}

#[adze::grammar("repetitions_optional_elem")]
pub mod grammar3 {
    use adze::Spanned;

    #[adze::language]
    #[derive(Debug)]
    #[allow(dead_code)]
    pub struct NumberList {
        #[adze::delimited(
            #[adze::leaf(text = ",")]
            ()
        )]
        #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
        pub numbers: Spanned<Vec<Spanned<Option<i32>>>>,
        #[adze::skip(123)]
        pub metadata: u32,
    }

    #[adze::extra]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s")]
        _whitespace: (),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repetitions_grammar() {
        insta::assert_debug_snapshot!(grammar::parse(""));
        insta::assert_debug_snapshot!(grammar::parse("1"));
        insta::assert_debug_snapshot!(grammar::parse("1, 2"));
    }

    #[test]
    fn repetitions_grammar2() {
        insta::assert_debug_snapshot!(grammar2::parse(""));
        insta::assert_debug_snapshot!(grammar2::parse("1"));
        insta::assert_debug_snapshot!(grammar2::parse("1 2"));
    }

    #[test]
    fn repetitions_grammar3() {
        insta::assert_debug_snapshot!(grammar3::parse(""));
        insta::assert_debug_snapshot!(grammar3::parse("1,"));
        insta::assert_debug_snapshot!(grammar3::parse("1, 2"));
        insta::assert_debug_snapshot!(grammar3::parse("1,, 2"));
        insta::assert_debug_snapshot!(grammar3::parse("1,, 2,"));
    }

    #[test]
    fn repetitions_singleton_exact_1062() {
        // #1062: valid singletons return exact numeric values without panic,
        // dropped items, or default values. This is the terminal regression
        // for the reported `snapshot_repetitions_single` panic class (the
        // repeat-aux sentinel's empty text reached the leaf transform).
        for (input, value, span) in [("1", 1, (0, 1)), ("42", 42, (0, 2))] {
            let parsed = grammar::parse(input)
                .unwrap_or_else(|errors| panic!("{input:?} parses: {errors:?}"));
            let items: Vec<(i32, (usize, usize))> = parsed
                .numbers
                .value
                .iter()
                .map(|item| (item.value, item.span))
                .collect();
            assert_eq!(items, vec![(value, span)], "exact singleton for {input:?}");
            assert_eq!(parsed.numbers.span, span, "container span for {input:?}");
        }
    }

    #[test]
    fn repetitions_singleton_matches_document_ast_1062() {
        // #1062: `parse()` and `parse_document().ast()` agree on covered rows.
        let doc = grammar::parse_document("42").expect("document parses");
        let via_doc: grammar::NumberList = doc.ast().expect("typed ast extracts");
        let items: Vec<(i32, (usize, usize))> = via_doc
            .numbers
            .value
            .iter()
            .map(|item| (item.value, item.span))
            .collect();
        assert_eq!(items, vec![(42, (0, 2))]);
        assert_eq!(via_doc.numbers.span, (0, 2));
    }

    #[test]
    fn repetitions_empty_rejected_nonempty_1062() {
        // #1062: empty input obeys the explicit nonempty contract.
        let errors = grammar::parse("").expect_err("non_empty rejects empty input");
        assert!(!errors.is_empty(), "rejection carries bounded diagnostics");
    }

    #[test]
    fn repetitions_multi_delimited_exact_1095() {
        // #1095: multi-element delimited repetitions parse and extract exact
        // values, order, and spans (previously the generated table rejected
        // the first comma with `expected end`).
        let cases: &[(&str, Vec<(i32, (usize, usize))>, (usize, usize))] = &[
            ("1,2,3", vec![(1, (0, 1)), (2, (2, 3)), (3, (4, 5))], (0, 5)),
            (
                "1, 2, 3",
                vec![(1, (0, 1)), (2, (3, 4)), (3, (6, 7))],
                (0, 7),
            ),
            (
                "1 , 2 , 3",
                vec![(1, (0, 1)), (2, (4, 5)), (3, (8, 9))],
                (0, 9),
            ),
            (
                "1, 2, 3, 4, 5",
                vec![
                    (1, (0, 1)),
                    (2, (3, 4)),
                    (3, (6, 7)),
                    (4, (9, 10)),
                    (5, (12, 13)),
                ],
                (0, 13),
            ),
        ];
        for (input, expected, container) in cases {
            let parsed = grammar::parse(input)
                .unwrap_or_else(|errors| panic!("{input:?} parses: {errors:?}"));
            let items: Vec<(i32, (usize, usize))> = parsed
                .numbers
                .value
                .iter()
                .map(|item| (item.value, item.span))
                .collect();
            assert_eq!(&items, expected, "exact values for {input:?}");
            assert_eq!(
                parsed.numbers.span, *container,
                "container span for {input:?}"
            );
            for (value, span) in &items {
                assert_eq!(
                    &input[span.0..span.1],
                    &value.to_string(),
                    "span slices exactly its digits for {input:?}"
                );
            }
        }

        let doc = grammar::parse_document("1,2,3").expect("document parses");
        let via_doc: grammar::NumberList = doc.ast().expect("typed ast extracts");
        let doc_items: Vec<(i32, (usize, usize))> = via_doc
            .numbers
            .value
            .iter()
            .map(|item| (item.value, item.span))
            .collect();
        assert_eq!(doc_items, vec![(1, (0, 1)), (2, (2, 3)), (3, (4, 5))]);
        assert_eq!(via_doc.numbers.span, (0, 5));
    }

    #[test]
    fn repetitions_error_cases() {
        // Trailing comma
        assert!(grammar::parse("1, 2,").is_err());
        // Leading comma
        assert!(grammar::parse(",1, 2").is_err());
        // Just comma
        assert!(grammar::parse(",").is_err());
        // Letters
        assert!(grammar::parse("abc").is_err());
    }

    #[test]
    fn repetitions_grammar2_long() {
        insta::assert_debug_snapshot!("no_delim_long", grammar2::parse("1 2 3 4 5"));
        insta::assert_debug_snapshot!("no_delim_single", grammar2::parse("42"));
    }

    #[test]
    fn repetitions_undelimited_exact_1062() {
        // #1062: the zero-or-more undelimited grammar returns exact values,
        // order, and spans. Token spans slice exactly their digits even when
        // trailing whitespace attaches inside the token node.
        let cases: &[(&str, Vec<(i32, (usize, usize))>, (usize, usize))] = &[
            ("", vec![], (0, 0)),
            ("1", vec![(1, (0, 1))], (0, 1)),
            ("42", vec![(42, (0, 2))], (0, 2)),
            ("1 2", vec![(1, (0, 1)), (2, (2, 3))], (0, 3)),
            (
                "1 2 3 4 5",
                vec![
                    (1, (0, 1)),
                    (2, (2, 3)),
                    (3, (4, 5)),
                    (4, (6, 7)),
                    (5, (8, 9)),
                ],
                (0, 9),
            ),
            // Non-numeric input matches zero elements under zero-or-more.
            // Flagged, not blessed: trailing-vs-leading garbage acceptance
            // belongs to the diagnostics lane; pinned so it cannot drift
            // silently.
            ("abc", vec![], (0, 0)),
        ];
        for (input, expected, container) in cases {
            let parsed = grammar2::parse(input)
                .unwrap_or_else(|errors| panic!("{input:?} parses: {errors:?}"));
            let items: Vec<(i32, (usize, usize))> = parsed
                .numbers
                .value
                .iter()
                .map(|item| (item.value, item.span))
                .collect();
            assert_eq!(&items, expected, "exact values for {input:?}");
            assert_eq!(
                parsed.numbers.span, *container,
                "container span for {input:?}"
            );
            for (value, span) in &items {
                assert_eq!(
                    &input[span.0..span.1],
                    &value.to_string(),
                    "span slices exactly its digits for {input:?}"
                );
            }
        }
    }

    #[test]
    fn repetitions_undelimited_matches_document_ast_1062() {
        // #1062: `parse()` and `parse_document().ast()` agree on covered rows.
        let doc = grammar2::parse_document("1 2").expect("document parses");
        let via_doc: grammar2::NumberList = doc.ast().expect("typed ast extracts");
        let items: Vec<(i32, (usize, usize))> = via_doc
            .numbers
            .value
            .iter()
            .map(|item| (item.value, item.span))
            .collect();
        assert_eq!(items, vec![(1, (0, 1)), (2, (2, 3))]);
        assert_eq!(via_doc.numbers.span, (0, 3));
    }

    #[test]
    fn repetitions_undelimited_comma_prefix_is_explicit_1062() {
        // Flagged, not blessed: the undelimited grammar stops at `,` with no
        // diagnostic and returns the numeric prefix, silently dropping
        // `, 2`. True rejection of trailing garbage belongs to the
        // diagnostics lane. Pinned so the behavior cannot drift silently.
        let parsed = grammar2::parse("1, 2").expect("current contract: prefix accepted");
        let items: Vec<(i32, (usize, usize))> = parsed
            .numbers
            .value
            .iter()
            .map(|item| (item.value, item.span))
            .collect();
        assert_eq!(items, vec![(1, (0, 1))]);
        assert_eq!(parsed.numbers.span, (0, 1));
    }

    #[test]
    fn repetitions_optional_pair_exact_1062() {
        // #1062: the nested `Spanned<Vec<Spanned<Option<i32>>>>` fixture
        // preserves values, order, ranges, and the declared skipped metadata
        // for ordinary input. Previously this panicked (`InvalidDigit`: the
        // delimiter reached the leaf transform).
        let parsed =
            grammar3::parse("1, 2").unwrap_or_else(|errors| panic!("pair parses: {errors:?}"));
        let items: Vec<(Option<i32>, (usize, usize))> = parsed
            .numbers
            .value
            .iter()
            .map(|item| (item.value, item.span))
            .collect();
        assert_eq!(items, vec![(Some(1), (0, 1)), (Some(2), (3, 4))]);
        assert_eq!(parsed.numbers.span, (0, 4));
        assert_eq!(parsed.metadata, 123, "declared skipped metadata preserved");

        let doc = grammar3::parse_document("1, 2").expect("document parses");
        let via_doc: grammar3::NumberList = doc.ast().expect("typed ast extracts");
        let doc_items: Vec<(Option<i32>, (usize, usize))> = via_doc
            .numbers
            .value
            .iter()
            .map(|item| (item.value, item.span))
            .collect();
        assert_eq!(
            doc_items, items,
            "parse() agrees with parse_document().ast()"
        );
    }

    #[test]
    fn repetitions_optional_empty_contract_1062() {
        // #1062: zero-or-more accepts empty input with the declared default.
        let parsed =
            grammar3::parse("").unwrap_or_else(|errors| panic!("empty parses: {errors:?}"));
        assert!(parsed.numbers.value.is_empty());
        assert_eq!(parsed.numbers.span, (0, 0));
        assert_eq!(parsed.metadata, 123);
    }

    #[test]
    fn repetitions_optional_singleton_rejected_is_explicit_1062() {
        // Flagged, not blessed: the generated table demands a delimiter even
        // for one element (`"1"` and `"1,"` are rejected), which does not
        // follow from the zero-or-more delimited annotation. Owned by the
        // grammar-construction lane. Pinned so the contract cannot drift
        // silently.
        for input in ["1", "1,"] {
            let errors = grammar3::parse(input).expect_err("singleton rejected by current table");
            assert!(!errors.is_empty(), "bounded diagnostics for {input:?}");
        }
    }

    #[test]
    fn repetitions_optional_malformed_delimiters_rejected_1062() {
        // #1062: malformed leading/repeated/trailing delimiters are rejected
        // with bounded diagnostics. Empty-gap acceptance semantics are
        // undecided at the table level, so these rows pin rejection rather
        // than any gap value.
        for input in ["1,, 2", "1,, 2,", ",,,", ",,", ",1,2"] {
            let errors = grammar3::parse(input).expect_err("malformed delimiters rejected");
            assert!(!errors.is_empty(), "bounded diagnostics for {input:?}");
        }
    }
}
