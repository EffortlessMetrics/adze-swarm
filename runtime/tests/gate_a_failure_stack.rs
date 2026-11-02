//! Gate A failure stack (#1030/#875/#898/#864): streaming GLR malformed
//! syntax must never be Complete, extraction must be guarded, and a
//! conversion failure on valid syntax must run exactly once.

#![cfg(all(feature = "pure-rust", feature = "glr", feature = "runtime-e2e"))]

use std::sync::atomic::Ordering;

use adze::document::{NodeId, ParseOutcome};
use adze_example::native_failure_facts_1030::NATIVE_FAILURE_FACTS_1030_CALLS;
use adze_example::native_failure_facts_1030::grammar;
use adze_example::native_failure_facts_1030::grammar::Expr;

fn reset() {
    NATIVE_FAILURE_FACTS_1030_CALLS.store(0, Ordering::SeqCst);
}
fn calls() -> usize {
    NATIVE_FAILURE_FACTS_1030_CALLS.load(Ordering::SeqCst)
}

fn all_nodes(
    doc: &adze::document::AdzeDocument,
) -> Vec<(bool, bool, bool, std::ops::Range<usize>)> {
    let tree = doc.tree();
    (0..tree.node_count())
        .filter_map(|i| tree.node(NodeId::new(i)))
        .map(|n| (n.is_error(), n.is_missing(), n.has_error(), n.byte_range()))
        .collect()
}

#[test]
fn gate_a_streaming_malformed_and_conversion_contract() {
    // (b)+(c)+(d) "7 +" streaming: not Complete, diagnostics/counts/flags/JSON agree,
    // parse() agrees, ast()/ast_from_node reject BEFORE converter runs.
    for source in ["7 +"] {
        reset();
        let parse_errs = grammar::parse(source).expect_err("malformed must fail parse()");
        assert_eq!(
            calls(),
            0,
            "{source}: syntax failure must not run converter"
        );
        let doc = grammar::parse_document(source).expect("document must retain facts");
        assert_ne!(doc.outcome(), ParseOutcome::Complete, "{source} outcome");
        assert!(!doc.diagnostics().is_empty(), "{source} diagnostics");
        assert!(doc.metadata().error_count > 0, "{source} error_count");
        assert_eq!(
            doc.metadata().error_count,
            doc.diagnostics().len(),
            "{source} count==diags"
        );
        assert!(doc.tree().has_errors(), "{source} has_errors");
        assert!(doc.tree().root().has_error(), "{source} root has_error");
        let nodes = all_nodes(&doc);
        assert!(
            nodes.iter().any(|(e, m, _, _)| *e || *m),
            "{source} must have error/missing node, got {nodes:?}"
        );
        // diagnostic spans bounded and parse()/document agree on first span
        for d in doc.diagnostics() {
            assert!(
                d.start_byte <= d.end_byte && d.end_byte <= source.len(),
                "{source} bounded {d:?}"
            );
        }
        assert_eq!(
            parse_errs[0].start..parse_errs[0].end,
            doc.diagnostics()[0].byte_span(),
            "{source} parse/document first span agree: {parse_errs:?} vs {:?}",
            doc.diagnostics()
        );
        // related node, if present, must be flagged or have_error
        for d in doc.diagnostics() {
            for id in &d.related_nodes {
                let n = doc.tree().node(*id).expect("related node exists");
                assert!(
                    n.is_error() || n.is_missing() || n.has_error(),
                    "{source} related {id:?} unflagged"
                );
            }
        }
        #[cfg(feature = "serialization")]
        {
            let json = doc.to_json_value();
            assert_eq!(
                json["metadata"]["outcome"].as_str(),
                Some(doc.outcome().as_str())
            );
            assert_eq!(
                json["metadata"]["error_count"].as_u64(),
                Some(doc.metadata().error_count as u64)
            );
            assert_eq!(
                json["diagnostics"].as_array().map(|a| a.len()),
                Some(doc.diagnostics().len())
            );
        }
        // extraction guard: no converter invocation
        reset();
        assert!(doc.ast::<Expr>().is_err(), "{source} ast must reject");
        assert_eq!(calls(), 0, "{source} ast must reject before converter");
        let root_id = doc.tree().root().node_id();
        assert!(
            doc.ast_from_node::<Expr>(root_id).is_err(),
            "{source} ast_from_node must reject"
        );
        assert_eq!(
            calls(),
            0,
            "{source} ast_from_node must reject before converter"
        );
    }

    // (c) valid syntax, converter fails: Complete, exactly once, same facts parse vs ast
    reset();
    let doc = grammar::parse_document("2147483648").expect("doc");
    assert_eq!(
        doc.outcome(),
        ParseOutcome::Complete,
        "valid syntax stays Complete despite conversion"
    );
    assert!(doc.diagnostics().is_empty());
    assert_eq!(calls(), 0, "parse_document must not convert");
    let doc_errs = doc.ast::<Expr>().expect_err("conversion must fail");
    assert_eq!(calls(), 1, "ast conversion exactly once");
    reset();
    let parse_errs = grammar::parse("2147483648").expect_err("conversion must fail");
    assert_eq!(calls(), 1, "parse conversion exactly once (no LR reparse)");
    assert_eq!(
        parse_errs[0].start..parse_errs[0].end,
        doc_errs[0].start..doc_errs[0].end
    );
    assert_eq!(parse_errs[0].to_string(), doc_errs[0].to_string());
}

#[test]
fn gate_a_recognized_token_recovery_keeps_error_flag() {
    // #875: a recognised grammar token in an invalid position ("+" at
    // bytes 3..4) must keep its error flag in the document, and the
    // diagnostic must link to that flagged node; parse()/document agree.
    use adze_example::typed_ast_contract::grammar as det;
    let source = "1 ++2";
    let parse_errs = det::parse(source).expect_err("malformed must fail");
    let doc = det::parse_document(source).expect("document must retain facts");
    assert_ne!(doc.outcome(), ParseOutcome::Complete);
    assert!(!doc.diagnostics().is_empty());
    assert!(doc.metadata().error_count > 0);
    assert!(doc.tree().has_errors());
    let diag = &doc.diagnostics()[0];
    assert_eq!(diag.byte_span(), parse_errs[0].start..parse_errs[0].end);
    assert_eq!(diag.byte_span(), 3..4, "diagnostic points at second +");
    assert!(
        !diag.related_nodes.is_empty(),
        "diagnostic must link to a recovered node: {diag:?}"
    );
    let mut flagged = false;
    for id in &diag.related_nodes {
        let node = doc.tree().node(*id).expect("related node exists");
        if (node.is_error() || node.is_missing()) && node.byte_range() == (3..4) {
            flagged = true;
        }
    }
    assert!(
        flagged,
        "related node at 3..4 must carry error flag, diag={diag:?}"
    );
    assert!(
        doc.ast::<det::Expr>().is_err(),
        "recovered doc must reject ast()"
    );
}
