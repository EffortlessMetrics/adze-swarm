//! Comprehensive roundtrip tests for table compression in adze-tablegen.
//!
//! Covers: compression of real parse tables from grammar pipelines,
//! roundtrip correctness, size reduction, edge cases, and ABI builder integration.

use adze_glr_core::{Action, FirstFollowSets, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{RuleId, StateId};
use adze_tablegen::compress::{CompressedGotoEntry, TableCompressor};
use adze_tablegen::compression::{
    compress_action_table, compress_goto_table, decompress_action, decompress_goto,
};
use adze_tablegen::{collect_token_indices, eof_accepts_or_reduces};
use std::collections::BTreeMap;

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Build grammar → FIRST/FOLLOW → LR(1) → compress, returning the parse table
/// and compressed tables.
fn pipeline(
    grammar_fn: impl FnOnce() -> adze_ir::Grammar,
) -> (adze_glr_core::ParseTable, adze_tablegen::CompressedTables) {
    let mut grammar = grammar_fn();
    let ff =
        FirstFollowSets::compute_normalized(&mut grammar).expect("FIRST/FOLLOW computation failed");
    let table = build_lr1_automaton(&grammar, &ff).expect("LR(1) automaton construction failed");
    let token_indices = collect_token_indices(&grammar, &table);
    let start_empty = eof_accepts_or_reduces(&table);
    let compressor = TableCompressor::new();
    let compressed = compressor
        .compress(&table, &token_indices, start_empty)
        .expect("Table compression failed");
    (table, compressed)
}

/// Wrap single actions into GLR action cells for the compression module.
fn single_action_table(rows: Vec<Vec<Action>>) -> Vec<Vec<Vec<Action>>> {
    rows.into_iter()
        .map(|row| {
            row.into_iter()
                .map(|a| {
                    if matches!(a, Action::Error) {
                        vec![]
                    } else {
                        vec![a]
                    }
                })
                .collect()
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Compression of parse tables from simple grammars
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compress_single_token_grammar() {
    let (_pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .rule("start", vec!["a"])
            .start("start")
            .build()
    });
    assert!(
        !compressed.action_table.data.is_empty(),
        "action table must have entries"
    );
}

#[test]
fn compress_two_alternative_grammar() {
    let (_pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .token("b", "b")
            .rule("start", vec!["a"])
            .rule("start", vec!["b"])
            .start("start")
            .build()
    });
    assert!(!compressed.action_table.data.is_empty());
    assert!(!compressed.goto_table.data.is_empty());
}

#[test]
fn compress_sequence_grammar() {
    let (_pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .token("b", "b")
            .rule("start", vec!["a", "b"])
            .start("start")
            .build()
    });
    assert!(!compressed.action_table.data.is_empty());
}

#[test]
fn compress_chain_grammar() {
    let (_pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("x", "x")
            .rule("c", vec!["x"])
            .rule("b", vec!["c"])
            .rule("start", vec!["b"])
            .start("start")
            .build()
    });
    assert!(!compressed.action_table.data.is_empty());
    assert!(!compressed.goto_table.data.is_empty());
}

#[test]
fn compress_left_recursive_grammar() {
    let (pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .rule("list", vec!["a"])
            .rule("list", vec!["list", "a"])
            .start("list")
            .build()
    });
    assert!(
        pt.state_count >= 3,
        "left-recursive grammar needs multiple states"
    );
    assert!(!compressed.action_table.data.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Roundtrip: compress then verify action lookups
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn roundtrip_action_table_dedup() {
    let table = single_action_table(vec![
        vec![Action::Shift(StateId(1)), Action::Error, Action::Accept],
        vec![Action::Reduce(RuleId(0)), Action::Error, Action::Error],
        vec![
            Action::Shift(StateId(2)),
            Action::Reduce(RuleId(1)),
            Action::Error,
        ],
    ]);
    let compressed = compress_action_table(&table);
    for (state, row) in table.iter().enumerate() {
        for (sym, cell) in row.iter().enumerate() {
            let expected = cell.first().cloned().unwrap_or(Action::Error);
            let got = decompress_action(&compressed, state, sym);
            assert_eq!(got, expected, "state={state} sym={sym}");
        }
    }
}

#[test]
fn roundtrip_goto_table_sparse() {
    let table = vec![
        vec![None, Some(StateId(3)), None, Some(StateId(5))],
        vec![Some(StateId(1)), None, Some(StateId(2)), None],
        vec![None, None, None, None],
    ];
    let compressed = compress_goto_table(&table);
    for (state, row) in table.iter().enumerate() {
        for (sym, &expected) in row.iter().enumerate() {
            let got = decompress_goto(&compressed, state, sym);
            assert_eq!(got, expected, "GOTO mismatch at state={state} sym={sym}");
        }
    }
}

#[test]
fn roundtrip_small_table_compressor_actions() {
    let compressor = TableCompressor::new();
    let action_table = vec![
        vec![
            vec![Action::Shift(StateId(1))],
            vec![],
            vec![Action::Accept],
        ],
        vec![vec![Action::Reduce(RuleId(0))], vec![], vec![]],
    ];
    let sym_map = BTreeMap::new();
    let compressed = compressor
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();

    // Verify row_offsets length = n_states + 1
    assert_eq!(compressed.row_offsets.len(), 3);

    // Walk each row and verify entries
    for (state, row) in action_table.iter().enumerate() {
        let start = compressed.row_offsets[state] as usize;
        let end = compressed.row_offsets[state + 1] as usize;
        let entries = &compressed.data[start..end];

        // Count non-error cells in original
        let non_error_count = row.iter().filter(|cell| !cell.is_empty()).count();
        assert_eq!(
            entries.len(),
            non_error_count,
            "state {state}: entry count mismatch"
        );
    }
}

#[test]
fn roundtrip_goto_run_length() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![
        StateId(1),
        StateId(1),
        StateId(1),
        StateId(1),
        StateId(2),
    ]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();

    // Run of 4 should produce RunLength
    let has_rl = compressed
        .data
        .iter()
        .any(|e| matches!(e, CompressedGotoEntry::RunLength { state: 1, count: 4 }));
    assert!(has_rl, "run of 4 identical states should produce RunLength");
}

#[test]
fn roundtrip_encode_decode_all_action_types() {
    let compressor = TableCompressor::new();
    let cases: Vec<(Action, u16)> = vec![
        (Action::Shift(StateId(0)), 0),
        (Action::Shift(StateId(100)), 100),
        (Action::Reduce(RuleId(0)), 0x8000 | 0x0001),
        (Action::Reduce(RuleId(42)), 0x8000 | 0x002b),
        (Action::Accept, 0xFFFF),
        (Action::Error, 0xFFFE),
        (Action::Recover, 0xFFFD),
    ];
    for (action, expected_encoding) in cases {
        let encoded = compressor.encode_action_small(&action).unwrap();
        assert_eq!(
            encoded, expected_encoding,
            "encoding mismatch for {:?}",
            action
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Table size reduction verification
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn dedup_reduces_identical_rows() {
    let row = vec![
        vec![Action::Shift(StateId(5))],
        vec![Action::Reduce(RuleId(1))],
    ];
    let table = vec![row.clone(), row.clone(), row.clone(), row];
    let compressed = compress_action_table(&table);
    assert_eq!(
        compressed.unique_rows.len(),
        1,
        "4 identical rows → 1 unique"
    );
    assert_eq!(compressed.state_to_row.len(), 4);
}

#[test]
fn validate_rejects_action_symbol_out_of_bounds() {
    let (pt, mut bad) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .rule("start", vec!["a"])
            .start("start")
            .build()
    });

    bad.action_table
        .data
        .push(adze_tablegen::compress::CompressedActionEntry {
            symbol: u16::MAX,
            action: Action::Shift(StateId(0)),
        });
    *bad.action_table
        .row_offsets
        .last_mut()
        .expect("compressed table has sentinel row offset") = bad.action_table.data.len() as u16;

    let err = bad
        .validate(&pt)
        .expect_err("out-of-bounds action symbol must be rejected");
    assert!(err.to_string().contains("outside symbol_count"));
}

#[test]
fn validate_rejects_goto_state_out_of_bounds() {
    let (pt, mut bad) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .rule("start", vec!["a"])
            .start("start")
            .build()
    });

    bad.goto_table
        .data
        .push(CompressedGotoEntry::Single(pt.state_count as u16));
    *bad.goto_table
        .row_offsets
        .last_mut()
        .expect("compressed table has sentinel row offset") = bad.goto_table.data.len() as u16;

    let err = bad
        .validate(&pt)
        .expect_err("out-of-bounds goto state must be rejected");
    assert!(err.to_string().contains("outside state_count"));
}

#[test]
fn sparse_goto_uses_fewer_entries_than_cells() {
    let n_states = 10;
    let n_syms = 10;
    let table: Vec<Vec<Option<StateId>>> = (0..n_states)
        .map(|s| {
            (0..n_syms)
                .map(|sym| {
                    if (s + sym) % 7 == 0 {
                        Some(StateId(1))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect();
    let compressed = compress_goto_table(&table);
    let total_cells = n_states * n_syms;
    assert!(
        compressed.entries.len() < total_cells,
        "sparse goto ({} entries) should use fewer than {} total cells",
        compressed.entries.len(),
        total_cells
    );
}

#[test]
fn small_table_compressor_only_stores_non_error() {
    let compressor = TableCompressor::new();
    let n_syms = 20;
    // Only 3 of 20 columns have real actions
    let action_table: Vec<Vec<Vec<Action>>> = vec![
        (0..n_syms)
            .map(|i| {
                if i == 2 || i == 7 || i == 15 {
                    vec![Action::Shift(StateId(i as u16))]
                } else {
                    vec![]
                }
            })
            .collect(),
    ];
    let sym_map = BTreeMap::new();
    let compressed = compressor
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    assert_eq!(compressed.data.len(), 3, "only 3 non-error entries stored");
}

#[test]
fn pipeline_compression_produces_fewer_entries_than_cells() {
    let (pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .token("b", "b")
            .rule("start", vec!["a"])
            .rule("start", vec!["b"])
            .start("start")
            .build()
    });
    let total_action_cells: usize = pt.action_table.iter().map(|row| row.len()).sum();
    assert!(
        compressed.action_table.data.len() < total_action_cells,
        "compressed entries ({}) should be < total cells ({})",
        compressed.action_table.data.len(),
        total_action_cells
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn single_state_action_table() {
    let table = vec![vec![vec![Action::Accept]]];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 1);
    assert_eq!(decompress_action(&compressed, 0, 0), Action::Accept);
}

#[test]
fn single_state_goto_table() {
    let table = vec![vec![Some(StateId(0))]];
    let compressed = compress_goto_table(&table);
    assert_eq!(decompress_goto(&compressed, 0, 0), Some(StateId(0)));
}

#[test]
fn all_error_table() {
    let table = vec![vec![vec![]; 10]; 5];
    let compressed = compress_action_table(&table);
    for state in 0..5 {
        for sym in 0..10 {
            assert_eq!(
                decompress_action(&compressed, state, sym),
                Action::Error,
                "all-error at state={state} sym={sym}"
            );
        }
    }
}

#[test]
fn all_none_goto_table() {
    let table = vec![vec![None; 8]; 4];
    let compressed = compress_goto_table(&table);
    assert!(compressed.entries.is_empty());
    for state in 0..4 {
        for sym in 0..8 {
            assert_eq!(decompress_goto(&compressed, state, sym), None);
        }
    }
}

#[test]
fn multi_action_cell_decompresses_first() {
    let table = vec![vec![vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(2)),
    ]]];
    let compressed = compress_action_table(&table);
    assert_eq!(
        decompress_action(&compressed, 0, 0),
        Action::Shift(StateId(1)),
        "GLR multi-action cell returns first action"
    );
}

#[test]
fn large_table_30_states_roundtrip() {
    let n_states = 30;
    let n_syms = 8;
    let table: Vec<Vec<Vec<Action>>> = (0..n_states)
        .map(|s| {
            (0..n_syms)
                .map(|sym| match (s + sym) % 4 {
                    0 => vec![Action::Shift(StateId(((s + sym) % 20) as u16))],
                    1 => vec![Action::Reduce(RuleId(((s * sym) % 10) as u16))],
                    2 => vec![Action::Accept],
                    _ => vec![],
                })
                .collect()
        })
        .collect();
    let compressed = compress_action_table(&table);
    for (state, row) in table.iter().enumerate() {
        for (sym, cell) in row.iter().enumerate() {
            let expected = cell.first().cloned().unwrap_or(Action::Error);
            let got = decompress_action(&compressed, state, sym);
            assert_eq!(got, expected, "state={state} sym={sym}");
        }
    }
}

#[test]
fn large_symbol_count_60_columns() {
    let n_syms = 60;
    let table: Vec<Vec<Vec<Action>>> = vec![
        (0..n_syms)
            .map(|sym| {
                if sym % 6 == 0 {
                    vec![Action::Shift(StateId((sym % 25) as u16))]
                } else {
                    vec![]
                }
            })
            .collect(),
    ];
    let compressed = compress_action_table(&table);
    for sym in 0..n_syms {
        let expected = if sym % 6 == 0 {
            Action::Shift(StateId((sym % 25) as u16))
        } else {
            Action::Error
        };
        assert_eq!(
            decompress_action(&compressed, 0, sym),
            expected,
            "sym={sym}"
        );
    }
}

#[test]
fn goto_all_same_state_uses_run_length() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![StateId(42); 10]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    let has_rl = compressed.data.iter().any(|e| {
        matches!(
            e,
            CompressedGotoEntry::RunLength {
                state: 42,
                count: 10
            }
        )
    });
    assert!(has_rl, "10 identical gotos should produce RunLength");
}

#[test]
fn goto_alternating_states_no_run_length() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![StateId(1), StateId(2), StateId(1), StateId(2)]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    let all_single = compressed
        .data
        .iter()
        .all(|e| matches!(e, CompressedGotoEntry::Single(_)));
    assert!(
        all_single,
        "alternating states should all be Single entries"
    );
}

#[test]
fn empty_action_table_compresses() {
    let compressor = TableCompressor::new();
    let action_table: Vec<Vec<Vec<Action>>> = vec![vec![]; 3];
    let sym_map = BTreeMap::new();
    let result = compressor.compress_action_table_small(&action_table, &sym_map);
    assert!(result.is_ok());
    let compressed = result.unwrap();
    assert_eq!(compressed.row_offsets.len(), 4); // 3 states + 1
    assert!(compressed.data.is_empty());
}

#[test]
fn empty_goto_table_compresses() {
    let compressor = TableCompressor::new();
    let goto_table: Vec<Vec<StateId>> = vec![];
    let result = compressor.compress_goto_table_small(&goto_table);
    assert!(result.is_ok());
    let compressed = result.unwrap();
    assert_eq!(compressed.row_offsets.len(), 1); // 0 states + 1
    assert!(compressed.data.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Determinism
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compression_deterministic_row_dedup() {
    let table = single_action_table(vec![
        vec![
            Action::Shift(StateId(1)),
            Action::Error,
            Action::Reduce(RuleId(0)),
        ],
        vec![Action::Error, Action::Accept, Action::Error],
    ]);
    let c1 = compress_action_table(&table);
    let c2 = compress_action_table(&table);
    assert_eq!(c1.unique_rows, c2.unique_rows);
    assert_eq!(c1.state_to_row, c2.state_to_row);
}

#[test]
fn compression_deterministic_small_table() {
    let compressor = TableCompressor::new();
    let action_table = vec![
        vec![
            vec![Action::Shift(StateId(1))],
            vec![],
            vec![Action::Accept],
        ],
        vec![vec![Action::Reduce(RuleId(0))], vec![], vec![]],
    ];
    let sym_map = BTreeMap::new();
    let c1 = compressor
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    let c2 = compressor
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    assert_eq!(c1.row_offsets, c2.row_offsets);
    assert_eq!(c1.default_actions, c2.default_actions);
    assert_eq!(c1.data.len(), c2.data.len());
    for (e1, e2) in c1.data.iter().zip(c2.data.iter()) {
        assert_eq!(e1.symbol, e2.symbol);
        assert_eq!(e1.action, e2.action);
    }
}

#[test]
fn pipeline_compression_deterministic() {
    let build = || {
        let mut grammar = GrammarBuilder::new("t")
            .token("a", "a")
            .token("b", "b")
            .rule("start", vec!["a", "b"])
            .start("start")
            .build();
        let ff = FirstFollowSets::compute_normalized(&mut grammar).unwrap();
        let table = build_lr1_automaton(&grammar, &ff).unwrap();
        let token_indices = collect_token_indices(&grammar, &table);
        let start_empty = eof_accepts_or_reduces(&table);
        let compressor = TableCompressor::new();
        compressor
            .compress(&table, &token_indices, start_empty)
            .unwrap()
    };
    let c1 = build();
    let c2 = build();
    assert_eq!(c1.action_table.data.len(), c2.action_table.data.len());
    assert_eq!(c1.action_table.row_offsets, c2.action_table.row_offsets);
    assert_eq!(c1.goto_table.data.len(), c2.goto_table.data.len());
    assert_eq!(c1.goto_table.row_offsets, c2.goto_table.row_offsets);
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Row offsets invariants
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn row_offsets_nondecreasing() {
    let compressor = TableCompressor::new();
    let action_table = vec![
        vec![vec![Action::Shift(StateId(0))]; 5],
        vec![vec![]; 5],
        vec![vec![Action::Reduce(RuleId(1))]; 5],
        vec![vec![]; 5],
    ];
    let sym_map = BTreeMap::new();
    let compressed = compressor
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    assert_eq!(compressed.row_offsets.len(), 5); // 4 states + 1
    for pair in compressed.row_offsets.windows(2) {
        assert!(pair[1] >= pair[0], "offsets must be non-decreasing");
    }
}

#[test]
fn goto_row_offsets_nondecreasing() {
    let compressor = TableCompressor::new();
    let goto_table = vec![
        vec![StateId(1), StateId(2)],
        vec![StateId(3), StateId(3), StateId(3)],
    ];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    assert_eq!(compressed.row_offsets.len(), 3); // 2 states + 1
    for pair in compressed.row_offsets.windows(2) {
        assert!(pair[1] >= pair[0]);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Encode/decode edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_max_valid_shift_state() {
    let compressor = TableCompressor::new();
    // Max valid shift state: 0x7FFF (32767)
    let encoded = compressor
        .encode_action_small(&Action::Shift(StateId(0x7FFF)))
        .unwrap();
    assert_eq!(encoded, 0x7FFF);
}

#[test]
fn encode_max_valid_reduce_rule() {
    let compressor = TableCompressor::new();
    // Max valid reduce rule: 0x3FFF (16383)
    let encoded = compressor
        .encode_action_small(&Action::Reduce(RuleId(0x3FFF)))
        .unwrap();
    assert_eq!(encoded, 0x8000 | (0x3FFF + 1));
}

#[test]
fn encode_shift_too_large_fails() {
    let compressor = TableCompressor::new();
    let result = compressor.encode_action_small(&Action::Shift(StateId(0x8000)));
    assert!(result.is_err(), "shift state >= 0x8000 should fail");
}

#[test]
fn encode_reduce_too_large_fails() {
    let compressor = TableCompressor::new();
    let result = compressor.encode_action_small(&Action::Reduce(RuleId(0x4000)));
    assert!(result.is_err(), "reduce rule >= 0x4000 should fail");
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. ABI builder integration
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn abi_builder_with_compressed_tables() {
    use adze_tablegen::abi_builder::AbiLanguageBuilder;

    let mut grammar = GrammarBuilder::new("roundtrip_abi")
        .token("x", "x")
        .rule("start", vec!["x"])
        .start("start")
        .build();
    let ff = FirstFollowSets::compute_normalized(&mut grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &ff).unwrap();
    let token_indices = collect_token_indices(&grammar, &table);
    let start_empty = eof_accepts_or_reduces(&table);
    let compressor = TableCompressor::new();
    let compressed = compressor
        .compress(&table, &token_indices, start_empty)
        .unwrap();

    let builder = AbiLanguageBuilder::new(&grammar, &table).with_compressed_tables(&compressed);
    let code = builder.generate();
    let code_str = code.to_string();

    assert!(
        code_str.contains("TSLanguage"),
        "generated code must reference TSLanguage"
    );
    assert!(
        code_str.contains("PARSE_TABLE"),
        "generated code must contain PARSE_TABLE"
    );
}

#[test]
fn abi_builder_generates_valid_syntax() {
    use adze_tablegen::abi_builder::AbiLanguageBuilder;

    let mut grammar = GrammarBuilder::new("syncheck")
        .token("a", "a")
        .token("b", "b")
        .rule("start", vec!["a"])
        .rule("start", vec!["b"])
        .start("start")
        .build();
    let ff = FirstFollowSets::compute_normalized(&mut grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &ff).unwrap();
    let token_indices = collect_token_indices(&grammar, &table);
    let start_empty = eof_accepts_or_reduces(&table);
    let compressor = TableCompressor::new();
    let compressed = compressor
        .compress(&table, &token_indices, start_empty)
        .unwrap();

    let builder = AbiLanguageBuilder::new(&grammar, &table).with_compressed_tables(&compressed);
    let code = builder.generate();
    let code_str = code.to_string();

    // Parse with syn to verify it's valid Rust syntax
    let result = syn::parse_str::<syn::File>(&code_str);
    assert!(
        result.is_ok(),
        "generated code must be valid Rust syntax: {:?}",
        result.err()
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. Pipeline with more complex grammars
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn pipeline_nested_rules() {
    let (_pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("num", "[0-9]+")
            .token("plus", "\\+")
            .rule("atom", vec!["num"])
            .rule("expr", vec!["atom"])
            .rule("expr", vec!["expr", "plus", "atom"])
            .start("expr")
            .build()
    });
    assert!(!compressed.action_table.data.is_empty());
    assert!(!compressed.goto_table.data.is_empty());
}

#[test]
fn pipeline_multiple_nonterminals() {
    let (pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .token("b", "b")
            .token("c", "c")
            .rule("x", vec!["a"])
            .rule("y", vec!["b"])
            .rule("z", vec!["c"])
            .rule("start", vec!["x", "y", "z"])
            .start("start")
            .build()
    });
    // Multiple nonterminals should create goto entries
    assert!(!compressed.goto_table.data.is_empty());
    assert!(pt.state_count >= 4, "need states for each shift + reduces");
}

#[test]
fn pipeline_validates_compressed_tables() {
    let (pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .rule("start", vec!["a"])
            .start("start")
            .build()
    });
    // validate() checks compressed-table shape and Accept-on-EOF preservation.
    let result = compressed.validate(&pt);
    assert!(result.is_ok(), "validation should pass: {:?}", result.err());
}

#[test]
fn pipeline_preserves_accept_on_eof_states_in_compressed_table() {
    let (pt, compressed) = pipeline(|| {
        GrammarBuilder::new("acc_eof")
            .token("a", "a")
            .rule("start", vec!["a"])
            .start("start")
            .build()
    });

    let eof_col = *pt
        .symbol_to_index
        .get(&pt.eof_symbol)
        .expect("EOF must be in symbol_to_index");

    let accepting_states: Vec<usize> = pt
        .action_table
        .iter()
        .enumerate()
        .filter_map(|(state, row)| {
            row.get(eof_col).and_then(|cell| {
                cell.iter()
                    .any(|a| matches!(a, Action::Accept))
                    .then_some(state)
            })
        })
        .collect();

    assert!(
        !accepting_states.is_empty(),
        "source table should contain Accept-on-EOF"
    );

    for state in accepting_states {
        let start = compressed.action_table.row_offsets[state] as usize;
        let end = compressed.action_table.row_offsets[state + 1] as usize;
        let has_accept_on_eof = compressed.action_table.data[start..end]
            .iter()
            .any(|entry| entry.symbol as usize == eof_col && entry.action == Action::Accept);
        assert!(
            has_accept_on_eof,
            "compressed table lost Accept-on-EOF for state {state}"
        );
    }
}

#[test]
fn compressed_validation_fails_when_accept_on_eof_is_dropped() {
    let (pt, mut compressed) = pipeline(|| {
        GrammarBuilder::new("acc_eof_neg")
            .token("a", "a")
            .rule("start", vec!["a"])
            .start("start")
            .build()
    });

    // Simulate corruption: rewrite Accept entries to Error while keeping table shape.
    for entry in &mut compressed.action_table.data {
        if entry.action == Action::Accept {
            entry.action = Action::Error;
        }
    }

    let err = compressed
        .validate(&pt)
        .expect_err("validator should reject missing Accept-on-EOF");
    let msg = err.to_string();
    assert!(msg.contains("Accept-on-EOF"), "unexpected error: {msg}");
}

#[test]
fn pipeline_compressed_parse_table_metadata() {
    use adze_tablegen::CompressedParseTable;

    let (pt, _compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .token("b", "b")
            .rule("start", vec!["a", "b"])
            .start("start")
            .build()
    });
    let cpt = CompressedParseTable::from_parse_table(&pt);
    assert_eq!(cpt.symbol_count(), pt.symbol_count);
    assert_eq!(cpt.state_count(), pt.state_count);
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. Default actions
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn default_actions_always_error_when_optimization_disabled() {
    let compressor = TableCompressor::new();
    // All cells are Reduce(0), but default should still be Error
    let action_table = vec![vec![vec![Action::Reduce(RuleId(0))]; 10]; 3];
    let sym_map = BTreeMap::new();
    let compressed = compressor
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    for d in &compressed.default_actions {
        assert_eq!(*d, Action::Error, "default optimization is disabled");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 11. CompressedParseTable construction & field access
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compressed_parse_table_new_for_testing() {
    let cpt = adze_tablegen::CompressedParseTable::new_for_testing(42, 99);
    assert_eq!(cpt.symbol_count(), 42);
    assert_eq!(cpt.state_count(), 99);
}

#[test]
fn compressed_parse_table_zero_dimensions() {
    let cpt = adze_tablegen::CompressedParseTable::new_for_testing(0, 0);
    assert_eq!(cpt.symbol_count(), 0);
    assert_eq!(cpt.state_count(), 0);
}

#[test]
fn compressed_tables_small_table_threshold_default() {
    let tables = adze_tablegen::CompressedTables {
        action_table: adze_tablegen::CompressedActionTable {
            data: vec![],
            row_offsets: vec![0],
            default_actions: vec![],
        },
        goto_table: adze_tablegen::CompressedGotoTable {
            data: vec![],
            row_offsets: vec![0],
        },
        small_table_threshold: 32768,
    };
    assert_eq!(tables.small_table_threshold, 32768);
}

// ═══════════════════════════════════════════════════════════════════════════
// 12. TableCompressor default trait
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn table_compressor_default_matches_new() {
    let a = TableCompressor::new();
    let b = TableCompressor::default();
    // Both should have the same threshold
    let action_table: Vec<Vec<Vec<Action>>> = vec![vec![vec![]; 2]; 2];
    let sym = BTreeMap::new();
    let ca = a.compress_action_table_small(&action_table, &sym).unwrap();
    let cb = b.compress_action_table_small(&action_table, &sym).unwrap();
    assert_eq!(ca.row_offsets, cb.row_offsets);
}

// ═══════════════════════════════════════════════════════════════════════════
// 13. Encoding edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_shift_zero() {
    let compressor = TableCompressor::new();
    let encoded = compressor
        .encode_action_small(&Action::Shift(StateId(0)))
        .unwrap();
    assert_eq!(encoded, 0);
}

#[test]
fn encode_reduce_zero() {
    let compressor = TableCompressor::new();
    // Reduce(0) encodes as 0x8000 | (0+1) = 0x8001
    let encoded = compressor
        .encode_action_small(&Action::Reduce(RuleId(0)))
        .unwrap();
    assert_eq!(encoded, 0x8001);
}

#[test]
fn encode_accept_value() {
    let compressor = TableCompressor::new();
    assert_eq!(
        compressor.encode_action_small(&Action::Accept).unwrap(),
        0xFFFF
    );
}

#[test]
fn encode_error_value() {
    let compressor = TableCompressor::new();
    assert_eq!(
        compressor.encode_action_small(&Action::Error).unwrap(),
        0xFFFE
    );
}

#[test]
fn encode_recover_value() {
    let compressor = TableCompressor::new();
    assert_eq!(
        compressor.encode_action_small(&Action::Recover).unwrap(),
        0xFFFD
    );
}

#[test]
fn encode_fork_requires_flattening() {
    let compressor = TableCompressor::new();
    assert!(
        compressor
            .encode_action_small(&Action::Fork(vec![
                Action::Shift(StateId(1)),
                Action::Reduce(RuleId(0)),
            ]))
            .is_err()
    );
}

#[test]
fn encode_shift_boundary_just_below_limit() {
    let compressor = TableCompressor::new();
    // 0x7FFE is just below the limit
    let encoded = compressor
        .encode_action_small(&Action::Shift(StateId(0x7FFE)))
        .unwrap();
    assert_eq!(encoded, 0x7FFE);
}

#[test]
fn encode_reduce_boundary_just_below_limit() {
    let compressor = TableCompressor::new();
    // 0x3FFE is just below the limit
    let encoded = compressor
        .encode_action_small(&Action::Reduce(RuleId(0x3FFE)))
        .unwrap();
    assert_eq!(encoded, 0x8000 | (0x3FFE + 1));
}

// ═══════════════════════════════════════════════════════════════════════════
// 14. Multi-state action table compression
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compress_five_states_mixed_actions() {
    let compressor = TableCompressor::new();
    let action_table: Vec<Vec<Vec<Action>>> = vec![
        vec![
            vec![Action::Shift(StateId(1))],
            vec![],
            vec![Action::Accept],
        ],
        vec![vec![], vec![Action::Reduce(RuleId(0))], vec![]],
        vec![vec![Action::Shift(StateId(2))], vec![], vec![]],
        vec![vec![], vec![], vec![Action::Reduce(RuleId(1))]],
        vec![vec![Action::Accept], vec![], vec![]],
    ];
    let sym_map = BTreeMap::new();
    let compressed = compressor
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    assert_eq!(compressed.row_offsets.len(), 6); // 5 states + 1
    assert_eq!(compressed.default_actions.len(), 5);
}

#[test]
fn compress_identical_rows_all_explicit() {
    let compressor = TableCompressor::new();
    let row = vec![
        vec![Action::Shift(StateId(1))],
        vec![Action::Reduce(RuleId(0))],
    ];
    let action_table = vec![row.clone(), row.clone(), row];
    let sym_map = BTreeMap::new();
    let compressed = compressor
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    // Each row should produce 2 entries (Shift + Reduce)
    assert_eq!(compressed.data.len(), 6); // 3 rows × 2 entries each
}

// ═══════════════════════════════════════════════════════════════════════════
// 15. Goto table edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn goto_single_entry_no_run_length() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![StateId(5)]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    assert_eq!(compressed.data.len(), 1);
    assert!(matches!(compressed.data[0], CompressedGotoEntry::Single(5)));
}

#[test]
fn goto_two_identical_no_run_length() {
    let compressor = TableCompressor::new();
    // Run of 2 should NOT use RunLength (threshold is >2)
    let goto_table = vec![vec![StateId(3), StateId(3)]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    let all_single = compressed
        .data
        .iter()
        .all(|e| matches!(e, CompressedGotoEntry::Single(3)));
    assert!(all_single, "run of 2 should use Single entries");
    assert_eq!(compressed.data.len(), 2);
}

#[test]
fn goto_exactly_three_uses_run_length() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![StateId(7), StateId(7), StateId(7)]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    assert!(
        compressed
            .data
            .iter()
            .any(|e| matches!(e, CompressedGotoEntry::RunLength { state: 7, count: 3 }))
    );
}

#[test]
fn goto_multiple_runs() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![
        StateId(1),
        StateId(1),
        StateId(1),
        StateId(2),
        StateId(2),
        StateId(2),
        StateId(2),
    ]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    // Should have RunLength(1,3) and RunLength(2,4)
    let has_rl1 = compressed
        .data
        .iter()
        .any(|e| matches!(e, CompressedGotoEntry::RunLength { state: 1, count: 3 }));
    let has_rl2 = compressed
        .data
        .iter()
        .any(|e| matches!(e, CompressedGotoEntry::RunLength { state: 2, count: 4 }));
    assert!(has_rl1, "run of 3 state=1 should produce RunLength");
    assert!(has_rl2, "run of 4 state=2 should produce RunLength");
}

#[test]
fn goto_multi_row_offsets() {
    let compressor = TableCompressor::new();
    let goto_table = vec![
        vec![StateId(0), StateId(1)],
        vec![StateId(2), StateId(3), StateId(4)],
        vec![],
    ];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    assert_eq!(compressed.row_offsets.len(), 4); // 3 rows + 1
    // Row 0: 2 singles, Row 1: 3 singles, Row 2: 0 entries
    assert_eq!(compressed.row_offsets[0], 0);
    let row0_len = compressed.row_offsets[1] - compressed.row_offsets[0];
    assert_eq!(row0_len, 2);
    let row2_len = compressed.row_offsets[3] - compressed.row_offsets[2];
    assert_eq!(row2_len, 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// 16. Roundtrip compression module (compress → decompress)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn roundtrip_action_single_accept() {
    let table = vec![vec![vec![Action::Accept]]];
    let compressed = compress_action_table(&table);
    assert_eq!(decompress_action(&compressed, 0, 0), Action::Accept);
}

#[test]
fn roundtrip_action_single_recover() {
    let table = vec![vec![vec![Action::Recover]]];
    let compressed = compress_action_table(&table);
    assert_eq!(decompress_action(&compressed, 0, 0), Action::Recover);
}

#[test]
fn roundtrip_action_all_shifts() {
    let table: Vec<Vec<Vec<Action>>> = (0..5)
        .map(|s| {
            (0..4)
                .map(|sym| vec![Action::Shift(StateId(((s * 4 + sym) % 10) as u16))])
                .collect()
        })
        .collect();
    let compressed = compress_action_table(&table);
    for (s, row) in table.iter().enumerate() {
        for (sym, cell) in row.iter().enumerate() {
            let expected = cell.first().cloned().unwrap_or(Action::Error);
            let got = decompress_action(&compressed, s, sym);
            assert_eq!(got, expected, "state={s} sym={sym}");
        }
    }
}

#[test]
fn roundtrip_action_all_reduces() {
    let table: Vec<Vec<Vec<Action>>> = (0..4)
        .map(|s| {
            (0..3)
                .map(|sym| vec![Action::Reduce(RuleId(((s + sym) % 5) as u16))])
                .collect()
        })
        .collect();
    let compressed = compress_action_table(&table);
    for (s, row) in table.iter().enumerate() {
        for (sym, cell) in row.iter().enumerate() {
            let expected = cell.first().cloned().unwrap_or(Action::Error);
            assert_eq!(decompress_action(&compressed, s, sym), expected);
        }
    }
}

#[test]
fn roundtrip_goto_single_entry() {
    let table = vec![vec![Some(StateId(42))]];
    let compressed = compress_goto_table(&table);
    assert_eq!(decompress_goto(&compressed, 0, 0), Some(StateId(42)));
}

#[test]
fn roundtrip_goto_diagonal_pattern() {
    let n = 5;
    let table: Vec<Vec<Option<StateId>>> = (0..n)
        .map(|s| {
            (0..n)
                .map(|sym| {
                    if s == sym {
                        Some(StateId(s as u16))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect();
    let compressed = compress_goto_table(&table);
    for s in 0..n {
        for sym in 0..n {
            let expected = if s == sym {
                Some(StateId(s as u16))
            } else {
                None
            };
            assert_eq!(
                decompress_goto(&compressed, s, sym),
                expected,
                "s={s} sym={sym}"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 17. Large table compression
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn large_table_50_states_20_symbols_roundtrip() {
    let n_states = 50;
    let n_syms = 20;
    let table: Vec<Vec<Vec<Action>>> = (0..n_states)
        .map(|s| {
            (0..n_syms)
                .map(|sym| match (s * n_syms + sym) % 5 {
                    0 => vec![Action::Shift(StateId(((s + sym) % 30) as u16))],
                    1 => vec![Action::Reduce(RuleId(((s * sym) % 8) as u16))],
                    2 => vec![Action::Accept],
                    3 => vec![Action::Recover],
                    _ => vec![],
                })
                .collect()
        })
        .collect();
    let compressed = compress_action_table(&table);
    for (s, row) in table.iter().enumerate() {
        for (sym, cell) in row.iter().enumerate() {
            let expected = cell.first().cloned().unwrap_or(Action::Error);
            assert_eq!(
                decompress_action(&compressed, s, sym),
                expected,
                "s={s} sym={sym}"
            );
        }
    }
}

#[test]
fn large_goto_table_40_states_15_symbols() {
    let n_states = 40;
    let n_syms = 15;
    let table: Vec<Vec<Option<StateId>>> = (0..n_states)
        .map(|s| {
            (0..n_syms)
                .map(|sym| {
                    if (s + sym) % 3 == 0 {
                        Some(StateId(((s * 2 + sym) % 20) as u16))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect();
    let compressed = compress_goto_table(&table);
    for (s, row) in table.iter().enumerate() {
        for (sym, &expected) in row.iter().enumerate() {
            assert_eq!(
                decompress_goto(&compressed, s, sym),
                expected,
                "s={s} sym={sym}"
            );
        }
    }
}

#[test]
fn large_symbol_count_100_columns_action() {
    let n_syms = 100;
    let table: Vec<Vec<Vec<Action>>> = vec![
        (0..n_syms)
            .map(|sym| {
                if sym % 10 == 0 {
                    vec![Action::Shift(StateId((sym / 10) as u16))]
                } else {
                    vec![]
                }
            })
            .collect(),
    ];
    let compressed = compress_action_table(&table);
    for sym in 0..n_syms {
        let expected = if sym % 10 == 0 {
            Action::Shift(StateId((sym / 10) as u16))
        } else {
            Action::Error
        };
        assert_eq!(decompress_action(&compressed, 0, sym), expected);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 18. Compression size reduction
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn dedup_reduces_many_identical_rows() {
    let row = vec![
        vec![Action::Shift(StateId(1))],
        vec![Action::Reduce(RuleId(0))],
        vec![],
    ];
    let table = vec![row; 20]; // 20 identical rows
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 1);
    assert_eq!(compressed.state_to_row.len(), 20);
}

#[test]
fn mixed_rows_dedup_correctly() {
    let row_a = vec![vec![Action::Shift(StateId(1))], vec![]];
    let row_b = vec![vec![], vec![Action::Reduce(RuleId(0))]];
    let table = vec![
        row_a.clone(),
        row_b.clone(),
        row_a.clone(),
        row_b.clone(),
        row_a,
    ];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 2, "two distinct row patterns");
    assert_eq!(compressed.state_to_row.len(), 5);
}

#[test]
fn goto_sparse_fewer_entries() {
    let n = 20;
    let table: Vec<Vec<Option<StateId>>> = (0..n)
        .map(|s| {
            (0..n)
                .map(|sym| if s == sym { Some(StateId(1)) } else { None })
                .collect()
        })
        .collect();
    let compressed = compress_goto_table(&table);
    assert_eq!(
        compressed.entries.len(),
        n,
        "diagonal pattern should have exactly n entries"
    );
    assert!(compressed.entries.len() < n * n);
}

// ═══════════════════════════════════════════════════════════════════════════
// 19. Determinism (additional)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn determinism_goto_compression() {
    let compressor = TableCompressor::new();
    let goto_table = vec![
        vec![StateId(1), StateId(1), StateId(1), StateId(2)],
        vec![StateId(3), StateId(3), StateId(3), StateId(3)],
    ];
    let c1 = compressor.compress_goto_table_small(&goto_table).unwrap();
    let c2 = compressor.compress_goto_table_small(&goto_table).unwrap();
    assert_eq!(c1.row_offsets, c2.row_offsets);
    assert_eq!(c1.data.len(), c2.data.len());
}

#[test]
fn determinism_row_dedup_goto() {
    let table: Vec<Vec<Option<StateId>>> = vec![
        vec![Some(StateId(1)), None, Some(StateId(2))],
        vec![None, Some(StateId(3)), None],
    ];
    let c1 = compress_goto_table(&table);
    let c2 = compress_goto_table(&table);
    assert_eq!(c1.entries.len(), c2.entries.len());
    for (key, val) in &c1.entries {
        assert_eq!(c2.entries.get(key), Some(val));
    }
}

#[test]
fn determinism_pipeline_left_recursive() {
    let build = || {
        let mut grammar = GrammarBuilder::new("t")
            .token("a", "a")
            .rule("list", vec!["a"])
            .rule("list", vec!["list", "a"])
            .start("list")
            .build();
        let ff = FirstFollowSets::compute_normalized(&mut grammar).unwrap();
        let table = build_lr1_automaton(&grammar, &ff).unwrap();
        let token_indices = collect_token_indices(&grammar, &table);
        let start_empty = eof_accepts_or_reduces(&table);
        let compressor = TableCompressor::new();
        compressor
            .compress(&table, &token_indices, start_empty)
            .unwrap()
    };
    let c1 = build();
    let c2 = build();
    assert_eq!(c1.action_table.data.len(), c2.action_table.data.len());
    assert_eq!(c1.goto_table.data.len(), c2.goto_table.data.len());
}

// ═══════════════════════════════════════════════════════════════════════════
// 20. Pipeline grammars (additional)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn pipeline_right_recursive_grammar() {
    let (_pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .rule("list", vec!["a"])
            .rule("list", vec!["a", "list"])
            .start("list")
            .build()
    });
    assert!(!compressed.action_table.data.is_empty());
}

#[test]
fn pipeline_single_empty_rule_start() {
    // Grammar with just one token and one rule
    let (pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("x", "x")
            .rule("start", vec!["x"])
            .start("start")
            .build()
    });
    assert!(pt.state_count >= 2);
    assert!(!compressed.action_table.data.is_empty());
}

#[test]
fn pipeline_three_token_sequence() {
    let (_pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .token("b", "b")
            .token("c", "c")
            .rule("start", vec!["a", "b", "c"])
            .start("start")
            .build()
    });
    assert!(!compressed.action_table.data.is_empty());
}

#[test]
fn pipeline_many_alternatives() {
    let (_pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .token("b", "b")
            .token("c", "c")
            .token("d", "d")
            .rule("start", vec!["a"])
            .rule("start", vec!["b"])
            .rule("start", vec!["c"])
            .rule("start", vec!["d"])
            .start("start")
            .build()
    });
    assert!(!compressed.action_table.data.is_empty());
    assert!(!compressed.goto_table.data.is_empty());
}

#[test]
fn pipeline_diamond_grammar() {
    // A → B C, B → x, C → x  (diamond shape in derivation)
    let (pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("x", "x")
            .rule("b", vec!["x"])
            .rule("c", vec!["x"])
            .rule("start", vec!["b", "c"])
            .start("start")
            .build()
    });
    assert!(pt.state_count >= 3);
    assert!(!compressed.action_table.data.is_empty());
}

#[test]
fn pipeline_deeply_nested_rules() {
    let (_pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("x", "x")
            .rule("d", vec!["x"])
            .rule("c", vec!["d"])
            .rule("b", vec!["c"])
            .rule("a", vec!["b"])
            .rule("start", vec!["a"])
            .start("start")
            .build()
    });
    assert!(!compressed.action_table.data.is_empty());
    assert!(!compressed.goto_table.data.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// 21. CompressedActionEntry construction
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compressed_action_entry_shift() {
    let entry = adze_tablegen::CompressedActionEntry::new(0, Action::Shift(StateId(10)));
    assert_eq!(entry.symbol, 0);
    assert!(matches!(entry.action, Action::Shift(StateId(10))));
}

#[test]
fn compressed_action_entry_reduce() {
    let entry = adze_tablegen::CompressedActionEntry::new(5, Action::Reduce(RuleId(3)));
    assert_eq!(entry.symbol, 5);
    assert!(matches!(entry.action, Action::Reduce(RuleId(3))));
}

#[test]
fn compressed_action_entry_accept() {
    let entry = adze_tablegen::CompressedActionEntry::new(0xFFFF, Action::Accept);
    assert_eq!(entry.symbol, 0xFFFF);
    assert!(matches!(entry.action, Action::Accept));
}

// ═══════════════════════════════════════════════════════════════════════════
// 22. Action table row slicing via offsets
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn row_offsets_slice_entries_correctly() {
    let compressor = TableCompressor::new();
    let action_table: Vec<Vec<Vec<Action>>> = vec![
        vec![
            vec![Action::Shift(StateId(0))],
            vec![Action::Shift(StateId(1))],
        ],
        vec![vec![], vec![]],
        vec![
            vec![Action::Accept],
            vec![],
            vec![Action::Reduce(RuleId(0))],
        ],
    ];
    let sym_map = BTreeMap::new();
    let compressed = compressor
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    // Row 0: 2 entries, Row 1: 0 entries, Row 2: 2 entries
    let r0_start = compressed.row_offsets[0] as usize;
    let r0_end = compressed.row_offsets[1] as usize;
    assert_eq!(r0_end - r0_start, 2);

    let r1_start = compressed.row_offsets[1] as usize;
    let r1_end = compressed.row_offsets[2] as usize;
    assert_eq!(r1_end - r1_start, 0);

    let r2_start = compressed.row_offsets[2] as usize;
    let r2_end = compressed.row_offsets[3] as usize;
    assert_eq!(r2_end - r2_start, 2);
}

#[test]
fn row_offsets_last_equals_data_len() {
    let compressor = TableCompressor::new();
    let action_table = vec![
        vec![vec![Action::Shift(StateId(1))]; 4],
        vec![vec![Action::Reduce(RuleId(0))]; 4],
    ];
    let sym_map = BTreeMap::new();
    let compressed = compressor
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    let last_offset = *compressed.row_offsets.last().unwrap() as usize;
    assert_eq!(last_offset, compressed.data.len());
}

// ═══════════════════════════════════════════════════════════════════════════
// 23. Integration: grammar → pipeline → validate
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn pipeline_arithmetic_expr_validates() {
    let (pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("num", "[0-9]+")
            .token("plus", "\\+")
            .token("star", "\\*")
            .rule("atom", vec!["num"])
            .rule("expr", vec!["atom"])
            .rule("expr", vec!["expr", "plus", "atom"])
            .rule("expr", vec!["expr", "star", "atom"])
            .start("expr")
            .build()
    });
    assert!(compressed.validate(&pt).is_ok());
}

#[test]
fn pipeline_compressed_metadata_matches_table() {
    let (pt, _compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .token("b", "b")
            .token("c", "c")
            .rule("start", vec!["a", "b", "c"])
            .start("start")
            .build()
    });
    let cpt = adze_tablegen::CompressedParseTable::from_parse_table(&pt);
    assert!(cpt.state_count() > 0);
    assert!(cpt.symbol_count() > 0);
    assert_eq!(cpt.state_count(), pt.state_count);
    assert_eq!(cpt.symbol_count(), pt.symbol_count);
}

// ═══════════════════════════════════════════════════════════════════════════
// 24. Empty table compression through pipeline
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compress_module_empty_action_table() {
    let table: Vec<Vec<Vec<Action>>> = vec![];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 0);
    assert_eq!(compressed.state_to_row.len(), 0);
}

#[test]
fn compress_module_empty_goto_table() {
    let table: Vec<Vec<Option<StateId>>> = vec![];
    let compressed = compress_goto_table(&table);
    assert!(compressed.entries.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// 25. Comprehensive symbol indices in compressed entries
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn action_entries_have_correct_symbol_indices() {
    let compressor = TableCompressor::new();
    let action_table: Vec<Vec<Vec<Action>>> = vec![vec![
        vec![],                          // col 0: empty
        vec![Action::Shift(StateId(1))], // col 1
        vec![],                          // col 2: empty
        vec![Action::Accept],            // col 3
    ]];
    let sym_map = BTreeMap::new();
    let compressed = compressor
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    assert_eq!(compressed.data.len(), 2);
    assert_eq!(compressed.data[0].symbol, 1);
    assert_eq!(compressed.data[1].symbol, 3);
}

#[test]
fn action_entries_preserve_action_types() {
    let compressor = TableCompressor::new();
    let action_table: Vec<Vec<Vec<Action>>> = vec![vec![
        vec![Action::Shift(StateId(5))],
        vec![Action::Reduce(RuleId(2))],
        vec![Action::Accept],
        vec![Action::Recover],
    ]];
    let sym_map = BTreeMap::new();
    let compressed = compressor
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    assert_eq!(compressed.data.len(), 4);
    assert!(matches!(
        compressed.data[0].action,
        Action::Shift(StateId(5))
    ));
    assert!(matches!(
        compressed.data[1].action,
        Action::Reduce(RuleId(2))
    ));
    assert!(matches!(compressed.data[2].action, Action::Accept));
    assert!(matches!(compressed.data[3].action, Action::Recover));
}
