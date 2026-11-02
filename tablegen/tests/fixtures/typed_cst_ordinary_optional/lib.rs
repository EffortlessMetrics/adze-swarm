#[adze::grammar("ordinary_optional_origin")]
pub mod grammar {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub struct Document {
        #[adze::leaf(text = "<")]
        _open: (),
        pub head: Option<Item>,
        #[adze::leaf(text = ">")]
        _close: (),
    }
    #[derive(Debug, PartialEq)]
    pub struct Item {
        #[adze::leaf(pattern = "[a-z]+", transform = |text| text.to_owned())]
        pub word: String,
    }
}

#[cfg(test)]
mod tests {
    use super::grammar;
    use adze::document::ParseOutcome;

    #[test]
    fn test_ordinary_optional_preserves_selected_value_and_absence() {
        for (source, expected) in [("<x>", Some("x")), ("<>", None)] {
            let value = grammar::parse(source).expect("ordinary Option typed parse");
            assert_eq!(value.head.as_ref().map(|item| item.word.as_str()), expected);
            let document = grammar::parse_document(source).expect("ordinary Option document");
            assert_eq!(document.outcome(), ParseOutcome::Complete);
            assert!(document.diagnostics().is_empty());
            assert_eq!(document.tree().root().byte_range(), 0..source.len());
            let items: Vec<_> = (0..document.tree().node_count())
                .map(|i| {
                    document
                        .tree()
                        .node(adze::document::NodeId::new(i))
                        .unwrap()
                })
                .filter(|node| node.kind_name() == Some("Item"))
                .collect();
            assert_eq!(items.len(), usize::from(expected.is_some()), "{document:?}");
            if expected.is_some() {
                assert_eq!(items[0].byte_range(), 1..2);
            }
        }
    }

    #[test]
    fn test_ordinary_optional_field_fact_belongs_to_source_owner() {
        for source in ["<x>", "<>"] {
            let document = grammar::parse_document(source).unwrap();
            let owners: Vec<_> = (0..document.tree().node_count())
                .map(|i| {
                    document
                        .tree()
                        .node(adze::document::NodeId::new(i))
                        .unwrap()
                })
                .filter(|node| node.kind_name() == Some("Document"))
                .collect();
            assert_eq!(owners.len(), 1, "one actual source semantic owner");
            assert_eq!(owners[0].byte_range(), 0..source.len());
            let fact = document
                .language()
                .checked_field_semantics(owners[0].grammar_id(), "head")
                .unwrap()
                .expect("source Document owns optional head, not allocation helper");
            assert!(!fact.required);
            assert!(!fact.multiple);
            assert_eq!(fact.target_kinds.len(), 1);
            assert_eq!(
                document.language().symbol_name(fact.target_kinds[0]),
                Some("Item")
            );
        }
    }
}
