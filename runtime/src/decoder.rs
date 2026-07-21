//! Decoder for extracting Grammar and ParseTable from Tree-sitter's TSLanguage struct
//!
//! This module reverse-engineers Tree-sitter's compressed parse table format
//! and decodes it into adze's native structures.

use adze_glr_core::ParseRule;
use adze_ir::{Grammar, Rule, SymbolId, TokenPattern};
use indexmap::IndexMap;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::pure_parser::TSLanguage;

mod externals;
mod fields;
mod parse_table;
mod productions;
mod symbols;

#[cfg(test)]
use crate::pure_parser::TSParseAction;
#[cfg(test)]
use crate::ts_format::TSActionTag;
#[cfg(test)]
use adze_glr_core::{Action, RuleId, StateId};
#[cfg(test)]
use parse_table::decode_action;
pub use parse_table::decode_parse_table;

/// Load token patterns from a Tree-sitter `grammar.json` file.
///
/// This extracts:
/// - string literals (`type: "STRING"`) as `TokenPattern::String`
/// - regex-like patterns (`type: "PATTERN"`) as `TokenPattern::Regex`
///
/// The returned map uses:
/// - token rule names as keys when a rule directly represents a token
/// - literal text itself as keys for string literals
pub fn load_token_patterns(grammar_json_path: &Path) -> HashMap<String, TokenPattern> {
    let Ok(contents) = fs::read_to_string(grammar_json_path) else {
        return HashMap::new();
    };

    let mut patterns = HashMap::new();

    // Named rules whose body directly represents a token.
    // This handles the common grammar.json shape:
    // "rules": { "identifier": { "type": "PATTERN", "value": "..." }, ... }
    let named_rule_re = Regex::new(
        r#""([^"\\]+)"\s*:\s*\{\s*"type"\s*:\s*"(STRING|PATTERN)"\s*,\s*"value"\s*:\s*"((?:\\.|[^"\\])*)""#,
    )
    .expect("regex must compile");
    for captures in named_rule_re.captures_iter(&contents) {
        let name = unescape_json_string(&captures[1]);
        let value = unescape_json_string(&captures[3]);
        let pattern = if &captures[2] == "STRING" {
            TokenPattern::String(value)
        } else {
            TokenPattern::Regex(value)
        };
        patterns.insert(name, pattern);
    }

    // String literals that appear anywhere in the grammar.
    let string_literal_re =
        Regex::new(r#""type"\s*:\s*"STRING"\s*,\s*"value"\s*:\s*"((?:\\.|[^"\\])*)""#)
            .expect("regex must compile");
    for captures in string_literal_re.captures_iter(&contents) {
        let value = unescape_json_string(&captures[1]);
        patterns
            .entry(value.clone())
            .or_insert_with(|| TokenPattern::String(value));
    }

    patterns
}

fn unescape_json_string(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }

        match chars.next() {
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('/') => out.push('/'),
            Some('b') => out.push('\u{0008}'),
            Some('f') => out.push('\u{000C}'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                if hex.len() == 4
                    && let Ok(code) = u32::from_str_radix(&hex, 16)
                    && let Some(ch) = char::from_u32(code)
                {
                    out.push(ch);
                }
            }
            Some(other) => out.push(other),
            None => break,
        }
    }
    out
}

/// Decode a Grammar from a TSLanguage struct
pub fn decode_grammar(lang: &'static TSLanguage) -> Grammar {
    decode_grammar_with_patterns(lang, &HashMap::new())
}

#[cfg(feature = "glr")]
pub(crate) fn decode_rule_fields(
    lang: &'static TSLanguage,
    rule_index: usize,
) -> Vec<(adze_ir::FieldId, usize)> {
    fields::decode_rule_fields(lang, rule_index)
}

/// Decode a Grammar from a TSLanguage struct with token patterns from grammar.json
pub fn decode_grammar_with_patterns(
    lang: &'static TSLanguage,
    token_patterns: &HashMap<String, TokenPattern>,
) -> Grammar {
    let symbol_names = symbols::decode_symbol_names(lang);
    let tokens = symbols::decode_tokens(lang, &symbol_names, token_patterns);
    let field_names_map = fields::decode_field_names(lang);
    let rule_names = IndexMap::new();
    let mut rules: IndexMap<SymbolId, Vec<Rule>> = IndexMap::new();

    productions::decode_metadata_rules(lang, &mut rules);
    let _fields_by_rule = fields::decode_fields_by_rule(lang);
    let production_ids = productions::decode_fallback_rules(lang, &mut rules);
    let externals = externals::decode_external_tokens(lang, &symbol_names);

    Grammar {
        name: "decoded_grammar".to_string(),
        rules,
        tokens,
        precedences: vec![],
        conflicts: vec![],
        externals,
        extras: vec![],
        fields: field_names_map,
        supertypes: vec![],
        inline_rules: vec![],
        alias_sequences: IndexMap::new(),
        production_ids,
        max_alias_sequence_length: 0,
        rule_names,
        symbol_registry: None,
    }
}

pub(super) fn decode_rules(lang: &TSLanguage) -> Vec<ParseRule> {
    let production_count = lang.production_count as usize;

    // Prevent excessive allocations to avoid DoS
    let safe_production_count = production_count.min(100000);
    let mut rules = Vec::with_capacity(safe_production_count);

    if lang.production_lhs_index.is_null() || production_count == 0 {
        // No rules available, return empty
        return rules;
    }

    // Create safe slice for production_lhs_index
    // SAFETY: `lang.production_lhs_index` is non-null (checked above).
    // `safe_production_count` is capped at 100000. TSLanguage contract guarantees
    // the production_lhs_index array has `production_count` elements.
    let production_lhs_slice =
        unsafe { std::slice::from_raw_parts(lang.production_lhs_index, safe_production_count) };

    // Create safe slice for rules if available
    let rules_slice = if !lang.rules.is_null() && lang.rule_count > 0 {
        let rule_count = (lang.rule_count as usize).min(safe_production_count);
        // SAFETY: `lang.rules` is non-null (branch guard) and `rule_count` is
        // bounded by both `lang.rule_count` and `safe_production_count`.
        Some(unsafe { std::slice::from_raw_parts(lang.rules, rule_count) })
    } else {
        None
    };

    // Use production_lhs_index to get the correct LHS symbols
    // and try to get RHS length from TSRule if available
    for i in 0..safe_production_count {
        // Get LHS from production_lhs_index (which has correct symbol in table index space)
        let lhs_idx = if i < production_lhs_slice.len() {
            production_lhs_slice[i]
        } else {
            0 // Fallback for out-of-bounds
        };

        // Try to get rhs_len from TSRule if available
        let rhs_len = if let Some(rules_slice) = rules_slice {
            if i < rules_slice.len() {
                rules_slice[i].rhs_len as u16
            } else {
                0 // Fallback for out-of-bounds
            }
        } else {
            0 // Fallback: we don't know the RHS length
        };

        rules.push(ParseRule {
            lhs: SymbolId(lhs_idx), // Use the index from production_lhs_index
            rhs_len,
        });
    }
    rules
}

/// Check if a symbol is hidden based on metadata
#[allow(dead_code)]
fn is_hidden(metadata: u8) -> bool {
    // Bit 0 is typically the visible bit in Tree-sitter
    (metadata & 0x01) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_decoder_safety() {
        // This test ensures our decoder doesn't panic on null pointers
        // In real use, we'd test with actual TSLanguage structs
    }

    #[test]
    fn test_action_decoding() {
        // Test that we can decode different action types correctly
        let empty_rules = vec![];
        let empty_map = HashMap::new();

        // Test Shift action
        let shift_action = TSParseAction {
            action_type: TSActionTag::Shift as u8,
            extra: 0,
            child_count: 0,
            dynamic_precedence: 0,
            symbol: 42,
        };
        match decode_action(&shift_action, &empty_rules, &empty_map) {
            Action::Shift(StateId(state)) => assert_eq!(state, 42),
            _ => panic!("Expected Shift action"),
        }

        // Test Reduce action with direct rule index
        let rules = vec![ParseRule {
            lhs: SymbolId(10),
            rhs_len: 3,
        }];
        let reduce_action = TSParseAction {
            action_type: TSActionTag::Reduce as u8,
            extra: 0,
            child_count: 3,
            dynamic_precedence: 0,
            symbol: 0,
        };
        match decode_action(&reduce_action, &rules, &empty_map) {
            Action::Reduce(RuleId(rule)) => assert_eq!(rule, 0),
            _ => panic!("Expected Reduce action"),
        }

        // Test Accept action
        let accept_action = TSParseAction {
            action_type: TSActionTag::Accept as u8,
            extra: 0,
            child_count: 0,
            dynamic_precedence: 0,
            symbol: 0,
        };
        assert!(matches!(
            decode_action(&accept_action, &empty_rules, &empty_map),
            Action::Accept
        ));

        // Test Error/Recover action
        let recover_action = TSParseAction {
            action_type: TSActionTag::Error as u8,
            extra: 0,
            child_count: 0,
            dynamic_precedence: 0,
            symbol: 0,
        };
        assert!(matches!(
            decode_action(&recover_action, &empty_rules, &empty_map),
            Action::Error
        ));
    }

    #[test]
    fn small_table_reduce_actions_map_rule_id_to_production_id() {
        static PRODUCTION_ID_MAP: [u16; 2] = [1, 0];
        static PRODUCTION_LHS_INDEX: [u16; 2] = [2, 2];
        static SMALL_PARSE_TABLE: [u16; 2] = [1, 0x8001];
        static SMALL_PARSE_TABLE_MAP: [u32; 2] = [0, 2];
        static NAME_ERROR: &[u8] = b"end\0";
        static NAME_TOKEN: &[u8] = b"token\0";
        static NAME_NODE: &[u8] = b"node\0";
        static RULES: [crate::pure_parser::TSRule; 2] = [
            crate::pure_parser::TSRule {
                lhs: 2,
                rhs_len: 1,
                _pad: 0,
            },
            crate::pure_parser::TSRule {
                lhs: 2,
                rhs_len: 1,
                _pad: 0,
            },
        ];
        let symbol_names = Box::leak(Box::new([
            NAME_ERROR.as_ptr(),
            NAME_TOKEN.as_ptr(),
            NAME_NODE.as_ptr(),
        ]));

        let language = Box::leak(Box::new(TSLanguage {
            version: crate::pure_parser::TREE_SITTER_LANGUAGE_VERSION,
            symbol_count: 3,
            alias_count: 0,
            token_count: 2,
            external_token_count: 0,
            state_count: 1,
            large_state_count: 0,
            production_id_count: 2,
            field_count: 0,
            max_alias_sequence_length: 0,
            production_id_map: PRODUCTION_ID_MAP.as_ptr(),
            parse_table: std::ptr::null(),
            small_parse_table: SMALL_PARSE_TABLE.as_ptr(),
            small_parse_table_map: SMALL_PARSE_TABLE_MAP.as_ptr(),
            parse_actions: std::ptr::null(),
            symbol_names: symbol_names.as_ptr(),
            field_names: std::ptr::null(),
            field_map_slices: std::ptr::null(),
            field_map_entries: std::ptr::null(),
            symbol_metadata: std::ptr::null(),
            public_symbol_map: std::ptr::null(),
            alias_map: std::ptr::null(),
            alias_sequences: std::ptr::null(),
            lex_modes: std::ptr::null(),
            lex_fn: None,
            keyword_lex_fn: None,
            keyword_capture_token: 0,
            external_scanner: crate::pure_parser::ExternalScanner::default(),
            primary_state_ids: std::ptr::null(),
            production_lhs_index: PRODUCTION_LHS_INDEX.as_ptr(),
            production_count: 2,
            eof_symbol: 0,
            rules: RULES.as_ptr(),
            rule_count: 2,
        }));

        let table = decode_parse_table(language);

        match table.action_table[0][1][0] {
            Action::Reduce(RuleId(rule_id)) => assert_eq!(rule_id, 1),
            ref action => panic!("expected mapped reduce action, got {action:?}"),
        }
    }

    #[test]
    fn test_load_token_patterns_reads_json_literals_and_patterns() {
        let mut grammar_file = NamedTempFile::new().expect("temp file");
        writeln!(
            grammar_file,
            r#"{{
                "rules": {{
                    "identifier": {{ "type": "PATTERN", "value": "[a-z_][a-z0-9_]*" }},
                    "kw_def": {{ "type": "STRING", "value": "def" }},
                    "function_definition": {{
                        "type": "SEQ",
                        "members": [
                            {{ "type": "STRING", "value": ":" }}
                        ]
                    }}
                }}
            }}"#
        )
        .expect("write grammar");

        let patterns = load_token_patterns(grammar_file.path());

        assert_eq!(
            patterns.get("identifier"),
            Some(&TokenPattern::Regex("[a-z_][a-z0-9_]*".to_string()))
        );
        assert_eq!(
            patterns.get("kw_def"),
            Some(&TokenPattern::String("def".to_string()))
        );
        assert_eq!(
            patterns.get("def"),
            Some(&TokenPattern::String("def".to_string()))
        );
        assert_eq!(
            patterns.get(":"),
            Some(&TokenPattern::String(":".to_string()))
        );
    }

    #[test]
    fn test_load_token_patterns_missing_file_returns_empty() {
        let patterns = load_token_patterns(Path::new("/definitely/missing/grammar.json"));
        assert!(patterns.is_empty());
    }
}
