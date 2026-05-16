
use super::*;
use adze_glr_core::LexMode;
use adze_ir::*;

fn token_stream_u16(token: &TokenStream) -> u16 {
    token.to_string().trim_end_matches("u16").parse().unwrap()
}

#[test]
fn test_deterministic_symbol_ordering() {
    let mut grammar = Grammar::new("test".to_string());

    // Add tokens in non-sorted order
    grammar.tokens.insert(
        SymbolId(5),
        Token {
            name: "token5".to_string(),
            pattern: TokenPattern::String("5".to_string()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "token1".to_string(),
            pattern: TokenPattern::String("1".to_string()),
            fragile: false,
        },
    );

    let mut symbol_to_index = std::collections::BTreeMap::new();
    symbol_to_index.insert(SymbolId(0), 0); // EOF
    symbol_to_index.insert(SymbolId(1), 1); // token1
    symbol_to_index.insert(SymbolId(5), 2); // token5

    // Create a minimal parse table for testing
    let mut parse_table = crate::empty_table!(states: 1, terms: 2, nonterms: 0);

    // Override the symbol mapping for the test
    parse_table.symbol_to_index = symbol_to_index;
    parse_table.symbol_count = 3;

    let builder = AbiLanguageBuilder::new(&grammar, &parse_table);
    let (names, _) = builder.generate_symbol_names();

    // Should have EOF + 2 tokens
    assert_eq!(names.len(), 3);

    // Check that tokens are sorted by ID
    let code = quote! { #(#names)* }.to_string();

    // The token names are encoded as u8 byte arrays
    // "token1" = [116u8, 111u8, 107u8, 101u8, 110u8, 49u8, 0u8]
    // "token5" = [116u8, 111u8, 107u8, 101u8, 110u8, 53u8, 0u8]
    // We check for the distinguishing bytes: 49u8 for '1' and 53u8 for '5'
    assert!(code.contains("49u8")); // '1' in token1
    assert!(code.contains("53u8")); // '5' in token5
    let token1_pos = code.find("49u8").unwrap();
    let token5_pos = code.find("53u8").unwrap();
    assert!(token1_pos < token5_pos);
}

#[test]
fn test_generate_production_id_map_includes_first_slot() {
    let mut grammar = Grammar::new("test".to_string());

    let start = SymbolId(1);
    let t = SymbolId(2);
    grammar.rule_names.insert(start, "start".to_string());
    grammar.tokens.insert(
        t,
        Token {
            name: "t".to_string(),
            pattern: TokenPattern::String("t".to_string()),
            fragile: false,
        },
    );

    grammar.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::Terminal(t)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    grammar.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::Terminal(t)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });
    grammar.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::Terminal(t)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(2),
    });

    let parse_table = crate::empty_table!(states: 1, terms: 1, nonterms: 1);
    let builder = AbiLanguageBuilder::new(&grammar, &parse_table);
    let production_map = builder.generate_production_id_map();

    assert_eq!(production_map.len(), 3);
    assert_eq!(production_map[0].to_string(), "0u16");
    assert_eq!(production_map[1].to_string(), "1u16");
    assert_eq!(production_map[2].to_string(), "2u16");
}

#[test]
fn test_generate_lex_modes_uses_parse_table_modes() {
    let grammar = Grammar::new("lex_modes".to_string());
    let mut parse_table = crate::empty_table!(states: 3, terms: 1, nonterms: 1, externals: 1);
    parse_table.lex_modes = vec![
        LexMode {
            lex_state: 4,
            external_lex_state: 0,
        },
        LexMode {
            lex_state: 7,
            external_lex_state: 2,
        },
        LexMode {
            lex_state: 4,
            external_lex_state: 9,
        },
    ];

    let builder = AbiLanguageBuilder::new(&grammar, &parse_table);
    let modes = builder.generate_lex_modes();

    assert!(modes[0].to_string().contains("lex_state : 4u16"));
    assert!(modes[1].to_string().contains("lex_state : 7u16"));
    assert!(modes[1].to_string().contains("external_lex_state : 2u16"));
    assert!(modes[2].to_string().contains("external_lex_state : 9u16"));
}

// --- ABI compatibility tests (correctness-tablegen-compat) ---

/// Single-production grammar yields a map of length 1.
#[test]
fn test_production_id_map_single_production() {
    let mut grammar = Grammar::new("single".to_string());
    let start = SymbolId(1);
    let t = SymbolId(2);
    grammar.rule_names.insert(start, "start".to_string());
    grammar.tokens.insert(
        t,
        Token {
            name: "t".to_string(),
            pattern: TokenPattern::String("t".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::Terminal(t)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let table = crate::empty_table!(states: 1, terms: 1, nonterms: 1);
    let builder = AbiLanguageBuilder::new(&grammar, &table);
    let map = builder.generate_production_id_map();

    assert_eq!(map.len(), 1, "single production → map length 1");
    assert_eq!(map[0].to_string(), "0u16");
}

/// EOF symbol metadata must be visible=true, named=false (Tree-sitter convention).
#[test]
fn test_eof_metadata_visible_unnamed() {
    let mut grammar = Grammar::new("eof_meta".to_string());
    let start = SymbolId(1);
    let t = SymbolId(2);
    grammar.rule_names.insert(start, "start".to_string());
    grammar.tokens.insert(
        t,
        Token {
            name: "tok".to_string(),
            pattern: TokenPattern::String("t".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::Terminal(t)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let table = crate::empty_table!(states: 1, terms: 1, nonterms: 1);
    let builder = AbiLanguageBuilder::new(&grammar, &table);
    let metadata = builder.generate_symbol_metadata();

    assert_eq!(
        metadata.len(),
        table.symbol_count,
        "metadata length must equal symbol_count"
    );

    // EOF metadata: visible=true(0x01), named=false → 0x01
    let eof_idx = table.symbol_to_index[&table.eof_symbol];
    assert_eq!(
        metadata[eof_idx].to_string(),
        "1u8",
        "EOF metadata must be 0x01 (visible, not named)"
    );
}

/// Metadata length matches parse table symbol_count exactly.
#[test]
fn test_symbol_metadata_length_matches_symbol_count() {
    let mut grammar = Grammar::new("meta_len".to_string());
    let start = SymbolId(1);
    let t1 = SymbolId(2);
    let t2 = SymbolId(3);
    grammar.rule_names.insert(start, "start".to_string());
    for (id, name) in [(t1, "a"), (t2, "b")] {
        grammar.tokens.insert(
            id,
            Token {
                name: name.to_string(),
                pattern: TokenPattern::String(name.to_string()),
                fragile: false,
            },
        );
    }
    grammar.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::Terminal(t1), Symbol::Terminal(t2)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let table = crate::empty_table!(states: 2, terms: 2, nonterms: 1);
    let builder = AbiLanguageBuilder::new(&grammar, &table);
    let metadata = builder.generate_symbol_metadata();

    assert_eq!(metadata.len(), table.symbol_count);
}

/// calculate_counts must reflect parse table dimensions.
#[test]
fn test_calculate_counts_matches_table_dimensions() {
    let mut grammar = Grammar::new("counts".to_string());
    let start = SymbolId(1);
    let t = SymbolId(2);
    grammar.rule_names.insert(start, "start".to_string());
    grammar.tokens.insert(
        t,
        Token {
            name: "t".to_string(),
            pattern: TokenPattern::String("t".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::Terminal(t)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    grammar.fields.insert(FieldId(0), "val".to_string());

    let table = crate::empty_table!(states: 5, terms: 1, nonterms: 1);
    let builder = AbiLanguageBuilder::new(&grammar, &table);
    let counts = builder.calculate_counts();

    assert_eq!(counts.symbol_count as usize, table.symbol_count);
    assert_eq!(counts.state_count as usize, table.state_count);
    assert_eq!(counts.token_count as usize, table.token_count);
    assert_eq!(counts.field_count, 1);
    assert_eq!(
        counts.external_token_count as usize,
        table.external_token_count
    );
}

/// generate() produces code with the correct ABI version.
#[test]
fn test_generate_contains_abi_version_15() {
    let table = crate::empty_table!(states: 1, terms: 1, nonterms: 1);
    // Use the table's start_symbol as the rule LHS to match non-terminal region.
    let start = table.start_symbol;
    let t = SymbolId(1); // terminal column

    let mut grammar = Grammar::new("ver".to_string());
    grammar.rule_names.insert(start, "start".to_string());
    grammar.tokens.insert(
        t,
        Token {
            name: "t".to_string(),
            pattern: TokenPattern::String("t".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::Terminal(t)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let builder = AbiLanguageBuilder::new(&grammar, &table);
    let code = builder.generate().to_string();

    assert!(
        code.contains("TREE_SITTER_LANGUAGE_VERSION"),
        "generated code must reference TREE_SITTER_LANGUAGE_VERSION"
    );
}

/// Encode/decode roundtrip for Shift, Reduce, Accept, Error through
/// the AbiLanguageBuilder's encode_action method.
#[test]
fn test_encode_action_roundtrip() {
    let grammar = Grammar::new("enc".to_string());
    let table = crate::empty_table!(states: 1, terms: 0, nonterms: 0);
    let builder = AbiLanguageBuilder::new(&grammar, &table);

    // Shift
    let enc = builder
        .encode_action(&Action::Shift(adze_ir::StateId(42)))
        .unwrap();
    assert_eq!(enc, 42, "Shift(42) → 42");

    // Reduce (1-based in Tree-sitter)
    let enc = builder.encode_action(&Action::Reduce(RuleId(3))).unwrap();
    assert_eq!(enc, 0x8000 | 0x0004, "Reduce(3) -> 0x8004");

    // Accept
    let enc = builder.encode_action(&Action::Accept).unwrap();
    assert_eq!(enc, 0xFFFF, "Accept → 0xFFFF");

    // Error
    let enc = builder.encode_action(&Action::Error).unwrap();
    assert_eq!(enc, 0, "Error → 0");
}

#[test]
fn test_fallback_parse_table_preserves_multi_action_cell() {
    let mut table = crate::empty_table!(states: 2, terms: 1, nonterms: 1);
    let start = table.start_symbol;
    let t = SymbolId(1);

    for row in &mut table.goto_table {
        row.fill(StateId(0));
    }
    table.action_table[0][1] = vec![
        Action::Error,
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(0)),
    ];

    let mut grammar = Grammar::new("fallback_conflict".to_string());
    grammar.rule_names.insert(start, "start".to_string());
    grammar.tokens.insert(
        t,
        Token {
            name: "t".to_string(),
            pattern: TokenPattern::String("t".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::Terminal(t)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let builder = AbiLanguageBuilder::new(&grammar, &table);
    let (table_data, table_map) = builder.generate_parse_tables();
    let values: Vec<u16> = table_data.iter().map(token_stream_u16).collect();
    let offsets: Vec<u32> = table_map
        .iter()
        .map(|token| token.to_string().trim_end_matches("u32").parse().unwrap())
        .collect();

    assert_eq!(offsets[0], 0, "state 0 starts at the first pair");
    assert_eq!(offsets[1], 4, "state 1 starts after both state 0 pairs");
    assert_eq!(values.len(), 4, "state 0 must emit two direct pairs");
    assert_eq!(values[0], 1, "first entry symbol");
    assert_eq!(
        values[1],
        builder.encode_action(&Action::Shift(StateId(1))).unwrap()
    );
    assert_eq!(values[2], 1, "second entry symbol");
    assert_eq!(
        values[3],
        builder.encode_action(&Action::Reduce(RuleId(0))).unwrap()
    );
}

#[test]
fn test_fallback_parse_table_emits_goto_once() {
    let mut table = crate::empty_table!(states: 2, terms: 1, nonterms: 1);
    let start = table.start_symbol;
    let t = SymbolId(1);

    for row in &mut table.goto_table {
        row.fill(StateId(0));
    }
    table.goto_table[0][start.0 as usize] = StateId(1);

    let mut grammar = Grammar::new("fallback_goto".to_string());
    grammar.rule_names.insert(start, "start".to_string());
    grammar.tokens.insert(
        t,
        Token {
            name: "t".to_string(),
            pattern: TokenPattern::String("t".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::Terminal(t)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let builder = AbiLanguageBuilder::new(&grammar, &table);
    let (table_data, table_map) = builder.generate_parse_tables();
    let values: Vec<u16> = table_data.iter().map(token_stream_u16).collect();
    let offsets: Vec<u32> = table_map
        .iter()
        .map(|token| token.to_string().trim_end_matches("u32").parse().unwrap())
        .collect();

    assert_eq!(offsets, vec![0, 2, 2]);
    assert_eq!(
        values,
        vec![start.0, 1],
        "state 0 should contain exactly one direct goto pair"
    );
}

/// Production LHS index entries must all reference non-terminal columns.
#[test]
fn test_production_lhs_index_nonterminal_columns() {
    let table = crate::empty_table!(states: 2, terms: 1, nonterms: 1);
    let start = table.start_symbol;
    let t = SymbolId(1); // terminal column

    let mut grammar = Grammar::new("lhs".to_string());
    grammar.rule_names.insert(start, "start".to_string());
    grammar.tokens.insert(
        t,
        Token {
            name: "t".to_string(),
            pattern: TokenPattern::String("t".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::Terminal(t)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let table = crate::empty_table!(states: 2, terms: 1, nonterms: 1);
    let builder = AbiLanguageBuilder::new(&grammar, &table);
    let lhs_index = builder.generate_production_lhs_index();

    // Every LHS must be ≥ token_count (non-terminal region)
    for (i, token) in lhs_index.iter().enumerate() {
        let val: u16 = token.to_string().trim_end_matches("u16").parse().unwrap();
        assert!(
            val as usize >= table.token_count,
            "production_lhs_index[{}] = {} must be >= token_count {}",
            i,
            val,
            table.token_count
        );
    }
}

#[test]
fn test_field_map_slices_are_dense_and_include_production_zero() {
    let table = crate::empty_table!(states: 1, terms: 1, nonterms: 1);
    let start = table.start_symbol;
    let t = SymbolId(1);

    let mut grammar = Grammar::new("field_maps".to_string());
    grammar.rule_names.insert(start, "start".to_string());
    grammar.tokens.insert(
        t,
        Token {
            name: "t".to_string(),
            pattern: TokenPattern::String("t".to_string()),
            fragile: false,
        },
    );
    grammar.fields.insert(FieldId(0), "first".to_string());
    grammar.fields.insert(FieldId(1), "third".to_string());
    grammar.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::Terminal(t)],
        precedence: None,
        associativity: None,
        fields: vec![(FieldId(0), 0)],
        production_id: ProductionId(0),
    });
    grammar.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::Terminal(t)],
        precedence: None,
        associativity: None,
        fields: vec![(FieldId(1), 0)],
        production_id: ProductionId(2),
    });

    let builder = AbiLanguageBuilder::new(&grammar, &table);
    let (slices, entries) = builder.generate_field_maps();
    let slices: Vec<String> = slices.iter().map(ToString::to_string).collect();

    assert_eq!(
        slices.len(),
        6,
        "field_map_slices must have two words per production ID"
    );
    assert_eq!(slices[0], "0u16", "production 0 start");
    assert_eq!(slices[1], "1u16", "production 0 length");
    assert_eq!(slices[2], "0u16", "production 1 gap start");
    assert_eq!(slices[3], "0u16", "production 1 gap length");
    assert_eq!(
        slices[4], "1u16",
        "production 2 start is entry index, not word offset"
    );
    assert_eq!(slices[5], "1u16", "production 2 length");
    assert_eq!(
        entries.len(),
        4,
        "two field-map entries should emit two u16 words each"
    );
}

#[test]
fn test_field_map_entries_use_abi_field_name_indices() {
    let table = crate::empty_table!(states: 1, terms: 1, nonterms: 1);
    let start = table.start_symbol;
    let t = SymbolId(1);

    let mut grammar = Grammar::new("field_map_name_indices".to_string());
    grammar.rule_names.insert(start, "start".to_string());
    grammar.tokens.insert(
        t,
        Token {
            name: "t".to_string(),
            pattern: TokenPattern::String("t".to_string()),
            fragile: false,
        },
    );
    grammar
        .fields
        .insert(FieldId(0), "TestModule_statements_vec_element".to_string());
    grammar.fields.insert(FieldId(1), "value".to_string());
    grammar.fields.insert(FieldId(2), "statements".to_string());
    grammar.fields.insert(FieldId(3), "_whitespace".to_string());
    grammar.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::Terminal(t)],
        precedence: None,
        associativity: None,
        fields: vec![(FieldId(1), 0)],
        production_id: ProductionId(0),
    });

    let builder = AbiLanguageBuilder::new(&grammar, &table);
    let (_, entries) = builder.generate_field_maps();
    let entries: Vec<String> = entries.iter().map(ToString::to_string).collect();

    assert_eq!(
        entries[0], "3u32 as u16",
        "field map entries must use the FIELD_NAME_PTRS ABI index for value"
    );
}

#[test]
fn test_empty_field_maps_keep_dense_slices_and_non_null_entry_placeholder() {
    let table = crate::empty_table!(states: 1, terms: 1, nonterms: 1);
    let start = table.start_symbol;
    let t = SymbolId(1);

    let mut grammar = Grammar::new("empty_field_maps".to_string());
    grammar.rule_names.insert(start, "start".to_string());
    grammar.tokens.insert(
        t,
        Token {
            name: "t".to_string(),
            pattern: TokenPattern::String("t".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::Terminal(t)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let builder = AbiLanguageBuilder::new(&grammar, &table);
    let (slices, entries) = builder.generate_field_maps();
    let slices: Vec<String> = slices.iter().map(ToString::to_string).collect();
    let entries: Vec<String> = entries.iter().map(ToString::to_string).collect();

    assert_eq!(slices, vec!["0u16", "0u16"]);
    assert_eq!(entries, vec!["0u16"]);
}
