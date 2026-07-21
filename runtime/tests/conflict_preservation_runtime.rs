/// Runtime Conflict Preservation Tests
///
/// These tests validate that GLR conflicts survive the encoding/decoding pipeline:
/// ParseTable → TSLanguage (encode) → decoder::decode_parse_table → ParseTable
///
/// This is the critical integration point between glr-core table generation
/// and the runtime decoder, ensuring conflicts are preserved through ABI boundaries.
///
/// Spec: docs/specs/TABLE_GENERATION_VALIDATION_CONTRACT.md
/// Phase: 2-3 Bridge - GLR Conflict Preservation across ABI
// These tests require example grammars to be built with pure-rust feature
#[cfg(feature = "pure-rust")]
mod runtime_conflict_preservation {
    #[allow(unused_imports)]
    use adze_glr_core::conflict_inspection::*;

    /// Test: Ambiguous Expression Grammar Conflicts Survive Encoding/Decoding
    ///
    /// This test validates the complete pipeline:
    /// 1. Example grammar (ambiguous_expr.rs) is compiled with GLR conflicts
    /// 2. glr-core generates ParseTable with multi-action cells
    /// 3. tablegen encodes to TSLanguage ABI
    /// 4. runtime decoder::decode_parse_table reconstructs ParseTable
    /// 5. conflict_inspection detects the same conflicts
    ///
    /// If this fails while glr-core tests pass, the bug is in:
    /// - tablegen::compress.rs (encoding), or
    /// - runtime::decoder::decode_parse_table (decoding)
    #[test]
    fn test_ambiguous_expr_conflicts_survive_encoding() {
        use adze::decoder::decode_parse_table;
        use adze_example::ambiguous_expr::grammar;

        let lang = grammar::language();
        let table = decode_parse_table(lang);
        let summary = count_conflicts(&table);

        assert!(
            summary.shift_reduce >= 1,
            "ambiguous_expr must preserve at least 1 S/R conflict after encode/decode, got {summary:?}"
        );

        let mut direct_multi_action_cells = 0usize;
        for state_actions in &table.action_table {
            for cell in state_actions {
                if cell.len() > 1 {
                    direct_multi_action_cells += 1;
                }
            }
        }
        assert!(
            direct_multi_action_cells >= 1,
            "ambiguous_expr decode must retain multi-action cells through ABI roundtrip"
        );
    }

    /// Test: Dangling Else Grammar Conflicts Survive Encoding/Decoding
    ///
    /// Validates the classic dangling-else ambiguity is preserved.
    #[test]
    fn test_dangling_else_conflicts_survive_encoding() {
        eprintln!("Runtime Conflict Preservation Test:");
        eprintln!("  Grammar: dangling_else");
        eprintln!("  Expected: At least 1 S/R conflict on 'else' symbol");
        eprintln!("  Status: Awaiting example grammar integration");

        // TODO: Similar to ambiguous_expr test above
    }

    /// Test: Arithmetic Grammar Remains Conflict-Free
    ///
    /// Validates that precedence-resolved grammars don't accidentally
    /// introduce conflicts through the encoding/decoding pipeline.
    #[test]
    fn test_arithmetic_remains_conflict_free_after_encoding() {
        eprintln!("Runtime Conflict Preservation Test:");
        eprintln!("  Grammar: arithmetic (with precedence)");
        eprintln!("  Expected: 0 conflicts (precedence resolves ambiguity)");
        eprintln!("  Status: Awaiting example grammar integration");

        // TODO: Validate that conflict-free grammars stay conflict-free
        /*
        use adze::decoder::decode_parse_table;

        let lang = unsafe { &adze_example::arithmetic::generated::LANGUAGE };
        let table = decode_parse_table(lang);
        let summary = count_conflicts(&table);

        assert_eq!(
            summary.shift_reduce + summary.reduce_reduce, 0,
            "arithmetic grammar should remain conflict-free after encode/decode, got {summary:?}"
        );
        */
    }
}

/// Non-feature-gated test to ensure module compiles
#[test]
fn test_conflict_preservation_runtime_module_exists() {
    // This test ensures the module structure is correct
    // even without pure-rust feature.
    // The fact that this file compiles is the verification.
}
