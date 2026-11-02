const GRAMMAR_NAME: &str = "mode_lookahead";
include!(concat!(env!("OUT_DIR"), "/generated.rs"));
#[cfg(test)]
mod tests {
    mod fixture {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/grammar.rs"));
    }
    use adze::document::ParseOutcome;
    use adze::glr_streaming_internal_lexer::{
        lex_generated_internal_at, make_generated_internal_streaming_lexer,
    };
    use adze_glr_core::{
        Action, Driver, FirstFollowSets, LexMode, ParseTable, build_lr1_automaton,
    };
    use adze_ir::SymbolId;
    fn table() -> ParseTable {
        let grammar = fixture::grammar();
        let sets = FirstFollowSets::compute(&grammar).unwrap();
        build_lr1_automaton(&grammar, &sets)
            .unwrap()
            .normalize_eof_to_zero()
            .with_detected_goto_indexing()
    }
    fn states(table: &ParseTable) -> (usize, usize) {
        let number = table.symbol_to_index[&SymbolId(2)];
        let word = table.symbol_to_index[&SymbolId(1)];
        assert!(
            word < number,
            "overlapping word must win erroneous cross-state tie"
        );
        let a = table
            .action_table
            .iter()
            .position(|row| {
                row[number]
                    .iter()
                    .any(|action| matches!(action, Action::Shift(_)))
                    && row[word]
                        .iter()
                        .all(|action| matches!(action, Action::Error))
            })
            .unwrap();
        let b = table
            .action_table
            .iter()
            .position(|row| {
                row[number]
                    .iter()
                    .any(|action| matches!(action, Action::Shift(_)))
                    && row[word]
                        .iter()
                        .any(|action| matches!(action, Action::Reduce(_)))
            })
            .unwrap();
        (a, b)
    }
    fn assert_native(source: &str, leaf: &str, range: std::ops::Range<usize>) {
        let mut parser = adze::pure_parser::Parser::new();
        parser.set_language(&super::LANGUAGE).unwrap();
        let parsed = parser.parse_string(source);
        assert!(parsed.errors.is_empty(), "{source}: {:?}", parsed.errors);
        let root = parsed.root.unwrap();
        assert_eq!(root.start_byte()..root.end_byte(), 0..source.len());
        let mut pending = vec![&root];
        let mut leaves = Vec::new();
        while let Some(node) = pending.pop() {
            if node.kind() == leaf {
                leaves.push(node.start_byte()..node.end_byte());
            }
            pending.extend(node.children());
        }
        assert_eq!(leaves, vec![range.clone()]);
        let document = super::parse_document(source).unwrap();
        assert_eq!(
            document.outcome(),
            ParseOutcome::Complete,
            "{:?}",
            document.diagnostics()
        );
        assert!(document.diagnostics().is_empty());
        assert_eq!(document.tree().root().byte_range(), 0..source.len());
        let leaves: Vec<_> = (0..document.tree().node_count())
            .map(|i| {
                document
                    .tree()
                    .node(adze::document::NodeId::new(i))
                    .unwrap()
            })
            .filter(|node| node.kind_name() == Some(leaf))
            .map(|node| (node.byte_range(), node.utf8_text().unwrap().to_owned()))
            .collect();
        assert_eq!(leaves, vec![(range.clone(), source[range].to_owned())]);
    }
    #[test]
    fn test_canonical_and_abi_modes_separate_distinct_usable_lookahead() {
        let table = table();
        let (a, b) = states(&table);
        assert_ne!(
            table.lex_modes[a].lex_state, table.lex_modes[b].lex_state,
            "identical shifts cannot merge different reduction lookahead"
        );
        assert_eq!(super::LANGUAGE.state_count as usize, table.state_count);
        let modes =
            unsafe { std::slice::from_raw_parts(super::LANGUAGE.lex_modes, table.state_count) };
        for (state, mode) in table.lex_modes.iter().enumerate() {
            assert_eq!(
                (modes[state].lex_state, modes[state].external_lex_state),
                (mode.lex_state, mode.external_lex_state),
                "ABI and streaming state {state} mode must agree"
            );
        }
    }
    #[test]
    fn test_emitted_mode_does_not_leak_overlapping_reduction_token() {
        let table = table();
        let (a, b) = states(&table);
        let number = table.symbol_to_index[&SymbolId(2)] as u32;
        let word = table.symbol_to_index[&SymbolId(1)] as u32;
        let a_token = lex_generated_internal_at(&super::LANGUAGE, "12", 0, table.lex_modes[a])
            .unwrap()
            .unwrap();
        assert_eq!((a_token.kind, a_token.start, a_token.end), (number, 0, 2));
        let b_token = lex_generated_internal_at(&super::LANGUAGE, "12", 0, table.lex_modes[b])
            .unwrap()
            .unwrap();
        assert_eq!((b_token.kind, b_token.start, b_token.end), (word, 0, 2));
        assert!(lex_generated_internal_at(&super::LANGUAGE, "foo", 0, table.lex_modes[a]).is_err());
    }
    #[test]
    fn test_native_and_document_overlap_keep_numeric_shift_value() {
        assert_native("a12", "number", 1..3);
        assert_native("b12", "word", 1..3);
    }
    #[test]
    fn test_reduction_only_word_lookahead_remains_usable() {
        assert_native("bfoo", "word", 1..4);
    }
    #[test]
    fn test_streaming_driver_executes_generated_abi_modes() {
        let table = table();
        for source in ["a12", "b12", "bfoo"] {
            let (lexer, _) = make_generated_internal_streaming_lexer(&super::LANGUAGE);
            let result = Driver::new(&table).parse_streaming(
                source,
                lexer,
                None::<
                    fn(&str, usize, &[bool], LexMode) -> Option<adze_glr_core::ts_lexer::NextToken>,
                >,
            );
            assert!(
                result.is_ok(),
                "actual streaming mode parse {source}: {:?}",
                result.as_ref().err()
            );
        }
    }
}
