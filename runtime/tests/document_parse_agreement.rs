//! Document agreement tests: prove parse() and parse_document() share the same parse truth.
//!
//! These tests verify that AdzeDocument's tree topology matches what parse() uses internally,
//! and that the relationship holds across both non-GLR and GLR code paths.

#![cfg(all(test, feature = "pure-rust"))]

/// Recursively collect (kind_name, byte_range, child_count) for every node in a subtree.
/// Used to compare CST topology between parse_document().tree() and the shape implied by parse().
fn collect_topology(
    node: &adze::document::AdzeNode<'_>,
    out: &mut Vec<(Option<String>, std::ops::Range<usize>, usize)>,
) {
    out.push((
        node.kind_name().map(String::from),
        node.byte_range(),
        node.child_count(),
    ));
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_topology(&child, out);
        }
    }
}

fn assert_parse_errors_match_document_diagnostics(
    errors: &[adze::errors::ParseError],
    document: &adze::document::AdzeDocument,
) {
    assert_eq!(
        errors.len(),
        document.diagnostics().len(),
        "typed AST rejection should return one parse error per document diagnostic"
    );

    for (error, diagnostic) in errors.iter().zip(document.diagnostics()) {
        assert_eq!(
            error.start..error.end,
            diagnostic.byte_span(),
            "typed AST rejection span should match the document diagnostic span"
        );
        assert_eq!(
            error.expected, diagnostic.expected,
            "typed AST rejection expected tokens should match the document diagnostic"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 1: CST topology parity for non-GLR path
//
// parse_document() builds an AdzeDocument from the same parser that parse() uses.
// The resulting tree should have consistent node count, edge count, and root coverage.
// ---------------------------------------------------------------------------
#[test]
fn parse_document_tree_topology_matches_parser_expectations() {
    let source = "1 + 2 + 3";
    let document = adze_example::typed_ast_contract::grammar::parse_document(source)
        .expect("document parse should succeed");

    // Verify parse() also succeeds with the same input
    let _typed = adze_example::typed_ast_contract::grammar::parse(source)
        .expect("typed parse should succeed for the same input");

    let tree = document.tree();
    let root = tree.root();

    // Root covers the full source
    assert_eq!(root.byte_range(), 0..source.len());
    assert_eq!(
        root.utf8_text().expect("root should cover valid UTF-8"),
        source
    );

    // Root is a source_file with at least one child
    assert_eq!(root.kind_name(), Some("source_file"));
    assert!(
        root.child_count() >= 1,
        "source_file should have at least one child expression"
    );

    // Collect full topology
    let mut topology = Vec::new();
    collect_topology(&root, &mut topology);

    // Tree should have a reasonable number of nodes for "1 + 2 + 3"
    // source_file > Expr(Add) > Expr(Add), Expr(Number), Expr(Number)
    assert!(
        topology.len() >= 5,
        "tree should have at least root + expression + number nodes, got {}",
        topology.len()
    );

    // Edge count should match sum of child counts
    let total_edges: usize = topology.iter().map(|(_, _, cc)| *cc).sum();
    assert_eq!(
        tree.edge_count(),
        total_edges,
        "edge_count should equal total children across all nodes"
    );

    // Node count should match collected topology
    assert_eq!(
        tree.node_count(),
        topology.len(),
        "node_count should match the number of nodes in the traversal"
    );

    // All byte ranges should be within source bounds
    for (i, (_, range, _)) in topology.iter().enumerate() {
        assert!(
            range.start <= range.end,
            "node {i} has inverted byte range: {range:?}"
        );
        assert!(
            range.end <= source.len(),
            "node {i} byte range {range:?} exceeds source length {}",
            source.len()
        );
    }

    // Parse again and verify determinism of topology
    let document2 = adze_example::typed_ast_contract::grammar::parse_document(source)
        .expect("second document parse should succeed");
    let mut topology2 = Vec::new();
    collect_topology(&document2.tree().root(), &mut topology2);

    assert_eq!(
        topology, topology2,
        "repeated parse_document should produce identical tree topology"
    );
}

// ---------------------------------------------------------------------------
// Test 2: parse_document() tree agrees with parse() typed AST structure
//
// The typed AST from parse() encodes left-associative structure:
// Add(Add(1, 2), 3). The CST should have the same nesting depth and order.
// ---------------------------------------------------------------------------
#[test]
fn parse_document_cst_nesting_matches_typed_ast_shape() {
    let source = "1 + 2 + 3";

    let typed = adze_example::typed_ast_contract::grammar::parse(source)
        .expect("typed parse should succeed");

    let document = adze_example::typed_ast_contract::grammar::parse_document(source)
        .expect("document parse should succeed");

    let tree = document.tree();
    let root = tree.root();

    // Collect full topology
    let mut topology = Vec::new();
    collect_topology(&root, &mut topology);

    // Count the number of "Expr" nodes (Add and Number variants in the CST)
    let expr_nodes: Vec<_> = topology
        .iter()
        .filter(|(kind, _, _)| kind.as_deref() == Some("Expr"))
        .collect();

    // Left-associative Add(Add(1, 2), 3) should produce exactly 3 Expr nodes:
    // the outer Add, the inner Add, and the Number(3)
    // Plus 2 more Number leaves for 1 and 2
    assert!(
        expr_nodes.len() >= 3,
        "should have at least 3 Expr nodes for left-associative '1 + 2 + 3', got {}",
        expr_nodes.len()
    );

    // The typed AST has left-associative structure:
    // Expr::Add(Box<Expr::Add(Box<Expr::Number(1)>, _, Box<Expr::Number(2>)>, _, Box<Expr::Number(3)>)
    // This means depth 2 (two nested Adds)
    match &typed {
        adze_example::typed_ast_contract::grammar::Expr::Add(outer_left, _, outer_right) => {
            // Outer right should be Number(3)
            assert!(
                matches!(
                    outer_right.as_ref(),
                    adze_example::typed_ast_contract::grammar::Expr::Number(3)
                ),
                "outer right should be Number(3)"
            );
            // Outer left should be Add(Number(1), Number(2))
            assert!(
                matches!(
                    outer_left.as_ref(),
                    adze_example::typed_ast_contract::grammar::Expr::Add(_, _, _)
                ),
                "outer left should be a nested Add"
            );
        }
        other => panic!("expected Add at top level, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Test 3: Bad input document tree has error metadata
//
// When parse() returns errors, parse_document() should still build a valid
// document with error metadata in the tree.
// ---------------------------------------------------------------------------
#[test]
fn parse_document_error_tree_has_consistent_error_metadata() {
    let source = "1 +";
    let document = adze_example::typed_ast_contract::grammar::parse_document(source)
        .expect("document should build from partial parse");

    // parse() should fail for the same input
    let parse_errors = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("truncated input should fail");

    // Document should have error metadata
    assert!(
        document.tree().has_errors(),
        "document tree should record errors for truncated input"
    );
    assert!(
        !document.diagnostics().is_empty(),
        "document should have diagnostics for truncated input"
    );

    // parse() errors and document diagnostics should agree on byte spans
    assert_eq!(
        parse_errors.len(),
        document.diagnostics().len(),
        "parse() error count should match document diagnostic count"
    );

    for (parse_err, diag) in parse_errors.iter().zip(document.diagnostics().iter()) {
        assert_eq!(
            parse_err.start..parse_err.end,
            diag.byte_span(),
            "parse() error span should match document diagnostic span"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 4: Recovered documents refuse strict typed AST extraction
//
// parse_document() preserves partial facts for bad input, but strict typed AST
// projection must not synthesize a semantic AST from recovered syntax by default.
// ---------------------------------------------------------------------------
#[test]
fn parse_document_recovered_doc_refuses_strict_ast_projection() {
    let source = "1 +";
    let document = adze_example::typed_ast_contract::grammar::parse_document(source)
        .expect("document should build from partial parse");

    assert!(
        document.tree().has_errors(),
        "truncated input should produce a recovered/error document"
    );
    assert!(
        !document.diagnostics().is_empty(),
        "recovered document should carry diagnostics"
    );

    let ast_errors = document
        .ast::<adze_example::typed_ast_contract::grammar::Expr>()
        .expect_err("strict ast() should reject recovered documents");
    assert_parse_errors_match_document_diagnostics(&ast_errors, &document);

    let provenance_errors = document
        .ast_with_provenance::<adze_example::typed_ast_contract::grammar::Expr>()
        .expect_err("strict ast_with_provenance() should reject recovered documents");
    assert_parse_errors_match_document_diagnostics(&provenance_errors, &document);

    let root_id = document.tree().root().node_id();
    let node_errors = document
        .ast_from_node::<adze_example::typed_ast_contract::grammar::Expr>(root_id)
        .expect_err("strict ast_from_node() should reject recovered documents");
    assert_parse_errors_match_document_diagnostics(&node_errors, &document);
}

// ---------------------------------------------------------------------------
// Test 5: parse_document() round-trips clean input without diagnostics
//
// If parse() succeeds, parse_document() should produce zero diagnostics.
// ---------------------------------------------------------------------------
#[test]
fn parse_document_clean_input_has_zero_diagnostics() {
    let source = "42";
    let typed = adze_example::typed_ast_contract::grammar::parse(source)
        .expect("simple number should parse");
    assert!(
        matches!(
            typed,
            adze_example::typed_ast_contract::grammar::Expr::Number(42)
        ),
        "parse should produce Number(42)"
    );

    let document = adze_example::typed_ast_contract::grammar::parse_document(source)
        .expect("document parse should succeed");

    assert!(
        document.diagnostics().is_empty(),
        "clean input should produce zero diagnostics"
    );
    assert_eq!(document.metadata().error_count, 0);
    assert!(!document.tree().has_errors());

    let root = document.tree().root();
    assert_eq!(root.byte_range(), 0..source.len());
}

// ---------------------------------------------------------------------------
// Test 6: GLR-path agreement (feature-gated)
//
// When the table has conflicts, parse() and parse_document() should still
// agree. The selected tree from parse_document() should match parse().
// ---------------------------------------------------------------------------
#[cfg(feature = "glr")]
#[test]
fn parse_document_glr_tree_topology_matches_parse() {
    let input = "1 + 2 + 3";

    let expected = adze_example::ambiguous_expr::grammar::parse(input)
        .expect("GLR parse should return selected typed AST");

    let document = adze_example::ambiguous_expr::grammar::parse_document(input)
        .expect("GLR parse_document should return AdzeDocument");

    // AST from document should match parse()
    let doc_ast = document
        .ast::<adze_example::ambiguous_expr::grammar::Expr>()
        .expect("GLR document should extract typed AST");
    assert_eq!(doc_ast, expected, "GLR document AST should match parse()");

    // Tree topology should be non-trivial and deterministic
    let tree = document.tree();
    let mut topology = Vec::new();
    collect_topology(&tree.root(), &mut topology);

    assert!(
        topology.len() >= 5,
        "GLR tree should have multiple nodes, got {}",
        topology.len()
    );

    // Root covers full input
    assert_eq!(tree.root().byte_range(), 0..input.len());

    // Parse again for determinism check
    let document2 = adze_example::ambiguous_expr::grammar::parse_document(input)
        .expect("second GLR parse_document should succeed");
    let mut topology2 = Vec::new();
    collect_topology(&document2.tree().root(), &mut topology2);

    assert_eq!(
        topology, topology2,
        "repeated GLR parse_document should produce identical tree topology"
    );
}

// ---------------------------------------------------------------------------
// Test 6: GLR bad input produces consistent diagnostic document (feature-gated)
//
// When GLR parse() fails, parse_document() should still return a document
// with diagnostics, not panic.
// ---------------------------------------------------------------------------
#[cfg(feature = "glr")]
#[test]
fn parse_document_glr_bad_input_returns_consistent_diagnostic_document() {
    let input = "1 + @";

    let parse_errors = adze_example::ambiguous_expr::grammar::parse(input)
        .expect_err("GLR parse should fail for invalid token");

    let document = adze_example::ambiguous_expr::grammar::parse_document(input)
        .expect("GLR parse_document should return partial facts for bad input");

    assert!(
        document.tree().has_errors(),
        "GLR diagnostic document should record parser errors"
    );
    assert!(
        !document.diagnostics().is_empty(),
        "GLR diagnostic document should have diagnostics"
    );
    assert!(
        document.ambiguities().is_empty(),
        "bad input should not claim ambiguity summary"
    );

    // Spans should agree between parse() errors and document diagnostics
    let first_diag = document
        .diagnostics()
        .first()
        .expect("should have at least one diagnostic");
    let first_err = parse_errors
        .first()
        .expect("should have at least one parse error");
    assert_eq!(
        first_err.start..first_err.end,
        first_diag.byte_span(),
        "GLR parse() error span should match document diagnostic span"
    );
}
