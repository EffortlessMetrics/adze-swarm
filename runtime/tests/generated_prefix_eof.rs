//! Generated lexical failures cannot masquerade as physical EOF (#1174).

#![cfg(feature = "pure-rust")]

use adze::document::ParseOutcome;
use adze_example::typed_ast_contract::grammar as deterministic;

#[test]
fn test_deterministic_complete_input_retains_exact_values_and_true_eof() {
    assert_eq!(
        deterministic::parse("17").unwrap(),
        deterministic::Expr::Number(17)
    );
    assert_eq!(
        deterministic::parse("17+2").unwrap(),
        deterministic::Expr::Add(
            Box::new(deterministic::Expr::Number(17)),
            (),
            Box::new(deterministic::Expr::Number(2)),
        )
    );
    let document = deterministic::parse_document("17").unwrap();
    assert_eq!(document.outcome(), ParseOutcome::Complete);
    assert_eq!(document.tree().root().byte_range(), 0..2);
    assert!(document.diagnostics().is_empty());
    let errors = deterministic::parse("17+").expect_err("actual EOF is still incomplete");
    assert_eq!(errors[0].byte_span(), 3..3);
    assert!(matches!(
        errors[0].reason,
        adze::errors::ParseErrorReason::UnexpectedEof
    ));
}

#[test]
fn test_deterministic_unknown_suffix_has_source_bounded_failure_facts() {
    for (source, span, found) in [
        ("17x", 2..3, "x"),
        ("17λ", 2..4, "λ"),
        ("17\0", 2..3, "\0"),
        ("17 x", 3..4, "x"),
    ] {
        let errors =
            deterministic::parse(source).expect_err("unknown suffix cannot accept a prefix");
        assert_eq!(errors[0].byte_span(), span, "{source:?}");
        assert!(matches!(
            errors[0].reason,
            adze::errors::ParseErrorReason::UnexpectedToken(_)
        ));
        let document =
            deterministic::parse_document(source).expect("retain invalid-input document");
        assert_ne!(document.outcome(), ParseOutcome::Complete, "{source:?}");
        assert_eq!(
            document.diagnostics().len(),
            1,
            "one invalid scalar: {source:?}"
        );
        let diagnostic = &document.diagnostics()[0];
        assert_eq!(diagnostic.byte_span(), span, "{source:?}");
        assert_eq!(diagnostic.found.as_deref(), Some(found), "{source:?}");
        assert_eq!(diagnostic.expected, errors[0].expected, "{source:?}");
        assert!(document.tree().has_errors());
        let node = diagnostic
            .related_nodes
            .iter()
            .find_map(|id| {
                document
                    .tree()
                    .node(*id)
                    .filter(|node| node.is_error() && node.byte_range() == span)
            })
            .expect("diagnostic must retain the offending error node, not missing EOF");
        assert!(!node.is_missing(), "non-EOF source is not a missing token");
        assert!(document.ast::<deterministic::Expr>().is_err());
        assert_eq!(
            deterministic::parse("17").unwrap(),
            deterministic::Expr::Number(17)
        );
    }
}

#[cfg(feature = "glr")]
#[test]
fn test_streaming_unknown_suffix_uses_native_failure_without_conversion() {
    use adze::__private::{EngineDispatch, engine_dispatch_for_language};
    use adze_example::native_failure_facts_1030::{
        NATIVE_FAILURE_FACTS_1030_CALLS as calls, grammar as conflicted,
    };
    use std::sync::atomic::Ordering;
    assert_eq!(
        engine_dispatch_for_language(conflicted::language()),
        EngineDispatch::StreamingGlr
    );
    assert_eq!(
        conflicted::parse("17").unwrap(),
        conflicted::Expr::Number(17)
    );
    for (source, span, found) in [("17x", 2..3, "x"), ("17 λ", 3..5, "λ")] {
        calls.store(0, Ordering::SeqCst);
        let errors = conflicted::parse(source).expect_err("streaming route must reject suffix");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "syntax failure cannot invoke conversion"
        );
        assert_eq!(errors[0].byte_span(), span);
        let document = conflicted::parse_document(source).expect("native failure document");
        assert_ne!(document.outcome(), ParseOutcome::Complete);
        assert_eq!(document.diagnostics()[0].byte_span(), span);
        assert_eq!(document.diagnostics()[0].found.as_deref(), Some(found));
        assert_eq!(document.diagnostics()[0].expected, errors[0].expected);
        assert!(document.ast::<conflicted::Expr>().is_err());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "no observer reparse or converter"
        );
    }
}

#[test]
fn test_deterministic_reduction_lookahead_preserves_complete_repetitions() {
    use adze_example::repetitions::grammar2;
    for (source, expected) in [
        ("1", vec![(1, (0, 1))]),
        ("1 2", vec![(1, (0, 1)), (2, (2, 3))]),
        ("1 2 3", vec![(1, (0, 1)), (2, (2, 3)), (3, (4, 5))]),
    ] {
        let parsed = grammar2::parse(source).expect("valid numbers after reductions");
        let values: Vec<_> = parsed
            .numbers
            .value
            .iter()
            .map(|item| (item.value, item.span))
            .collect();
        assert_eq!(values, expected, "{source}");
        let document = grammar2::parse_document(source).unwrap();
        assert_eq!(document.outcome(), ParseOutcome::Complete);
        let via_document: grammar2::NumberList = document.ast().unwrap();
        let values: Vec<_> = via_document
            .numbers
            .value
            .iter()
            .map(|item| (item.value, item.span))
            .collect();
        assert_eq!(values, expected, "document: {source}");
    }
    for source in ["abc", "1x", "1, 2"] {
        assert!(
            grammar2::parse(source).is_err(),
            "invalid suffix cannot become a prefix: {source}"
        );
    }
    // Keep conversion coverage on a declared fallible numeric leaf.
    let errors =
        deterministic::parse("1+2147483648").expect_err("second number still converts fallibly");
    assert!(matches!(
        errors[0].reason,
        adze::errors::ParseErrorReason::ConversionFailed(_)
    ));
    assert_eq!(errors[0].byte_span(), 2..12);
}
