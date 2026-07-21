// Pure-Rust parser builder that uses the new IR and GLR infrastructure
//! Pure-Rust parser builder bypassing C code generation.

// This module replaces the old Tree-sitter C generation with pure Rust code

use crate::grammar_js::{GrammarJsConverter, parse_grammar_js_v2};
use adze_glr_core::Action;
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, TokenPattern};
use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::Path;

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

#[cfg(not(debug_assertions))]
fn open_builder_debug_file(_: &str) -> Option<fs::File> {
    None
}

#[cfg(debug_assertions)]
fn open_builder_debug_file(grammar_name: &str) -> Option<fs::File> {
    fs::File::create(std::env::temp_dir().join(format!("adze_debug_{}.log", grammar_name))).ok()
}

#[cfg(not(debug_assertions))]
macro_rules! debug_file_writeln {
    ($dst:expr, $($arg:tt)*) => {};
}

#[cfg(debug_assertions)]
macro_rules! debug_file_writeln {
    ($dst:expr, $($arg:tt)*) => {
        if let Some(file) = $dst.as_mut() {
            let _ = writeln!(file, $($arg)*);
        }
    };
}

mod build_pipeline;

/// Options for building a parser
#[derive(Debug, Clone)]
pub struct BuildOptions {
    /// Output directory for generated files
    pub out_dir: String,
    /// Whether to emit debug artifacts
    pub emit_artifacts: bool,
    /// Whether to generate compressed tables
    pub compress_tables: bool,
}

impl Default for BuildOptions {
    fn default() -> Self {
        BuildOptions {
            out_dir: std::env::var("OUT_DIR").unwrap_or_else(|_| "target/debug".to_string()),
            emit_artifacts: std::env::var("ADZE_EMIT_ARTIFACTS")
                .map(|s| s.parse().unwrap_or(false))
                .unwrap_or(false),
            compress_tables: true,
        }
    }
}

/// Result of building a parser
#[derive(Debug)]
pub struct BuildResult {
    /// Name of the grammar
    pub grammar_name: String,
    /// Path to generated parser module
    pub parser_path: String,
    /// Generated parser code
    pub parser_code: String,
    /// Generated NODE_TYPES.json content
    pub node_types_json: String,
    /// Parser construction metrics for diagnostics and CLI consumers
    pub build_stats: BuildStats,
}

#[derive(Clone, Debug)]
pub struct BuildStats {
    /// Number of states in the generated parse table
    pub state_count: usize,
    /// Number of symbols in the generated parse table
    pub symbol_count: usize,
    /// Number of cells in the action table with multiple possible actions
    pub conflict_cells: usize,
}

fn compute_build_stats(parse_table: &adze_glr_core::ParseTable) -> BuildStats {
    let mut conflict_cells = 0;

    for state_actions in &parse_table.action_table {
        for action_cell in state_actions {
            let is_conflict = action_cell
                .iter()
                .any(|action| matches!(action, Action::Fork(_)))
                || action_cell.len() > 1;
            if is_conflict {
                conflict_cells += 1;
            }
        }
    }

    BuildStats {
        state_count: parse_table.state_count,
        symbol_count: parse_table.symbol_count,
        conflict_cells,
    }
}

/// Allocate a valid ProductionId safely
fn alloc_production_id(grammar: &Grammar) -> Result<ProductionId> {
    let max = grammar
        .rules
        .values()
        .flat_map(|rs| rs.iter().map(|r| r.production_id.0))
        .max()
        .unwrap_or(0);
    let next = max
        .checked_add(1)
        .context("too many productions (u16 overflow)")?;
    Ok(ProductionId(next))
}

/// Ensures every wrapper non-terminal that directly produces a pattern has an explicit unit rule N -> T.
/// This guarantees LR items expose terminal lookaheads, enabling token shifts from initial states.
///
/// A wrapper is any non-terminal N that:
/// 1. Has no rules at all (empty wrapper)
/// 2. Has unit rules (RHS length == 1) that need desugaring
fn desugar_pattern_wrappers(grammar: &mut Grammar) -> Result<()> {
    // Track non-terminals that need unit rules to tokens
    let mut wrappers_needing_rules = Vec::new();

    // First pass: honor explicit wrapper-to-token relations.
    for (wrapper_id, token_id) in &grammar.wrapper_token_relations {
        let has_rules = grammar
            .rules
            .get(wrapper_id)
            .map(|rules| !rules.is_empty())
            .unwrap_or(false);
        if !has_rules {
            wrappers_needing_rules.push((*wrapper_id, *token_id));
        }
    }

    // First pass: Find non-terminals with no rules at all
    let all_nonterminals: Vec<SymbolId> = grammar
        .rule_names
        .keys()
        .filter(|id| !grammar.tokens.contains_key(*id))
        .copied()
        .collect();

    for nt_id in all_nonterminals {
        if grammar.wrapper_token_relations.contains_key(&nt_id) {
            continue;
        }

        let has_rules = grammar
            .rules
            .get(&nt_id)
            .map(|rules| !rules.is_empty())
            .unwrap_or(false);

        if !has_rules {
            // This non-terminal has no rules - it's likely a wrapper for a pattern
            // Try to find a matching token structurally

            let mut matched_token = None;

            if let Some(nt_name) = grammar.rule_names.get(&nt_id) {
                // Heuristic 1: Look for a token with the exact same name or related name
                // e.g., NT "Identifier" -> Token "identifier" or "Identifier_token"
                let nt_name_lower = nt_name.to_lowercase();

                // Collect candidate tokens
                for (tid, token) in &grammar.tokens {
                    let token_name_lower = token.name.to_lowercase();

                    // Direct match or close variant
                    if token.name == *nt_name
                        || token_name_lower == nt_name_lower
                        || token_name_lower.contains(&nt_name_lower)
                        || nt_name_lower.contains(&token_name_lower)
                    {
                        matched_token = Some(*tid);
                        break;
                    }

                    // Check for generated name pattern from GrammarJsConverter (_{SymbolId})
                    if token.name == format!("_{}", nt_id.0) {
                        matched_token = Some(*tid);
                        break;
                    }
                }
            }

            // Heuristic 2 (Legacy fallback): If the name contains "Number", look for a number token
            if matched_token.is_none()
                && let Some(nt_name) = grammar.rule_names.get(&nt_id)
                && nt_name.to_lowercase().contains("number")
            {
                // Find a number token (one with \d pattern)
                for (tid, token) in &grammar.tokens {
                    if let TokenPattern::Regex(r) = &token.pattern
                        && (r.contains(r"\d") || r.contains("[0-9]"))
                    {
                        matched_token = Some(*tid);
                        break;
                    }
                }
            }

            if let Some(tid) = matched_token {
                wrappers_needing_rules.push((nt_id, tid));
            }
        }
    }

    // Second pass: Look for existing unit rules that might need desugaring
    // (This handles cases where the wrapper has a rule but it's to an inline pattern)
    let mut rules_to_add = Vec::new();
    for (_, rules) in &grammar.rules {
        for rule in rules {
            if rule.rhs.len() == 1 {
                // This is a unit rule
                match &rule.rhs[0] {
                    Symbol::Terminal(_) => {
                        // Already a terminal unit rule, good
                    }
                    Symbol::NonTerminal(_) => {
                        // Unit rule to another non-terminal, leave it alone
                    }
                    // Handle other symbol types that might represent inline patterns
                    _ => {
                        // For now, we don't handle these - would need to create tokens for patterns
                    }
                }
            }
        }
    }

    // Add unit rules for all wrappers that need them
    for (nt_id, token_id) in wrappers_needing_rules {
        // Check if this exact unit rule already exists to avoid duplicates
        let already_exists = grammar
            .rules
            .get(&nt_id)
            .map(|existing_rules| {
                existing_rules.iter().any(|r| {
                    r.rhs.len() == 1
                        && matches!(&r.rhs[0], Symbol::Terminal(tid) if *tid == token_id)
                })
            })
            .unwrap_or(false);

        if !already_exists {
            let production_id = alloc_production_id(grammar)?;
            let unit_rule = Rule {
                lhs: nt_id,
                rhs: vec![Symbol::Terminal(token_id)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id,
            };
            grammar.add_rule(unit_rule);
            rules_to_add.push((nt_id, token_id));
        }
    }

    // Rebuild symbol registry after changes
    let _ = grammar.get_or_build_registry();

    if !rules_to_add.is_empty() {
        debug_trace!(
            "Desugaring: Added {} unit rules for pattern wrappers",
            rules_to_add.len()
        );
        for (nt, tok) in rules_to_add {
            debug_trace!("  {} -> Terminal({})", nt.0, tok.0);
        }
    }

    Ok(())
}

/// Build a parser from a grammar.js file
pub fn build_parser_from_grammar_js(
    grammar_js_path: &Path,
    options: BuildOptions,
) -> Result<BuildResult> {
    // Read and parse grammar.js
    let grammar_js_content = fs::read_to_string(grammar_js_path)
        .with_context(|| format!("Failed to read grammar.js file at {:?}", grammar_js_path))?;

    // Try v3 parser first, fall back to v2 if needed
    let grammar_js = {
        let mut parser_v3 = crate::grammar_js::GrammarJsParserV3::new(grammar_js_content.clone());
        match parser_v3.parse() {
            Ok(g) => g,
            Err(_) => {
                // Fall back to v2 parser
                parse_grammar_js_v2(&grammar_js_content).context("Failed to parse grammar.js")?
            }
        }
    };

    // Parse grammar.js successfully

    // Convert to IR
    let converter = GrammarJsConverter::new(grammar_js);
    let grammar = converter
        .convert()
        .context("Failed to convert grammar.js to IR")?;

    // Grammar converted successfully

    // TODO: Re-enable optimization after fixing unit rule elimination.
    //
    // The grammar.js path produces pattern-wrapper unit productions such as
    // `source -> item` and `item -> TOKEN`. The current optimizer can remove
    // those unit productions without preserving an equivalent start rule,
    // leaving a grammar with tokens but no rules under `--all-features`.
    // Keep this aligned with `build_parser_from_json` until that optimizer
    // behavior is fixed.

    // Build the parser
    build_parser(grammar, options)
}

/// Build a parser for all grammars in a crate
pub fn build_parser_for_crate(root_file: &Path, options: BuildOptions) -> Result<Vec<BuildResult>> {
    let mut results = Vec::new();

    // Find all grammar definitions
    let grammars = crate::generate_grammars(root_file)?;

    // Debug: write to file
    if cfg!(debug_assertions)
        && let Ok(mut f) = std::fs::File::create("/tmp/adze_grammars.txt")
    {
        writeln!(
            f,
            "Found {} grammars from {}",
            grammars.len(),
            root_file.display()
        )
        .ok();
    }

    for grammar_json in grammars {
        // Convert serde_json::Value to string
        let grammar_json_str = serde_json::to_string(&grammar_json).unwrap();
        let result = build_parser_from_json(grammar_json_str, options.clone())?;
        results.push(result);
    }

    Ok(results)
}

/// Build a parser from a JSON grammar (Tree-sitter format)
pub fn build_parser_from_json(grammar_json: String, options: BuildOptions) -> Result<BuildResult> {
    // Parse the JSON string
    let grammar_value: Value =
        serde_json::from_str(&grammar_json).context("Failed to parse grammar JSON")?;

    // Extract grammar name from JSON
    let grammar_name = grammar_value
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    // Debug: Print the grammar JSON to understand the extras
    if grammar_name.contains("arithmetic") {
        debug_trace!("DEBUG: Arithmetic grammar JSON:");
        debug_trace!("{}", serde_json::to_string_pretty(&grammar_value).unwrap());
    }

    // Convert directly from JSON to GrammarJs structure
    let grammar_js = crate::grammar_js::from_json(&grammar_value)
        .context("Failed to convert JSON to GrammarJs")?;

    let converter = GrammarJsConverter::new(grammar_js);
    let grammar = converter
        .convert()
        .context("Failed to convert grammar JSON to IR")?;

    // Grammar converted from JSON

    // Optimize the grammar
    // TODO: Re-enable optimization after fixing unit rule elimination
    // #[cfg(not(feature = "no_opt"))]
    // {
    //     grammar = optimize_grammar(grammar).context("Failed to optimize grammar")?;
    // }

    // Grammar optimized successfully

    // Build the parser
    build_parser(grammar, options)
}

/// Build a parser from an IR Grammar
pub fn build_parser(grammar: Grammar, options: BuildOptions) -> Result<BuildResult> {
    build_pipeline::BuildPipeline::new(grammar, options).run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::{FieldId, Grammar, Symbol, SymbolId, Token, TokenPattern};
    use tempfile::TempDir;

    #[test]
    fn test_build_simple_parser() {
        let grammar_js = r#"
module.exports = grammar({
  name: 'test',
  
  rules: {
    source_file: $ => $.expression,
    expression: $ => /\d+/
  }
});
        "#;

        let temp_dir = TempDir::new().unwrap();
        let grammar_path = temp_dir.path().join("grammar.js");
        fs::write(&grammar_path, grammar_js).unwrap();

        let options = BuildOptions {
            out_dir: temp_dir.path().to_string_lossy().to_string(),
            emit_artifacts: true,
            compress_tables: false,
        };

        let result = build_parser_from_grammar_js(&grammar_path, options).unwrap();
        assert_eq!(result.grammar_name, "test");

        // Check that files were created
        let parser_path = Path::new(&result.parser_path);
        assert!(parser_path.exists());

        // Check NODE_TYPES content
        let node_types: Value = serde_json::from_str(&result.node_types_json).unwrap();
        assert!(node_types.is_array());
    }

    #[test]
    fn build_parser_emits_typed_cst_syntax_module() {
        let mut grammar = Grammar::new("typed_cst_arithmetic".to_string());

        let number = SymbolId(0);
        let minus = SymbolId(1);
        let source_file = SymbolId(2);
        let expression = SymbolId(3);

        grammar.tokens.insert(
            number,
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );
        grammar.tokens.insert(
            minus,
            Token {
                name: "minus".to_string(),
                pattern: TokenPattern::String("-".to_string()),
                fragile: false,
            },
        );

        grammar
            .rule_names
            .insert(source_file, "source_file".to_string());
        grammar
            .rule_names
            .insert(expression, "expression".to_string());
        grammar.fields.insert(FieldId(0), "left".to_string());
        grammar.fields.insert(FieldId(1), "operator".to_string());
        grammar.fields.insert(FieldId(2), "right".to_string());

        grammar.add_rule(Rule {
            lhs: source_file,
            rhs: vec![Symbol::NonTerminal(expression)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });
        grammar.add_rule(Rule {
            lhs: expression,
            rhs: vec![Symbol::Terminal(number)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(1),
        });
        grammar.add_rule(Rule {
            lhs: expression,
            rhs: vec![
                Symbol::Terminal(number),
                Symbol::Terminal(minus),
                Symbol::Terminal(number),
            ],
            precedence: None,
            associativity: None,
            fields: vec![(FieldId(0), 0), (FieldId(1), 1), (FieldId(2), 2)],
            production_id: ProductionId(2),
        });

        let temp_dir = TempDir::new().unwrap();
        let result = build_parser(
            grammar,
            BuildOptions {
                out_dir: temp_dir.path().to_string_lossy().to_string(),
                emit_artifacts: false,
                compress_tables: false,
            },
        )
        .unwrap();

        syn::parse_str::<syn::File>(&result.parser_code)
            .expect("generated parser code should remain valid Rust syntax");

        assert!(result.parser_code.contains("pub mod syntax"));
        assert!(result.parser_code.contains("pub fn parse_document"));
        assert!(
            result
                .parser_code
                .contains(":: adze :: __private :: parse_document")
        );
        assert!(
            result
                .parser_code
                .contains(":: adze :: document :: AdzeDocument")
        );
        assert!(result.parser_code.contains("pub struct SourceFile"));
        assert!(result.parser_code.contains("pub struct Expression"));
        assert!(result.parser_code.contains("pub struct MinusToken"));
        assert!(result.parser_code.contains("pub struct NumberToken"));
        assert!(result.parser_code.contains("edge_by_field_name (\"left\")"));
        assert!(
            result
                .parser_code
                .contains("edge_by_field_name (\"operator\")")
        );
        assert!(
            result
                .parser_code
                .contains("edge_by_field_name (\"right\")")
        );
        assert!(result.parser_code.contains("NumberToken :: cast"));
        assert!(result.parser_code.contains("MinusToken :: cast"));

        let written = fs::read_to_string(&result.parser_path).unwrap();
        assert!(written.contains("pub mod syntax"));
        assert!(written.contains("pub fn parse_document"));
        assert!(written.contains("edge_by_field_name(\"left\")"));
    }

    #[test]
    fn test_desugar_pattern_wrappers_reproduction() {
        // Reproduce the state where a non-terminal has no rules but matches a token
        let mut grammar = Grammar::new("test_grammar".to_string());

        // NT: "Number" (SymbolId 0)
        let nt_id = SymbolId(0);
        grammar.rule_names.insert(nt_id, "Number".to_string());

        // Token: /[0-9]+/ (SymbolId 1)
        let token_id = SymbolId(1);
        let token = Token {
            name: "Number_token".to_string(), // Named differently than NT
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        };
        grammar.tokens.insert(token_id, token);

        // We do NOT add any rules for nt_id.
        // This simulates the "no rules" condition.

        // Ensure registry is built
        grammar.get_or_build_registry();

        // Run desugar_pattern_wrappers
        let result = desugar_pattern_wrappers(&mut grammar);
        assert!(result.is_ok());

        // Check if a rule was added: Number -> Number_token
        let rules = grammar.rules.get(&nt_id);
        assert!(rules.is_some(), "Should have added rules for Number");
        let rules = rules.unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].rhs.len(), 1);
        match &rules[0].rhs[0] {
            Symbol::Terminal(tid) => assert_eq!(*tid, token_id),
            _ => panic!("Expected rule to produce terminal"),
        }
    }

    #[test]
    fn test_desugar_pattern_wrappers_structural() {
        // Test that it works even if name is NOT "Number", but structural match exists
        let mut grammar = Grammar::new("test_grammar".to_string());

        // NT: "Identifier" (SymbolId 0)
        let nt_id = SymbolId(0);
        grammar.rule_names.insert(nt_id, "Identifier".to_string());

        // Token: /[a-z]+/ (SymbolId 1)
        let token_id = SymbolId(1);
        let token = Token {
            name: "Identifier_token".to_string(),
            pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
            fragile: false,
        };
        grammar.tokens.insert(token_id, token);

        // No rules for NT.

        grammar.get_or_build_registry();

        // Run desugar_pattern_wrappers
        let result = desugar_pattern_wrappers(&mut grammar);
        assert!(result.is_ok());

        // Check if a rule was added: Identifier -> Identifier_token
        // With structural matching, this should now succeed
        let rules = grammar.rules.get(&nt_id);
        assert!(
            rules.is_some(),
            "Should have added rules for Identifier using structural matching"
        );
        let rules = rules.unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].rhs.len(), 1);
        match &rules[0].rhs[0] {
            Symbol::Terminal(tid) => assert_eq!(*tid, token_id),
            _ => panic!("Expected rule to produce terminal"),
        }
    }
}
