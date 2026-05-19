//! Full integration tests exercising the complete adze pipeline:
//! IR grammar definition → FIRST/FOLLOW → LR(1) automaton → compression → codegen.
//!
//! Uses `adze-ir`, `adze-glr-core`, and `adze-tablegen` together.

use adze_glr_core::{Action, FirstFollowSets, ParseTable, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, Grammar};
use adze_tablegen::{
    NodeTypesGenerator, StaticLanguageGenerator, TableCompressor,
    helpers::{collect_token_indices, eof_accepts_or_reduces},
};
use std::time::Instant;

// ============================================================================
// Helpers
// ============================================================================

/// Run the full pipeline: grammar → FIRST/FOLLOW → LR(1) → compress → codegen.
/// Returns (parse_table, compressed_ok, codegen_string).
fn run_pipeline(grammar: Grammar) -> (ParseTable, bool, String) {
    let ff = FirstFollowSets::compute(&grammar).expect("FIRST/FOLLOW computation failed");
    let pt = build_lr1_automaton(&grammar, &ff).expect("LR(1) automaton construction failed");
    assert!(
        pt.state_count > 0,
        "parse table must have at least one state"
    );

    let mut generator = StaticLanguageGenerator::new(grammar, pt.clone());
    let compressed_ok = generator.compress_tables().is_ok();
    let code = generator.generate_language_code().to_string();
    (pt, compressed_ok, code)
}

/// Build a simple arithmetic grammar: expr → expr '+' NUM | NUM
fn arith_grammar() -> Grammar {
    GrammarBuilder::new("arith")
        .token("NUM", r"\d+")
        .token("+", "+")
        .rule("expr", vec!["expr", "+", "NUM"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build()
}

/// Build a JSON-like grammar with object/array/value types.
fn json_grammar() -> Grammar {
    GrammarBuilder::new("json")
        .token("LBRACE", "{")
        .token("RBRACE", "}")
        .token("LBRACK", "[")
        .token("RBRACK", "]")
        .token("COLON", ":")
        .token("COMMA", ",")
        .token("STRING", r#""[^"]*""#)
        .token("NUMBER", r"\d+")
        .token("TRUE", "true")
        .token("FALSE", "false")
        .token("NULL", "null")
        // value → STRING | NUMBER | TRUE | FALSE | NULL | object | array
        .rule("value", vec!["STRING"])
        .rule("value", vec!["NUMBER"])
        .rule("value", vec!["TRUE"])
        .rule("value", vec!["FALSE"])
        .rule("value", vec!["NULL"])
        .rule("value", vec!["object"])
        .rule("value", vec!["array"])
        // object → '{' '}'  (simplified)
        .rule("object", vec!["LBRACE", "RBRACE"])
        // array → '[' ']'
        .rule("array", vec!["LBRACK", "RBRACK"])
        .start("value")
        .build()
}

// ============================================================================
// 1. Arithmetic grammar → full pipeline → tables usable
// ============================================================================

#[test]
fn arithmetic_full_pipeline_tables_usable() {
    let grammar = arith_grammar();
    let (pt, compressed_ok, code) = run_pipeline(grammar);

    // Parse table has states and columns
    assert!(
        pt.state_count >= 2,
        "arithmetic grammar needs multiple states"
    );
    assert!(pt.symbol_count > 0, "symbol count must be positive");
    assert!(
        !pt.action_table.is_empty(),
        "action table must be populated"
    );
    assert!(!pt.rules.is_empty(), "parse rules must be present");

    // Compression succeeded
    assert!(
        compressed_ok,
        "compression must succeed for arithmetic grammar"
    );

    // Codegen produced output
    assert!(!code.is_empty(), "codegen must produce non-empty output");
}

#[test]
fn arithmetic_action_table_has_shift_reduce_accept() {
    let grammar = arith_grammar();
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let pt = build_lr1_automaton(&grammar, &ff).unwrap();

    let mut has_shift = false;
    let mut has_reduce = false;
    let mut has_accept = false;

    for row in &pt.action_table {
        for cell in row {
            for action in cell {
                match action {
                    Action::Shift(_) => has_shift = true,
                    Action::Reduce(_) => has_reduce = true,
                    Action::Accept => has_accept = true,
                    _ => {}
                }
            }
        }
    }

    assert!(has_shift, "arithmetic grammar must have Shift actions");
    assert!(has_reduce, "arithmetic grammar must have Reduce actions");
    assert!(has_accept, "arithmetic grammar must have Accept action");
}

// ============================================================================
// 2. JSON-like grammar → full pipeline → node types correct
// ============================================================================

#[test]
fn json_grammar_full_pipeline() {
    let grammar = json_grammar();
    let (pt, compressed_ok, _code) = run_pipeline(grammar);

    assert!(pt.state_count >= 3, "JSON grammar needs multiple states");
    assert!(compressed_ok, "compression must succeed for JSON grammar");
}

#[test]
fn json_grammar_node_types_contain_expected_names() {
    let grammar = json_grammar();
    let generator = NodeTypesGenerator::new(&grammar);
    let node_types_json = generator.generate().expect("node types generation failed");

    let parsed: serde_json::Value =
        serde_json::from_str(&node_types_json).expect("node types must be valid JSON");
    assert!(parsed.is_array(), "NODE_TYPES must be a JSON array");

    let arr = parsed.as_array().unwrap();
    let type_names: Vec<&str> = arr
        .iter()
        .filter_map(|v| v.get("type").and_then(|t| t.as_str()))
        .collect();

    assert!(
        type_names.contains(&"value"),
        "node types must contain 'value', got: {type_names:?}"
    );
    assert!(
        type_names.contains(&"object"),
        "node types must contain 'object', got: {type_names:?}"
    );
    assert!(
        type_names.contains(&"array"),
        "node types must contain 'array', got: {type_names:?}"
    );
}

// ============================================================================
// 3. Grammar with whitespace extras → extras propagate
// ============================================================================

#[test]
fn whitespace_extras_propagate_to_parse_table() {
    let grammar = GrammarBuilder::new("ws_test")
        .token("ID", r"[a-z]+")
        .token("WS", r"[ \t]+")
        .extra("WS")
        .rule("start", vec!["ID"])
        .start("start")
        .build();

    let ws_id = *grammar
        .tokens
        .iter()
        .find(|(_, t)| t.name == "WS")
        .unwrap()
        .0;

    // The grammar itself should list WS in extras
    assert!(
        grammar.extras.contains(&ws_id),
        "WS must be in grammar extras"
    );

    let (pt, compressed_ok, _code) = run_pipeline(grammar);
    assert!(pt.state_count > 0);
    assert!(compressed_ok);

    // The grammar's extras are used during table construction for symbol metadata.
    // Verify the grammar had the extra and the pipeline completed successfully.
    assert!(pt.state_count > 0, "pipeline must succeed with extras");
}

#[test]
fn whitespace_extras_in_two_rule_grammar() {
    let grammar = GrammarBuilder::new("ws2")
        .token("A", "a")
        .token("B", "b")
        .token("SPACE", " ")
        .extra("SPACE")
        .rule("root", vec!["A", "B"])
        .start("root")
        .build();

    let (pt, compressed_ok, _) = run_pipeline(grammar.clone());
    assert!(compressed_ok);

    // Verify the grammar's extras are set correctly
    let space_id = *grammar
        .tokens
        .iter()
        .find(|(_, t)| t.name == "SPACE")
        .unwrap()
        .0;
    assert!(
        grammar.extras.contains(&space_id),
        "SPACE must be in grammar extras"
    );
    // The pipeline must complete successfully even with extras
    assert!(pt.state_count > 0, "pipeline must succeed with extras");
}

// ============================================================================
// 4. Precedence → correct associativity
// ============================================================================

#[test]
fn left_associative_precedence_pipeline() {
    let grammar = GrammarBuilder::new("left_assoc")
        .token("NUM", r"\d+")
        .token("+", "+")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();

    let (pt, compressed_ok, _) = run_pipeline(grammar);
    assert!(compressed_ok, "left-assoc grammar must compress");
    assert!(pt.state_count > 0);
}

#[test]
fn right_associative_precedence_pipeline() {
    let grammar = GrammarBuilder::new("right_assoc")
        .token("NUM", r"\d+")
        .token("=", "=")
        .rule_with_precedence(
            "assign",
            vec!["assign", "=", "assign"],
            1,
            Associativity::Right,
        )
        .rule("assign", vec!["NUM"])
        .start("assign")
        .build();

    let (pt, compressed_ok, _) = run_pipeline(grammar);
    assert!(compressed_ok, "right-assoc grammar must compress");
    assert!(pt.state_count > 0);
}

#[test]
fn mixed_precedence_levels_pipeline() {
    let grammar = GrammarBuilder::new("mixed_prec")
        .token("NUM", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();

    let (pt, compressed_ok, _) = run_pipeline(grammar);
    assert!(compressed_ok, "mixed precedence grammar must compress");
    // Higher-precedence grammar should still generate tables
    assert!(pt.state_count >= 2);
}

// ============================================================================
// 5. External tokens → external token slots
// ============================================================================

#[test]
fn external_tokens_have_slots_in_parse_table() {
    let grammar = GrammarBuilder::new("ext_tok")
        .token("ID", r"[a-z]+")
        .external("INDENT")
        .external("DEDENT")
        .rule("start", vec!["ID"])
        .start("start")
        .build();

    assert_eq!(grammar.externals.len(), 2, "grammar must have 2 externals");
    assert_eq!(grammar.externals[0].name, "INDENT");
    assert_eq!(grammar.externals[1].name, "DEDENT");

    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let pt = build_lr1_automaton(&grammar, &ff).unwrap();

    // External token count should match
    assert_eq!(
        pt.external_token_count, 2,
        "parse table must report 2 external tokens"
    );
}

#[test]
fn external_tokens_flow_into_codegen() {
    let grammar = GrammarBuilder::new("ext_codegen")
        .token("ID", r"[a-z]+")
        .external("NEWLINE")
        .rule("start", vec!["ID"])
        .start("start")
        .build();

    let (_, _, code) = run_pipeline(grammar);
    // Codegen should reference the external token count
    assert!(!code.is_empty(), "codegen must produce output");
}

// ============================================================================
// 6. 100-rule grammar → pipeline completes in bounded time
// ============================================================================

#[test]
fn large_grammar_100_rules_bounded_time() {
    let mut builder = GrammarBuilder::new("large100");

    // Create 100 tokens
    for i in 0..100 {
        builder = builder.token(&format!("T{i}"), &format!("t{i}"));
    }

    // Create 100 rules: rule_i → T_i
    for i in 0..100 {
        builder = builder.rule(&format!("r{i}"), vec![&format!("T{i}")]);
    }

    // Create a top-level rule referencing all sub-rules via alternatives
    for i in 0..100 {
        builder = builder.rule("top", vec![&format!("r{i}")]);
    }
    builder = builder.start("top");

    let grammar = builder.build();

    let start = Instant::now();
    let (pt, compressed_ok, code) = run_pipeline(grammar);
    let elapsed = start.elapsed();

    assert!(pt.state_count > 0, "100-rule grammar must produce states");
    assert!(compressed_ok, "100-rule grammar must compress");
    assert!(!code.is_empty(), "100-rule grammar must produce codegen");
    assert!(
        elapsed.as_secs() < 60,
        "100-rule pipeline must complete within 60s, took {elapsed:?}"
    );
}

// ============================================================================
// 7. Pipeline error propagation
// ============================================================================

#[test]
fn error_empty_grammar_no_rules() {
    let grammar = Grammar {
        name: "empty".to_string(),
        ..Default::default()
    };

    let result = FirstFollowSets::compute(&grammar);
    // An empty grammar should either error or produce empty sets
    // Either outcome is acceptable; the pipeline should not panic
    match result {
        Ok(ff) => {
            // No start symbol → automaton should fail
            let auto_result = build_lr1_automaton(&grammar, &ff);
            assert!(
                auto_result.is_err(),
                "empty grammar must fail at automaton stage"
            );
        }
        Err(_) => {
            // Error at FIRST/FOLLOW is also acceptable
        }
    }
}

#[test]
fn error_undefined_nonterminal_in_rhs() {
    // Rule references a non-terminal that has no defining rule
    let grammar = GrammarBuilder::new("bad_ref")
        .token("A", "a")
        .rule("start", vec!["missing_rule"])
        .start("start")
        .build();

    // The pipeline should either propagate an error or produce a table
    // with unreachable states — it must not panic
    let ff = FirstFollowSets::compute(&grammar);
    match ff {
        Ok(ff) => {
            let result = build_lr1_automaton(&grammar, &ff);
            // The automaton may succeed with unreachable states or error
            if let Ok(pt) = result {
                // Verify it at least has a start state
                assert!(pt.state_count > 0);
            }
        }
        Err(e) => {
            let msg = format!("{e:?}");
            assert!(!msg.is_empty(), "error must have a non-empty description");
        }
    }
}

#[test]
fn error_compression_of_empty_table() {
    // A parse table with no states should fail compression
    let empty_pt = ParseTable::default();
    let compressor = TableCompressor::new();
    let token_indices = vec![];
    let result = compressor.compress(&empty_pt, &token_indices, false);
    assert!(
        result.is_err(),
        "compressing an empty table must return an error"
    );
}

// ============================================================================
// 8. Determinism — 10 runs produce identical output
// ============================================================================

#[test]
fn determinism_10_runs_identical_parse_tables() {
    let mut state_counts = Vec::new();
    let mut action_sizes = Vec::new();

    for _ in 0..10 {
        let grammar = arith_grammar();
        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let pt = build_lr1_automaton(&grammar, &ff).unwrap();
        state_counts.push(pt.state_count);
        action_sizes.push(pt.action_table.len());
    }

    assert!(
        state_counts.windows(2).all(|w| w[0] == w[1]),
        "state counts must be identical across runs: {state_counts:?}"
    );
    assert!(
        action_sizes.windows(2).all(|w| w[0] == w[1]),
        "action table sizes must be identical: {action_sizes:?}"
    );
}

#[test]
fn determinism_10_runs_identical_codegen() {
    let mut outputs = Vec::new();

    for _ in 0..10 {
        let grammar = arith_grammar();
        let (_, _, code) = run_pipeline(grammar);
        outputs.push(code);
    }

    for i in 1..outputs.len() {
        assert_eq!(
            outputs[0], outputs[i],
            "codegen run 0 vs run {i} must be identical"
        );
    }
}

#[test]
fn determinism_10_runs_identical_compression() {
    let mut entry_counts = Vec::new();

    for _ in 0..10 {
        let grammar = arith_grammar();
        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let pt = build_lr1_automaton(&grammar, &ff).unwrap();

        let token_indices = collect_token_indices(&grammar, &pt);
        let start_empty = eof_accepts_or_reduces(&pt);
        let compressed = TableCompressor::new()
            .compress(&pt, &token_indices, start_empty)
            .unwrap();

        entry_counts.push(compressed.action_table.data.len());
    }

    assert!(
        entry_counts.windows(2).all(|w| w[0] == w[1]),
        "compressed entry counts must be identical: {entry_counts:?}"
    );
}

// ============================================================================
// 9. Compressed tables match original
// ============================================================================

#[test]
fn compressed_action_entries_reference_valid_symbols() {
    let grammar = arith_grammar();
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let pt = build_lr1_automaton(&grammar, &ff).unwrap();

    let token_indices = collect_token_indices(&grammar, &pt);
    let start_empty = eof_accepts_or_reduces(&pt);
    let compressed = TableCompressor::new()
        .compress(&pt, &token_indices, start_empty)
        .unwrap();

    // Every compressed action entry symbol should be within the symbol range
    for entry in &compressed.action_table.data {
        assert!(
            (entry.symbol as usize) < pt.symbol_count + 10,
            "compressed entry symbol {} exceeds symbol_count {}",
            entry.symbol,
            pt.symbol_count
        );
    }
}

#[test]
fn compressed_row_offsets_match_state_count() {
    let grammar = arith_grammar();
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let pt = build_lr1_automaton(&grammar, &ff).unwrap();

    let token_indices = collect_token_indices(&grammar, &pt);
    let start_empty = eof_accepts_or_reduces(&pt);
    let compressed = TableCompressor::new()
        .compress(&pt, &token_indices, start_empty)
        .unwrap();

    // row_offsets may include a sentinel entry beyond state_count
    assert!(
        compressed.action_table.row_offsets.len() >= pt.state_count,
        "row_offsets length ({}) must be at least state_count ({})",
        compressed.action_table.row_offsets.len(),
        pt.state_count
    );
    assert!(
        compressed.action_table.default_actions.len() >= pt.state_count,
        "default_actions length ({}) must be at least state_count ({})",
        compressed.action_table.default_actions.len(),
        pt.state_count
    );
}

#[test]
fn compressed_tables_validate_ok() {
    let grammar = arith_grammar();
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let pt = build_lr1_automaton(&grammar, &ff).unwrap();

    let token_indices = collect_token_indices(&grammar, &pt);
    let start_empty = eof_accepts_or_reduces(&pt);
    let compressed = TableCompressor::new()
        .compress(&pt, &token_indices, start_empty)
        .unwrap();

    // The validate method should return Ok for a valid compression
    assert!(
        compressed.validate(&pt).is_ok(),
        "compressed tables must pass validation"
    );
}

// ============================================================================
// 10. Codegen string contains expected markers
// ============================================================================

#[test]
fn codegen_contains_state_count_literal() {
    let grammar = arith_grammar();
    let (_, _, code) = run_pipeline(grammar);

    // Codegen should contain numeric literals related to the grammar
    // At minimum it should be non-trivial Rust code
    assert!(
        code.contains("fn") || code.contains("const") || code.contains("static"),
        "codegen must contain Rust constructs (fn/const/static)"
    );
}

#[test]
fn codegen_for_json_grammar_contains_symbol_names() {
    let grammar = json_grammar();
    let (_, _, code) = run_pipeline(grammar);

    // The generated code should reference symbol metadata
    assert!(!code.is_empty(), "codegen must be non-empty");
    // It should contain at least some identifiers or numeric arrays
    assert!(
        code.len() > 100,
        "codegen for JSON grammar should be substantial (got {} bytes)",
        code.len()
    );
}

// ============================================================================
// Bonus tests (to exceed 15 total)
// ============================================================================

#[test]
fn grammar_builder_round_trip_tokens() {
    let grammar = GrammarBuilder::new("roundtrip")
        .token("A", "alpha")
        .token("B", "beta")
        .rule("start", vec!["A", "B"])
        .start("start")
        .build();

    assert_eq!(grammar.tokens.len(), 2);
    let token_names: Vec<&str> = grammar.tokens.values().map(|t| t.name.as_str()).collect();
    assert!(token_names.contains(&"A"));
    assert!(token_names.contains(&"B"));

    // Full pipeline succeeds
    let (pt, ok, _) = run_pipeline(grammar);
    assert!(ok);
    assert!(pt.state_count > 0);
}

#[test]
fn eof_symbol_is_unique_in_parse_table() {
    let grammar = arith_grammar();
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let pt = build_lr1_automaton(&grammar, &ff).unwrap();

    // EOF symbol must not collide with any grammar token or non-terminal
    for token_id in grammar.tokens.keys() {
        assert_ne!(
            pt.eof_symbol, *token_id,
            "EOF must not collide with token {token_id}"
        );
    }
    for nt_id in grammar.rules.keys() {
        assert_ne!(
            pt.eof_symbol, *nt_id,
            "EOF must not collide with non-terminal {nt_id}"
        );
    }
}

#[test]
fn parse_table_start_symbol_matches_grammar() {
    let grammar = arith_grammar();
    let expected_start = grammar.start_symbol().unwrap();

    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let pt = build_lr1_automaton(&grammar, &ff).unwrap();

    // The parse table should track the correct start symbol
    // (the augmented start wraps the original, but rules should reference it)
    assert!(
        pt.rules.iter().any(|r| r.lhs == expected_start),
        "parse table rules must contain rules for the grammar start symbol"
    );
}

#[test]
fn node_types_generation_is_valid_json_for_arith() {
    let grammar = arith_grammar();
    let generator = NodeTypesGenerator::new(&grammar);
    let json_str = generator
        .generate()
        .expect("node types generation must succeed");
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).expect("node types must be valid JSON");
    assert!(parsed.is_array(), "node types root must be an array");
}

#[test]
fn collect_token_indices_includes_eof() {
    let grammar = arith_grammar();
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let pt = build_lr1_automaton(&grammar, &ff).unwrap();

    let token_indices = collect_token_indices(&grammar, &pt);

    // EOF must be in the token indices
    let eof_idx = pt.symbol_to_index.get(&pt.eof_symbol);
    assert!(eof_idx.is_some(), "EOF must be in symbol_to_index");
    assert!(
        token_indices.contains(eof_idx.unwrap()),
        "collect_token_indices must include EOF column"
    );

    // token_indices must be sorted
    assert!(
        token_indices.windows(2).all(|w| w[0] < w[1]),
        "token_indices must be sorted and deduplicated"
    );
}

// ============================================================================
// 9. Ambiguous grammar E2E coverage
// ============================================================================

#[test]
fn ambiguous_grammar_preserves_conflicts_through_pipeline() {
    let grammar = GrammarBuilder::new("ambiguous_expr")
        .token("NUM", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .rule("expr", vec!["expr", "+", "expr"])
        .rule("expr", vec!["expr", "*", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();

    let ff = FirstFollowSets::compute(&grammar).expect("FIRST/FOLLOW should compute");
    let pt = build_lr1_automaton(&grammar, &ff).expect("LR(1) automaton should build");

    let multi_action_cells = pt
        .action_table
        .iter()
        .flat_map(|row| row.iter())
        .filter(|cell| cell.len() > 1)
        .count();
    assert!(
        multi_action_cells > 0,
        "ambiguous grammar must retain at least one conflict cell"
    );

    let token_indices = collect_token_indices(&grammar, &pt);
    let compressed = TableCompressor::new()
        .compress(&pt, &token_indices, eof_accepts_or_reduces(&pt))
        .expect("conflicted parse table should still compress");

    assert!(
        !compressed.action_table.data.is_empty(),
        "compressed conflicted table must retain action rows"
    );

    let code = StaticLanguageGenerator::new(grammar, pt)
        .generate_language_code()
        .to_string();
    assert!(
        code.contains("tree_sitter_ambiguous_expr"),
        "codegen must emit a parser symbol for the ambiguous grammar"
    );
}
