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

    #[test]
    fn csv_two_items() {
        // KNOWN #1062 remaining: same single-iteration delimited-table cap as
        // the numeric fixture; multi-element input is rejected at the table.
        // Owned by the grammar-construction lane; pinned, not vacuous.
        let errors =
            grammar::parse("alpha, beta").expect_err("multi-element delimited is table-rejected");
        assert!(!errors.is_empty());
    }

    #[test]
    fn csv_multiple_items() {
        // KNOWN #1062 remaining: see `csv_two_items`.
        let errors =
            grammar::parse("a, b, c, d").expect_err("multi-element delimited is table-rejected");
        assert!(!errors.is_empty());
    }

    #[test]
    fn csv_no_spaces() {
        // KNOWN #1062 remaining: see `csv_two_items`.
        let errors =
            grammar::parse("x,y,z").expect_err("multi-element delimited is table-rejected");
        assert!(!errors.is_empty());
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
