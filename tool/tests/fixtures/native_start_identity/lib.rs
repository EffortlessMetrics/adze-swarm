macro_rules! imported {
    ($module:ident, $file:literal) => {
        mod $module {
            include!(concat!(env!("OUT_DIR"), "/", $file));
        }
    };
}
imported!(pattern_choice, "imported_pattern_choice.rs");
imported!(pattern_sequence, "imported_pattern_sequence.rs");
imported!(literal_choice, "imported_literal_choice.rs");
imported!(literal_sequence, "imported_literal_sequence.rs");
#[cfg(test)]
mod tests {
    use super::*;
    use adze::document::{AdzeDocument, NodeId, ParseOutcome};
    fn assert_leaf(document: &AdzeDocument, source: &str, binding: u16, wrapper: u16, start: u16) {
        assert_eq!(document.outcome(), ParseOutcome::Complete);
        assert!(document.diagnostics().is_empty());
        assert_eq!(document.tree().root().byte_range(), 0..source.len());
        let nodes: Vec<_> = (0..document.tree().node_count())
            .map(NodeId::new)
            .map(|id| document.tree().node(id).unwrap())
            .collect();
        // This control isolates the imported-reference lowering at the
        // declared semantic owner. The independently retained global root
        // identity red is not relaxed by this additional scoped oracle.
        let owners: Vec<_> = nodes
            .iter()
            .filter(|node| node.grammar_id().0 == start)
            .collect();
        assert_eq!(owners.len(), 1);
        assert_eq!(owners[0].byte_range(), 0..source.len());
        assert_eq!(owners[0].child_count(), 1);
        assert_ne!(owners[0].child(0).unwrap().grammar_id().0, wrapper);
        let selected: Vec<_> = nodes
            .iter()
            .filter(|node| node.grammar_id().0 == binding)
            .collect();
        assert_eq!(selected.len(), 1);
        assert!(selected[0].flags().is_terminal());
        assert_eq!(selected[0].utf8_text().unwrap(), source);
        assert_eq!(selected[0].byte_range(), 0..source.len());
        assert_eq!(owners[0].child(0).unwrap().node_id(), selected[0].node_id());
    }
    fn assert_other_choice(document: &AdzeDocument, binding: u16, wrapper: u16, start: u16) {
        assert_eq!(document.outcome(), ParseOutcome::Complete);
        assert!(document.diagnostics().is_empty());
        let owner = (0..document.tree().node_count())
            .map(NodeId::new)
            .map(|id| document.tree().node(id).unwrap())
            .find(|node| node.grammar_id().0 == start)
            .unwrap();
        let child = owner.child(0).unwrap();
        assert!(child.flags().is_terminal());
        assert_eq!(child.utf8_text().unwrap(), "!");
        assert_eq!(child.byte_range(), 0..1);
        assert_ne!(child.grammar_id().0, binding);
        assert_ne!(child.grammar_id().0, wrapper);
    }
    #[test]
    fn test_imported_pattern_choice_preserves_direct_actual_token() {
        assert_leaf(
            &pattern_choice::parse_document(pattern_choice::SOURCE).unwrap(),
            pattern_choice::SOURCE,
            pattern_choice::BINDING,
            pattern_choice::WRAPPER,
            pattern_choice::START,
        );
        assert_other_choice(
            &pattern_choice::parse_document("!").unwrap(),
            pattern_choice::BINDING,
            pattern_choice::WRAPPER,
            pattern_choice::START,
        );
    }
    #[test]
    fn test_imported_pattern_sequence_preserves_direct_actual_token() {
        assert_leaf(
            &pattern_sequence::parse_document(pattern_sequence::SOURCE).unwrap(),
            pattern_sequence::SOURCE,
            pattern_sequence::BINDING,
            pattern_sequence::WRAPPER,
            pattern_sequence::START,
        );
    }
    #[test]
    fn test_imported_literal_choice_preserves_direct_actual_token() {
        assert_leaf(
            &literal_choice::parse_document(literal_choice::SOURCE).unwrap(),
            literal_choice::SOURCE,
            literal_choice::BINDING,
            literal_choice::WRAPPER,
            literal_choice::START,
        );
        assert_other_choice(
            &literal_choice::parse_document("!").unwrap(),
            literal_choice::BINDING,
            literal_choice::WRAPPER,
            literal_choice::START,
        );
    }
    #[test]
    fn test_imported_literal_sequence_preserves_direct_actual_token() {
        assert_leaf(
            &literal_sequence::parse_document(literal_sequence::SOURCE).unwrap(),
            literal_sequence::SOURCE,
            literal_sequence::BINDING,
            literal_sequence::WRAPPER,
            literal_sequence::START,
        );
    }
    fn assert_global_root(document: &AdzeDocument, start: u16, wrapper: u16) {
        assert_eq!(document.outcome(), ParseOutcome::Complete);
        assert!(document.diagnostics().is_empty());
        assert_eq!(
            document.tree().root().grammar_id().0,
            start,
            "native document root must preserve declared source identity: {document:?}"
        );
        assert!(
            (0..document.tree().node_count()).all(|index| document
                .tree()
                .node(NodeId::new(index))
                .unwrap()
                .grammar_id()
                .0
                != wrapper),
            "the original global no-semantic-intermediate oracle remains required"
        );
    }
    #[test]
    fn test_imported_pattern_choice_global_root_preserves_declared_start() {
        assert_global_root(
            &pattern_choice::parse_document(pattern_choice::SOURCE).unwrap(),
            pattern_choice::START,
            pattern_choice::WRAPPER,
        );
    }
    #[test]
    fn test_imported_literal_choice_global_root_preserves_declared_start() {
        assert_global_root(
            &literal_choice::parse_document(literal_choice::SOURCE).unwrap(),
            literal_choice::START,
            literal_choice::WRAPPER,
        );
    }
}

mod sparse_ambiguous {
    include!(concat!(env!("OUT_DIR"), "/sparse_ambiguous.rs"));
}

mod sparse {
    include!(concat!(env!("OUT_DIR"), "/sparse.rs"));
}

#[adze::grammar("checked_number")]
mod number {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub struct Number {
        #[adze::leaf(pattern = "[0-9]+", transform = |s| s.parse::<u32>().unwrap())]
        pub value: u32,
    }
}

#[adze::grammar("checked_else")]
mod conditional {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Statement {
        IfThen(
            #[adze::leaf(text = "if")] (),
            Box<Expr>,
            #[adze::leaf(text = "then")] (),
            Box<Statement>,
        ),
        IfThenElse(
            #[adze::leaf(text = "if")] (),
            Box<Expr>,
            #[adze::leaf(text = "then")] (),
            Box<Statement>,
            #[adze::leaf(text = "else")] (),
            Box<Statement>,
        ),
        Other(#[adze::leaf(text = "other")] ()),
    }
    #[derive(Debug, PartialEq)]
    pub enum Expr {
        Var(#[adze::leaf(pattern = "[a-z]+")] String),
    }
    #[adze::extra]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s")]
        _whitespace: (),
    }
}

#[cfg(test)]
mod checked {
    use super::*;
    use adze::errors::ParseErrorReason;
    use adze::generated_identity::{NativeIdentityError as E, NativeLanguageIdentityV1};
    fn identity_failure(identity: Option<&NativeLanguageIdentityV1>) -> E {
        let errors = adze::__private::parse_document_with_identity(
            "x",
            &sparse::LANGUAGE,
            identity,
            "sparse_entry",
        )
        .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!((errors[0].start, errors[0].end), (0, 0));
        let typed_errors = adze::__private::parse_with_identity::<number::Number>(
            "x",
            &sparse::LANGUAGE,
            identity,
        )
        .unwrap_err();
        assert_eq!(typed_errors.len(), 1);
        match (&errors[0].reason, &typed_errors[0].reason) {
            (
                ParseErrorReason::InvalidLanguageIdentity(error),
                ParseErrorReason::InvalidLanguageIdentity(typed_error),
            ) => {
                assert_eq!(error, typed_error);
                error.clone()
            }
            other => panic!("wrong initialization errors: {other:?}"),
        }
    }
    #[test]
    fn test_sparse_renamed_entry_uses_explicit_identity() {
        let identity = &sparse::START_IDENTITY;
        assert_eq!((identity.analysis_start, identity.document_start), (17, 17));
        assert!(identity.public_symbols.contains(&150));
        let table = identity.decode_for(&sparse::LANGUAGE).unwrap();
        assert_eq!(table.start_symbol.0, 17);
        assert_eq!(table.grammar.explicit_start_symbol().unwrap().0, 17);
        assert!(table.grammar.rules.contains_key(&table.start_symbol));
        assert_eq!(
            adze::__private::engine_dispatch_for_table(&sparse::LANGUAGE, &table),
            adze::__private::EngineDispatch::DeterministicLr
        );
        let doc = sparse::parse_document("x").unwrap();
        assert!(doc.diagnostics().is_empty());
        assert_eq!(doc.tree().root().grammar_id().0, 17);
        assert_eq!(doc.tree().root().kind().unwrap().name(), "renamed_entry");
        assert_eq!(doc.tree().root().byte_range(), 0..1);
        assert_eq!(doc.tree().root().child(0).unwrap().grammar_id().0, 99);
    }
    #[test]
    fn test_checked_identity_rejects_unavailable_version_and_foreign_pairing() {
        assert_eq!(identity_failure(None), E::Unavailable);
        let mut identity = sparse::START_IDENTITY;
        identity.version = 2;
        assert_eq!(identity_failure(Some(&identity)), E::UnsupportedVersion(2));
        identity = sparse::START_IDENTITY;
        identity.language = &pattern_choice::LANGUAGE;
        assert_eq!(identity_failure(Some(&identity)), E::LanguageMismatch);
        // Equal native bytes and equal names still do not identify one allocation.
        identity.language = Box::leak(Box::new(sparse::LANGUAGE));
        assert_eq!(identity_failure(Some(&identity)), E::LanguageMismatch);
    }
    #[test]
    fn test_checked_identity_rejects_unknown_terminal_and_stale_map() {
        let mut identity = sparse::START_IDENTITY;
        identity.analysis_start = 65535;
        assert_eq!(
            identity_failure(Some(&identity)),
            E::InvalidStart {
                role: "analysis",
                symbol: 65535
            }
        );
        identity = sparse::START_IDENTITY;
        identity.document_start = 99;
        assert_eq!(
            identity_failure(Some(&identity)),
            E::InvalidStart {
                role: "document",
                symbol: 99
            }
        );
        identity = sparse::START_IDENTITY;
        identity.public_symbols = &[0, 99, 150, 17, 2];
        assert_eq!(identity_failure(Some(&identity)), E::InvalidPublicMap);
        identity.public_symbols = &[0, 99, 99, 2, 17];
        assert_eq!(identity_failure(Some(&identity)), E::InvalidPublicMap);
        let mut duplicate_native = sparse::LANGUAGE;
        duplicate_native.public_symbol_map = identity.public_symbols.as_ptr();
        let duplicate_native = Box::leak(Box::new(duplicate_native));
        identity.language = duplicate_native;
        assert_eq!(
            identity.validate_for(duplicate_native),
            Err(E::InvalidPublicMap)
        );
    }
    #[test]
    fn test_generated_lr_typed_and_document_roles_are_distinct() {
        assert_eq!(number::parse("17").unwrap(), number::Number { value: 17 });
        let identity = &number::START_IDENTITY;
        assert_ne!(identity.analysis_start, identity.document_start);
        let doc = number::parse_document("17").unwrap();
        assert!(doc.diagnostics().is_empty());
        assert_eq!(doc.tree().root().grammar_id().0, identity.document_start);
        assert_eq!(doc.tree().root().kind().unwrap().name(), "source_file");
        assert_eq!(doc.tree().root().byte_range(), 0..2);
        assert_eq!(
            doc.ast::<number::Number>().unwrap(),
            number::Number { value: 17 }
        );
    }
    #[test]
    fn test_raw_helpers_remain_callable_without_canonical_identity_claim() {
        let mut parser = adze::pure_parser::Parser::new();
        parser.set_language(&sparse::LANGUAGE).unwrap();
        let parsed = parser.parse_string("x");
        assert!(parsed.errors.is_empty());
        let raw_root = parsed.root.unwrap();
        assert_eq!(
            sparse::START_IDENTITY.public_symbols[raw_root.symbol as usize],
            17
        );
        assert_eq!(
            adze::__private::parse::<number::Number>("17", number::language).unwrap(),
            number::Number { value: 17 }
        );
        let _legacy_table = adze::decoder::decode_parse_table(&sparse::LANGUAGE);
        let _legacy_document =
            adze::__private::parse_document("x", || &sparse::LANGUAGE, "legacy").unwrap();
        // These old signatures are callable; no canonical start is inferred by this oracle.
    }
    #[cfg(feature = "glr")]
    #[test]
    fn test_generated_conflicted_route_retains_nearest_else_and_document_owner() {
        use conditional::{Expr, Statement as S};
        let source = "if a then if b then other else other";
        let identity = &conditional::START_IDENTITY;
        let table = identity.decode_for(&conditional::LANGUAGE).unwrap();
        assert_eq!(
            adze::__private::engine_dispatch_for_table(&conditional::LANGUAGE, &table),
            adze::__private::EngineDispatch::StreamingGlr
        );
        let expected = S::IfThen(
            (),
            Box::new(Expr::Var("a".into())),
            (),
            Box::new(S::IfThenElse(
                (),
                Box::new(Expr::Var("b".into())),
                (),
                Box::new(S::Other(())),
                (),
                Box::new(S::Other(())),
            )),
        );
        assert_eq!(conditional::parse(source).unwrap(), expected);
        let doc = conditional::parse_document(source).unwrap();
        assert!(doc.diagnostics().is_empty());
        assert!(!doc.ambiguities().is_empty());
        assert_eq!(doc.tree().root().grammar_id().0, identity.document_start);
        assert_eq!(doc.tree().root().kind().unwrap().name(), "source_file");
        assert_eq!(doc.tree().root().byte_range(), 0..source.len());
        assert_eq!(doc.ast::<S>().unwrap(), expected);
    }
    #[cfg(feature = "glr")]
    #[test]
    fn test_execution_view_keeps_rules_and_grammar_in_one_namespace() {
        let public = sparse::START_IDENTITY
            .decode_for(&sparse::LANGUAGE)
            .unwrap();
        let execution = adze::glr_streaming_runtime::prepare_streaming_parse_table(
            &sparse::LANGUAGE,
            public.clone(),
        );
        let raw_start = public.symbol_to_index[&public.start_symbol] as u16;
        assert_eq!(execution.start_symbol.0, raw_start);
        assert_eq!(
            execution.grammar.explicit_start_symbol().unwrap().0,
            raw_start
        );
        assert_eq!(
            execution.grammar.document_start_symbol.unwrap().0,
            raw_start
        );
        // This view promises the execution namespace, not a repair of the
        // legacy decoder's separate token-role classification.
        assert!(
            execution
                .grammar
                .tokens
                .keys()
                .all(|id| usize::from(id.0) < execution.symbol_count)
        );
        let public_token = *public.index_to_symbol.iter().find(|id| id.0 == 99).unwrap();
        let token_column = execution.index_to_symbol[public.symbol_to_index[&public_token]];
        assert!(execution.grammar.tokens.contains_key(&token_column));
        assert!(!execution.grammar.tokens.contains_key(&public_token));
        for (decoded, syntax) in execution.grammar.all_rules().zip(&execution.rules) {
            assert_eq!(decoded.lhs, syntax.lhs);
        }
        assert_eq!(public.grammar.explicit_start_symbol().unwrap().0, 17);
        assert!(public.grammar.tokens.keys().any(|id| id.0 == 99));
    }
    #[cfg(feature = "glr")]
    #[test]
    fn test_nonidentity_no_lex_native_route_keeps_execution_tokens_dense() {
        let mut raw = sparse_ambiguous::LANGUAGE;
        raw.lex_fn = None;
        let language = Box::leak(Box::new(raw));
        let mut identity = sparse_ambiguous::START_IDENTITY;
        identity.language = language;
        let table = identity.decode_for(language).unwrap();
        assert_eq!(
            adze::__private::engine_dispatch_for_table(language, &table),
            adze::__private::EngineDispatch::LegacyGlrWithoutLexFn
        );
        let doc = adze::__private::parse_document_with_identity(
            "x",
            language,
            Some(&identity),
            "sparse_ambiguous",
        )
        .unwrap();
        assert_eq!(
            doc.outcome(),
            adze::document::ParseOutcome::Complete,
            "{doc:?}"
        );
        assert!(doc.diagnostics().is_empty());
        assert_eq!(doc.tree().root().grammar_id().0, 17);
        assert_eq!(doc.tree().root().byte_range(), 0..1);
        let leaf = doc.tree().root().child(0).unwrap();
        assert_eq!(leaf.grammar_id().0, 99);
        assert_eq!(leaf.utf8_text().unwrap(), "x");
        assert!(leaf.flags().is_terminal());
    }
    #[cfg(feature = "glr")]
    #[test]
    fn test_checked_glr_bad_prefix_and_mid_input_have_exact_native_diagnostics_then_clean() {
        for (source, start) in [("$ if a then other", 0), ("if a then $ other", 10)] {
            let errors = conditional::parse(source).unwrap_err();
            assert_eq!(errors.len(), 1, "{errors:?}");
            assert_eq!((errors[0].start, errors[0].end), (start, start + 1));
            assert_eq!(errors[0].expected, vec!["\"other\"", "if"]);
            assert!(
                matches!(&errors[0].reason,ParseErrorReason::UnexpectedToken(message) if message.starts_with('$')),
                "{errors:?}"
            );
            let doc = conditional::parse_document(source).unwrap();
            assert_eq!(doc.diagnostics().len(), 1, "{doc:?}");
            let diagnostic = &doc.diagnostics()[0];
            assert_eq!(
                (diagnostic.start_byte, diagnostic.end_byte),
                (start, start + 1)
            );
            assert_eq!(diagnostic.expected, vec!["\"other\"", "if"]);
            assert_eq!(diagnostic.found.as_deref(), Some("$"));
            assert_eq!(
                doc.tree().root().grammar_id().0,
                conditional::START_IDENTITY.document_start
            );
        }
        use conditional::{Expr, Statement as S};
        let expected = S::IfThen(
            (),
            Box::new(Expr::Var("a".into())),
            (),
            Box::new(S::Other(())),
        );
        assert_eq!(conditional::parse("if a then other").unwrap(), expected);
        assert!(
            conditional::parse_document("if a then other")
                .unwrap()
                .diagnostics()
                .is_empty()
        );
    }
}
