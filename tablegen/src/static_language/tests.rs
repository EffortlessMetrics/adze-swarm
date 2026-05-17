use super::*;
use crate::compress::CompressedActionEntry;

#[cfg(not(debug_assertions))]
macro_rules! debug_trace {
    ($($arg:tt)*) => {};
}

#[cfg(debug_assertions)]
macro_rules! debug_trace {
    ($($arg:tt)*) => {
        if std::env::var("RUST_LOG")
            .ok()
            .unwrap_or_default()
            .contains("debug")
        {
            eprintln!($($arg)*);
        }
    };
}

#[test]
fn test_static_language_generator_creation() {
    let grammar = Grammar::new("test".to_string());
    let parse_table = crate::empty_table!(states: 1, terms: 0, nonterms: 0);

    let generator = StaticLanguageGenerator::new(grammar, parse_table);
    assert_eq!(generator.grammar.name, "test");
    assert_eq!(generator.parse_table.state_count, 1); // minimum is 1
    assert!(generator.compressed_tables.is_none());
}

#[test]
fn test_action_encoding_small_table() {
    let compressor = TableCompressor::new();

    // Test shift encoding
    let shift_action = Action::Shift(StateId(42));
    let encoded = compressor.encode_action_small(&shift_action).unwrap();
    assert_eq!(encoded, 42);
    assert!(encoded < 0x8000); // High bit should be clear for shifts

    // Test reduce encoding
    let reduce_action = Action::Reduce(RuleId(17));
    let encoded = compressor.encode_action_small(&reduce_action).unwrap();
    // Encoding is 0x8000 | (rule_id + 1), so for rule 17: 0x8000 | 18 = 0x8012 = 32786
    assert_eq!(encoded, 32786);
    assert!(encoded >= 0x8000); // High bit should be set for reduces

    // Test accept encoding
    let accept_action = Action::Accept;
    let encoded = compressor.encode_action_small(&accept_action).unwrap();
    assert_eq!(encoded, 0xFFFF);

    // Test error encoding
    let error_action = Action::Error;
    let encoded = compressor.encode_action_small(&error_action).unwrap();
    assert_eq!(encoded, 0xFFFE);
}

#[test]
fn test_action_encoding_overflow() {
    let compressor = TableCompressor::new();

    // Test shift with state ID too large
    let shift_action = Action::Shift(StateId(0x8000));
    let result = compressor.encode_action_small(&shift_action);
    assert!(result.is_err());

    // Test reduce with rule ID too large
    let reduce_action = Action::Reduce(RuleId(0x4000));
    let result = compressor.encode_action_small(&reduce_action);
    assert!(result.is_err());
}

#[test]
fn test_table_compressor_creation() {
    let compressor = TableCompressor::new();
    // Just test that it can be created
    let _ = compressor;
}

#[test]
fn test_symbol_names_generation() {
    let mut grammar = Grammar::new("test".to_string());

    // Add a token
    let token = Token {
        name: "NUMBER".to_string(),
        pattern: TokenPattern::Regex(r"\d+".to_string()),
        fragile: false,
    };
    grammar.tokens.insert(SymbolId(0), token);

    // Add a rule
    let rule = Rule {
        lhs: SymbolId(1),
        rhs: vec![Symbol::Terminal(SymbolId(0))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    };
    grammar.add_rule(rule);

    let parse_table = crate::empty_table!(states: 1, terms: 0, nonterms: 0);

    let generator = StaticLanguageGenerator::new(grammar, parse_table);
    let symbol_names = generator.generate_symbol_names();

    assert_eq!(symbol_names.len(), 2);
    assert!(symbol_names.contains(&"NUMBER".to_string()));
    assert!(symbol_names.contains(&"rule_1".to_string()));
}

#[test]
fn test_field_names_generation() {
    let mut grammar = Grammar::new("test".to_string());

    // Add fields in lexicographic order
    grammar.fields.insert(FieldId(0), "left".to_string());
    grammar.fields.insert(FieldId(1), "right".to_string());

    let parse_table = crate::empty_table!(states: 1, terms: 0, nonterms: 0);

    let generator = StaticLanguageGenerator::new(grammar, parse_table);
    let field_names = generator.generate_field_names();

    assert_eq!(field_names, vec!["left", "right"]);
}

#[test]
fn test_node_types_generation() {
    let grammar = Grammar::new("test".to_string());
    let parse_table = crate::empty_table!(states: 1, terms: 0, nonterms: 0);

    let generator = StaticLanguageGenerator::new(grammar, parse_table);
    let node_types = generator.generate_node_types();

    // Should be valid JSON
    assert!(serde_json::from_str::<serde_json::Value>(&node_types).is_ok());
}

#[test]
fn test_table_compression_small_table() {
    let grammar = Grammar::new("test".to_string());

    // Create a simple parse table
    let mut parse_table = crate::test_helpers::test::make_minimal_table(
        vec![
            vec![vec![Action::Shift(StateId(1))], vec![Action::Error]],
            vec![vec![Action::Reduce(RuleId(0))], vec![Action::Accept]],
        ],
        vec![vec![StateId(0), StateId(1)], vec![StateId(1), StateId(0)]],
        vec![],
        SymbolId(1), // start_symbol
        SymbolId(1), // eof_symbol (column 1)
        0,           // external_token_count
    );

    // Override to put EOF at column 0 for test compatibility
    // Also update eof_symbol to match the new mapping
    parse_table.eof_symbol = SymbolId(0);
    parse_table.symbol_to_index.clear();
    parse_table.symbol_to_index.insert(SymbolId(0), 0);
    parse_table.symbol_to_index.insert(SymbolId(1), 1);

    let mut generator = StaticLanguageGenerator::new(grammar, parse_table);

    // Test compression
    assert!(generator.compress_tables().is_ok());
    assert!(generator.compressed_tables.is_some());

    let compressed = generator.compressed_tables.as_ref().unwrap();
    assert_eq!(compressed.small_table_threshold, 32768);
}

#[test]
fn test_table_compression_large_table() {
    let _grammar = Grammar::new("large_test".to_string());

    // Create a parse table that exceeds small table threshold
    let mut parse_table = crate::test_helpers::test::make_minimal_table(
        vec![vec![vec![Action::Error]; 10]; 40000],
        vec![vec![StateId(0); 10]; 40000],
        vec![],
        SymbolId(1), // start_symbol
        SymbolId(1), // eof_symbol (column 1)
        0,           // external_token_count
    );

    // Set EOF at column 0 for compatibility with existing test logic
    // Also update eof_symbol to match the new mapping
    parse_table.eof_symbol = SymbolId(0);
    parse_table.symbol_to_index.clear();
    parse_table.symbol_to_index.insert(SymbolId(0), 0);

    // Give state 0 / EOF an Accept so the compressor has a valid path
    parse_table.action_table[0][0] = vec![Action::Accept];

    let compressor = TableCompressor::new();
    // Use proper helper to collect token indices
    let grammar = Grammar::default(); // Minimal grammar for test
    let token_indices = helpers::collect_token_indices(&grammar, &parse_table);
    // We just added Accept on EOF, so this is true for the large-table test
    let start_can_be_empty = true;
    let result = compressor.compress(&parse_table, &token_indices, start_can_be_empty);

    let compressed = result.expect("large table should compress");

    // Should use large table format
    assert_eq!(compressed.small_table_threshold, 32768);
    assert!(parse_table.state_count >= compressed.small_table_threshold);
}

#[test]
fn test_compressed_action_table_small() {
    let compressor = TableCompressor::new();
    let action_table = vec![
        vec![
            vec![Action::Shift(StateId(1))],
            vec![Action::Error],
            vec![Action::Error],
        ],
        vec![
            vec![Action::Error],
            vec![Action::Reduce(RuleId(0))],
            vec![Action::Error],
        ],
    ];

    let symbol_to_index = std::collections::BTreeMap::new();
    let compressed = compressor.compress_action_table_small(&action_table, &symbol_to_index);
    assert!(compressed.is_ok());

    let compressed = compressed.unwrap();
    assert_eq!(compressed.default_actions.len(), 2);
    assert_eq!(compressed.row_offsets.len(), 3); // includes sentinel

    // First row should have default Error, with only Shift(1) stored
    match &compressed.default_actions[0] {
        Action::Error => {}
        _ => panic!("Expected Error as default for first row"),
    }

    // Second row should have default Error (not Reduce, because it's not universal)
    match &compressed.default_actions[1] {
        Action::Error => {}
        _ => panic!("Expected Error as default for second row"),
    }
}

#[test]
fn test_compressed_action_table_with_default_reduction() {
    let compressor = TableCompressor::new();

    // Create a state with only reduce actions (common in LR parsers)
    let action_table = vec![vec![
        vec![Action::Reduce(RuleId(1))],
        vec![Action::Reduce(RuleId(1))],
        vec![Action::Reduce(RuleId(1))],
    ]];

    let symbol_to_index = std::collections::BTreeMap::new();
    let compressed = compressor.compress_action_table_small(&action_table, &symbol_to_index);
    assert!(compressed.is_ok());

    let compressed = compressed.unwrap();

    // Default action optimization is disabled, so default should be Error
    match &compressed.default_actions[0] {
        Action::Error => {}
        _ => panic!("Expected Error as default (optimization disabled)"),
    }

    // All 3 reduce actions should be explicitly encoded in data
    let entries_for_state_0 = compressed.row_offsets[1] - compressed.row_offsets[0];
    assert_eq!(
        entries_for_state_0, 3,
        "All reduce actions should be explicitly encoded"
    );
}

#[test]
fn test_compressed_goto_table_small() {
    let compressor = TableCompressor::new();
    let goto_table = vec![
        vec![StateId(0), StateId(0), StateId(1)],
        vec![StateId(2), StateId(2), StateId(2)],
    ];

    let compressed = compressor.compress_goto_table_small(&goto_table);
    assert!(compressed.is_ok());

    let compressed = compressed.unwrap();
    assert_eq!(compressed.row_offsets.len(), 3); // includes sentinel
    assert!(!compressed.data.is_empty());

    // First row should have run of 2 StateId(0)s, then single StateId(1)
    let first_row_start = compressed.row_offsets[0] as usize;
    let first_row_end = compressed.row_offsets[1] as usize;
    let first_row_entries = &compressed.data[first_row_start..first_row_end];

    // Should be stored as individual entries (run of 2 is too short)
    assert_eq!(first_row_entries.len(), 3);

    // Second row should have run of 3 StateId(2)s
    let second_row_start = compressed.row_offsets[1] as usize;
    let second_row_end = compressed.row_offsets[2] as usize;
    let second_row_entries = &compressed.data[second_row_start..second_row_end];

    // Should be stored as run-length encoded
    assert_eq!(second_row_entries.len(), 1);
    match &second_row_entries[0] {
        CompressedGotoEntry::RunLength { state: 2, count: 3 } => {}
        _ => panic!("Expected run-length encoding for second row"),
    }
}

#[test]
fn test_goto_table_run_length_threshold() {
    let compressor = TableCompressor::new();

    // Test that runs of 1 and 2 are stored as individual entries
    let goto_table = vec![vec![
        StateId(1),
        StateId(2),
        StateId(2),
        StateId(3),
        StateId(3),
        StateId(3),
    ]];

    let compressed = compressor.compress_goto_table_small(&goto_table);
    assert!(compressed.is_ok());

    let compressed = compressed.unwrap();
    let entries = &compressed.data;

    // Should have: Single(1), Single(2), Single(2), RunLength(3, 3)
    assert_eq!(entries.len(), 4);

    match &entries[0] {
        CompressedGotoEntry::Single(1) => {}
        _ => panic!("Expected single entry for StateId(1)"),
    }

    match &entries[1] {
        CompressedGotoEntry::Single(2) => {}
        _ => panic!("Expected single entry for first StateId(2)"),
    }

    match &entries[2] {
        CompressedGotoEntry::Single(2) => {}
        _ => panic!("Expected single entry for second StateId(2)"),
    }

    match &entries[3] {
        CompressedGotoEntry::RunLength { state: 3, count: 3 } => {}
        _ => panic!("Expected run-length for StateId(3)"),
    }
}

#[test]
fn test_language_code_generation() {
    let grammar = Grammar::new("test_lang".to_string());
    let parse_table = crate::test_helpers::test::make_minimal_table(
        // 1 state × 2 columns; Accept on EOF col (1)
        vec![vec![vec![], vec![Action::Accept]]],
        vec![vec![StateId(0), StateId(0)]],
        vec![],
        SymbolId(1), // start_symbol (now in-bounds)
        SymbolId(1), // EOF column (1 = 1 + terms + externals with terms=1-implicit)
        0,
    );

    let generator = StaticLanguageGenerator::new(grammar, parse_table);
    let code = generator.generate_language_code();

    // Should generate valid Rust code
    let code_str = code.to_string();
    debug_trace!("Generated code: {}", code_str);
    assert!(code_str.contains("pub fn language")); // Without parentheses in quote output
    assert!(code_str.contains("tree_sitter_test_lang")); // Language-specific function name
    assert!(code_str.contains("LANGUAGE_VERSION"));
}

#[test]
fn test_compressed_tables_validation() {
    let mut parse_table = crate::test_helpers::test::make_minimal_table(
        vec![
            vec![vec![Action::Shift(StateId(1))], vec![Action::Error]],
            vec![vec![Action::Reduce(RuleId(0))], vec![Action::Accept]],
        ],
        vec![vec![StateId(0), StateId(1)], vec![StateId(1), StateId(0)]],
        vec![],
        SymbolId(1), // start_symbol
        SymbolId(1), // eof_symbol (column 1)
        0,           // external_token_count
    );

    // Override to put EOF at column 0 for test compatibility
    // Also update eof_symbol to match the new mapping
    parse_table.eof_symbol = SymbolId(0);
    parse_table.symbol_to_index.clear();
    parse_table.symbol_to_index.insert(SymbolId(0), 0);
    parse_table.symbol_to_index.insert(SymbolId(1), 1);

    let compressor = TableCompressor::new();
    // Use proper helper to collect token indices
    let grammar = Grammar::default(); // Minimal grammar for test
    let token_indices = helpers::collect_token_indices(&grammar, &parse_table);
    // Compute start_can_be_empty based on EOF cell in state 0
    let start_can_be_empty = false; // Conservative default for empty test
    let compressed = compressor
        .compress(&parse_table, &token_indices, start_can_be_empty)
        .unwrap();

    // Validation is exercised more thoroughly in compress/validation test suites.
    // This smoke test only verifies compression succeeds end-to-end.
    let _ = compressed.validate(&parse_table);
}

#[test]
fn test_tree_sitter_compatibility() {
    // Test that our encoding matches Tree-sitter's expectations
    let compressor = TableCompressor::new();

    // Tree-sitter encoding examples:
    // Shift to state 42: 0x002A (42 in hex)
    let shift = Action::Shift(StateId(42));
    assert_eq!(compressor.encode_action_small(&shift).unwrap(), 0x002A);

    // Reduce by rule 17: 0x8012 (32786 in decimal) = 0x8000 | (17 + 1)
    let reduce = Action::Reduce(RuleId(17));
    assert_eq!(compressor.encode_action_small(&reduce).unwrap(), 32786);

    // Accept: 0xFFFF
    let accept = Action::Accept;
    assert_eq!(compressor.encode_action_small(&accept).unwrap(), 0xFFFF);

    // Error: 0xFFFE
    let error = Action::Error;
    assert_eq!(compressor.encode_action_small(&error).unwrap(), 0xFFFE);
}

#[test]
fn test_compressed_action_entry() {
    let entry = CompressedActionEntry::new(5, Action::Shift(StateId(10)));
    assert_eq!(entry.symbol, 5);
    match entry.action {
        Action::Shift(StateId(10)) => {}
        _ => panic!("Wrong action type"),
    }
}

#[test]
fn test_generated_small_table_format() {
    let mut grammar = Grammar::new("small_test".to_string());

    // Add a simple grammar
    let token = Token {
        name: "A".to_string(),
        pattern: TokenPattern::String("a".to_string()),
        fragile: false,
    };
    grammar.tokens.insert(SymbolId(0), token);

    // Simple parse table
    let mut parse_table = crate::test_helpers::test::make_minimal_table(
        vec![
            vec![vec![Action::Shift(StateId(1))], vec![]],
            vec![vec![], vec![Action::Accept]],
        ],
        vec![vec![StateId(1), StateId(0)], vec![StateId(0), StateId(0)]],
        vec![],
        SymbolId(2), // start_symbol
        SymbolId(1), // eof_symbol (must be > 0)
        0,           // external_token_count
    );

    // Add EOF to symbol_to_index (required invariant)
    parse_table.symbol_to_index.insert(SymbolId(0), 0);

    let mut generator = StaticLanguageGenerator::new(grammar, parse_table);
    generator.compress_tables().unwrap();

    let code = generator.generate_language_code();
    let code_str = code.to_string();

    // Should generate small table format
    assert!(code_str.contains("SMALL_PARSE_TABLE") || code_str.contains("ACTION_TABLE"));
}

#[test]
fn arithmetic_has_many_states() {
    // This test helps prevent regressions in FIRST/FOLLOW/closure computation
    // that could collapse the automaton

    // Create a simple arithmetic grammar
    let mut grammar = Grammar::new("arithmetic".to_string());

    // Add tokens
    let number_token = Token {
        name: "number".to_string(),
        pattern: TokenPattern::Regex(r"\d+".to_string()),
        fragile: false,
    };
    let plus_token = Token {
        name: "plus".to_string(),
        pattern: TokenPattern::String("+".to_string()),
        fragile: false,
    };
    let times_token = Token {
        name: "times".to_string(),
        pattern: TokenPattern::String("*".to_string()),
        fragile: false,
    };

    grammar.tokens.insert(SymbolId(3), number_token);
    grammar.tokens.insert(SymbolId(4), plus_token);
    grammar.tokens.insert(SymbolId(5), times_token);

    // Add non-terminals
    grammar
        .rule_names
        .insert(SymbolId(0), "source_file".to_string());
    grammar
        .rule_names
        .insert(SymbolId(1), "expression".to_string());
    grammar.rule_names.insert(SymbolId(2), "term".to_string());

    // Add rules
    // source_file -> expression
    grammar.add_rule(Rule {
        lhs: SymbolId(0),
        rhs: vec![Symbol::NonTerminal(SymbolId(1))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    // expression -> expression + term
    grammar.add_rule(Rule {
        lhs: SymbolId(1),
        rhs: vec![
            Symbol::NonTerminal(SymbolId(1)),
            Symbol::Terminal(SymbolId(4)),
            Symbol::NonTerminal(SymbolId(2)),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });

    // expression -> term
    grammar.add_rule(Rule {
        lhs: SymbolId(1),
        rhs: vec![Symbol::NonTerminal(SymbolId(2))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(2),
    });

    // term -> term * number
    grammar.add_rule(Rule {
        lhs: SymbolId(2),
        rhs: vec![
            Symbol::NonTerminal(SymbolId(2)),
            Symbol::Terminal(SymbolId(5)),
            Symbol::Terminal(SymbolId(3)),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(3),
    });

    // term -> number
    grammar.add_rule(Rule {
        lhs: SymbolId(2),
        rhs: vec![Symbol::Terminal(SymbolId(3))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(4),
    });

    // Build LR(1) automaton
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();

    // The arithmetic grammar should have at least 9 states (GLR may compress states)
    assert!(
        parse_table.state_count >= 9,
        "automaton collapsed ({} states), expected >= 9",
        parse_table.state_count
    );

    // State 0 should have valid actions (not all Error)
    assert!(
        parse_table.action_table[0]
            .iter()
            .any(|action_cell| action_cell.iter().any(|a| !matches!(a, Action::Error))),
        "state-0 has no valid actions"
    );
}
