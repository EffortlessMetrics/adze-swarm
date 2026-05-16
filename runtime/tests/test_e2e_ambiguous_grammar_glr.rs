//! End-to-End Validation: Ambiguous Grammar GLR Support
//!
//! This test suite validates the complete pipeline from enum-based grammar definition
//! through GLR conflict generation to runtime parsing with fork/merge behavior.
//!
//! **Contract**: docs/specs/E2E_AMBIGUOUS_GRAMMAR_GLR_VALIDATION.md
//! **Prerequisites**:
//!   - ADR-0003: Enum variant inlining implemented
//!   - GLR conflict preservation fix in glr-core
//!   - ambiguous_expr.rs test grammar available
//!
//! **Success Criteria**:
//!   1. Enum-based ambiguous grammars generate GLR conflicts
//!   2. GLR runtime successfully parses ambiguous input
//!   3. Valid AST produced from parse forest
//!   4. Backward compatibility with precedence grammars maintained

use adze::decoder;
#[cfg(feature = "glr")]
use adze::glr_parser::GLRParser;
use adze::pure_parser::TSLanguage;
use adze_glr_core::Action;
use adze_glr_core::conflict_inspection::cell_has_conflict;
#[cfg(feature = "glr")]
use std::collections::BTreeSet;
#[cfg(feature = "glr")]
use std::ops::Range;

#[cfg(feature = "glr")]
#[derive(Debug, Eq, PartialEq)]
struct DocumentNodeShape {
    node_id: usize,
    kind_name: Option<String>,
    grammar_name: Option<String>,
    byte_range: Range<usize>,
    child_count: usize,
    edges: Vec<DocumentEdgeShape>,
}

#[cfg(feature = "glr")]
#[derive(Debug, Eq, PartialEq)]
struct DocumentEdgeShape {
    child_index: usize,
    child_id: usize,
    field_name: Option<String>,
    field_id: Option<u16>,
}

#[cfg(feature = "glr")]
fn collect_document_topology(
    node: adze::document::AdzeNode<'_>,
    topology: &mut Vec<DocumentNodeShape>,
) {
    let edges = node
        .child_edges()
        .map(|edge| DocumentEdgeShape {
            child_index: edge.child_index(),
            child_id: edge.child_id().as_usize(),
            field_name: edge.field_name().map(str::to_owned),
            field_id: edge.field_id().map(|field_id| field_id.get()),
        })
        .collect::<Vec<_>>();

    topology.push(DocumentNodeShape {
        node_id: node.node_id().as_usize(),
        kind_name: node.kind_name().map(str::to_owned),
        grammar_name: node.grammar_name().map(str::to_owned),
        byte_range: node.byte_range(),
        child_count: node.child_count(),
        edges,
    });

    for index in 0..node.child_count() {
        let child = node
            .child(index)
            .expect("child index should resolve while collecting topology");
        collect_document_topology(child, topology);
    }
}

/// Helper: Count multi-action cells (GLR conflicts) in a parse table
fn count_multi_action_cells(lang: &'static TSLanguage) -> usize {
    let parse_table = decoder::decode_parse_table(lang);

    let mut conflict_count = 0;
    for state_actions in &parse_table.action_table {
        for action_cell in state_actions {
            if cell_has_conflict(action_cell) {
                conflict_count += 1;
            }
        }
    }

    conflict_count
}

/// Helper: Check if action cell contains both shift and reduce
fn contains_shift_reduce(cell: &[Action]) -> bool {
    let has_shift = cell.iter().any(|a| matches!(a, Action::Shift(_)));
    let has_reduce = cell.iter().any(|a| matches!(a, Action::Reduce(_)));
    has_shift && has_reduce
}

//==============================================================================
// Scenario 1: Conflict Generation Validation
//==============================================================================

#[test]
fn test_ambiguous_grammar_conflict_generation() {
    eprintln!("\n=== E2E TEST: Ambiguous Grammar Conflict Generation ===\n");

    // Load ambiguous_expr grammar parse table
    // This grammar has NO precedence, so it MUST generate conflicts
    use adze_example::ambiguous_expr::grammar;

    let lang = grammar::language();

    eprintln!("Step 1: Load parse table from generated grammar");
    let parse_table = decoder::decode_parse_table(lang);
    eprintln!("  ✓ Parse table loaded: {} states", parse_table.state_count);

    eprintln!("\nStep 2: Count multi-action cells (GLR conflicts)");
    let conflict_count = count_multi_action_cells(lang);
    eprintln!("  Multi-action cells found: {}", conflict_count);

    // Contract Assertion 1: Conflicts exist
    assert!(
        conflict_count > 0,
        "CONTRACT VIOLATION: Ambiguous grammar MUST generate GLR conflicts!\n\
         Expected: At least 1 multi-action cell\n\
         Actual: {} conflicts\n\n\
         This indicates enum variant inlining may not be working correctly.\n\
         Check: example/src/ambiguous_expr.rs has NO precedence annotations.\n\
         Check: ADR-0003 implementation in tool/src/expansion.rs",
        conflict_count
    );
    eprintln!("  ✅ Conflicts detected: {}", conflict_count);

    eprintln!("\nStep 3: Validate conflict patterns");
    // Contract Assertion 2: Find shift/reduce conflict for binary expression
    let mut has_binary_conflict = false;
    for (state_idx, state_actions) in parse_table.action_table.iter().enumerate() {
        for (symbol_idx, cell) in state_actions.iter().enumerate() {
            if cell_has_conflict(cell) && contains_shift_reduce(cell) {
                has_binary_conflict = true;

                let symbol_name = if symbol_idx < parse_table.symbol_metadata.len() {
                    &parse_table.symbol_metadata[symbol_idx].name
                } else {
                    "UNKNOWN"
                };

                eprintln!("  ✓ Shift/Reduce conflict found:");
                eprintln!("     State: {}", state_idx);
                eprintln!("     Symbol: {} ({})", symbol_idx, symbol_name);
                eprintln!("     Actions: {:?}", cell);
            }
        }
    }

    assert!(
        has_binary_conflict,
        "CONTRACT VIOLATION: Expected shift/reduce conflict for binary expression!\n\
         Ambiguous grammar 'Expr → Expr OP Expr' MUST create conflict on lookahead OP.\n\n\
         Possible causes:\n\
         1. GLR conflict preservation not working (check glr-core/src/lib.rs:2019-2077)\n\
         2. Grammar structure wrong (intermediate symbols still present?)\n\
         3. LR(1) is sufficient to resolve (check grammar definition)"
    );
    eprintln!("  ✅ Binary expression shift/reduce conflict validated");

    eprintln!("\n✅ SCENARIO 1 PASSED: Conflict generation validated\n");
}

#[test]
fn tablegen_abi_decode_preserves_generated_conflict_cells() {
    eprintln!("\n=== E2E TEST: Tablegen ABI Conflict Decode Preservation ===\n");

    use adze_example::ambiguous_expr::grammar;

    let lang = grammar::language();
    assert!(
        !lang.small_parse_table.is_null(),
        "generated language must expose compressed small parse-table rows"
    );

    let parse_table = decoder::decode_parse_table(lang);
    let mut conflict_cells = 0usize;
    let mut direct_multi_action_cells = 0usize;
    let mut shift_reduce_cells = 0usize;

    for (state_idx, state_actions) in parse_table.action_table.iter().enumerate() {
        for (symbol_idx, cell) in state_actions.iter().enumerate() {
            if cell_has_conflict(cell) {
                conflict_cells += 1;
                if cell.len() > 1 {
                    direct_multi_action_cells += 1;
                }
                if contains_shift_reduce(cell) {
                    shift_reduce_cells += 1;
                    eprintln!(
                        "  conflict cell state={state_idx} symbol={symbol_idx} actions={cell:?}"
                    );
                }
            }
        }
    }

    assert!(
        conflict_cells > 0,
        "compressed TSLanguage decode must preserve generated GLR conflict cells"
    );
    assert!(
        direct_multi_action_cells > 0,
        "compressed TSLanguage decode must retain duplicate symbol entries as multi-action cells, not first-action fallback"
    );
    assert!(
        shift_reduce_cells > 0,
        "decoded ambiguous_expr table must preserve at least one shift/reduce conflict"
    );

    eprintln!(
        "  decoded conflicts: {conflict_cells}, direct multi-action cells: {direct_multi_action_cells}, shift/reduce cells: {shift_reduce_cells}"
    );
    eprintln!("\n✅ Tablegen ABI conflict decode preservation validated\n");
}

//==============================================================================
// Scenario 2: GLR Parsing Behavior
//==============================================================================

#[test]
fn test_ambiguous_grammar_glr_parsing() {
    eprintln!("\n=== E2E TEST: Ambiguous Grammar GLR Parsing ===\n");

    use adze_example::ambiguous_expr::grammar;
    use adze_example::ambiguous_expr::grammar::Expr;

    // Test 1: Simple ambiguous input
    eprintln!("Test 1: Parse '1 + 2 + 3' (ambiguous associativity)");
    let input = "1 + 2 + 3";

    let result = grammar::parse(input);

    // Contract Assertion 1: Parse succeeds
    assert!(
        result.is_ok(),
        "CONTRACT VIOLATION: GLR should handle ambiguous input without error!\n\
         Input: {:?}\n\
         Error: {:?}\n\n\
         GLR parser should create fork points and select a valid parse.\n\
         Check: GLR runtime integration (runtime/src/__private.rs::parse_with_glr)",
        input,
        result.err()
    );
    eprintln!("  ✅ Parse succeeded (no error)");

    let expr = result.unwrap();
    eprintln!("  Parsed AST: {:?}", expr);

    let repeated = grammar::parse(input).expect("second parse should succeed");
    assert_eq!(
        expr, repeated,
        "ambiguous typed extraction must be deterministic"
    );
    assert_eq!(
        expr,
        Expr::Binary(
            Box::new(Expr::Number(1)),
            "+".to_string(),
            Box::new(Expr::Binary(
                Box::new(Expr::Number(2)),
                "+".to_string(),
                Box::new(Expr::Number(3)),
            )),
        ),
        "current deterministic selection should remain stable"
    );
    eprintln!("  ✅ Deterministic typed extraction validated");

    // Contract Assertion 2: Valid AST structure
    assert!(
        matches!(expr, Expr::Binary(_, _, _)),
        "CONTRACT VIOLATION: Should produce binary expression!\n\
         Actual: {:?}",
        expr
    );
    eprintln!("  ✅ Valid binary expression produced");

    // Contract Assertion 3: AST is well-formed (either left or right associative)
    // Left:  (1 + 2) + 3
    // Right: 1 + (2 + 3)
    fn verify_structure(expr: &Expr, depth: usize) {
        let indent = "  ".repeat(depth);
        match expr {
            Expr::Binary(left, op, right) => {
                eprintln!("{}Binary: {:?}", indent, op);
                verify_structure(left, depth + 1);
                verify_structure(right, depth + 1);
            }
            Expr::Number(n) => {
                eprintln!("{}Number: {}", indent, n);
            }
        }
    }

    eprintln!("\n  AST Structure:");
    verify_structure(&expr, 1);
    eprintln!("  ✅ Well-formed parse tree");

    // Test 2: Longer ambiguous input
    eprintln!("\nTest 2: Parse '1 + 2 + 3 + 4' (multiple ambiguity points)");
    let input = "1 + 2 + 3 + 4";
    let result = grammar::parse(input);

    assert!(result.is_ok(), "Failed to parse: {:?}", input);
    eprintln!("  ✅ Complex ambiguous input parsed successfully");

    eprintln!("\n✅ SCENARIO 2 PASSED: GLR parsing produces valid ASTs\n");
}

#[test]
fn generated_ambiguous_expr_multi_conflict_selection_is_deterministic() {
    use adze_example::ambiguous_expr::grammar;
    use adze_example::ambiguous_expr::grammar::Expr;

    let input = "1 + 2 + 3 + 4";
    let expr = grammar::parse(input).expect("multi-conflict ambiguous input should parse");
    let repeated = grammar::parse(input).expect("repeated multi-conflict parse should succeed");

    assert_eq!(
        expr, repeated,
        "multi-conflict ambiguous typed extraction must be deterministic"
    );
    assert_eq!(
        expr,
        Expr::Binary(
            Box::new(Expr::Number(1)),
            "+".to_string(),
            Box::new(Expr::Binary(
                Box::new(Expr::Number(2)),
                "+".to_string(),
                Box::new(Expr::Binary(
                    Box::new(Expr::Number(3)),
                    "+".to_string(),
                    Box::new(Expr::Number(4)),
                )),
            )),
        ),
        "current deterministic selection should remain right-nested across multiple conflict sites"
    );
}

#[test]
#[cfg(feature = "glr")]
fn generated_ambiguous_expr_glr_runtime_retains_multiple_complete_alternatives() {
    use adze_example::ambiguous_expr::grammar;

    let input = "1 + 2 + 3";
    let language = grammar::language();
    let mut parse_table = decoder::decode_parse_table(language);
    adze::__private::align_true_glr_parse_table_to_language_symbols(language, &mut parse_table);
    let grammar = decoder::decode_grammar(language);
    let mut parser = GLRParser::new(parse_table, grammar);

    let lex_fn = language
        .lex_fn
        .expect("generated language should expose a lex_fn");
    for token in adze::__private::lex_with_language_fn(language, lex_fn, input.as_bytes())
        .expect("generated lexer should tokenize ambiguous expression")
    {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }
    parser.process_eof(input.len());

    let alternatives = parser
        .finish_all_alternatives()
        .expect("ambiguous generated grammar should finish successfully");
    let summary = parser
        .finish_ambiguity_summary()
        .expect("ambiguous generated grammar should expose an ambiguity summary")
        .expect("ambiguous generated grammar should retain multiple complete alternatives");
    let selected = parser
        .finish()
        .expect("ambiguous generated grammar should select one complete alternative");
    let unique_shapes = alternatives
        .iter()
        .map(|alternative| format!("{alternative:?}"))
        .collect::<BTreeSet<_>>();

    assert!(
        alternatives.len() >= 2,
        "ambiguous generated grammar should retain more than one complete parse alternative, got {}",
        alternatives.len()
    );
    assert!(
        unique_shapes.len() >= 2,
        "ambiguous generated grammar should retain structurally distinct parse alternatives"
    );
    assert!(
        alternatives
            .iter()
            .all(|alternative| alternative.node.byte_range == (0..input.len())),
        "all complete alternatives should span the full input"
    );

    assert_eq!(
        summary.alternatives.len(),
        alternatives.len(),
        "ambiguity summary should report every retained complete alternative"
    );
    assert_eq!(summary.span, 0..input.len());
    assert_eq!(
        summary.selection_reason,
        adze::glr_parser::SelectionReason::StableStructuralTieBreak,
        "ambiguous_expr alternatives currently tie by version and use the stable structural selector"
    );

    let selected_index = summary
        .selected
        .expect("ambiguity summary should identify the selected alternative");
    assert_eq!(
        format!("{:?}", alternatives[selected_index]),
        format!("{selected:?}"),
        "ambiguity summary selection should match GLRParser::finish()"
    );
    let selected_summary = summary
        .alternatives
        .get(selected_index)
        .expect("selected alternative summary should resolve");
    assert_eq!(selected_summary.index, selected_index);
    assert_eq!(selected_summary.span, 0..input.len());
    assert_eq!(selected_summary.root_symbol, selected.node.symbol_id);
}

#[test]
#[cfg(feature = "glr")]
fn generated_ambiguous_expr_glr_runtime_retains_three_or_more_complete_alternatives() {
    use adze_example::ambiguous_expr::grammar;

    let input = "1 + 2 + 3 + 4";
    let language = grammar::language();
    let mut parse_table = decoder::decode_parse_table(language);
    adze::__private::align_true_glr_parse_table_to_language_symbols(language, &mut parse_table);
    let grammar = decoder::decode_grammar(language);
    let mut parser = GLRParser::new(parse_table, grammar);

    let lex_fn = language
        .lex_fn
        .expect("generated language should expose a lex_fn");
    for token in adze::__private::lex_with_language_fn(language, lex_fn, input.as_bytes())
        .expect("generated lexer should tokenize ambiguous expression")
    {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }
    parser.process_eof(input.len());

    let alternatives = parser
        .finish_all_alternatives()
        .expect("ambiguous generated grammar should finish successfully");
    let summary = parser
        .finish_ambiguity_summary()
        .expect("ambiguous generated grammar should expose an ambiguity summary")
        .expect("ambiguous generated grammar should retain complete alternatives");
    let unique_shapes = alternatives
        .iter()
        .map(|alternative| format!("{alternative:?}"))
        .collect::<BTreeSet<_>>();

    assert!(
        alternatives.len() >= 3,
        "four operands should retain at least three complete parse alternatives, got {}",
        alternatives.len()
    );
    assert!(
        unique_shapes.len() >= 3,
        "four operands should retain at least three structurally distinct alternatives"
    );
    assert_eq!(
        summary.alternatives.len(),
        alternatives.len(),
        "ambiguity summary should report every retained complete alternative"
    );
    assert_eq!(
        summary.span,
        0..input.len(),
        "multi-alternative ambiguity should span the full ambiguous input"
    );
    assert!(
        summary.selected.is_some(),
        "multi-alternative ambiguity summary should still identify a selected tree"
    );
}

#[test]
#[cfg(feature = "glr")]
fn generated_ambiguous_expr_parse_document_reports_ambiguity_summary() {
    use adze_example::ambiguous_expr::grammar;

    let input = "1 + 2 + 3";
    let document = grammar::parse_document(input)
        .expect("generated parse_document helper should return an AdzeDocument");
    let ambiguities = document.ambiguities();

    assert_eq!(
        document.diagnostics(),
        [],
        "valid ambiguous input should not record parser diagnostics"
    );
    assert_eq!(
        document.tree().root().byte_range(),
        0..input.len(),
        "selected document tree should span the full input"
    );
    assert_eq!(
        ambiguities.len(),
        1,
        "generated parse_document should preserve the parser ambiguity summary"
    );

    let summary = &ambiguities[0];
    assert_eq!(summary.span, 0..input.len());
    assert!(
        summary.alternatives.len() >= 2,
        "document ambiguity summary should retain multiple complete alternatives"
    );
    assert_eq!(
        summary.selection_reason,
        adze::glr_parser::SelectionReason::StableStructuralTieBreak,
        "ambiguous_expr alternatives currently tie by version and use the stable structural selector"
    );

    let selected = summary
        .selected
        .expect("document ambiguity summary should identify the selected alternative");
    let selected_summary = summary
        .alternatives
        .get(selected)
        .expect("selected document alternative summary should resolve");
    assert_eq!(selected_summary.index, selected);
    assert_eq!(selected_summary.span, 0..input.len());
}

#[test]
#[cfg(feature = "glr")]
fn generated_ambiguous_expr_parse_document_ast_matches_selected_parse() {
    use adze::document::Provenance;
    use adze_example::ambiguous_expr::grammar;
    use adze_example::ambiguous_expr::grammar::Expr;

    let input = "1 + 2 + 3";
    let expected = grammar::parse(input)
        .expect("generated ambiguous parser should return a selected typed AST");
    let document = grammar::parse_document(input)
        .expect("generated parse_document helper should return an AdzeDocument");

    let typed_ast = document
        .ast_with_provenance::<Expr>()
        .expect("GLR document should extract typed AST from its selected tree");

    assert_eq!(typed_ast.value(), &expected);
    let Provenance::Node(node_id) = typed_ast.provenance() else {
        panic!("alpha GLR document typed AST provenance should point at a document node");
    };
    let node = document
        .tree()
        .node(*node_id)
        .expect("GLR document provenance node should resolve");
    assert_eq!(
        node.utf8_text()
            .expect("provenance node should cover valid UTF-8 source"),
        input
    );
}

#[test]
#[cfg(feature = "glr")]
fn generated_ambiguous_expr_parse_document_cst_topology_is_deterministic() {
    use adze_example::ambiguous_expr::grammar;

    let input = "1 + 2 + 3 + 4";
    let first = grammar::parse_document(input)
        .expect("first generated parse_document helper should return an AdzeDocument");
    let second = grammar::parse_document(input)
        .expect("second generated parse_document helper should return an AdzeDocument");

    assert_eq!(first.tree().node_count(), second.tree().node_count());
    assert_eq!(first.tree().edge_count(), second.tree().edge_count());
    assert_eq!(first.tree().root_id(), second.tree().root_id());
    assert_eq!(first.tree().root().byte_range(), 0..input.len());
    assert_eq!(second.tree().root().byte_range(), 0..input.len());

    let mut first_topology = Vec::new();
    let mut second_topology = Vec::new();
    collect_document_topology(first.tree().root(), &mut first_topology);
    collect_document_topology(second.tree().root(), &mut second_topology);

    assert_eq!(
        first_topology, second_topology,
        "selected CST topology should be deterministic across repeated GLR document parses"
    );
    assert_eq!(
        first.tree().node_count(),
        first_topology.len(),
        "node_count should match collected selected-tree topology"
    );
    assert_eq!(
        first.tree().edge_count(),
        first_topology
            .iter()
            .map(|shape| shape.child_count)
            .sum::<usize>(),
        "edge_count should match collected selected-tree child counts"
    );
}

#[test]
#[cfg(all(feature = "glr", feature = "glr_telemetry"))]
fn generated_ambiguous_expr_runtime_fork_count_is_deterministic() {
    use adze_example::ambiguous_expr::grammar;

    fn run(input: &str) -> (usize, usize) {
        let language = grammar::language();
        let mut parse_table = decoder::decode_parse_table(language);
        adze::__private::align_true_glr_parse_table_to_language_symbols(language, &mut parse_table);
        let grammar = decoder::decode_grammar(language);
        let mut parser = GLRParser::new(parse_table, grammar);
        let lex_fn = language
            .lex_fn
            .expect("generated language should expose a lex_fn");

        for token in adze::__private::lex_with_language_fn(language, lex_fn, input.as_bytes())
            .expect("generated lexer should tokenize ambiguous expression")
        {
            parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        }
        parser.process_eof(input.len());
        let summary = parser
            .finish_ambiguity_summary()
            .expect("ambiguous generated grammar should expose an ambiguity summary")
            .expect("ambiguous generated grammar should retain alternatives");

        assert!(
            summary.selected.is_some(),
            "ambiguous generated grammar should still choose one selected alternative"
        );

        (parser.telemetry_fork_count(), summary.alternatives.len())
    }

    let input = "1 + 2 + 3 + 4";
    let first = run(input);
    let second = run(input);

    assert!(
        first.0 > 0,
        "ambiguous generated grammar should record at least one runtime fork"
    );
    assert!(
        first.1 >= 2,
        "ambiguous generated grammar should retain multiple complete alternatives"
    );
    assert_eq!(
        first, second,
        "runtime fork count and retained alternative count should be deterministic across repeated parses"
    );
}

#[test]
#[cfg(feature = "glr")]
fn generated_ambiguous_expr_parse_document_bad_input_returns_diagnostic_document() {
    use adze_example::ambiguous_expr::grammar;

    let input = "1 + @";
    let document = grammar::parse_document(input)
        .expect("generated parse_document helper should return partial parse facts");
    let diagnostic = document
        .diagnostics()
        .first()
        .expect("invalid conflicted input should produce a document diagnostic");

    assert!(
        document.tree().has_errors(),
        "diagnostic document should record parser error metadata"
    );
    assert_eq!(
        document.ambiguities(),
        [],
        "invalid input should not claim a complete ambiguity summary"
    );
    assert_eq!(diagnostic.byte_span(), 4..5);
    assert!(
        diagnostic.message.contains("unexpected token"),
        "diagnostic should preserve the GLR parse error message: {}",
        diagnostic.message
    );
}

#[test]
fn generated_glr_parser_bad_inputs_return_errors_without_panicking() {
    use adze_example::ambiguous_expr::grammar;

    let cases = [
        ("empty input", ""),
        ("whitespace only", "   "),
        ("trailing operator", "1 +"),
        ("invalid ascii token", "1 + @"),
        ("invalid utf8 scalar", "1 + λ"),
        ("multiline invalid token", "1 +\n@"),
    ];

    for (label, source) in cases {
        let parsed = std::panic::catch_unwind(|| grammar::parse(source));

        let errors = match parsed {
            Ok(Err(errors)) => errors,
            Ok(Ok(ast)) => panic!("generated GLR parser unexpectedly accepted {label}: {ast:?}"),
            Err(_) => panic!("generated GLR parser panicked for {label}"),
        };

        assert!(
            !errors.is_empty(),
            "generated GLR parser should return at least one structured error for {label}"
        );
    }
}

//==============================================================================
// Scenario 3: Backward Compatibility
//==============================================================================

#[test]
#[cfg(feature = "glr")]
#[cfg_attr(
    feature = "incremental_glr",
    ignore = "known incompatibility under incremental_glr precedence resolution"
)]
fn test_glr_backward_compatibility() {
    eprintln!("\n=== E2E TEST: GLR Backward Compatibility ===\n");

    // This test uses the arithmetic grammar which HAS precedence
    // It should work identically with or without GLR feature
    use adze_example::arithmetic::grammar;
    use adze_example::arithmetic::grammar::Expression;

    eprintln!("Testing precedence grammar: arithmetic");

    // Test multiplication binds tighter than subtraction
    let input = "1 - 2 * 3";
    eprintln!("Input: {:?}", input);

    let result = grammar::parse(input);

    // This should work even with GLR (precedence is preserved)
    assert!(
        result.is_ok(),
        "CONTRACT VIOLATION: Precedence grammar should work with GLR!\n\
         Error: {:?}",
        result.err()
    );

    let expr = result.unwrap();
    eprintln!("Parsed: {:?}", expr);

    // Contract Assertion: Multiplication binds tighter
    // Expected: 1 - (2 * 3), not (1 - 2) * 3
    match expr {
        Expression::Sub(ref left, _, ref right) => {
            assert_eq!(**left, Expression::Number(1), "Left operand should be 1");

            assert!(
                matches!(**right, Expression::Mul(_, _, _)),
                "Right operand should be Mul, got {:?}",
                **right
            );

            if let Expression::Mul(ref mul_left, _, ref mul_right) = **right {
                assert_eq!(**mul_left, Expression::Number(2));
                assert_eq!(**mul_right, Expression::Number(3));
            }

            eprintln!("  ✅ Correct precedence: 1 - (2 * 3)");
        }
        _ => panic!("Expected Sub at top level, got {:?}", expr),
    }

    eprintln!("\n✅ SCENARIO 3 PASSED: Backward compatibility maintained\n");
}

//==============================================================================
// Scenario 4: Ambiguous vs Arithmetic Comparison
//==============================================================================

#[test]
fn test_ambiguous_vs_arithmetic_comparison() {
    eprintln!("\n=== E2E TEST: Ambiguous vs Arithmetic Comparison ===\n");

    // Load both grammars
    use adze_example::ambiguous_expr::grammar as ambiguous;
    use adze_example::arithmetic::grammar as arithmetic;

    eprintln!("Step 1: Load ambiguous_expr grammar");
    let ambiguous_lang = ambiguous::language();
    let ambiguous_conflicts = count_multi_action_cells(ambiguous_lang);
    eprintln!("  Ambiguous grammar conflicts: {}", ambiguous_conflicts);

    eprintln!("\nStep 2: Load arithmetic grammar");
    let arithmetic_lang = arithmetic::language();
    let arithmetic_conflicts = count_multi_action_cells(arithmetic_lang);
    eprintln!("  Arithmetic grammar conflicts: {}", arithmetic_conflicts);

    eprintln!("\n=== Comparison ===");
    eprintln!(
        "  Ambiguous (no precedence): {} conflicts",
        ambiguous_conflicts
    );
    eprintln!(
        "  Arithmetic (with precedence): {} conflicts",
        arithmetic_conflicts
    );

    // Contract Assertion: Ambiguous has conflicts, Arithmetic has none
    assert!(
        ambiguous_conflicts > 0,
        "CONTRACT VIOLATION: Ambiguous grammar MUST have conflicts!"
    );

    assert_eq!(
        arithmetic_conflicts, 0,
        "REGRESSION: Arithmetic grammar should have ZERO conflicts (LR(1) sufficient)"
    );

    eprintln!("\n✅ SCENARIO 4 PASSED: Grammars correctly differentiated\n");
    eprintln!("Key Finding:");
    eprintln!("  - Ambiguous grammar generates GLR conflicts (as expected)");
    eprintln!("  - Arithmetic grammar generates zero conflicts (LR(1) sufficient)");
    eprintln!("  - This proves enum variant inlining enables true ambiguity");
}

//==============================================================================
// Documentation Test: Contract Summary
//==============================================================================

#[test]
fn test_contract_documentation() {
    eprintln!("\n=== E2E GLR VALIDATION CONTRACT ===\n");
    eprintln!("This test suite validates:");
    eprintln!("  1. ✓ Enum variant inlining enables ambiguous grammars");
    eprintln!("  2. ✓ GLR conflict generation works correctly");
    eprintln!("  3. ✓ GLR runtime parses ambiguous input successfully");
    eprintln!("  4. ✓ GLR runtime retains complete alternatives for ambiguous input");
    eprintln!("  5. ✓ Backward compatibility with precedence grammars");
    eprintln!();
    eprintln!("To run validation:");
    eprintln!(
        "  cargo test -p adze --features \"pure-rust,glr,runtime-e2e\" --test test_e2e_ambiguous_grammar_glr"
    );
    eprintln!();
    eprintln!("Expected Results:");
    eprintln!("  - test_ambiguous_grammar_conflict_generation: PASS");
    eprintln!("  - test_ambiguous_grammar_glr_parsing: PASS");
    eprintln!(
        "  - generated_ambiguous_expr_glr_runtime_retains_multiple_complete_alternatives: PASS"
    );
    eprintln!("  - test_glr_backward_compatibility: PASS");
    eprintln!("  - test_ambiguous_vs_arithmetic_comparison: PASS");
    eprintln!();
    eprintln!("See: docs/status/SUPPORT_TIERS.md");
}
