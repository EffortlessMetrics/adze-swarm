//! Preserve explicit compiler identity from imported grammar JSON/grammar.js (#862 PR3).

use super::{GrammarJs, Rule};
use crate::grammar_js::hidden_pattern_token_name;

/// Record declared start and wrapper-token metadata for imported grammars.
///
/// Tree-sitter grammars treat the first `rules` entry as the start production when
/// no explicit `start_symbol` is present. Pattern and literal wrapper rules map to
/// their backing token names using the same conventions as macro extraction.
pub fn infer_imported_compiler_identity(grammar: &mut GrammarJs) {
    if grammar.start_symbol.is_none()
        && let Some(first_rule) = grammar.rules.keys().next()
    {
        grammar.start_symbol = Some(first_rule.clone());
    }

    for (rule_name, rule) in &grammar.rules {
        if grammar.wrapper_token_relations.contains_key(rule_name) {
            continue;
        }

        match rule {
            Rule::Pattern { value } => {
                grammar
                    .wrapper_token_relations
                    .insert(rule_name.clone(), hidden_pattern_token_name(value));
            }
            Rule::String { value } => {
                grammar
                    .wrapper_token_relations
                    .insert(rule_name.clone(), value.clone());
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;

    #[test]
    fn imported_json_without_start_symbol_uses_first_rule() {
        let mut grammar = GrammarJs::new("imported".to_string());
        grammar.rules.insert(
            "source_file".to_string(),
            Rule::Symbol {
                name: "expression".to_string(),
            },
        );
        grammar.rules.insert(
            "expression".to_string(),
            Rule::Pattern {
                value: r"\d+".to_string(),
            },
        );

        infer_imported_compiler_identity(&mut grammar);

        assert_eq!(grammar.start_symbol.as_deref(), Some("source_file"));
    }

    #[test]
    fn imported_json_preserves_explicit_start_symbol() {
        let mut grammar = GrammarJs::new("imported".to_string());
        grammar.start_symbol = Some("Root9".to_string());
        grammar.rules.insert(
            "_helper".to_string(),
            Rule::Pattern {
                value: r"\s+".to_string(),
            },
        );
        grammar.rules.insert(
            "Root9".to_string(),
            Rule::Symbol {
                name: "_helper".to_string(),
            },
        );

        infer_imported_compiler_identity(&mut grammar);

        assert_eq!(grammar.start_symbol.as_deref(), Some("Root9"));
    }

    #[test]
    fn imported_pattern_wrappers_record_explicit_relations() {
        let mut grammar = GrammarJs::new("wrappers".to_string());
        grammar.rules.insert(
            "identifier".to_string(),
            Rule::Pattern {
                value: r"[a-z]+".to_string(),
            },
        );
        grammar.wrapper_token_relations = IndexMap::new();

        infer_imported_compiler_identity(&mut grammar);

        assert_eq!(
            grammar
                .wrapper_token_relations
                .get("identifier")
                .map(String::as_str),
            Some("_/[a-z]+/")
        );
    }
}
