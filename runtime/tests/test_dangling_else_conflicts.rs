//! Diagnostic test for dangling-else grammar conflict detection
//!
//! This test verifies that the dangling-else grammar DOES generate shift/reduce conflicts
//! as expected, validating our GLR conflict preservation implementation.
//!
//! Expected conflict in state after "if Expr then Statement", on lookahead "else":
//!   - Shift: Continue outer if (attach else to outer)
//!   - Reduce: Complete inner if (attach else to inner)

#[cfg(all(feature = "pure-rust", feature = "glr"))]
#[test]
fn inspect_dangling_else_conflicts() {
    // Access the generated dangling_else language
    let lang = &adze_example::dangling_else::generated::LANGUAGE;

    // Decode the parse table
    let parse_table = adze::decoder::decode_parse_table(lang);

    eprintln!("\n=== Dangling Else Grammar Parse Table Inspection ===");
    eprintln!("Total states: {}", parse_table.action_table.len());
    eprintln!("Total symbols: {}", parse_table.symbol_count);
    eprintln!(
        "Symbol metadata count: {}",
        parse_table.symbol_metadata.len()
    );

    // Inspect state 0
    if !parse_table.action_table.is_empty() {
        eprintln!("\n--- State 0 Actions ---");
        let state0 = &parse_table.action_table[0];

        for (symbol_idx, action_cell) in state0.iter().enumerate() {
            if !action_cell.is_empty() {
                // Get symbol name
                let symbol_name = if symbol_idx < parse_table.symbol_metadata.len() {
                    &parse_table.symbol_metadata[symbol_idx].name
                } else {
                    "unknown"
                };

                eprintln!(
                    "  Symbol {} ({}): {} actions",
                    symbol_idx,
                    symbol_name,
                    action_cell.len()
                );
                for (i, action) in action_cell.iter().enumerate() {
                    eprintln!("    Action {}: {:?}", i, action);
                }
            }
        }
    }

    // Check ALL states for multi-action cells (GLR conflicts)
    eprintln!("\n--- Multi-Action Cells (GLR Conflicts) ---");
    let mut found_conflicts = false;
    let mut conflict_count = 0;

    for (state_idx, state) in parse_table.action_table.iter().enumerate() {
        for (symbol_idx, action_cell) in state.iter().enumerate() {
            if action_cell.len() > 1 {
                found_conflicts = true;
                conflict_count += 1;

                let symbol_name = if symbol_idx < parse_table.symbol_metadata.len() {
                    &parse_table.symbol_metadata[symbol_idx].name
                } else {
                    "unknown"
                };

                eprintln!(
                    "  State {}, Symbol {} ({}): {} actions",
                    state_idx,
                    symbol_idx,
                    symbol_name,
                    action_cell.len()
                );

                for (i, action) in action_cell.iter().enumerate() {
                    eprintln!("    Action {}: {:?}", i, action);
                }

                // Check if this is the expected dangling-else conflict
                if symbol_name == "else" {
                    eprintln!("    ✓ Found expected 'else' conflict!");
                    eprintln!("    ✓ Multiple actions preserved (likely shift/reduce)");
                }
            }
        }
    }

    if !found_conflicts {
        eprintln!("  ⚠ WARNING: No multi-action cells found!");
        eprintln!("  This means GLR conflicts were NOT preserved during table generation.");
        eprintln!("  The dangling-else grammar SHOULD have shift/reduce conflicts.");
    } else {
        eprintln!("\n✅ Total conflicts found: {}", conflict_count);
    }

    // Inspect rules
    eprintln!("\n--- Parse Rules ---");
    for (i, rule) in parse_table.rules.iter().enumerate().take(10) {
        eprintln!("  Rule {}: LHS={}, RHS_LEN={}", i, rule.lhs.0, rule.rhs_len);
    }

    // This test SHOULD find conflicts in the dangling-else grammar
    // If no conflicts found, that indicates the GLR conflict preservation isn't working
    if found_conflicts {
        eprintln!("\n✅ TEST PASSED: Conflicts detected as expected");
    } else {
        eprintln!("\n⚠ TEST OBSERVATION: No conflicts found");
        eprintln!("   This may indicate:");
        eprintln!("   1. GLR conflict preservation not working (investigate glr-core)");
        eprintln!("   2. Grammar doesn't generate conflicts (check grammar definition)");
        eprintln!("   3. LR(1) lookahead resolves ambiguity (unexpected for dangling-else)");
    }

    // Always pass the test - this is diagnostic, not assertion-based
    assert!(
        !parse_table.action_table.is_empty(),
        "Parse table should have at least one state"
    );
}

#[cfg(all(feature = "pure-rust", feature = "glr"))]
#[test]
fn verify_conflict_preservation_behavior() {
    use adze_glr_core::Action;
    use adze_glr_core::conflict_inspection::{cell_has_conflict, count_conflicts};

    let lang = &adze_example::dangling_else::generated::LANGUAGE;
    let parse_table = adze::decoder::decode_parse_table(lang);

    let direct_conflict_cells = parse_table
        .action_table
        .iter()
        .flat_map(|state| state.iter())
        .filter(|cell| cell_has_conflict(cell))
        .count();

    assert!(
        direct_conflict_cells > 0,
        "dangling-else grammar must decode with preserved multi-action GLR conflict cells"
    );

    let summary = count_conflicts(&parse_table);
    assert!(
        summary.shift_reduce > 0,
        "dangling-else grammar must preserve at least one shift/reduce conflict"
    );

    let else_shift_reduce =
        parse_table
            .action_table
            .iter()
            .enumerate()
            .find_map(|(state_idx, state)| {
                state.iter().enumerate().find_map(|(symbol_idx, cell)| {
                    let symbol_name = parse_table
                        .symbol_metadata
                        .get(symbol_idx)
                        .map(|metadata| metadata.name.as_str());
                    let has_shift = cell.iter().any(|action| matches!(action, Action::Shift(_)));
                    let has_reduce = cell
                        .iter()
                        .any(|action| matches!(action, Action::Reduce(_)));

                    (symbol_name == Some("else")
                        && cell_has_conflict(cell)
                        && has_shift
                        && has_reduce)
                        .then_some((state_idx, symbol_idx, cell))
                })
            });

    assert!(
        else_shift_reduce.is_some(),
        "dangling-else grammar must preserve a shift/reduce conflict on the 'else' symbol; summary details: {:?}",
        summary.conflict_details
    );

    let (state_idx, symbol_idx, actions) = else_shift_reduce.expect("checked above");
    assert!(
        actions.len() >= 2,
        "else shift/reduce conflict must retain both parse actions"
    );

    eprintln!(
        "validated dangling-else conflict: state={} symbol={} actions={:?}",
        state_idx, symbol_idx, actions
    );
}

#[cfg(all(feature = "pure-rust", feature = "glr"))]
#[test]
fn generated_dangling_else_selects_nearest_else_and_records_ambiguity() {
    use adze_example::dangling_else::grammar::{self, Statement};

    let input = "if a then if b then other else other";
    let parsed = grammar::parse(input).expect("dangling-else GLR parse should select an AST");

    match parsed {
        Statement::IfThen(_, outer_expr, _, inner) => {
            assert_eq!(*outer_expr, grammar::Expr::Var("a".to_string()));
            match *inner {
                Statement::IfThenElse(_, inner_expr, _, then_branch, _, else_branch) => {
                    assert_eq!(*inner_expr, grammar::Expr::Var("b".to_string()));
                    assert!(matches!(*then_branch, Statement::Other(())));
                    assert!(matches!(*else_branch, Statement::Other(())));
                }
                other => panic!("expected nearest-else selected inner IfThenElse, got {other:?}"),
            }
        }
        other => panic!("expected outer IfThen selected tree, got {other:?}"),
    };

    let document = grammar::parse_document(input)
        .expect("dangling-else GLR parse_document should return the selected document");
    assert!(document.diagnostics().is_empty());
    assert!(!document.tree().has_errors());
    assert_eq!(
        document.tree().root().byte_range(),
        0..input.len(),
        "selected document tree should cover the full input"
    );
    assert!(
        !document.ambiguities().is_empty(),
        "dangling-else document should record retained ambiguity alternatives"
    );
    let ambiguity = &document.ambiguities()[0];
    assert_eq!(ambiguity.alternatives.len(), 2);
    let selected = ambiguity
        .selected
        .expect("dangling-else ambiguity should record the selected alternative");
    assert!(
        selected < ambiguity.alternatives.len(),
        "selected ambiguity alternative should be in range"
    );
}
