// LSP feature implementations for adze grammars

use adze_ir::{Grammar, TokenPattern};
use std::collections::{BTreeSet, HashMap};

/// Trait for LSP features
pub trait LspFeature: Send + Sync {
    /// Get the name of this feature
    fn name(&self) -> &str;

    /// Generate handler code for this feature
    fn generate_handler(&self) -> String;

    /// Get required imports for this feature
    fn required_imports(&self) -> Vec<String>;

    /// Get capabilities for this feature
    fn capabilities(&self) -> serde_json::Value;
}

/// Completion provider for LSP
pub struct CompletionProvider {
    keywords: Vec<String>,
    symbols: Vec<String>,
}

impl CompletionProvider {
    pub fn new(grammar: &Grammar) -> Self {
        let keywords = grammar
            .tokens
            .values()
            .filter_map(|token| match &token.pattern {
                TokenPattern::String(value) if Self::is_word_keyword(value) => Some(value.clone()),
                _ => None,
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        let symbols = grammar
            .rule_names
            .values()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        Self { keywords, symbols }
    }

    fn is_word_keyword(value: &str) -> bool {
        value.chars().all(|c| c.is_alphabetic() || c == '_')
    }

    fn render_completion_items(labels: &[String], kind: &str, indent: &str) -> String {
        labels
            .iter()
            .map(|label| {
                format!(
                    r#"lsp_types::CompletionItem {{
{indent}    label: "{label}".to_string(),
{indent}    kind: Some(lsp_types::CompletionItemKind::{kind}),
{indent}    ..Default::default()
{indent}}}"#
                )
            })
            .collect::<Vec<_>>()
            .join(&format!(",\n{indent}"))
    }
}

impl LspFeature for CompletionProvider {
    fn name(&self) -> &str {
        "completion"
    }

    fn generate_handler(&self) -> String {
        let keyword_items = Self::render_completion_items(&self.keywords, "KEYWORD", "        ");
        let symbol_items = Self::render_completion_items(&self.symbols, "CLASS", "        ");

        format!(
            r#"
pub async fn handle_completion(
    params: lsp_types::CompletionParams,
) -> Result<Option<lsp_types::CompletionResponse>> {{
    let mut items = create_keyword_completions();
    items.extend(create_symbol_completions());
    
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
            keyword_items, symbol_items
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

/// Hover provider for LSP
pub struct HoverProvider {
    #[allow(dead_code)]
    pub documentation: HashMap<String, String>,
}

impl HoverProvider {
    pub fn new(grammar: &Grammar) -> Self {
        let mut documentation = HashMap::new();

        // Generate documentation from grammar rules
        for (_symbol_id, rule_name) in &grammar.rule_names {
            let doc = format!("Grammar rule: {}", rule_name);
            documentation.insert(rule_name.clone(), doc);
        }

        Self { documentation }
    }
}

impl LspFeature for HoverProvider {
    fn name(&self) -> &str {
        "hover"
    }

    fn generate_handler(&self) -> String {
        // Build the generated handler code with proper documentation lookup
        let documentation_map = HoverProvider::build_documentation_map();

        format!(
            r#"
use anyhow::{{Context, Result}};
use lsp_types::{{HoverParams, Hover, HoverContents, MarkedString, Position}};
use std::collections::HashMap;
use std::fs;

pub async fn handle_hover(
    params: HoverParams,
) -> Result<Option<Hover>> {{
    // Get the word under cursor
    let word = get_word_at_position(&params)?;

    // Look up documentation
    let contents = match lookup_documentation(&word) {{
        Some(doc) => HoverContents::Scalar(
            MarkedString::String(doc)
        ),
        None => return Ok(None),
    }};
    
    Ok(Some(Hover {{
        contents,
        range: None,
    }}))
}}

fn get_word_at_position(params: &HoverParams) -> Result<String> {{
    use anyhow::anyhow;
    let uri = &params.text_document_position_params.text_document.uri;
    let path = uri.to_file_path().map_err(|_| anyhow("invalid uri"))?;
    let text = fs::read_to_string(path)?;
    let position = params.text_document_position_params.position;
    let line = text
        .lines()
        .nth(position.line as usize)
        .ok_or_else(|| anyhow("line out of bounds"))?;
    let chars: Vec<char> = line.chars().collect();
    let mut start = position.character as usize;
    let mut end = start;
    while start > 0 {{
        let c = chars[start - 1];
        if c.is_alphanumeric() || c == '_' {{
            start -= 1;
        }} else {{
            break;
        }}
    }}
    while end < chars.len() {{
        let c = chars[end];
        if c.is_alphanumeric() || c == '_' {{
            end += 1;
        }} else {{
            break;
        }}
    }}
    Ok(chars[start..end].iter().collect())
}}

fn lookup_documentation(word: &str) -> Option<String> {{
    // Documentation map with common language constructs
    let docs: HashMap<&str, &str> = [
{}
    ].into_iter().collect();
    
    docs.get(word).map(|doc| format!("**{{}}**: {{}}", word, doc))
}}
"#,
            HoverProvider::format_documentation_entries(&documentation_map)
        )
    }

    fn required_imports(&self) -> Vec<String> {
        vec!["use lsp_types::{HoverParams, Hover, HoverContents, MarkedString};".to_string()]
    }

    fn capabilities(&self) -> serde_json::Value {
        serde_json::json!({
            "hoverProvider": true
        })
    }
}

impl HoverProvider {
    pub fn build_documentation_map() -> Vec<(&'static str, &'static str)> {
        vec![
            // Use the documentation from the grammar if available
            // For now, provide common programming language keywords
            ("fn", "Declares a function"),
            ("let", "Declares a variable binding"),
            ("mut", "Makes a binding mutable"),
            ("if", "Conditional expression"),
            ("else", "Alternative branch of conditional"),
            ("match", "Pattern matching expression"),
            ("struct", "Defines a struct type"),
            ("enum", "Defines an enum type"),
            ("impl", "Implements methods or traits"),
            ("trait", "Defines a trait"),
            ("pub", "Makes an item public"),
            ("use", "Imports items into scope"),
            ("mod", "Declares a module"),
            ("String", "UTF-8 encoded, growable string type"),
            ("str", "String slice type"),
            ("i32", "32-bit signed integer type"),
            ("u32", "32-bit unsigned integer type"),
            ("bool", "Boolean type with values true and false"),
            ("Vec", "Growable array type"),
            ("Option", "Type representing optional values"),
            ("Result", "Type for recoverable errors"),
            ("function", "Declares a function"),
            ("const", "Declares a constant"),
            ("var", "Declares a variable"),
            ("class", "Declares a class"),
            ("interface", "Declares a TypeScript interface"),
            ("type", "Declares a type alias"),
            ("import", "Imports modules or values"),
            ("export", "Exports values from module"),
            ("def", "Defines a function"),
            ("return", "Returns a value from function"),
            ("yield", "Yields a value from generator"),
            ("async", "Declares async function"),
            ("await", "Waits for async operation"),
            ("break", "Exits from a loop"),
            ("continue", "Skips to next iteration of loop"),
            ("while", "Loop that continues while condition is true"),
            ("for", "Loop that iterates over a sequence"),
            ("try", "Begins error handling block"),
            ("catch", "Handles errors in try block"),
            ("finally", "Code that always runs after try/catch"),
        ]
    }

    pub fn format_documentation_entries(entries: &[(&str, &str)]) -> String {
        entries
            .iter()
            .map(|(key, value)| format!("        (\"{}\", \"{}\")", key, value))
            .collect::<Vec<_>>()
            .join(",\n")
    }
}

/// Diagnostics provider for LSP
pub struct DiagnosticsProvider {
    grammar_name: String,
}

impl DiagnosticsProvider {
    pub fn new(grammar: &Grammar) -> Self {
        Self {
            grammar_name: grammar.name.clone(),
        }
    }
}

impl LspFeature for DiagnosticsProvider {
    fn name(&self) -> &str {
        "diagnostics"
    }

    fn generate_handler(&self) -> String {
        format!(
            r#"
pub async fn handle_diagnostics(
    uri: lsp_types::Url,
    text: &str,
) -> Result<Vec<lsp_types::Diagnostic>> {{
    let mut diagnostics = Vec::new();
    
    // Parse the text
    match {}::parse(text) {{
        Ok(_ast) => {{
            // No syntax errors
        }}
        Err(errors) => {{
            for error in errors {{
                diagnostics.push(lsp_types::Diagnostic {{
                    range: lsp_types::Range {{
                        start: offset_to_position(text, error.start),
                        end: offset_to_position(text, error.end),
                    }},
                    severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                    code: None,
                    code_description: None,
                    source: Some("adze".to_string()),
                    message: error.message,
                    related_information: None,
                    tags: None,
                    data: None,
                }});
            }}
        }}
    }}
    
    Ok(diagnostics)
}}

fn offset_to_position(text: &str, offset: usize) -> lsp_types::Position {{
    let mut line = 0;
    let mut character = 0;
    
    for (i, ch) in text.char_indices() {{
        if i >= offset {{
            break;
        }}
        if ch == '\n' {{
            line += 1;
            character = 0;
        }} else {{
            character += 1;
        }}
    }}
    
    lsp_types::Position {{ line, character }}
}}"#,
            self.grammar_name
        )
    }

    fn required_imports(&self) -> Vec<String> {
        vec!["use lsp_types::{Diagnostic, DiagnosticSeverity, Range, Position, Url};".to_string()]
    }

    fn capabilities(&self) -> serde_json::Value {
        serde_json::json!({
            "textDocumentSync": {
                "openClose": true,
                "change": 1,  // Full document sync
                "save": true
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::builder::GrammarBuilder;
    use anyhow::Result;
    use lsp_types::{
        HoverParams, Position, TextDocumentIdentifier, TextDocumentPositionParams, Url,
    };
    use std::collections::HashMap;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn test_get_word_at_position(params: &HoverParams) -> Result<String> {
        use anyhow::anyhow;
        use std::fs;
        let uri = &params.text_document_position_params.text_document.uri;
        let path = uri.to_file_path().map_err(|_| anyhow!("invalid uri"))?;
        let text = fs::read_to_string(path)?;
        let position = params.text_document_position_params.position;
        let line = text
            .lines()
            .nth(position.line as usize)
            .ok_or_else(|| anyhow!("line out of bounds"))?;
        let chars: Vec<char> = line.chars().collect();
        let mut start = position.character as usize;
        let mut end = start;
        while start > 0 {
            let c = chars[start - 1];
            if c.is_alphanumeric() || c == '_' {
                start -= 1;
            } else {
                break;
            }
        }
        while end < chars.len() {
            let c = chars[end];
            if c.is_alphanumeric() || c == '_' {
                end += 1;
            } else {
                break;
            }
        }
        Ok(chars[start..end].iter().collect())
    }

    fn test_lookup_documentation(word: &str, docs: &HashMap<String, String>) -> Option<String> {
        docs.get(word).cloned()
    }

    #[test]
    fn extracts_word_under_cursor() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "let sample_word = 1;").unwrap();

        let uri = Url::from_file_path(file.path()).unwrap();
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 0,
                    character: 6,
                },
            },
            work_done_progress_params: Default::default(),
        };

        let word = test_get_word_at_position(&params).unwrap();
        assert_eq!(word, "sample_word");
    }

    #[test]
    fn given_cursor_on_whitespace_when_extracting_word_then_previous_identifier_is_selected() {
        // Given
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "let value = 1;").unwrap();
        let uri = Url::from_file_path(file.path()).unwrap();
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 0,
                    character: 3,
                },
            },
            work_done_progress_params: Default::default(),
        };

        // When
        let word = test_get_word_at_position(&params).unwrap();

        // Then
        assert_eq!(word, "let");
    }

    #[test]
    fn given_utf8_identifier_when_extracting_word_then_full_identifier_is_returned() {
        // Given
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "let café_name = 1;").unwrap();
        let uri = Url::from_file_path(file.path()).unwrap();
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 0,
                    character: 7,
                },
            },
            work_done_progress_params: Default::default(),
        };

        // When
        let word = test_get_word_at_position(&params).unwrap();

        // Then
        assert_eq!(word, "café_name");
    }

    #[test]
    fn finds_documentation_for_word() {
        let mut docs = HashMap::new();
        docs.insert(
            "sample_word".to_string(),
            "Sample documentation".to_string(),
        );

        assert_eq!(
            test_lookup_documentation("sample_word", &docs),
            Some("Sample documentation".to_string())
        );
        assert_eq!(test_lookup_documentation("missing", &docs), None);
    }

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
        assert_eq!(handler.matches("label: \"if\".to_string()").count(), 1);
        assert!(handler.contains("let mut items = create_keyword_completions();"));
        assert!(handler.contains("items.extend(create_symbol_completions());"));
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

    #[test]
    fn given_grammar_name_when_generating_diagnostics_handler_then_parse_uses_that_grammar() {
        // Given
        let grammar = GrammarBuilder::new("mini_parser")
            .token("IDENT", "[a-zA-Z_][a-zA-Z0-9_]*")
            .rule("stmt", vec!["IDENT"])
            .start("stmt")
            .build();
        let provider = DiagnosticsProvider::new(&grammar);

        // When
        let handler = provider.generate_handler();
        let imports = provider.required_imports();
        let capabilities = provider.capabilities();

        // Then
        assert!(handler.contains("match mini_parser::parse(text)"));
        assert!(handler.contains("fn offset_to_position"));
        assert!(
            imports
                .iter()
                .any(|i| i.contains("DiagnosticSeverity") && i.contains("Url"))
        );
        assert_eq!(
            capabilities["textDocumentSync"]["change"],
            serde_json::json!(1)
        );
        assert_eq!(
            capabilities["textDocumentSync"]["openClose"],
            serde_json::json!(true)
        );
    }
}
