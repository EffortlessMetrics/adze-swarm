use adze_ir::{Grammar, TokenPattern};
use std::collections::BTreeSet;

use super::LspFeature;

/// Completion provider for LSP
pub struct CompletionProvider {
    keywords: Vec<String>,
    symbols: Vec<String>,
}

impl CompletionProvider {
    pub fn new(grammar: &Grammar) -> Self {
        let mut keywords = BTreeSet::new();
        let mut symbols = BTreeSet::new();

        // Extract keywords from tokens
        for (_id, token) in &grammar.tokens {
            if let TokenPattern::String(value) = &token.pattern
                && value.chars().all(|c| c.is_alphabetic() || c == '_')
            {
                keywords.insert(value.clone());
            }
        }

        // Extract symbols from rule names
        for (_symbol_id, name) in &grammar.rule_names {
            symbols.insert(name.clone());
        }

        Self {
            keywords: keywords.into_iter().collect(),
            symbols: symbols.into_iter().collect(),
        }
    }
}

impl LspFeature for CompletionProvider {
    fn name(&self) -> &str {
        "completion"
    }

    fn generate_handler(&self) -> String {
        format!(
            r#"
pub async fn handle_completion(
    params: lsp_types::CompletionParams,
) -> Result<Option<lsp_types::CompletionResponse>> {{
    let items = vec![
        {}
    ];
    
    Ok(Some(lsp_types::CompletionResponse::Array(items)))
}}

fn create_keyword_completions() -> Vec<lsp_types::CompletionItem> {{
    vec![
        {}
    ]
}}

fn create_symbol_completions() -> Vec<lsp_types::CompletionItem> {{
    vec![
        {}
    ]
}}"#,
            // Keywords completion items
            self.keywords
                .iter()
                .map(|k| format!(
                    r#"lsp_types::CompletionItem {{
                        label: "{}".to_string(),
                        kind: Some(lsp_types::CompletionItemKind::KEYWORD),
                        ..Default::default()
                    }}"#,
                    k
                ))
                .collect::<Vec<_>>()
                .join(",\n        "),
            // Keyword function
            self.keywords
                .iter()
                .map(|k| format!(
                    r#"lsp_types::CompletionItem {{
                        label: "{}".to_string(),
                        kind: Some(lsp_types::CompletionItemKind::KEYWORD),
                        ..Default::default()
                    }}"#,
                    k
                ))
                .collect::<Vec<_>>()
                .join(",\n        "),
            // Symbol function
            self.symbols
                .iter()
                .map(|s| format!(
                    r#"lsp_types::CompletionItem {{
                        label: "{}".to_string(),
                        kind: Some(lsp_types::CompletionItemKind::CLASS),
                        ..Default::default()
                    }}"#,
                    s
                ))
                .collect::<Vec<_>>()
                .join(",\n        ")
        )
    }

    fn required_imports(&self) -> Vec<String> {
        vec![
            "use lsp_types::{CompletionParams, CompletionResponse, CompletionItem, CompletionItemKind};".to_string()
        ]
    }

    fn capabilities(&self) -> serde_json::Value {
        serde_json::json!({
            "completionProvider": {
                "resolveProvider": false,
                "triggerCharacters": [".", ":"]
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::LspFeature;
    use adze_ir::builder::GrammarBuilder;

    #[test]
    fn given_mixed_token_patterns_when_building_completion_provider_then_only_word_keywords_are_suggested()
     {
        // Given
        let grammar = GrammarBuilder::new("completion_lang")
            .token("KW_IF", "if")
            .token("KW_ASYNC", "async")
            .token("PLUS", "+")
            .token("NUMBER", "[0-9]+")
            .rule("expr", vec!["KW_IF"])
            .start("expr")
            .build();

        // When
        let provider = CompletionProvider::new(&grammar);

        // Then
        let mut keywords = provider.keywords.clone();
        keywords.sort();
        assert_eq!(keywords, vec!["async".to_string(), "if".to_string()]);
        assert!(provider.symbols.contains(&"expr".to_string()));

        let handler = provider.generate_handler();
        assert!(handler.contains("label: \"if\".to_string()"));
        assert!(handler.contains("label: \"expr\".to_string()"));
        assert!(!handler.contains("label: \"+\".to_string()"));
    }

    #[test]
    fn given_duplicate_and_unsorted_grammar_entries_when_building_completion_provider_then_suggestions_are_unique_and_sorted()
     {
        // Given
        let grammar = GrammarBuilder::new("completion_ordering")
            .token("KW_Z", "zeta")
            .token("KW_IF", "if")
            .token("KW_IF_ALIAS", "if")
            .rule("z_statement", vec!["KW_Z"])
            .rule("a_statement", vec!["KW_IF"])
            .start("z_statement")
            .build();

        // When
        let provider = CompletionProvider::new(&grammar);
        let handler = provider.generate_handler();

        // Then
        assert_eq!(
            provider.keywords,
            vec!["if".to_string(), "zeta".to_string()]
        );
        assert_eq!(
            provider.symbols,
            vec!["a_statement".to_string(), "z_statement".to_string()]
        );
        assert_eq!(handler.matches("label: \"if\".to_string()").count(), 2);
    }

    #[test]
    fn given_completion_provider_when_requesting_capabilities_then_trigger_characters_are_exposed()
    {
        // Given
        let grammar = GrammarBuilder::new("completion_caps")
            .token("KW_LET", "let")
            .rule("statement", vec!["KW_LET"])
            .start("statement")
            .build();
        let provider = CompletionProvider::new(&grammar);

        // When
        let capabilities = provider.capabilities();

        // Then
        assert_eq!(
            capabilities["completionProvider"]["resolveProvider"],
            serde_json::json!(false)
        );
        assert_eq!(
            capabilities["completionProvider"]["triggerCharacters"],
            serde_json::json!([".", ":"])
        );
    }
}
