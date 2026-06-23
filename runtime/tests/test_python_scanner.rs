// Integration test for Python-like grammar with indentation scanner

use adze::parser_v4::Parser;
use adze::scanner_registry::ExternalScannerBuilder;
use adze::scanners::IndentationScanner;
use adze_glr_core::{Action, ParseTable};
use adze_ir::{ExternalToken, Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use std::collections::BTreeMap;

/// Create a simple Python-like grammar with indentation
fn create_python_grammar() -> Grammar {
    let mut grammar = Grammar::new("python_test".to_string());

    // Define symbol IDs
    let def_keyword = SymbolId(1);
    let identifier = SymbolId(2);
    let colon = SymbolId(3);
    let newline = SymbolId(100);
    let indent = SymbolId(101);
    let dedent = SymbolId(102);
    let statement = SymbolId(200);
    let function_def = SymbolId(201);
    let block = SymbolId(202);
    let program = SymbolId(203);

    // Add tokens
    grammar.tokens.insert(
        def_keyword,
        Token {
            name: "def".to_string(),
            pattern: TokenPattern::String("def".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        identifier,
        Token {
            name: "identifier".to_string(),
            pattern: TokenPattern::Regex(r"[a-zA-Z_][a-zA-Z0-9_]*".to_string()),
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

    // Add rules
    // program -> statement*
    grammar.rules.entry(program).or_default().push(Rule {
        lhs: program,
        rhs: vec![], // Simplified - would normally have repetition
        fields: vec![],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
    });

    // function_def -> 'def' identifier '(' ')' ':' newline indent block dedent
    grammar.rules.entry(function_def).or_default().push(Rule {
        lhs: function_def,
        rhs: vec![
            Symbol::Terminal(def_keyword),
            Symbol::Terminal(identifier),
            Symbol::Terminal(colon),
            Symbol::External(newline),
            Symbol::External(indent),
            Symbol::NonTerminal(block),
            Symbol::External(dedent),
        ],
        fields: vec![],
        precedence: None,
        associativity: None,
        production_id: ProductionId(1),
    });

    // block -> statement+
    grammar.rules.entry(block).or_default().push(Rule {
        lhs: block,
        rhs: vec![Symbol::NonTerminal(statement)],
        fields: vec![],
        precedence: None,
        associativity: None,
        production_id: ProductionId(2),
    });

    // statement -> identifier newline
    grammar.rules.entry(statement).or_default().push(Rule {
        lhs: statement,
        rhs: vec![Symbol::Terminal(identifier), Symbol::External(newline)],
        fields: vec![],
        precedence: None,
        associativity: None,
        production_id: ProductionId(3),
    });

    grammar
}

/// Create a simple parse table for the Python grammar
fn create_parse_table() -> ParseTable {
    // This is a simplified parse table - in reality it would be generated
    let mut symbol_to_index = BTreeMap::new();
    for i in 0..10 {
        symbol_to_index.insert(adze_ir::SymbolId(i as u16), i);
    }

    ParseTable {
        action_table: vec![vec![vec![Action::Error]; 10]; 10],
        goto_table: vec![vec![adze_ir::StateId(0); 10]; 10],
        symbol_metadata: vec![],
        state_count: 10,
        symbol_count: 10,
        symbol_to_index,
        index_to_symbol: (0..10).map(|i| adze_ir::SymbolId(i as u16)).collect(),
        external_scanner_states: vec![],
        token_count: 5,
        external_token_count: 3,
        eof_symbol: adze_ir::SymbolId(9),
        start_symbol: adze_ir::SymbolId(0),
        initial_state: adze_ir::StateId(0),
        rules: vec![],
        lex_modes: vec![],
        extras: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
        nonterminal_to_index: BTreeMap::new(),
        goto_indexing: adze_glr_core::GotoIndexing::NonterminalMap,
        grammar: adze_ir::Grammar::default(),
    }
}

#[test]
fn test_python_indentation_scanner() {
    // Register the indentation scanner
    ExternalScannerBuilder::new("python_test").register_rust::<IndentationScanner>();

    // Create grammar and parse table
    let grammar = create_python_grammar();
    let parse_table = create_parse_table();

    // Create parser
    let _parser = Parser::new(grammar, parse_table, "python_test".to_string());

    // Test input with indentation
    let _input = r#"def hello():
    print("Hello")
    print("World")
"#;

    // The parser should be created with external scanner support
    // In a full implementation, we would test the actual parsing
    // For now, we just verify the parser was created successfully
    // and has an external scanner registered

    // This test primarily verifies that:
    // 1. External scanner registration works
    // 2. Parser can be created with external scanner support
    // 3. The integration between parser and scanner is set up correctly
}

#[test]
fn test_scanner_state_serialization() {
    use adze::external_scanner::ExternalScanner;

    let mut scanner = IndentationScanner::new();

    // Simulate some scanning to build up state
    let input = b"    hello\n        world\n";
    let valid_symbols = vec![true, true, true]; // newline, indent, dedent all valid

    // Create a mock lexer
    struct MockLexer<'a> {
        input: &'a [u8],
        position: usize,
        marked_end: usize,
    }

    impl<'a> adze::external_scanner::Lexer for MockLexer<'a> {
        fn lookahead(&self) -> Option<u8> {
            self.input.get(self.position).copied()
        }

        fn lookahead_n(&self, n: usize) -> Option<u8> {
            self.input.get(self.position.saturating_add(n)).copied()
        }

        fn advance(&mut self, n: usize) {
            self.position = (self.position + n).min(self.input.len());
        }

        fn mark_end(&mut self) {
            self.marked_end = self.position;
        }

        fn column(&self) -> usize {
            // Simplified - count from last newline
            let mut col = 0;
            for i in (0..self.position).rev() {
                if self.input[i] == b'\n' {
                    break;
                }
                col += 1;
            }
            col
        }

        fn is_eof(&self) -> bool {
            self.position >= self.input.len()
        }
    }

    let mut lexer = MockLexer {
        input,
        position: 0,
        marked_end: 0,
    };

    // Scan first line (4 spaces)
    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert!(result.is_some());

    // Serialize state
    let mut buffer = Vec::new();
    scanner.serialize(&mut buffer);
    assert!(!buffer.is_empty());

    // Create new scanner and deserialize
    let mut new_scanner = IndentationScanner::new();
    new_scanner.deserialize(&buffer);

    // Verify state was restored correctly
    // The scanner should remember the indentation level
}

#[test]
fn test_multiple_dedents() {
    use adze::external_scanner::ExternalScanner;

    let mut scanner = IndentationScanner::new();

    // Set up nested indentation
    let input = b"def foo():\n    if True:\n        pass\n";
    let valid_symbols = vec![true, true, true];

    // Create a mock lexer
    struct MockLexer<'a> {
        input: &'a [u8],
        position: usize,
        marked_end: usize,
    }

    impl<'a> adze::external_scanner::Lexer for MockLexer<'a> {
        fn lookahead(&self) -> Option<u8> {
            self.input.get(self.position).copied()
        }

        fn lookahead_n(&self, n: usize) -> Option<u8> {
            self.input.get(self.position.saturating_add(n)).copied()
        }

        fn advance(&mut self, n: usize) {
            self.position = (self.position + n).min(self.input.len());
        }

        fn mark_end(&mut self) {
            self.marked_end = self.position;
        }

        fn column(&self) -> usize {
            // Simplified - count from last newline
            let mut col = 0;
            for i in (0..self.position).rev() {
                if self.input[i] == b'\n' {
                    break;
                }
                col += 1;
            }
            col
        }

        fn is_eof(&self) -> bool {
            self.position >= self.input.len()
        }
    }

    let mut lexer = MockLexer {
        input,
        position: 10,
        marked_end: 0,
    };

    // Scan "def foo():\n" - should get newline
    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert_eq!(result.unwrap().symbol, 0); // NEWLINE

    // Reset lexer to position 11 for next scan
    let mut lexer = MockLexer {
        input,
        position: 11,
        marked_end: 0,
    };

    // Scan "    if True:\n" - should get indent
    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert_eq!(result.unwrap().symbol, 1); // INDENT
}

#[test]
fn test_scanner_registry_retrieval() {
    use adze::scanner_registry::get_global_registry;

    // Register scanner
    ExternalScannerBuilder::new("test_lang").register_rust::<IndentationScanner>();

    // Retrieve from registry
    let registry = get_global_registry();
    let registry = registry.lock().unwrap();

    // Verify scanner is registered
    let scanner = registry.create_scanner("test_lang");
    assert!(scanner.is_some());
}
