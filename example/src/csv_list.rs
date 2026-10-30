#[adze::grammar("csv_list")]
pub mod grammar {
    use adze::Spanned;

    /// A comma-separated list of identifiers, e.g. `alpha, beta, gamma`
    #[adze::language]
    #[derive(Debug)]
    pub struct CsvList {
        #[adze::repeat(non_empty = true)]
        #[adze::delimited(
            #[adze::leaf(text = ",")]
            ()
        )]
        #[adze::leaf(pattern = r"[a-zA-Z_][a-zA-Z0-9_]*", transform = |v| v.to_string())]
        pub items: Spanned<Vec<Spanned<String>>>,
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
    fn csv_single_item_exact_1062() {
        // #1062: a delimited singleton selects exactly its element. The
        // repeat-aux sentinel previously surfaced as a phantom `""` second
        // item for `String` leaves (and panicked for numeric transforms).
        let parsed =
            grammar::parse("alpha").unwrap_or_else(|errors| panic!("singleton parses: {errors:?}"));
        let items: Vec<(&str, (usize, usize))> = parsed
            .items
            .value
            .iter()
            .map(|item| (item.value.as_str(), item.span))
            .collect();
        assert_eq!(items, vec![("alpha", (0, 5))]);
        assert_eq!(parsed.items.span, (0, 5));

        let doc = grammar::parse_document("alpha").expect("document parses");
        let via_doc: grammar::CsvList = doc.ast().expect("typed ast extracts");
        assert_eq!(via_doc.items.value.len(), 1);
        assert_eq!(via_doc.items.value[0].value, "alpha");
    }

    fn exact_items(input: &str) -> Vec<(String, (usize, usize))> {
        let parsed =
            grammar::parse(input).unwrap_or_else(|errors| panic!("{input:?} parses: {errors:?}"));
        parsed
            .items
            .value
            .iter()
            .map(|item| (item.value.clone(), item.span))
            .collect()
    }

    #[test]
    fn csv_two_items() {
        // #1095: multi-element delimited repetitions parse with exact
        // values, order, and spans.
        assert_eq!(
            exact_items("alpha, beta"),
            vec![("alpha".to_string(), (0, 5)), ("beta".to_string(), (7, 11))]
        );
    }

    #[test]
    fn csv_multiple_items() {
        assert_eq!(
            exact_items("a, b, c, d"),
            vec![
                ("a".to_string(), (0, 1)),
                ("b".to_string(), (3, 4)),
                ("c".to_string(), (6, 7)),
                ("d".to_string(), (9, 10)),
            ]
        );
    }

    #[test]
    fn csv_no_spaces() {
        assert_eq!(
            exact_items("x,y,z"),
            vec![
                ("x".to_string(), (0, 1)),
                ("y".to_string(), (2, 3)),
                ("z".to_string(), (4, 5)),
            ]
        );
    }

    #[test]
    fn csv_error_cases() {
        // Empty input (non_empty = true requires at least one)
        assert!(grammar::parse("").is_err());
        // Leading comma
        assert!(grammar::parse(", alpha").is_err());
        // Trailing comma
        assert!(grammar::parse("alpha,").is_err());
        // Just a number (doesn't match identifier pattern)
        assert!(grammar::parse("123").is_err());
    }
}
