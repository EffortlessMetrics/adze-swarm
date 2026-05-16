#![cfg_attr(feature = "strict_docs", allow(missing_docs))]
//! Serialization of parse tables and language structures for testing.

// Table serialization for testing and debugging
// This module allows us to serialize parse tables and language structures for comparison

use crate::abi::*;
use crate::compress::CompressedTables;
use adze_glr_core::ParseTable;
use adze_ir::Grammar;
use serde::{Deserialize, Serialize};

/// Serializable representation of a Language for testing
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SerializableLanguage {
    pub version: u32,
    pub symbol_count: u32,
    pub alias_count: u32,
    pub token_count: u32,
    pub external_token_count: u32,
    pub state_count: u32,
    pub large_state_count: u32,
    pub production_id_count: u32,
    pub field_count: u32,
    pub symbol_names: Vec<String>,
    pub field_names: Vec<String>,
    pub symbol_metadata: Vec<u8>,
    pub parse_table: Vec<u16>,
    pub small_parse_table_map: Vec<u32>,
    pub lex_modes: Vec<SerializableLexState>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SerializableLexState {
    pub lex_state: u16,
    pub external_lex_state: u16,
}

/// Serialize a grammar and parse table to JSON
pub fn serialize_language(
    grammar: &Grammar,
    parse_table: &ParseTable,
    compressed: Option<&CompressedTables>,
) -> Result<String, serde_json::Error> {
    let language = build_serializable_language(grammar, parse_table, compressed);
    serde_json::to_string_pretty(&language)
}

fn build_serializable_language(
    grammar: &Grammar,
    parse_table: &ParseTable,
    compressed: Option<&CompressedTables>,
) -> SerializableLanguage {
    // Generate symbol names with deterministic ordering
    let symbol_names = generate_symbol_names(grammar);
    let field_names = generate_field_names(grammar);
    let symbol_metadata = generate_symbol_metadata(grammar);
    let (parse_table_data, small_table_map) = generate_parse_table_data(compressed);
    let lex_modes = generate_lex_modes(parse_table);

    SerializableLanguage {
        version: TREE_SITTER_LANGUAGE_VERSION,
        symbol_count: calculate_symbol_count(grammar) as u32,
        alias_count: 0,
        token_count: grammar.tokens.len() as u32,
        external_token_count: grammar.externals.len() as u32,
        state_count: parse_table.state_count as u32,
        large_state_count: 0,
        production_id_count: calculate_production_count(grammar) as u32,
        field_count: grammar.fields.len() as u32,
        symbol_names,
        field_names,
        symbol_metadata,
        parse_table: parse_table_data,
        small_parse_table_map: small_table_map,
        lex_modes,
    }
}

fn generate_symbol_names(grammar: &Grammar) -> Vec<String> {
    let mut names = vec!["end".to_string()]; // EOF

    // Sort tokens by ID
    let mut tokens: Vec<_> = grammar.tokens.iter().collect();
    tokens.sort_by_key(|(id, _)| id.0);
    for (_, token) in tokens {
        names.push(token.name.clone());
    }

    // Sort non-terminals by ID
    let mut rules: Vec<_> = grammar.rules.iter().collect();
    rules.sort_by_key(|(id, _)| id.0);
    for (id, _) in rules {
        let name = grammar
            .rule_names
            .get(id)
            .cloned()
            .unwrap_or_else(|| format!("rule_{}", id.0));
        names.push(name);
    }

    // Add externals
    for external in &grammar.externals {
        names.push(external.name.clone());
    }

    names
}

fn generate_field_names(grammar: &Grammar) -> Vec<String> {
    // Fields must be in lexicographic order
    let mut fields: Vec<_> = grammar.fields.iter().collect();
    fields.sort_by_key(|(_, name)| name.as_str());
    fields.into_iter().map(|(_, name)| name.clone()).collect()
}

fn generate_symbol_metadata(grammar: &Grammar) -> Vec<u8> {
    let mut metadata = Vec::new();

    // EOF
    metadata.push(create_symbol_metadata(true, false, false, false, false));

    // Tokens
    let mut tokens: Vec<_> = grammar.tokens.iter().collect();
    tokens.sort_by_key(|(id, _)| id.0);
    for (_, token) in tokens {
        let visible = !token.name.starts_with('_');
        let named = visible && matches!(&token.pattern, adze_ir::TokenPattern::Regex(_));
        metadata.push(create_symbol_metadata(visible, named, false, false, false));
    }

    // Non-terminals
    let mut rules: Vec<_> = grammar.rules.iter().collect();
    rules.sort_by_key(|(id, _)| id.0);
    for (id, _) in rules {
        let name = grammar
            .rule_names
            .get(id)
            .cloned()
            .unwrap_or_else(|| format!("rule_{}", id.0));
        let visible = !name.starts_with('_');
        let named = visible;
        let supertype = grammar.supertypes.contains(id);
        metadata.push(create_symbol_metadata(
            visible, named, false, false, supertype,
        ));
    }

    // Externals
    for external in &grammar.externals {
        let visible = !external.name.starts_with('_');
        let named = visible;
        metadata.push(create_symbol_metadata(visible, named, false, false, false));
    }

    metadata
}

fn generate_parse_table_data(compressed: Option<&CompressedTables>) -> (Vec<u16>, Vec<u32>) {
    if let Some(compressed) = compressed {
        let mut table_data = Vec::new();
        let mut map_data = Vec::new();

        // Simplified: just collect basic data
        for entry in &compressed.action_table.data {
            table_data.push(entry.symbol);
            // Encode action based on Tree-sitter format
            match &entry.action {
                adze_glr_core::Action::Shift(state) => table_data.push(state.0),
                adze_glr_core::Action::Reduce(rule) => {
                    // Tree-sitter uses 1-based production IDs
                    table_data.push(0x8000 | (rule.0 + 1))
                }
                adze_glr_core::Action::Accept => table_data.push(0xFFFF),
                adze_glr_core::Action::Error => table_data.push(0xFFFE),
                adze_glr_core::Action::Recover => table_data.push(0xFFFD),
                adze_glr_core::Action::Fork(_) => table_data.push(0xFFFE),
                _ => table_data.push(0xFFFE), // Unknown action type // Expected: V for Recover
            }
        }

        for &offset in &compressed.action_table.row_offsets {
            map_data.push(offset as u32);
        }

        (table_data, map_data)
    } else {
        (vec![], vec![])
    }
}

fn generate_lex_modes(parse_table: &ParseTable) -> Vec<SerializableLexState> {
    (0..parse_table.state_count)
        .map(|state_index| {
            let mode =
                parse_table
                    .lex_modes
                    .get(state_index)
                    .copied()
                    .unwrap_or(adze_glr_core::LexMode {
                        lex_state: 0,
                        external_lex_state: 0,
                    });
            SerializableLexState {
                lex_state: mode.lex_state,
                external_lex_state: mode.external_lex_state,
            }
        })
        .collect()
}

fn calculate_symbol_count(grammar: &Grammar) -> usize {
    1 + // EOF
    grammar.tokens.len() +
    grammar.rules.len() +
    grammar.externals.len()
}

fn calculate_production_count(grammar: &Grammar) -> usize {
    grammar
        .rules
        .values()
        .flat_map(|rules| rules.iter())
        .count()
}

/// Serialize compressed tables for comparison
pub fn serialize_compressed_tables(tables: &CompressedTables) -> Result<String, serde_json::Error> {
    #[derive(Serialize)]
    struct SerializableTables {
        action_table: SerializableActionTable,
        goto_table: SerializableGotoTable,
        small_table_threshold: usize,
    }

    #[derive(Serialize)]
    struct SerializableActionTable {
        entries: Vec<(u16, String)>, // (symbol, action description)
        row_offsets: Vec<u16>,
        default_actions: Vec<String>,
    }

    #[derive(Serialize)]
    struct SerializableGotoTable {
        entries: Vec<String>, // String representation of entries
        row_offsets: Vec<u16>,
    }

    let action_entries: Vec<_> = tables
        .action_table
        .data
        .iter()
        .map(|entry| {
            let action_str = match &entry.action {
                adze_glr_core::Action::Shift(s) => format!("Shift({})", s.0),
                adze_glr_core::Action::Reduce(r) => format!("Reduce({})", r.0),
                adze_glr_core::Action::Accept => "Accept".to_string(),
                adze_glr_core::Action::Error => "Error".to_string(),
                adze_glr_core::Action::Recover => "Recover".to_string(),
                adze_glr_core::Action::Fork(actions) => format!("Fork({})", actions.len()),
                _ => "Unknown".to_string(),
            };
            (entry.symbol, action_str)
        })
        .collect();

    let default_actions: Vec<_> = tables
        .action_table
        .default_actions
        .iter()
        .map(|action| match action {
            adze_glr_core::Action::Shift(s) => format!("Shift({})", s.0),
            adze_glr_core::Action::Reduce(r) => format!("Reduce({})", r.0),
            adze_glr_core::Action::Accept => "Accept".to_string(),
            adze_glr_core::Action::Error => "Error".to_string(),
            adze_glr_core::Action::Recover => "Recover".to_string(),
            adze_glr_core::Action::Fork(actions) => format!("Fork({})", actions.len()),
            _ => "Unknown".to_string(),
        })
        .collect();

    let goto_entries: Vec<_> = tables
        .goto_table
        .data
        .iter()
        .map(|entry| match entry {
            crate::compress::CompressedGotoEntry::Single(s) => format!("Single({})", s),
            crate::compress::CompressedGotoEntry::RunLength { state, count } => {
                format!("RunLength({}, {})", state, count)
            }
        })
        .collect();

    let serializable = SerializableTables {
        action_table: SerializableActionTable {
            entries: action_entries,
            row_offsets: tables.action_table.row_offsets.clone(),
            default_actions,
        },
        goto_table: SerializableGotoTable {
            entries: goto_entries,
            row_offsets: tables.goto_table.row_offsets.clone(),
        },
        small_table_threshold: tables.small_table_threshold,
    };

    serde_json::to_string_pretty(&serializable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::*;

    #[test]
    fn test_deterministic_serialization() {
        let mut grammar = Grammar::new("test".to_string());

        // Add tokens in random order
        grammar.tokens.insert(
            SymbolId(3),
            Token {
                name: "c".to_string(),
                pattern: TokenPattern::String("c".to_string()),
                fragile: false,
            },
        );
        grammar.tokens.insert(
            SymbolId(1),
            Token {
                name: "a".to_string(),
                pattern: TokenPattern::String("a".to_string()),
                fragile: false,
            },
        );
        grammar.tokens.insert(
            SymbolId(2),
            Token {
                name: "b".to_string(),
                pattern: TokenPattern::String("b".to_string()),
                fragile: false,
            },
        );

        let parse_table = crate::empty_table!(states: 1, terms: 3, nonterms: 0);

        let language = build_serializable_language(&grammar, &parse_table, None);

        // Check that symbols are sorted by ID
        assert_eq!(language.symbol_names[0], "end");
        assert_eq!(language.symbol_names[1], "a");
        assert_eq!(language.symbol_names[2], "b");
        assert_eq!(language.symbol_names[3], "c");
    }

    #[test]
    fn test_field_ordering() {
        let mut grammar = Grammar::new("test".to_string());

        // Add fields in random order
        grammar.fields.insert(FieldId(0), "zebra".to_string());
        grammar.fields.insert(FieldId(1), "apple".to_string());
        grammar.fields.insert(FieldId(2), "mango".to_string());

        let parse_table = crate::empty_table!(states: 1, terms: 0, nonterms: 0);

        let language = build_serializable_language(&grammar, &parse_table, None);

        // Check that fields are sorted lexicographically
        assert_eq!(language.field_names[0], "apple");
        assert_eq!(language.field_names[1], "mango");
        assert_eq!(language.field_names[2], "zebra");
    }

    // ---- Coverage additions for previously-untested branches ----

    use crate::compress::{
        CompressedActionEntry, CompressedActionTable, CompressedGotoEntry, CompressedGotoTable,
        CompressedTables,
    };
    use adze_glr_core::{Action, LexMode, RuleId, StateId};

    fn tiny_compressed_with_all_actions() -> CompressedTables {
        // Symbols and a representative action per encodable variant
        let data = vec![
            CompressedActionEntry::new(1, Action::Shift(StateId(7))),
            CompressedActionEntry::new(2, Action::Reduce(RuleId(3))),
            CompressedActionEntry::new(3, Action::Accept),
            CompressedActionEntry::new(4, Action::Error),
            CompressedActionEntry::new(5, Action::Recover),
            CompressedActionEntry::new(
                6,
                Action::Fork(vec![Action::Shift(StateId(1)), Action::Accept]),
            ),
        ];
        CompressedTables {
            action_table: CompressedActionTable {
                data,
                row_offsets: vec![0, 6],
                default_actions: vec![
                    Action::Shift(StateId(7)),
                    Action::Reduce(RuleId(3)),
                    Action::Accept,
                    Action::Error,
                    Action::Recover,
                    Action::Fork(vec![Action::Accept]),
                ],
            },
            goto_table: CompressedGotoTable {
                data: vec![
                    CompressedGotoEntry::Single(11),
                    CompressedGotoEntry::RunLength {
                        state: 22,
                        count: 4,
                    },
                ],
                row_offsets: vec![0, 2],
            },
            small_table_threshold: 32768,
        }
    }

    #[test]
    fn serialize_language_returns_parseable_json() {
        let grammar = Grammar::new("g".to_string());
        let parse_table = crate::empty_table!(states: 1, terms: 0, nonterms: 0);
        let json = serialize_language(&grammar, &parse_table, None).unwrap();
        // Round-trip into our serializable container.
        let parsed: SerializableLanguage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, TREE_SITTER_LANGUAGE_VERSION);
        assert_eq!(parsed.alias_count, 0);
        // symbol_count = 1 (EOF) + 0 tokens + 0 rules + 0 externals.
        assert_eq!(parsed.symbol_count, 1);
        assert_eq!(parsed.symbol_names, vec!["end".to_string()]);
        assert_eq!(parsed.parse_table, Vec::<u16>::new());
        assert_eq!(parsed.small_parse_table_map, Vec::<u32>::new());
        // One lex mode per state.
        assert_eq!(parsed.lex_modes.len(), 1);
        assert_eq!(parsed.lex_modes[0].lex_state, 0);
        assert_eq!(parsed.lex_modes[0].external_lex_state, 0);
    }

    #[test]
    fn build_serializable_language_counts_externals_fields_and_productions() {
        let mut grammar = Grammar::new("test".to_string());
        // 2 tokens (one underscore-prefixed -> hidden), 2 rules (one with two productions),
        // 1 external, 1 supertype-marked rule.
        grammar.tokens.insert(
            SymbolId(1),
            Token {
                name: "_internal".to_string(),
                pattern: TokenPattern::String("x".to_string()),
                fragile: false,
            },
        );
        grammar.tokens.insert(
            SymbolId(2),
            Token {
                name: "ident".to_string(),
                pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
                fragile: false,
            },
        );

        // Rule with two productions for the same LHS.
        grammar.add_rule(Rule {
            lhs: SymbolId(10),
            rhs: vec![Symbol::Terminal(SymbolId(2))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });
        grammar.add_rule(Rule {
            lhs: SymbolId(10),
            rhs: vec![],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(1),
        });
        // Hidden internal rule and a supertype rule.
        grammar.add_rule(Rule {
            lhs: SymbolId(11),
            rhs: vec![],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(2),
        });
        grammar.rule_names.insert(SymbolId(10), "expr".to_string());
        grammar
            .rule_names
            .insert(SymbolId(11), "_hidden_rule".to_string());
        grammar.supertypes.push(SymbolId(10));

        grammar.externals.push(ExternalToken {
            name: "ext".to_string(),
            symbol_id: SymbolId(100),
        });
        grammar.fields.insert(FieldId(0), "name".to_string());

        let parse_table = crate::empty_table!(states: 2, terms: 2, nonterms: 2);

        let lang = build_serializable_language(&grammar, &parse_table, None);

        // EOF + 2 tokens + 2 rule LHS + 1 external.
        assert_eq!(lang.symbol_count, 1 + 2 + 2 + 1);
        assert_eq!(lang.token_count, 2);
        assert_eq!(lang.external_token_count, 1);
        // Two rules with three total productions across them.
        assert_eq!(lang.production_id_count, 3);
        assert_eq!(lang.field_count, 1);
        // State count comes from parse_table.
        assert_eq!(lang.state_count, parse_table.state_count as u32);

        // symbol_names: end, tokens by id, rules by id, externals.
        assert_eq!(lang.symbol_names[0], "end");
        assert_eq!(lang.symbol_names[1], "_internal");
        assert_eq!(lang.symbol_names[2], "ident");
        assert_eq!(lang.symbol_names[3], "expr");
        assert_eq!(lang.symbol_names[4], "_hidden_rule");
        assert_eq!(lang.symbol_names[5], "ext");

        // symbol_metadata length matches symbol_count (EOF + tokens + rules + externals).
        assert_eq!(lang.symbol_metadata.len() as u32, lang.symbol_count);
    }

    #[test]
    fn build_serializable_language_synthesizes_missing_rule_names() {
        // Add a rule with no entry in rule_names so the `rule_X` fallback fires.
        let mut grammar = Grammar::new("g".to_string());
        grammar.add_rule(Rule {
            lhs: SymbolId(42),
            rhs: vec![],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

        let parse_table = crate::empty_table!(states: 1, terms: 0, nonterms: 1);
        let lang = build_serializable_language(&grammar, &parse_table, None);

        assert!(
            lang.symbol_names.iter().any(|n| n == "rule_42"),
            "expected synthesized rule_42 name in {:?}",
            lang.symbol_names,
        );
    }

    #[test]
    fn build_serializable_language_encodes_actions_from_compressed() {
        let grammar = Grammar::new("g".to_string());
        let parse_table = crate::empty_table!(states: 1, terms: 0, nonterms: 0);
        let compressed = tiny_compressed_with_all_actions();

        let lang = build_serializable_language(&grammar, &parse_table, Some(&compressed));

        // The serializer emits (symbol, encoded_action) pairs per entry.
        // 6 entries -> 12 u16 values.
        assert_eq!(lang.parse_table.len(), 12);
        // Shift(7) -> raw 7.
        assert_eq!(lang.parse_table[0], 1);
        assert_eq!(lang.parse_table[1], 7);
        // Reduce(RuleId(3)) -> 0x8000 | (3 + 1) = 0x8004.
        assert_eq!(lang.parse_table[2], 2);
        assert_eq!(lang.parse_table[3], 0x8004);
        // Accept -> 0xFFFF.
        assert_eq!(lang.parse_table[4], 3);
        assert_eq!(lang.parse_table[5], 0xFFFF);
        // Error -> 0xFFFE.
        assert_eq!(lang.parse_table[6], 4);
        assert_eq!(lang.parse_table[7], 0xFFFE);
        // Recover -> 0xFFFD.
        assert_eq!(lang.parse_table[8], 5);
        assert_eq!(lang.parse_table[9], 0xFFFD);
        // Fork -> 0xFFFE (treated as error sentinel by this simplified encoder).
        assert_eq!(lang.parse_table[10], 6);
        assert_eq!(lang.parse_table[11], 0xFFFE);

        // small_parse_table_map mirrors row_offsets widened to u32.
        assert_eq!(lang.small_parse_table_map, vec![0u32, 6u32]);
    }

    #[test]
    fn generate_lex_modes_pads_with_defaults_when_missing() {
        // ParseTable with state_count=3 but only one lex mode populated.
        let mut parse_table = crate::empty_table!(states: 3, terms: 0, nonterms: 0);
        parse_table.lex_modes = vec![LexMode {
            lex_state: 5,
            external_lex_state: 9,
        }];
        let modes = generate_lex_modes(&parse_table);
        assert_eq!(modes.len(), 3);
        assert_eq!(modes[0].lex_state, 5);
        assert_eq!(modes[0].external_lex_state, 9);
        // Missing entries fall back to LexMode default (zeroes).
        for missing in &modes[1..] {
            assert_eq!(missing.lex_state, 0);
            assert_eq!(missing.external_lex_state, 0);
        }
    }

    #[test]
    fn serialize_compressed_tables_emits_all_variant_strings() {
        let tables = tiny_compressed_with_all_actions();
        let json = serialize_compressed_tables(&tables).unwrap();
        // Action variants rendered as human-readable strings.
        assert!(json.contains("Shift(7)"));
        assert!(json.contains("Reduce(3)"));
        assert!(json.contains("Accept"));
        assert!(json.contains("Error"));
        assert!(json.contains("Recover"));
        assert!(json.contains("Fork(2)"));
        // Default actions render the same variants too.
        assert!(json.contains("Fork(1)"));
        // Goto entries render both variants.
        assert!(json.contains("Single(11)"));
        assert!(json.contains("RunLength(22, 4)"));
        // small_table_threshold included.
        assert!(json.contains("\"small_table_threshold\""));
    }

    #[test]
    fn serializable_lex_state_round_trips_through_json() {
        let original = SerializableLexState {
            lex_state: 17,
            external_lex_state: 42,
        };
        let s = serde_json::to_string(&original).unwrap();
        let back: SerializableLexState = serde_json::from_str(&s).unwrap();
        assert_eq!(back, original);
    }

    #[test]
    fn serializable_language_round_trips_through_json() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.tokens.insert(
            SymbolId(1),
            Token {
                name: "tok".to_string(),
                pattern: TokenPattern::String("t".to_string()),
                fragile: false,
            },
        );
        let parse_table = crate::empty_table!(states: 1, terms: 1, nonterms: 0);
        let lang = build_serializable_language(&grammar, &parse_table, None);

        let json = serde_json::to_string(&lang).unwrap();
        let parsed: SerializableLanguage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, lang);
    }
}
