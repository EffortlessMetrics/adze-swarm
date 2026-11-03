//! Generated native streaming EOF after declared extras (#1030/#875).
#![cfg(all(feature = "pure-rust", feature = "glr"))]

use adze_example::csv_list::grammar;
fn clean(source: &str) {
    assert_eq!(
        adze::__private::engine_dispatch_for_language(grammar::language()),
        adze::__private::EngineDispatch::StreamingGlr
    );
    let parsed =
        grammar::parse(source).expect("declared trailing extras after valid CSV must accept");
    let items: Vec<_> = parsed
        .items
        .value
        .iter()
        .map(|i| (i.value.as_str(), i.span))
        .collect();
    assert_eq!(items, vec![("alpha", (0, 5))]);
    let document = grammar::parse_document(source).expect("document must be available");
    assert_eq!(document.outcome(), adze::document::ParseOutcome::Complete);
    assert!(document.diagnostics().is_empty());
    let extracted: grammar::CsvList = document.ast().expect("document AST must accept");
    assert_eq!(extracted.items.value[0].value, "alpha");
    assert_eq!(extracted.items.value[0].span, (0, 5));
}
#[test]
fn test_generated_csv_accepts_trailing_lf_extra() {
    clean("alpha\n");
}
#[test]
fn test_generated_csv_accepts_trailing_crlf_extras() {
    clean("alpha\r\n");
}
#[test]
fn test_unknown_suffix_utf8_and_real_nul_are_not_eof() {
    for (source, range) in [
        ("alpha\n@", 6..7),
        ("alpha\nλ", 6..8),
        ("alpha\n\0", 6..7),
        ("alpha\0", 5..6),
    ] {
        let errors = grammar::parse(source).expect_err("unknown or NUL suffix must reject");
        assert_eq!(errors.len(), 1, "{source:?}: {errors:?}");
        assert_eq!(errors[0].byte_span(), range, "{source:?}");
        assert!(
            matches!(
                errors[0].reason,
                adze::errors::ParseErrorReason::UnexpectedToken(_)
            ),
            "{source:?}: {errors:?}"
        );
        assert_eq!(errors[0].expected, vec![",".to_string(), "end".to_string()]);
        let doc = grammar::parse_document(source).unwrap();
        assert_eq!(doc.outcome(), adze::document::ParseOutcome::Failed);
        assert_eq!(doc.diagnostics()[0].byte_span(), range);
    }
}

fn prepared_table() -> adze_glr_core::ParseTable {
    let language = grammar::language();
    let mut table = adze::decoder::decode_parse_table(language);
    adze::__private::align_true_glr_parse_table_to_language_symbols(language, &mut table);
    adze::glr_streaming_runtime::prepare_streaming_parse_table(language, table)
}

#[test]
fn test_generated_adapter_proves_eof_after_declared_extras_and_clears_stall() {
    use adze::glr_streaming_internal_lexer::{
        StreamingInternalLexError, lex_generated_internal_at,
        make_generated_internal_streaming_lexer,
    };
    let language = grammar::language();
    let table = prepared_table();
    let mode = table.lex_mode(table.initial_state);
    // The public single-call API retains its existing explicit end-of-source None.
    assert_eq!(
        lex_generated_internal_at(language, "\r\n", 0, mode),
        Ok(None)
    );
    let (mut lexer, stall) = make_generated_internal_streaming_lexer(language);
    assert!(lexer("\n@", 0, mode).is_none());
    assert_eq!(stall.get(), Some(1));
    let eof = lexer("\r\n", 0, mode).expect("successful extra scan must deliver EOF to Driver");
    assert_eq!(
        (eof.kind, eof.start, eof.end),
        (u32::from(language.eof_symbol), 2, 2)
    );
    assert_eq!(stall.get(), None);
    assert!(
        lexer("\r\n", 3, mode).is_none(),
        "out-of-range request is not EOF evidence"
    );
    for source in ["\n@", "\nλ", "\n\0"] {
        assert_eq!(
            lex_generated_internal_at(language, source, 0, mode),
            Err(StreamingInternalLexError::NoProgress { pos: 1 })
        );
        assert!(lexer(source, 0, mode).is_none());
        assert_eq!(stall.get(), Some(1));
    }
}

#[test]
fn test_generated_adapter_keeps_requested_mode_and_absolute_token_ranges() {
    use adze::glr_streaming_internal_lexer::lex_generated_internal_at;
    let language = grammar::language();
    let table = prepared_table();
    let initial = table.lex_mode(table.initial_state);
    let first = lex_generated_internal_at(language, "zalpha", 1, initial)
        .unwrap()
        .unwrap();
    assert_eq!((first.start, first.end), (1, 6));
    assert_eq!(
        lex_generated_internal_at(language, "zalpha", 1, initial).unwrap(),
        Some(first)
    );
    assert!(lex_generated_internal_at(language, ",", 0, initial).is_err());
    let mode = table
        .lex_modes
        .iter()
        .copied()
        .find(|mode| {
            lex_generated_internal_at(language, ",", 0, *mode).is_ok_and(|token| token.is_some())
        })
        .expect("CSV comma has an admitted noninitial lex mode");
    assert_ne!(mode.lex_state, initial.lex_state);
    let delimiter = lex_generated_internal_at(language, ",", 0, mode)
        .unwrap()
        .unwrap();
    assert_ne!(delimiter.kind, u32::from(language.eof_symbol));
    assert_eq!((delimiter.start, delimiter.end), (0, 1));
}

#[test]
fn test_streaming_missing_item_after_trailing_extras_uses_physical_eof() {
    use adze::glr_streaming_internal_lexer::make_generated_internal_streaming_lexer;
    use adze_glr_core::ts_lexer::NextToken;
    use adze_glr_core::{Driver, LexMode};
    for source in ["alpha,", "alpha,\n", "alpha,\r\n"] {
        let table = prepared_table();
        let (lexer, _) = make_generated_internal_streaming_lexer(grammar::language());
        let forest = Driver::new(&table)
            .parse_streaming(
                source,
                lexer,
                None::<fn(&str, usize, &[bool], LexMode) -> Option<NextToken>>,
            )
            .expect("native missing-item recovery must see actual EOF");
        let view = forest.view();
        let mut work = view.roots().to_vec();
        let mut seen = std::collections::HashSet::new();
        let mut missing = std::collections::BTreeSet::new();
        while let Some(id) = work.pop() {
            if !seen.insert(id) {
                continue;
            }
            if view.error_meta(id).missing {
                let span = view.span(id);
                missing.insert((span.start as usize, span.end as usize));
            }
            work.extend_from_slice(view.best_children(id));
        }
        assert_eq!(
            missing,
            std::collections::BTreeSet::from([(source.len(), source.len())]),
            "{source:?}"
        );
        let typed = grammar::parse(source).expect_err("missing item cannot extract a valid CSV");
        assert_eq!(typed.len(), 1);
        assert_eq!(typed[0].byte_span(), 5..6);
        assert_eq!(typed[0].expected, ["end"]);
        let doc = grammar::parse_document(source).unwrap();
        assert_eq!(doc.outcome(), adze::document::ParseOutcome::Recovered);
        assert_eq!(doc.diagnostics().len(), 1);
        assert_eq!(doc.diagnostics()[0].byte_span(), 5..6);
        assert_eq!(doc.diagnostics()[0].found.as_deref(), Some(","));
        assert_eq!(doc.diagnostics()[0].expected, ["end"]);
    }
}
