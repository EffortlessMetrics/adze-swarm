use adze_ir::Grammar;

use super::LspFeature;

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
    use crate::features::LspFeature;
    use adze_ir::builder::GrammarBuilder;

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
