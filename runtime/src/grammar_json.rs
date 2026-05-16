#[cfg(feature = "serialization")]
use adze_ir::{SymbolId, TokenPattern};
#[cfg(feature = "serialization")]
use std::collections::HashMap;
#[cfg(feature = "serialization")]
use std::fs::File;
#[cfg(feature = "serialization")]
use std::path::Path;

/// Load token patterns from a Tree-sitter grammar.json file
#[cfg(feature = "serialization")]
pub fn load_patterns_from_grammar_json(
    path: &Path,
) -> Result<HashMap<String, TokenPattern>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let json: serde_json::Value = serde_json::from_reader(file)?;
    let mut patterns = HashMap::new();

    // The rules object contains all grammar rules
    if let Some(rules) = json.get("rules").and_then(|r| r.as_object()) {
        for (symbol_name, rule) in rules {
            // Extract pattern from the rule
            let pattern = extract_pattern_from_rule(rule);
            if let Some(p) = pattern {
                patterns.insert(symbol_name.clone(), p);
            }
        }
    }

    Ok(patterns)
}

/// Extract a TokenPattern from a grammar rule JSON value
#[cfg(feature = "serialization")]
fn extract_pattern_from_rule(rule: &serde_json::Value) -> Option<TokenPattern> {
    // Handle different rule types
    match rule.get("type").and_then(|t| t.as_str()) {
        Some("STRING") => {
            // String literal: { "type": "STRING", "value": "def" }
            rule.get("value")
                .and_then(|v| v.as_str())
                .map(|s| TokenPattern::String(s.to_string()))
        }
        Some("PATTERN") => {
            // Regex pattern: { "type": "PATTERN", "value": "[a-zA-Z_][a-zA-Z0-9_]*" }
            rule.get("value")
                .and_then(|v| v.as_str())
                .map(|s| TokenPattern::Regex(s.to_string()))
        }
        Some("TOKEN") => {
            // Token with immediate content: { "type": "TOKEN", "content": { ... } }
            rule.get("content").and_then(extract_pattern_from_rule)
        }
        Some("IMMEDIATE_TOKEN") => {
            // Immediate token: { "type": "IMMEDIATE_TOKEN", "content": { ... } }
            rule.get("content").and_then(extract_pattern_from_rule)
        }
        Some("ALIAS") => {
            // Alias wraps another rule: { "type": "ALIAS", "content": { ... } }
            rule.get("content").and_then(extract_pattern_from_rule)
        }
        Some("CHOICE") => {
            // For CHOICE, we can't easily represent it as a single pattern
            // We'd need to combine alternatives into a regex, which is complex
            // For now, skip CHOICE rules
            None
        }
        Some("SYMBOL") => {
            // Reference to another rule, not a terminal pattern
            None
        }
        Some("SEQ") | Some("REPEAT") | Some("REPEAT1") | Some("PREC") | Some("PREC_LEFT")
        | Some("PREC_RIGHT") | Some("PREC_DYNAMIC") => {
            // These are non-terminals or complex rules
            None
        }
        _ => {
            // Unknown or complex rule type
            None
        }
    }
}

/// Load patterns and create a symbol name to ID mapping
#[cfg(feature = "serialization")]
pub fn load_patterns_with_symbol_map(
    grammar_json_path: &Path,
    symbol_names: &[String],
) -> Result<HashMap<SymbolId, TokenPattern>, Box<dyn std::error::Error>> {
    let patterns_by_name = load_patterns_from_grammar_json(grammar_json_path)?;
    let mut patterns_by_id = HashMap::new();

    // Map patterns from name to symbol ID
    for (idx, name) in symbol_names.iter().enumerate() {
        if let Some(pattern) = patterns_by_name.get(name) {
            patterns_by_id.insert(SymbolId(idx as u16), pattern.clone());
        }
    }

    Ok(patterns_by_id)
}

#[cfg(all(test, feature = "serialization"))]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Helper: write JSON text to a temporary file and return the handle so the
    /// caller controls the file's lifetime (it is deleted on drop).
    fn write_grammar(json: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("temp file");
        file.write_all(json.as_bytes()).expect("write json");
        file.flush().expect("flush");
        file
    }

    // ---- load_patterns_from_grammar_json --------------------------------

    #[test]
    fn load_patterns_missing_file_errors() {
        let path = Path::new("/nonexistent/path/should/not/exist/grammar.json");
        let result = load_patterns_from_grammar_json(path);
        assert!(result.is_err(), "missing file must error");
    }

    #[test]
    fn load_patterns_invalid_json_errors() {
        let file = write_grammar("{ not valid json ::: ");
        let result = load_patterns_from_grammar_json(file.path());
        assert!(result.is_err(), "malformed JSON must error");
    }

    #[test]
    fn load_patterns_no_rules_returns_empty() {
        // Valid JSON but without a `rules` object -> empty map (no error).
        let file = write_grammar(r#"{"name": "empty_grammar"}"#);
        let patterns = load_patterns_from_grammar_json(file.path()).expect("ok");
        assert!(patterns.is_empty());
    }

    #[test]
    fn load_patterns_rules_not_object_returns_empty() {
        // `rules` is present but not a JSON object -> branch falls through to empty map.
        let file = write_grammar(r#"{"rules": []}"#);
        let patterns = load_patterns_from_grammar_json(file.path()).expect("ok");
        assert!(patterns.is_empty());
    }

    #[test]
    fn load_patterns_string_and_pattern_happy_path() {
        let json = r#"{
            "name": "tiny",
            "rules": {
                "kw_def": {"type": "STRING", "value": "def"},
                "identifier": {"type": "PATTERN", "value": "[a-zA-Z_][a-zA-Z0-9_]*"}
            }
        }"#;
        let file = write_grammar(json);
        let patterns = load_patterns_from_grammar_json(file.path()).expect("ok");
        assert_eq!(patterns.len(), 2);
        assert_eq!(
            patterns.get("kw_def"),
            Some(&TokenPattern::String("def".to_string()))
        );
        assert_eq!(
            patterns.get("identifier"),
            Some(&TokenPattern::Regex("[a-zA-Z_][a-zA-Z0-9_]*".to_string()))
        );
    }

    #[test]
    fn load_patterns_token_and_immediate_token_unwrap_content() {
        let json = r#"{
            "rules": {
                "tok": {"type": "TOKEN", "content": {"type": "STRING", "value": "tok_val"}},
                "imm": {"type": "IMMEDIATE_TOKEN", "content": {"type": "PATTERN", "value": "\\d+"}}
            }
        }"#;
        let file = write_grammar(json);
        let patterns = load_patterns_from_grammar_json(file.path()).expect("ok");
        assert_eq!(
            patterns.get("tok"),
            Some(&TokenPattern::String("tok_val".to_string()))
        );
        assert_eq!(
            patterns.get("imm"),
            Some(&TokenPattern::Regex("\\d+".to_string()))
        );
    }

    #[test]
    fn load_patterns_alias_unwraps_content() {
        let json = r#"{
            "rules": {
                "alias_str": {
                    "type": "ALIAS",
                    "content": {"type": "STRING", "value": "aliased"}
                }
            }
        }"#;
        let file = write_grammar(json);
        let patterns = load_patterns_from_grammar_json(file.path()).expect("ok");
        assert_eq!(
            patterns.get("alias_str"),
            Some(&TokenPattern::String("aliased".to_string()))
        );
    }

    #[test]
    fn load_patterns_skips_non_terminals_and_complex_rules() {
        // CHOICE, SYMBOL, SEQ, REPEAT, REPEAT1, PREC*, and unknown rule
        // types are deliberately skipped by extract_pattern_from_rule.
        let json = r#"{
            "rules": {
                "choose": {"type": "CHOICE", "members": []},
                "ref":    {"type": "SYMBOL", "name": "other"},
                "seq":    {"type": "SEQ",    "members": []},
                "rep":    {"type": "REPEAT", "content": {}},
                "rep1":   {"type": "REPEAT1","content": {}},
                "pr":     {"type": "PREC",       "value": 1, "content": {}},
                "prl":    {"type": "PREC_LEFT",  "value": 1, "content": {}},
                "prr":    {"type": "PREC_RIGHT", "value": 1, "content": {}},
                "prd":    {"type": "PREC_DYNAMIC","value": 1, "content": {}},
                "weird":  {"type": "WHO_KNOWS"}
            }
        }"#;
        let file = write_grammar(json);
        let patterns = load_patterns_from_grammar_json(file.path()).expect("ok");
        assert!(
            patterns.is_empty(),
            "no token-bearing rules should be extracted, got {patterns:?}"
        );
    }

    #[test]
    fn load_patterns_token_missing_content_yields_none() {
        // TOKEN/IMMEDIATE_TOKEN/ALIAS with no `content` field -> None, skipped.
        let json = r#"{
            "rules": {
                "broken_tok":   {"type": "TOKEN"},
                "broken_imm":   {"type": "IMMEDIATE_TOKEN"},
                "broken_alias": {"type": "ALIAS"}
            }
        }"#;
        let file = write_grammar(json);
        let patterns = load_patterns_from_grammar_json(file.path()).expect("ok");
        assert!(patterns.is_empty());
    }

    #[test]
    fn load_patterns_string_without_value_yields_none() {
        // STRING/PATTERN with missing or non-string `value` -> skipped.
        let json = r#"{
            "rules": {
                "no_val":   {"type": "STRING"},
                "bad_val":  {"type": "PATTERN", "value": 42},
                "good":     {"type": "STRING", "value": "x"}
            }
        }"#;
        let file = write_grammar(json);
        let patterns = load_patterns_from_grammar_json(file.path()).expect("ok");
        assert_eq!(patterns.len(), 1);
        assert_eq!(
            patterns.get("good"),
            Some(&TokenPattern::String("x".to_string()))
        );
    }

    #[test]
    fn load_patterns_single_rule_grammar() {
        let json = r#"{
            "rules": {"only": {"type": "STRING", "value": "solo"}}
        }"#;
        let file = write_grammar(json);
        let patterns = load_patterns_from_grammar_json(file.path()).expect("ok");
        assert_eq!(patterns.len(), 1);
        assert_eq!(
            patterns.get("only"),
            Some(&TokenPattern::String("solo".to_string()))
        );
    }

    #[test]
    fn load_patterns_nested_token_wrapping_alias_string() {
        // TOKEN -> ALIAS -> STRING should still unwrap to a String pattern.
        let json = r#"{
            "rules": {
                "nested": {
                    "type": "TOKEN",
                    "content": {
                        "type": "ALIAS",
                        "content": {"type": "STRING", "value": "deep"}
                    }
                }
            }
        }"#;
        let file = write_grammar(json);
        let patterns = load_patterns_from_grammar_json(file.path()).expect("ok");
        assert_eq!(
            patterns.get("nested"),
            Some(&TokenPattern::String("deep".to_string()))
        );
    }

    // ---- load_patterns_with_symbol_map ----------------------------------

    #[test]
    fn symbol_map_propagates_missing_file_error() {
        let path = Path::new("/nonexistent/path/grammar.json");
        let names = vec!["a".to_string()];
        let result = load_patterns_with_symbol_map(path, &names);
        assert!(result.is_err(), "underlying loader error must propagate");
    }

    #[test]
    fn symbol_map_assigns_ids_by_index_and_skips_unknown() {
        let json = r#"{
            "rules": {
                "kw":  {"type": "STRING",  "value": "kw"},
                "id":  {"type": "PATTERN", "value": "[a-z]+"}
            }
        }"#;
        let file = write_grammar(json);
        // Note: `missing` has no rule in the grammar -> no entry in the map.
        let names = vec!["kw".to_string(), "missing".to_string(), "id".to_string()];
        let map = load_patterns_with_symbol_map(file.path(), &names).expect("ok");
        assert_eq!(map.len(), 2);
        assert_eq!(
            map.get(&SymbolId(0)),
            Some(&TokenPattern::String("kw".to_string()))
        );
        assert!(!map.contains_key(&SymbolId(1)));
        assert_eq!(
            map.get(&SymbolId(2)),
            Some(&TokenPattern::Regex("[a-z]+".to_string()))
        );
    }

    #[test]
    fn symbol_map_empty_names_yields_empty_map() {
        let json = r#"{"rules": {"a": {"type": "STRING", "value": "a"}}}"#;
        let file = write_grammar(json);
        let map = load_patterns_with_symbol_map(file.path(), &[]).expect("ok");
        assert!(map.is_empty());
    }

    #[test]
    fn symbol_map_empty_grammar_yields_empty_map() {
        let file = write_grammar(r#"{"name": "g"}"#);
        let names = vec!["a".to_string(), "b".to_string()];
        let map = load_patterns_with_symbol_map(file.path(), &names).expect("ok");
        assert!(map.is_empty());
    }
}
