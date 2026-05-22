//! v7 tests for external scanner handling in table generation.
//!
//! 8 categories × 8 tests = 64 tests:
//! 1. ext_basic_*    — basic external token handling
//! 2. ext_codegen_*  — code generation with externals
//! 3. ext_abi_*      — ABI layout with externals
//! 4. ext_node_types_* — node types with externals
//! 5. ext_combined_* — mixed internal/external tokens
//! 6. ext_validate_* — validation with externals
//! 7. ext_serialize_* — serialization of externals
//! 8. ext_edge_*     — edge cases

mod test_helpers;

use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{ExternalToken, Grammar, SymbolId};
use adze_tablegen::AbiLanguageBuilder;
use adze_tablegen::StaticLanguageGenerator;
use adze_tablegen::external_scanner::ExternalScannerGenerator as V1Generator;
use adze_tablegen::external_scanner_v2::ExternalScannerGenerator as V2Generator;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a real LR(1) parse table from a grammar (normalizes in place).
fn build_table(grammar: &mut Grammar) -> adze_glr_core::ParseTable {
    let ff = FirstFollowSets::compute_normalized(grammar).expect("FIRST/FOLLOW computation failed");
    build_lr1_automaton(grammar, &ff).expect("LR(1) automaton construction failed")
}

/// Minimal parse table via test_helpers (no LR build).
fn minimal_table(grammar: &Grammar) -> adze_glr_core::ParseTable {
    test_helpers::create_minimal_parse_table(grammar.clone())
}

/// Grammar with one rule, one token, and N externals via GrammarBuilder.
fn grammar_ext(names: &[&str]) -> Grammar {
    let mut b = GrammarBuilder::new("ext_v7")
        .token("WORD", r"[a-z]+")
        .rule("start", vec!["WORD"])
        .start("start");
    for name in names {
        b = b.token(name, name).external(name);
    }
    b.build()
}

/// Push N raw ExternalTokens into a Grammar.
fn add_externals(g: &mut Grammar, count: u16, base_id: u16) {
    for i in 0..count {
        g.externals.push(ExternalToken {
            name: format!("X_{i}"),
            symbol_id: SymbolId(base_id + i),
        });
    }
}

// ===========================================================================
// 1. ext_basic — basic external token handling (8 tests)
// ===========================================================================

#[test]
fn ext_basic_grammar_builder_registers_externals() {
    let g = grammar_ext(&["INDENT"]);
    assert!(!g.externals.is_empty());
    assert_eq!(g.externals.len(), 1);
}

#[test]
fn ext_basic_grammar_builder_two_externals() {
    let g = grammar_ext(&["INDENT", "DEDENT"]);
    assert_eq!(g.externals.len(), 2);
}

#[test]
fn ext_basic_external_token_symbol_id_nonzero() {
    let g = grammar_ext(&["SCAN"]);
    // The builder assigns symbol IDs automatically; they should be > 0
    assert!(g.externals[0].symbol_id.0 > 0);
}

#[test]
fn ext_basic_external_token_name_matches_builder_arg() {
    let g = grammar_ext(&["HEREDOC"]);
    assert_eq!(g.externals[0].name, "HEREDOC");
}

#[test]
fn ext_basic_v1_count_matches_grammar() {
    let mut g = Grammar::new("t".to_string());
    add_externals(&mut g, 4, 10);
    let scanner = V1Generator::new(g);
    assert_eq!(scanner.external_token_count(), 4);
}

#[test]
fn ext_basic_v2_count_matches_grammar() {
    let mut g = Grammar::new("t".to_string());
    add_externals(&mut g, 7, 20);
    let table = minimal_table(&g);
    let scanner = V2Generator::new(g, table);
    assert_eq!(scanner.external_token_count(), 7);
}

#[test]
fn ext_basic_v1_has_external_tokens_true_after_push() {
    let mut g = Grammar::new("t".to_string());
    add_externals(&mut g, 1, 99);
    assert!(V1Generator::new(g).has_external_tokens());
}

#[test]
fn ext_basic_v2_has_external_tokens_true_after_push() {
    let mut g = Grammar::new("t".to_string());
    add_externals(&mut g, 1, 99);
    let table = minimal_table(&g);
    assert!(V2Generator::new(g, table).has_external_tokens());
}

// ===========================================================================
// 2. ext_codegen — code generation with externals (8 tests)
// ===========================================================================

#[test]
fn ext_codegen_v1_interface_contains_scanner_states() {
    let mut g = Grammar::new("t".to_string());
    add_externals(&mut g, 2, 10);
    let code = V1Generator::new(g).generate_scanner_interface().to_string();
    assert!(code.contains("EXTERNAL_SCANNER_STATES"));
}

#[test]
fn ext_codegen_v1_interface_contains_symbol_map() {
    let mut g = Grammar::new("t".to_string());
    add_externals(&mut g, 2, 10);
    let code = V1Generator::new(g).generate_scanner_interface().to_string();
    assert!(code.contains("EXTERNAL_SCANNER_SYMBOL_MAP"));
}

#[test]
fn ext_codegen_v2_interface_contains_token_count_const() {
    let mut g = Grammar::new("t".to_string());
    add_externals(&mut g, 3, 10);
    let table = minimal_table(&g);
    let code = V2Generator::new(g, table)
        .generate_scanner_interface()
        .to_string();
    assert!(code.contains("EXTERNAL_TOKEN_COUNT"));
}

#[test]
fn ext_codegen_v2_interface_contains_state_count_const() {
    let mut g = Grammar::new("t".to_string());
    add_externals(&mut g, 1, 10);
    let table = minimal_table(&g);
    let code = V2Generator::new(g, table)
        .generate_scanner_interface()
        .to_string();
    assert!(code.contains("STATE_COUNT"));
}

#[test]
fn ext_codegen_v2_interface_contains_helper_fn() {
    let mut g = Grammar::new("t".to_string());
    add_externals(&mut g, 1, 10);
    let table = minimal_table(&g);
    let code = V2Generator::new(g, table)
        .generate_scanner_interface()
        .to_string();
    assert!(code.contains("get_valid_external_tokens"));
}

#[test]
fn ext_codegen_static_lang_gen_with_externals_nonempty() {
    let mut g = grammar_ext(&["SCAN_A"]);
    let table = build_table(&mut g);
    let slg = StaticLanguageGenerator::new(g, table);
    let code = slg.generate_language_code().to_string();
    assert!(!code.is_empty());
}

#[test]
fn ext_codegen_v1_bitmap_dimensions_match_args() {
    let mut g = Grammar::new("t".to_string());
    add_externals(&mut g, 5, 0);
    let bitmap = V1Generator::new(g).generate_state_bitmap(10);
    assert_eq!(bitmap.len(), 10);
    for row in &bitmap {
        assert_eq!(row.len(), 5);
    }
}

#[test]
fn ext_codegen_v2_bitmap_mirrors_parse_table() {
    let mut g = Grammar::new("t".to_string());
    add_externals(&mut g, 2, 10);
    let mut table = minimal_table(&g);
    table.external_scanner_states = vec![vec![false, true], vec![true, false]];
    let bitmap = V2Generator::new(g, table).generate_state_bitmap();
    assert_eq!(bitmap, vec![vec![false, true], vec![true, false]]);
}

// ===========================================================================
// 3. ext_abi — ABI layout with externals (8 tests)
// ===========================================================================

#[test]
fn ext_abi_no_externals_mentions_null() {
    let g = Grammar::new("t".to_string());
    let table = minimal_table(&g);
    let code = AbiLanguageBuilder::new(&g, &table).generate().to_string();
    assert!(code.contains("null"));
}

#[test]
fn ext_abi_with_externals_mentions_external_scanner() {
    let mut g = Grammar::new("t".to_string());
    add_externals(&mut g, 1, 50);
    let table = minimal_table(&g);
    let code = AbiLanguageBuilder::new(&g, &table).generate().to_string();
    assert!(code.contains("external_scanner") || code.contains("ExternalScanner"));
}

#[test]
fn ext_abi_external_token_count_in_output() {
    let mut g = grammar_ext(&["SCAN"]);
    let table = build_table(&mut g);
    let code = AbiLanguageBuilder::new(&g, &table).generate().to_string();
    assert!(code.contains("external_token_count"));
}

#[test]
fn ext_abi_no_scanner_states_without_externals() {
    let g = Grammar::new("t".to_string());
    let table = minimal_table(&g);
    let code = AbiLanguageBuilder::new(&g, &table).generate().to_string();
    assert!(!code.contains("EXTERNAL_SCANNER_STATES"));
}

#[test]
fn ext_abi_builder_generates_token_stream() {
    let mut g = grammar_ext(&["TOK_A", "TOK_B"]);
    let table = build_table(&mut g);
    let ts = AbiLanguageBuilder::new(&g, &table).generate();
    // TokenStream should be non-empty
    assert!(!ts.is_empty());
}

#[test]
fn ext_abi_symbol_count_includes_externals() {
    let mut g = grammar_ext(&["SCAN_X"]);
    let table = build_table(&mut g);
    // symbol_count in the table should be ≥ number of externals
    assert!(table.symbol_count >= g.externals.len());
}

#[test]
fn ext_abi_external_token_count_field_in_table() {
    let mut g = grammar_ext(&["EXT_1", "EXT_2", "EXT_3"]);
    let table = build_table(&mut g);
    // The parse table should reflect external token count
    assert_eq!(table.external_token_count, 3);
}

#[test]
fn ext_abi_lex_modes_present_with_externals() {
    let mut g = grammar_ext(&["INDENT"]);
    let table = build_table(&mut g);
    // Every state should have a lex mode entry
    assert_eq!(table.lex_modes.len(), table.state_count);
}

// ===========================================================================
// 4. ext_node_types — node types with externals (8 tests)
// ===========================================================================

#[test]
fn ext_node_types_visible_external_in_output() {
    let mut g = Grammar::new("t".to_string());
    g.externals.push(ExternalToken {
        name: "HEREDOC_BODY".to_string(),
        symbol_id: SymbolId(200),
    });
    let table = minimal_table(&g);
    let json = StaticLanguageGenerator::new(g, table).generate_node_types();
    assert!(json.contains("HEREDOC_BODY"));
}

#[test]
fn ext_node_types_hidden_external_not_in_output() {
    let mut g = Grammar::new("t".to_string());
    g.externals.push(ExternalToken {
        name: "_internal_scan".to_string(),
        symbol_id: SymbolId(201),
    });
    let table = minimal_table(&g);
    let json = StaticLanguageGenerator::new(g, table).generate_node_types();
    assert!(!json.contains("_internal_scan"));
}

#[test]
fn ext_node_types_multiple_externals_all_visible() {
    let mut g = Grammar::new("t".to_string());
    g.externals.push(ExternalToken {
        name: "TOK_A".to_string(),
        symbol_id: SymbolId(100),
    });
    g.externals.push(ExternalToken {
        name: "TOK_B".to_string(),
        symbol_id: SymbolId(101),
    });
    let table = minimal_table(&g);
    let json = StaticLanguageGenerator::new(g, table).generate_node_types();
    assert!(json.contains("TOK_A"));
    assert!(json.contains("TOK_B"));
}

#[test]
fn ext_node_types_no_externals_still_valid_json() {
    let g = Grammar::new("t".to_string());
    let table = minimal_table(&g);
    let json = StaticLanguageGenerator::new(g, table).generate_node_types();
    // Should still be parseable JSON
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.is_array());
}

#[test]
fn ext_node_types_generator_new_with_externals() {
    let mut g = Grammar::new("t".to_string());
    g.externals.push(ExternalToken {
        name: "SCAN".to_string(),
        symbol_id: SymbolId(50),
    });
    let scanner = adze_tablegen::NodeTypesGenerator::new(&g);
    let result = scanner.generate();
    assert!(result.is_ok());
}

#[test]
fn ext_node_types_generator_empty_grammar() {
    let g = Grammar::new("t".to_string());
    let scanner = adze_tablegen::NodeTypesGenerator::new(&g);
    let result = scanner.generate();
    assert!(result.is_ok());
    let json_str = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert!(parsed.is_array());
}

#[test]
fn ext_node_types_mixed_hidden_visible_external() {
    let mut g = Grammar::new("t".to_string());
    g.externals.push(ExternalToken {
        name: "_hidden".to_string(),
        symbol_id: SymbolId(10),
    });
    g.externals.push(ExternalToken {
        name: "VISIBLE".to_string(),
        symbol_id: SymbolId(11),
    });
    let table = minimal_table(&g);
    let json = StaticLanguageGenerator::new(g, table).generate_node_types();
    assert!(json.contains("VISIBLE"));
    assert!(!json.contains("_hidden"));
}

#[test]
fn ext_node_types_external_name_with_digits() {
    let mut g = Grammar::new("t".to_string());
    g.externals.push(ExternalToken {
        name: "TOKEN_42".to_string(),
        symbol_id: SymbolId(42),
    });
    let table = minimal_table(&g);
    let json = StaticLanguageGenerator::new(g, table).generate_node_types();
    assert!(json.contains("TOKEN_42"));
}

// ===========================================================================
// 5. ext_combined — mixed internal/external tokens (8 tests)
// ===========================================================================

#[test]
fn ext_combined_regular_token_not_in_externals() {
    let g = grammar_ext(&["SCAN"]);
    let ext_names: Vec<&str> = g.externals.iter().map(|e| e.name.as_str()).collect();
    // "WORD" is a regular token; it must NOT be in externals
    assert!(!ext_names.contains(&"WORD"));
}

#[test]
fn ext_combined_external_also_in_tokens_map() {
    let g = grammar_ext(&["SCAN"]);
    let token_names: Vec<&str> = g.tokens.values().map(|t| t.name.as_str()).collect();
    assert!(token_names.contains(&"SCAN"));
}

#[test]
fn ext_combined_both_regular_and_external_present() {
    let g = grammar_ext(&["NEWLINE"]);
    let token_names: Vec<&str> = g.tokens.values().map(|t| t.name.as_str()).collect();
    assert!(token_names.contains(&"WORD"));
    assert!(token_names.contains(&"NEWLINE"));
}

#[test]
fn ext_combined_v1_counts_only_externals() {
    let g = grammar_ext(&["SCAN_ONE", "SCAN_TWO"]);
    assert_eq!(V1Generator::new(g).external_token_count(), 2);
}

#[test]
fn ext_combined_language_builder_external_count() {
    let g = grammar_ext(&["EXT_A", "EXT_B"]);
    let table = minimal_table(&g);
    let builder = adze_tablegen::LanguageBuilder::new(g, table);
    let lang = builder.generate_language().unwrap();
    assert_eq!(lang.external_token_count, 2);
}

#[test]
fn ext_combined_abi_output_nonempty() {
    let mut g = grammar_ext(&["EXT"]);
    let table = build_table(&mut g);
    let code = AbiLanguageBuilder::new(&g, &table).generate().to_string();
    assert!(!code.is_empty());
}

#[test]
fn ext_combined_node_types_include_external() {
    let g = grammar_ext(&["TEMPLATE"]);
    let table = minimal_table(&g);
    let json = StaticLanguageGenerator::new(g, table).generate_node_types();
    assert!(json.contains("TEMPLATE"));
}

#[test]
fn ext_combined_three_regulars_two_externals() {
    let g = GrammarBuilder::new("comb")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .token("E1", "e1")
        .token("E2", "e2")
        .external("E1")
        .external("E2")
        .rule("root", vec!["A", "B", "C"])
        .start("root")
        .build();
    assert_eq!(g.externals.len(), 2);
    // At least 3 regular tokens plus the 2 external ones in the token map
    assert!(g.tokens.len() >= 5);
}

// ===========================================================================
// 6. ext_validate — validation with externals (8 tests)
// ===========================================================================

#[test]
fn ext_validate_language_builder_succeeds_no_ext() {
    let g = GrammarBuilder::new("v")
        .token("ID", r"[a-z]+")
        .rule("root", vec!["ID"])
        .start("root")
        .build();
    let table = minimal_table(&g);
    let builder = adze_tablegen::LanguageBuilder::new(g, table);
    assert!(builder.generate_language().is_ok());
}

#[test]
fn ext_validate_language_builder_succeeds_with_ext() {
    let g = grammar_ext(&["EXT_TOK"]);
    let table = minimal_table(&g);
    let builder = adze_tablegen::LanguageBuilder::new(g, table);
    assert!(builder.generate_language().is_ok());
}

#[test]
fn ext_validate_language_external_count_zero_no_ext() {
    let g = GrammarBuilder::new("v")
        .token("NUM", r"\d+")
        .rule("root", vec!["NUM"])
        .start("root")
        .build();
    let table = minimal_table(&g);
    let lang = adze_tablegen::LanguageBuilder::new(g, table)
        .generate_language()
        .unwrap();
    assert_eq!(lang.external_token_count, 0);
}

#[test]
fn ext_validate_language_external_count_three() {
    let g = grammar_ext(&["S1", "S2", "S3"]);
    let table = minimal_table(&g);
    let lang = adze_tablegen::LanguageBuilder::new(g, table)
        .generate_language()
        .unwrap();
    assert_eq!(lang.external_token_count, 3);
}

#[test]
fn ext_validate_language_symbol_count_positive() {
    let g = grammar_ext(&["EXT"]);
    let table = minimal_table(&g);
    let lang = adze_tablegen::LanguageBuilder::new(g, table)
        .generate_language()
        .unwrap();
    assert!(lang.symbol_count > 0);
}

#[test]
fn ext_validate_language_state_count_positive() {
    let g = grammar_ext(&["EXT"]);
    let table = minimal_table(&g);
    let lang = adze_tablegen::LanguageBuilder::new(g, table)
        .generate_language()
        .unwrap();
    assert!(lang.state_count > 0);
}

#[test]
fn ext_validate_language_version_nonzero() {
    let g = grammar_ext(&["EXT"]);
    let table = minimal_table(&g);
    let lang = adze_tablegen::LanguageBuilder::new(g, table)
        .generate_language()
        .unwrap();
    assert!(lang.version > 0);
}

#[test]
fn ext_validate_real_table_with_externals() {
    let mut g = grammar_ext(&["INDENT", "DEDENT"]);
    let table = build_table(&mut g);
    let builder = adze_tablegen::LanguageBuilder::new(g, table);
    assert!(builder.generate_language().is_ok());
}

// ===========================================================================
// 7. ext_serialize — serialization of externals (8 tests)
// ===========================================================================

#[test]
fn ext_serialize_external_token_roundtrip_json() {
    let tok = ExternalToken {
        name: "HEREDOC".to_string(),
        symbol_id: SymbolId(100),
    };
    let json = serde_json::to_string(&tok).unwrap();
    let restored: ExternalToken = serde_json::from_str(&json).unwrap();
    assert_eq!(tok, restored);
}

#[test]
fn ext_serialize_external_token_json_has_name_field() {
    let tok = ExternalToken {
        name: "MY_SCAN".to_string(),
        symbol_id: SymbolId(5),
    };
    let json = serde_json::to_string(&tok).unwrap();
    assert!(json.contains("MY_SCAN"));
}

#[test]
fn ext_serialize_external_token_json_has_symbol_id_field() {
    let tok = ExternalToken {
        name: "TOK".to_string(),
        symbol_id: SymbolId(999),
    };
    let json = serde_json::to_string(&tok).unwrap();
    assert!(json.contains("999"));
}

#[test]
fn ext_serialize_language_json_external_token_count() {
    let mut g = Grammar::new("t".to_string());
    add_externals(&mut g, 3, 10);
    let table = minimal_table(&g);
    let json_str = adze_tablegen::serializer::serialize_language(&g, &table, None).unwrap();
    let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(val["external_token_count"], 3);
}

#[test]
fn ext_serialize_language_json_zero_external_when_none() {
    let g = Grammar::new("t".to_string());
    let table = minimal_table(&g);
    let json_str = adze_tablegen::serializer::serialize_language(&g, &table, None).unwrap();
    let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(val["external_token_count"], 0);
}

#[test]
fn ext_serialize_language_json_symbol_names_include_ext() {
    let mut g = Grammar::new("t".to_string());
    g.externals.push(ExternalToken {
        name: "SCAN_EXT".to_string(),
        symbol_id: SymbolId(77),
    });
    let table = minimal_table(&g);
    let json_str = adze_tablegen::serializer::serialize_language(&g, &table, None).unwrap();
    let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let sym_names = val["symbol_names"].as_array().unwrap();
    assert!(sym_names.iter().any(|n| n == "SCAN_EXT"));
}

#[test]
fn ext_serialize_grammar_roundtrip_preserves_externals() {
    let mut g = Grammar::new("t".to_string());
    add_externals(&mut g, 2, 30);
    let json = serde_json::to_string(&g).unwrap();
    let restored: Grammar = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.externals.len(), 2);
    assert_eq!(restored.externals[0].name, "X_0");
    assert_eq!(restored.externals[1].name, "X_1");
}

#[test]
fn ext_serialize_grammar_roundtrip_preserves_symbol_ids() {
    let mut g = Grammar::new("t".to_string());
    add_externals(&mut g, 3, 50);
    let json = serde_json::to_string(&g).unwrap();
    let restored: Grammar = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.externals[0].symbol_id, SymbolId(50));
    assert_eq!(restored.externals[1].symbol_id, SymbolId(51));
    assert_eq!(restored.externals[2].symbol_id, SymbolId(52));
}

// ===========================================================================
// 8. ext_edge — edge cases (8 tests)
// ===========================================================================

#[test]
fn ext_edge_zero_externals_v1_empty_symbol_map() {
    let g = Grammar::new("t".to_string());
    let map = V1Generator::new(g).generate_symbol_map();
    assert!(map.is_empty());
}

#[test]
fn ext_edge_zero_externals_v2_empty_symbol_map() {
    let g = Grammar::new("t".to_string());
    let table = minimal_table(&g);
    let map = V2Generator::new(g, table).generate_symbol_map();
    assert!(map.is_empty());
}

#[test]
fn ext_edge_symbol_id_zero_in_map() {
    let mut g = Grammar::new("t".to_string());
    g.externals.push(ExternalToken {
        name: "ZERO_ID".to_string(),
        symbol_id: SymbolId(0),
    });
    let map = V1Generator::new(g).generate_symbol_map();
    assert_eq!(map, [0]);
}

#[test]
fn ext_edge_symbol_id_max_u16() {
    let mut g = Grammar::new("t".to_string());
    g.externals.push(ExternalToken {
        name: "MAX_ID".to_string(),
        symbol_id: SymbolId(u16::MAX),
    });
    let map = V1Generator::new(g).generate_symbol_map();
    assert_eq!(map, [u16::MAX]);
}

#[test]
fn ext_edge_many_externals_50() {
    let mut g = Grammar::new("t".to_string());
    add_externals(&mut g, 50, 500);
    let scanner = V1Generator::new(g);
    assert_eq!(scanner.external_token_count(), 50);
    let map = scanner.generate_symbol_map();
    assert_eq!(map.len(), 50);
    for (i, &val) in map.iter().enumerate() {
        assert_eq!(val, 500 + i as u16);
    }
}

#[test]
fn ext_edge_v1_bitmap_zero_states_returns_empty() {
    let mut g = Grammar::new("t".to_string());
    add_externals(&mut g, 3, 10);
    let bitmap = V1Generator::new(g).generate_state_bitmap(0);
    assert!(bitmap.is_empty());
}

#[test]
fn ext_edge_non_contiguous_symbol_ids_preserved() {
    let mut g = Grammar::new("t".to_string());
    g.externals.push(ExternalToken {
        name: "A".to_string(),
        symbol_id: SymbolId(5),
    });
    g.externals.push(ExternalToken {
        name: "B".to_string(),
        symbol_id: SymbolId(1000),
    });
    g.externals.push(ExternalToken {
        name: "C".to_string(),
        symbol_id: SymbolId(3),
    });
    let map = V1Generator::new(g).generate_symbol_map();
    assert_eq!(map, [5, 1000, 3]);
}

#[test]
fn ext_edge_duplicate_external_names_allowed() {
    // Grammars may have duplicate names in edge cases; they shouldn't panic.
    let mut g = Grammar::new("t".to_string());
    g.externals.push(ExternalToken {
        name: "DUP".to_string(),
        symbol_id: SymbolId(1),
    });
    g.externals.push(ExternalToken {
        name: "DUP".to_string(),
        symbol_id: SymbolId(2),
    });
    let scanner = V1Generator::new(g);
    assert_eq!(scanner.external_token_count(), 2);
    let map = scanner.generate_symbol_map();
    assert_eq!(map, [1, 2]);
}
