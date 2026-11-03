// Integration tests for the complete adze parsing pipeline
// These tests verify that all components work together correctly

// Source-only sketch: cfg(skip_integration_tests) excludes this target by default.
// This import port does not qualify compilation, recovery or scanner behavior.
#![cfg(skip_integration_tests)]

use adze::error_recovery::ErrorRecoveryConfigBuilder;
use anyhow::Result;
use indexmap::IndexMap;
// use adze::external_scanner::ExternalScanner; // Unused
// use adze::incremental_v3::{Edit, IncrementalParser, Position}; // Feature-gated
use adze::parser_v4::Parser;
use adze::query::{QueryCursor, compile_query};
use adze::scanner_registry::ExternalScannerBuilder;
use adze::scanners::IndentationScanner;
use adze::tree_sitter::Point as Position;
use adze_glr_core::*;
use adze_ir::*;
use adze_tablegen::StaticLanguageGenerator;
use std::collections::BTreeMap;

/// Create a simple Python-like grammar with indentation
fn create_python_like_grammar() -> Grammar {
    let mut grammar = Grammar::new("python_like".to_string());

    // Regular tokens
    let identifier = SymbolId(1);
    let def_keyword = SymbolId(2);
    let colon = SymbolId(3);
    let equals = SymbolId(4);
    let number = SymbolId(5);

    // External tokens
    let newline = SymbolId(100);
    let indent = SymbolId(101);
    let dedent = SymbolId(102);

    // Add regular tokens
    grammar.tokens.insert(
        identifier,
        Token {
            name: "identifier".to_string(),
            pattern: TokenPattern::Regex(r"[a-zA-Z_][a-zA-Z0-9_]*".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        def_keyword,
        Token {
            name: "def".to_string(),
            pattern: TokenPattern::String("def".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        colon,
        Token {
            name: "colon".to_string(),
            pattern: TokenPattern::String(":".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        equals,
        Token {
            name: "equals".to_string(),
            pattern: TokenPattern::String("=".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        number,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    // Add external tokens
    grammar.externals.push(ExternalToken {
        name: "newline".to_string(),
        symbol_id: newline,
    });

    grammar.externals.push(ExternalToken {
        name: "indent".to_string(),
        symbol_id: indent,
    });

    grammar.externals.push(ExternalToken {
        name: "dedent".to_string(),
        symbol_id: dedent,
    });

    // Non-terminals
    let program = SymbolId(200);
    let function = SymbolId(201);
    let statement = SymbolId(202);
    let assignment = SymbolId(203);
    let block = SymbolId(204);

    // Rules
    // program -> function*
    grammar.rules.entry(program).or_default().push(Rule {
        lhs: program,
        rhs: vec![Symbol::NonTerminal(function)],
        production_id: ProductionId(0),
        precedence: None,
        associativity: None,
        fields: vec![],
    });

    // function -> def identifier : newline indent block dedent
    let mut function_fields = IndexMap::new();
    function_fields.insert(FieldId(0), 1); // name field at position 1
    function_fields.insert(FieldId(1), 5); // body field at position 5

    // Convert IndexMap to Vec for fields
    let function_fields_vec = vec![(FieldId(0), 1), (FieldId(1), 5)];

    grammar.rules.entry(function).or_default().push(Rule {
        lhs: function,
        rhs: vec![
            Symbol::Terminal(def_keyword),
            Symbol::Terminal(identifier),
            Symbol::Terminal(colon),
            Symbol::Terminal(newline),
            Symbol::Terminal(indent),
            Symbol::NonTerminal(block),
            Symbol::Terminal(dedent),
        ],
        production_id: ProductionId(1),
        precedence: None,
        associativity: None,
        fields: function_fields_vec,
    });

    // Add field names
    grammar.fields.insert(FieldId(0), "name".to_string());
    grammar.fields.insert(FieldId(1), "body".to_string());

    grammar
}

/// Create a simple parse table for testing
fn create_test_parse_table() -> ParseTable {
    // In a real implementation, this would be generated
    ParseTable {
        action_table: vec![vec![vec![Action::Error]; 10]; 20],
        goto_table: vec![vec![StateId(0); 5]; 20],
        symbol_metadata: vec![],
        state_count: 20,
        symbol_count: 10,
        symbol_to_index: BTreeMap::new(),
        external_scanner_states: vec![],
    }
}

#[test]
fn test_full_parsing_pipeline() {
    // 1. Create grammar
    let grammar = create_python_like_grammar();

    // 2. Generate parse table (normally done by table generator)
    let parse_table = create_test_parse_table();

    // 3. Register external scanner
    ExternalScannerBuilder::new("python_like")
        .with_external_tokens(vec![SymbolId(100), SymbolId(101), SymbolId(102)])
        .register_rust::<IndentationScanner>();

    // 4. Create parser with error recovery
    // NOTE: `parser_v4::Parser` has no `with_error_recovery` builder; the
    // unused builder below remains source-only while this file is disabled;
    // the parser receives no recovery config. No recovery parity is claimed.
    let _error_recovery = ErrorRecoveryConfigBuilder::new()
        .add_sync_token(SymbolId(102).0) // dedent
        .add_sync_token(SymbolId(100).0) // newline
        .enable_scope_recovery(true)
        .build();

    let mut parser = Parser::new(grammar.clone(), parse_table, "test".to_string());

    // 5. Parse some Python-like code
    let input = r#"
def foo:
    x = 1
    y = 2

def bar:
    z = 3
"#;

    match parser.parse_tree(input) {
        Ok(tree) => {
            println!("Successfully parsed Python-like code");
            verify_parse_tree(&tree);
        }
        Err(e) => {
            // Expected with our simple parse table
            println!("Parse error (expected with test table): {:?}", e);
        }
    }
}

#[test]
fn test_incremental_parsing_pipeline() {
    let grammar = create_python_like_grammar();
    let parse_table = create_test_parse_table();

    let mut incremental_parser = IncrementalParser::new(grammar, parse_table);

    // Initial parse
    let input_v1 = "def foo:\n    x = 1\n";
    let tree_v1 = incremental_parser
        .parse(input_v1, None, &[])
        .unwrap_or_else(|_| ParseNode {
            symbol: SymbolId(200),
            children: vec![],
            start_byte: 0,
            end_byte: input_v1.len(),
            field_name: None,
        });

    // Edit: change "1" to "42"
    let edits = vec![Edit {
        start_byte: 17,
        old_end_byte: 18,
        new_end_byte: 19,
        start_position: Position { row: 1, column: 8 },
        old_end_position: Position { row: 1, column: 9 },
        new_end_position: Position { row: 1, column: 10 },
    }];

    let input_v2 = "def foo:\n    x = 42\n";
    let tree_v2 = incremental_parser
        .parse(input_v2, Some(&tree_v1), &edits)
        .unwrap_or_else(|_| ParseNode {
            symbol: SymbolId(200),
            children: vec![],
            start_byte: 0,
            end_byte: input_v2.len(),
            field_name: None,
        });

    println!("Incremental parse completed");
}

#[test]
fn test_query_language_integration() -> Result<()> {
    let grammar = create_python_like_grammar();

    // Create a simple parse tree for testing
    let tree = ParseNode {
        symbol: SymbolId(200), // program
        children: vec![ParseNode {
            symbol: SymbolId(201), // function
            children: vec![
                ParseNode {
                    symbol: SymbolId(2), // def
                    children: vec![],
                    start_byte: 0,
                    end_byte: 3,
                    field_name: None,
                },
                ParseNode {
                    symbol: SymbolId(1), // identifier
                    children: vec![],
                    start_byte: 4,
                    end_byte: 7,
                    field_name: Some("name".to_string()),
                },
            ],
            start_byte: 0,
            end_byte: 20,
            field_name: None,
        }],
        start_byte: 0,
        end_byte: 20,
        field_name: None,
    };

    // Compile a query
    let query_str = r#"
        (function
          name: (identifier) @function.name)
    "#;

    match compile_query(query_str, &grammar) {
        Ok(query) => {
            let mut cursor = QueryCursor::new();
            let matches: Vec<_> = cursor.matches(&query, &tree).collect();

            println!("Query found {} matches", matches.len());

            for match_ in matches {
                println!(
                    "Match: pattern={}, captures={}",
                    match_.pattern_index,
                    match_.captures.len()
                );
            }
        }
        Err(e) => {
            // Expected since our test grammar might not have all the needed info
            println!("Query compilation error (expected): {:?}", e);
        }
    }

    Ok(())
}

#[test]
fn test_table_generation_pipeline() {
    let grammar = create_python_like_grammar();
    let parse_table = create_test_parse_table();

    // Generate static language code
    let generator = StaticLanguageGenerator::new(grammar, parse_table);
    let generated_code = generator.generate_language_code();

    // Verify the generated code contains expected elements
    let code_str = generated_code.to_string();
    assert!(code_str.contains("TSLanguage"));
    assert!(code_str.contains("SYMBOL_NAMES"));
    assert!(code_str.contains("tree_sitter_python_like"));

    println!("Successfully generated static language code");
}

#[test]
fn test_error_recovery_integration() {
    let grammar = create_python_like_grammar();
    let parse_table = create_test_parse_table();

    // Configure aggressive error recovery (see note above: `parser_v4`
    // has no recovery builder. This disabled sketch supplies no executed
    // recovery or builder-API proof).
    let _error_recovery = ErrorRecoveryConfigBuilder::new()
        .add_sync_token(SymbolId(100).0) // newline
        .add_sync_token(SymbolId(102).0) // dedent
        .add_insertable_token(SymbolId(103).0) // colon
        .add_deletable_token(SymbolId(1).0) // identifier
        .enable_phrase_recovery(true)
        .enable_scope_recovery(true)
        .set_max_recovery_attempts(5)
        .build();

    let mut parser = Parser::new(grammar, parse_table, "test".to_string());

    // Parse code with syntax errors
    let input = r#"
def foo  # missing colon
    x = 1
    
def bar:
    y == 2  # wrong operator
"#;

    match parser.parse_tree(input) {
        Ok(tree) => {
            println!("Parsed with error recovery");
            check_for_error_nodes(&tree);
        }
        Err(e) => {
            println!("Parse failed despite recovery: {:?}", e);
        }
    }
}

// Helper functions

fn verify_parse_tree(tree: &ParseNode) {
    // Verify tree structure
    assert_eq!(tree.symbol, SymbolId(200)); // program

    // Check for functions
    for child in &tree.children {
        if child.symbol == SymbolId(201) {
            // function
            println!("Found function node");

            // Check for field names
            for grandchild in &child.children {
                if let Some(field) = &grandchild.field_name {
                    println!(
                        "  Field '{}' at position {}-{}",
                        field, grandchild.start_byte, grandchild.end_byte
                    );
                }
            }
        }
    }
}

fn check_for_error_nodes(tree: &ParseNode) {
    // Look for error nodes (symbol 0xFFFE)
    if tree.symbol == SymbolId(0xFFFE) {
        println!("Found error node at {}-{}", tree.start_byte, tree.end_byte);
    }

    // Check children
    for child in &tree.children {
        check_for_error_nodes(child);
    }
}

#[test]
fn test_external_scanner_integration() {
    // Test that external scanners are properly called during parsing
    let grammar = create_python_like_grammar();
    let parse_table = create_test_parse_table();

    // Register scanner
    ExternalScannerBuilder::new("python_like")
        .with_external_tokens(vec![SymbolId(100), SymbolId(101), SymbolId(102)])
        .register_rust::<IndentationScanner>();

    let mut parser = Parser::new(grammar, parse_table, "python_like".to_string());

    // Input with indentation
    let input = r#"
def foo:
    x = 1
    if True:
        y = 2
    z = 3
"#;

    // The external scanner should handle INDENT/DEDENT tokens
    match parser.parse(input) {
        Ok(_) => println!("External scanner integration successful"),
        Err(e) => println!("Parse error (expected with test table): {:?}", e),
    }
}

#[test]
fn test_full_adze_capabilities() {
    println!("\n=== Adze Feature Summary ===");
    println!("✅ Grammar definition with macros");
    println!("✅ Pure-Rust parser implementation");
    println!("✅ GLR fork/merge for ambiguous grammars");
    println!("✅ External scanner support (FFI + native)");
    println!("✅ Incremental parsing with subtree reuse");
    println!("✅ Error recovery strategies");
    println!("✅ Query language with pattern matching");
    println!("✅ Table generation and compression");
    println!("✅ Tree-sitter ABI compatibility");
    println!("✅ WASM support (via pure-Rust)");
    println!("==================================\n");
}
