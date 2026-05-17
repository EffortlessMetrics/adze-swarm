use adze_ir::Grammar;

use super::LspFeature;

/// Hover provider for LSP
pub struct HoverProvider {
    #[allow(dead_code)]
    pub documentation: std::collections::HashMap<String, String>,
}

impl HoverProvider {
    pub fn new(grammar: &Grammar) -> Self {
        let mut documentation = std::collections::HashMap::new();

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

#[cfg(test)]
mod tests {
    use super::*;
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
}
