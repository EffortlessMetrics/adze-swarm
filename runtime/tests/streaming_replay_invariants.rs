//! Bounded streaming/batch replay invariants (#1071 slice).
//!
//! Fixed corpus, fixed seed, no randomness. Proves on replayable inputs:
//! 1. Batch (fixed pretokenization bridge) and streaming (stack-aware
//!    driver) routes agree with the production `parse_document` surface on
//!    accepted outcomes for conflicted grammars; the deterministic grammar
//!    is covered through its production `parse`/`parse_document` surfaces.
//! 2. `validate_structure() == Ok` plus root/parent/edge symmetry and
//!    bounded spans hold on every replayed document, accepted or rejected.
//! 3. Multiparse inputs select a deterministic winner across repeated runs.
//!
//! Raw bridge/driver invocation is in-contract only for streaming-routed
//! (conflicted) tables; the deterministic table routes to DeterministicLr
//! in production, so it is exercised via production surfaces only.
//! Fixture patterns reuse `glr_route_gate_equivalence.rs` (fixed bridge)
//! and `engine_convergence_dispatch.rs` (parse/document agreement).

#![cfg(all(feature = "pure-rust", feature = "glr", feature = "runtime-e2e"))]

use adze::__private::lex_with_language_fn;
use adze::decoder::{decode_grammar, decode_parse_table};
use adze::document::{AdzeDocument, NodeId, ParseOutcome};
use adze::glr_parser::{GLRParser, SelectionReason};
use adze::glr_streaming_runtime::parse_with_streaming_driver;
use adze::pure_parser::TSLanguage;
use adze::subtree::Subtree;
use std::sync::Arc;

/// Checked-in replay seed. The only order "randomness": a tiny LCG shuffles
/// corpus replay order deterministically to prove order independence.
const REPLAY_SEED: u64 = 0x1071_9E37_5EED_42u64;
/// Multiparse winner determinism loop bound.
const DETERMINISM_RUNS: usize = 25;

/// Deterministic Fisher-Yates shuffle driven by [`REPLAY_SEED`].
fn replay_order(len: usize) -> Vec<usize> {
    let mut state = REPLAY_SEED;
    let mut next = move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as usize
    };
    let mut order: Vec<usize> = (0..len).collect();
    for i in (1..len).rev() {
        let j = next() % (i + 1);
        order.swap(i, j);
    }
    order
}

fn language_for(id: &str) -> &'static TSLanguage {
    match id {
        "det" => adze_example::typed_ast_contract::grammar::language(),
        "cnf" => adze_example::native_failure_facts_1030::grammar::language(),
        "rr" => adze_example::reduce_reduce::grammar::language(),
        _ => panic!("unknown corpus grammar {id}"),
    }
}

/// Batch route: fixed pretokenization bridge (established fixture pattern).
fn parse_batch(input: &str, language: &'static TSLanguage) -> Result<Arc<Subtree>, String> {
    let source = input.as_bytes();
    let mut parse_table = decode_parse_table(language);
    let grammar = decode_grammar(language);
    adze::__private::align_true_glr_parse_table_to_language_symbols(language, &mut parse_table);
    let mut parser = GLRParser::new(parse_table, grammar);
    let lex_fn = language
        .lex_fn
        .expect("generated grammar should expose lex_fn");
    let tokens =
        lex_with_language_fn(language, lex_fn, source).map_err(|errs| format!("lex: {errs:?}"))?;
    for token in tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }
    parser.process_eof(source.len());
    parser.finish().map_err(|message| message)
}

/// Streaming route: stack-aware streaming driver; Ok carries the root span.
fn parse_streaming(input: &str, language: &'static TSLanguage) -> Result<(usize, usize), String> {
    let parse_table = decode_parse_table(language);
    let grammar = decode_grammar(language);
    parse_with_streaming_driver(input, language, parse_table, &grammar)
        .map(|result| {
            (
                result.root.node.byte_range.start,
                result.root.node.byte_range.end,
            )
        })
        .map_err(|error| error.to_string())
}

fn production_err_spans(id: &str, source: &str) -> Vec<(usize, usize)> {
    let errs = match id {
        "det" => adze_example::typed_ast_contract::grammar::parse(source).unwrap_err(),
        "cnf" => adze_example::native_failure_facts_1030::grammar::parse(source).unwrap_err(),
        "rr" => adze_example::reduce_reduce::grammar::parse(source).unwrap_err(),
        _ => panic!("unknown corpus grammar {id}"),
    };
    errs.iter().map(|e| (e.start, e.end)).collect()
}

fn production_document(id: &str, source: &str) -> AdzeDocument {
    match id {
        "det" => adze_example::typed_ast_contract::grammar::parse_document(source),
        "cnf" => adze_example::native_failure_facts_1030::grammar::parse_document(source),
        "rr" => adze_example::reduce_reduce::grammar::parse_document(source),
        _ => panic!("unknown corpus grammar {id}"),
    }
    .expect("document retains facts on every replayed input")
}

/// Structural invariants asserted on every replayed document.
fn assert_document_invariants(doc: &AdzeDocument, source: &str, label: &str) {
    let tree = doc.tree();
    assert_eq!(
        tree.validate_structure(),
        Ok(()),
        "{label}: validate_structure for {source:?}"
    );
    assert!(tree.node_count() > 0, "{label}: non-empty tree");
    let root = tree.root();
    assert_eq!(root.parent_id(), None, "{label}: root has no parent");
    assert_eq!(root.node_id(), tree.root_id(), "{label}: root id");
    for raw in 0..tree.node_count() {
        let id = NodeId::new(raw);
        let node = tree
            .node(id)
            .unwrap_or_else(|| panic!("{label}: node {raw} exists"));
        let span = node.byte_range();
        assert!(
            span.start <= span.end && span.end <= source.len(),
            "{label}: node {raw} span {span:?} bounded by {source:?}"
        );
        match node.parent_id() {
            None => assert_eq!(id, tree.root_id(), "{label}: only root lacks a parent"),
            Some(parent_id) => {
                let parent = tree
                    .node(parent_id)
                    .unwrap_or_else(|| panic!("{label}: parent exists"));
                assert!(
                    (0..parent.child_count())
                        .any(|i| parent.child(i).map(|c| c.node_id()) == Some(id)),
                    "{label}: parent links back to child {raw}"
                );
                assert!(
                    node.parent_edge().is_some(),
                    "{label}: child {raw} has edge"
                );
            }
        }
        for i in 0..node.child_count() {
            let child = node
                .child(i)
                .unwrap_or_else(|| panic!("{label}: child exists"));
            assert_eq!(child.parent_id(), Some(id), "{label}: child {i} links back");
            assert!(node.child_edge(i).is_some(), "{label}: child edge exists");
        }
    }
    for edge_index in 0..tree.edge_count() {
        let edge = tree
            .edge_record(edge_index)
            .unwrap_or_else(|| panic!("{label}: edge exists"));
        assert!(
            tree.node_record(edge.parent_id()).is_some()
                && tree.node_record(edge.child_id()).is_some(),
            "{label}: edge {edge_index} endpoints exist"
        );
    }
}

#[test]
fn replay_seed_is_pinned_and_order_covers_corpus() {
    assert_eq!(REPLAY_SEED, 0x1071_9E37_5EED_42u64);
    let first = replay_order(21);
    assert_eq!(first, replay_order(21), "replay order is deterministic");
    let mut sorted = first.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        (0..21).collect::<Vec<_>>(),
        "order is a permutation"
    );
}

/// Clean conflicted corpus: batch, streaming and document all accept with
/// identical exact root spans.
#[test]
fn streaming_batch_accept_clean_corpus() {
    let corpus: &[(&str, &str, (usize, usize))] = &[
        ("cnf", "42", (0, 2)),
        ("cnf", "7 + 2", (0, 5)),
        ("cnf", "7 + 2 * 3", (0, 9)),
        ("rr", "x", (0, 1)),
    ];
    for index in replay_order(corpus.len()) {
        let (id, source, span) = corpus[index];
        let label = format!("{id}:{source:?}");
        let language = language_for(id);
        let bridge =
            parse_batch(source, language).unwrap_or_else(|e| panic!("{label}: batch accepts: {e}"));
        let driver = parse_streaming(source, language)
            .unwrap_or_else(|e| panic!("{label}: streaming accepts: {e}"));
        assert_eq!(
            (bridge.node.byte_range.start, bridge.node.byte_range.end),
            span,
            "{label}: exact batch root span"
        );
        assert_eq!(driver, span, "{label}: exact streaming root span");
        let doc = production_document(id, source);
        assert_eq!(doc.outcome(), ParseOutcome::Complete, "{label}: outcome");
        assert!(doc.diagnostics().is_empty(), "{label}: no diagnostics");
        assert_eq!(
            (
                doc.tree().root().byte_range().start,
                doc.tree().root().byte_range().end
            ),
            span,
            "{label}: document root span agrees"
        );
        assert_document_invariants(&doc, source, &label);
    }
}

/// Hard-malformed corpus: every surface rejects; parse/document agree on the
/// exact first failure span.
#[test]
fn routes_reject_hard_malformed_corpus() {
    let corpus: &[(&str, &str, (usize, usize))] = &[
        ("cnf", "   ", (0, 1)),
        ("cnf", "7 + @", (4, 5)),
        ("cnf", "7\x00+2", (1, 2)),
        ("rr", "y", (0, 1)),
    ];
    for index in replay_order(corpus.len()) {
        let (id, source, span) = corpus[index];
        let label = format!("{id}:{source:?}");
        let language = language_for(id);
        assert!(
            parse_batch(source, language).is_err(),
            "{label}: batch rejects"
        );
        assert!(
            parse_streaming(source, language).is_err(),
            "{label}: streaming rejects"
        );
        let errs = production_err_spans(id, source);
        assert!(!errs.is_empty(), "{label}: parse rejects");
        assert_eq!((errs[0].0, errs[0].1), span, "{label}: exact parse span");
        let doc = production_document(id, source);
        assert_ne!(doc.outcome(), ParseOutcome::Complete, "{label}: outcome");
        assert!(!doc.diagnostics().is_empty(), "{label}: diagnostics");
        assert_eq!(doc.diagnostics()[0].byte_span(), errs[0].0..errs[0].1);
        assert_document_invariants(&doc, source, &label);
    }
}

/// Truncated/overlong corpus: acceptance oracle (batch + production) rejects.
/// The bare driver returns a partial-span Ok; those exact spans are pinned as
/// observed driver semantics, not as acceptance.
#[test]
fn truncated_inputs_never_accept_and_pin_bare_driver_spans() {
    let corpus: &[(&str, &str, (usize, usize), (usize, usize))] = &[
        ("cnf", "", (0, 0), (0, 0)),
        ("cnf", "7 +", (0, 3), (3, 3)),
        ("rr", "", (0, 0), (0, 0)),
        ("rr", "xx", (1, 2), (1, 2)),
    ];
    for index in replay_order(corpus.len()) {
        let (id, source, partial, span) = corpus[index];
        let label = format!("{id}:{source:?}");
        let language = language_for(id);
        assert!(
            parse_batch(source, language).is_err(),
            "{label}: batch rejects"
        );
        assert_eq!(
            parse_streaming(source, language)
                .unwrap_or_else(|e| panic!("{label}: bare driver partial Ok: {e}")),
            partial,
            "{label}: exact bare-driver partial span"
        );
        let errs = production_err_spans(id, source);
        assert_eq!((errs[0].0, errs[0].1), span, "{label}: exact parse span");
        let doc = production_document(id, source);
        assert_ne!(doc.outcome(), ParseOutcome::Complete, "{label}: outcome");
        assert_eq!(doc.diagnostics()[0].byte_span(), errs[0].0..errs[0].1);
        assert_document_invariants(&doc, source, &label);
    }
}

/// Deterministic grammar through production surfaces: exact typed values on
/// clean seeds, exact failure spans (with parse/document agreement) on
/// malformed seeds.
#[test]
fn deterministic_grammar_production_surfaces_agree() {
    use adze_example::typed_ast_contract::grammar::{self, Expr};
    for (source, expected, nodes) in [
        ("42", Expr::Number(42), 3),
        (
            "1 + 2",
            Expr::Add(Box::new(Expr::Number(1)), (), Box::new(Expr::Number(2))),
            10,
        ),
        ("  7  ", Expr::Number(7), 5),
    ] {
        assert_eq!(grammar::parse(source).unwrap(), expected, "{source:?}");
        let doc = grammar::parse_document(source).unwrap();
        assert_eq!(doc.outcome(), ParseOutcome::Complete, "{source:?}");
        assert_eq!(doc.tree().node_count(), nodes, "{source:?} node count");
        assert_eq!(
            doc.ast::<Expr>().unwrap(),
            expected,
            "{source:?} document ast"
        );
        assert_document_invariants(&doc, source, "det:accept");
    }
    for (source, spans) in [
        ("", vec![(0, 0)]),
        ("   ", vec![(3, 3)]),
        ("1 +", vec![(3, 3)]),
        ("1 ++2", vec![(3, 4)]),
        ("1 + @", vec![(4, 5), (5, 5)]),
        ("\x00", vec![(0, 1), (1, 1)]),
    ] {
        let errs = grammar::parse(source).expect_err("malformed must fail");
        let got: Vec<(usize, usize)> = errs.iter().map(|e| (e.start, e.end)).collect();
        assert_eq!(got, spans, "{source:?} exact parse spans");
        let doc = grammar::parse_document(source).unwrap();
        assert_ne!(doc.outcome(), ParseOutcome::Complete, "{source:?} outcome");
        let diag_spans: Vec<(usize, usize)> = doc
            .diagnostics()
            .iter()
            .map(|d| (d.byte_span().start, d.byte_span().end))
            .collect();
        assert_eq!(diag_spans, spans, "{source:?} document agrees");
        assert!(doc.ast::<Expr>().is_err(), "{source:?} ast rejects");
        assert_document_invariants(&doc, source, "det:reject");
    }
}

/// Embedded/trailing NUL on the deterministic route is silently truncated:
/// production accepts the strict prefix as Complete. This divergence from
/// the streaming lexer (which rejects NUL as an unexpected token, see
/// `routes_reject_hard_malformed_corpus`) is explicitly counted here and
/// locked as observed behavior for #1071 follow-up; it is not blessed as
/// correct. Owning seam: generated `lex_fn` NUL handling (engine lex seam,
/// cf. #895/#896/#897) vs the streaming internal lexer.
#[test]
fn nul_truncation_divergence_is_explicitly_counted() {
    use adze_example::typed_ast_contract::grammar::{self, Expr};
    let cases: &[(&str, Expr)] = &[
        ("1\x00+2", Expr::Number(1)),
        (
            "1 + 2\x00",
            Expr::Add(Box::new(Expr::Number(1)), (), Box::new(Expr::Number(2))),
        ),
    ];
    assert_eq!(cases.len(), 2, "divergent NUL cases are counted explicitly");
    for (source, expected) in cases {
        assert_eq!(grammar::parse(source).unwrap(), *expected, "{source:?}");
        let doc = grammar::parse_document(source).unwrap();
        assert_eq!(doc.outcome(), ParseOutcome::Complete, "{source:?}");
        assert!(doc.diagnostics().is_empty(), "{source:?}");
        assert_eq!(doc.ast::<Expr>().unwrap(), *expected, "{source:?}");
        assert_document_invariants(&doc, source, "det:nul");
    }
}

/// Multiparse winners are deterministic across repeated runs, with exact
/// ambiguity summaries; the unambiguous conflicted seed retains no summary.
#[test]
fn multiparse_winners_are_deterministic() {
    use adze_example::native_failure_facts_1030::grammar as cnf;
    use adze_example::reduce_reduce::grammar as rr;
    let rr_first = rr::parse("x").unwrap();
    assert_eq!(format!("{rr_first:?}"), "FromA(FromA(()))");
    for _ in 0..DETERMINISM_RUNS {
        assert_eq!(rr::parse("x").unwrap(), rr_first);
    }
    let cnf_winner = cnf::Expr::Binary(
        Box::new(cnf::Expr::Number(7)),
        "+".to_string(),
        Box::new(cnf::Expr::Binary(
            Box::new(cnf::Expr::Number(2)),
            "*".to_string(),
            Box::new(cnf::Expr::Number(3)),
        )),
    );
    for _ in 0..DETERMINISM_RUNS {
        assert_eq!(cnf::parse("7 + 2 * 3").unwrap(), cnf_winner);
    }
    let doc = rr::parse_document("x").unwrap();
    assert_document_invariants(&doc, "x", "rr:multi");
    assert_eq!(doc.ast::<rr::Choice>().unwrap(), rr_first);
    assert_eq!(doc.ambiguities().len(), 1, "reduce/reduce summary count");
    let summary = &doc.ambiguities()[0];
    assert_eq!(summary.span, 0..1);
    assert_eq!(summary.alternatives.len(), 2, "two complete alternatives");
    assert!(summary.alternatives.iter().all(|a| a.span == (0..1)));
    assert!(summary.alternatives.iter().all(|a| !a.in_error));
    assert!(summary.alternatives.iter().all(|a| a.node_count == 3));
    assert_eq!(summary.selected, Some(0));
    assert_eq!(
        summary.selection_reason,
        SelectionReason::StableStructuralTieBreak
    );
    let doc = cnf::parse_document("7 + 2 * 3").unwrap();
    assert_document_invariants(&doc, "7 + 2 * 3", "cnf:multi");
    assert_eq!(doc.ast::<cnf::Expr>().unwrap(), cnf_winner);
    assert_eq!(doc.ambiguities().len(), 1, "shift/reduce summary count");
    let summary = &doc.ambiguities()[0];
    assert_eq!(summary.span, 0..9);
    assert_eq!(summary.alternatives.len(), 2, "two complete alternatives");
    assert!(summary.alternatives.iter().all(|a| a.span == (0..9)));
    assert!(summary.alternatives.iter().all(|a| !a.in_error));
    assert!(summary.alternatives.iter().all(|a| a.node_count == 10));
    assert_eq!(summary.selected, Some(0));
    assert_eq!(
        summary.selection_reason,
        SelectionReason::StableStructuralTieBreak
    );
    let doc = cnf::parse_document("42").unwrap();
    assert!(
        doc.ambiguities().is_empty(),
        "unambiguous seed keeps no summary"
    );
    assert_document_invariants(&doc, "42", "cnf:single");
}
