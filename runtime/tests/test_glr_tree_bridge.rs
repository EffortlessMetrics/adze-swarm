// Test GLR tree bridge functionality
use adze::glr_lexer::GLRLexer;
use adze::glr_parser::GLRParser;
use adze::glr_tree_bridge::subtree_to_tree;
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use std::sync::Arc;

fn create_json_grammar() -> Grammar {
    let mut grammar = Grammar::new("json".to_string());

    // Tokens
    let number_id = SymbolId(1);
    let string_id = SymbolId(2);
    let lbrace_id = SymbolId(3);
    let rbrace_id = SymbolId(4);
    let lbracket_id = SymbolId(5);
    let rbracket_id = SymbolId(6);
    let comma_id = SymbolId(7);
    let colon_id = SymbolId(8);

    // Non-terminals
    let value_id = SymbolId(9);
    let object_id = SymbolId(10);
    let array_id = SymbolId(11);
    let members_id = SymbolId(12);
    let member_id = SymbolId(13);
    let elements_id = SymbolId(14);

    // Add tokens
    grammar.tokens.insert(
        number_id,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex(r"-?\d+(\.\d+)?".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        string_id,
        Token {
            name: "string".to_string(),
            pattern: TokenPattern::Regex(r#""[^"]*""#.to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        lbrace_id,
        Token {
            name: "lbrace".to_string(),
            pattern: TokenPattern::String("{".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        rbrace_id,
        Token {
            name: "rbrace".to_string(),
            pattern: TokenPattern::String("}".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        lbracket_id,
        Token {
            name: "lbracket".to_string(),
            pattern: TokenPattern::String("[".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        rbracket_id,
        Token {
            name: "rbracket".to_string(),
            pattern: TokenPattern::String("]".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        comma_id,
        Token {
            name: "comma".to_string(),
            pattern: TokenPattern::String(",".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        colon_id,
        Token {
            name: "colon".to_string(),
            pattern: TokenPattern::String(":".to_string()),
            fragile: false,
        },
    );

    // Add rule names
    grammar.rule_names.insert(value_id, "value".to_string());
    grammar.rule_names.insert(object_id, "object".to_string());
    grammar.rule_names.insert(array_id, "array".to_string());
    grammar.rule_names.insert(members_id, "members".to_string());
    grammar.rule_names.insert(member_id, "member".to_string());
    grammar
        .rule_names
        .insert(elements_id, "elements".to_string());

    // Rules: value → number | string | object | array

    grammar.rules.entry(value_id).or_default().push(Rule {
        lhs: value_id,
        rhs: vec![Symbol::Terminal(number_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    grammar.rules.entry(value_id).or_default().push(Rule {
        lhs: value_id,
        rhs: vec![Symbol::Terminal(string_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });

    grammar.rules.entry(value_id).or_default().push(Rule {
        lhs: value_id,
        rhs: vec![Symbol::NonTerminal(object_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(2),
    });

    grammar.rules.entry(value_id).or_default().push(Rule {
        lhs: value_id,
        rhs: vec![Symbol::NonTerminal(array_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(3),
    });

    // object → { members } | { }
    grammar.rules.entry(object_id).or_default().push(Rule {
        lhs: object_id,
        rhs: vec![
            Symbol::Terminal(lbrace_id),
            Symbol::NonTerminal(members_id),
            Symbol::Terminal(rbrace_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(4),
    });

    grammar.rules.entry(object_id).or_default().push(Rule {
        lhs: object_id,
        rhs: vec![Symbol::Terminal(lbrace_id), Symbol::Terminal(rbrace_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(5),
    });

    // array → [ elements ] | [ ]
    grammar.rules.entry(array_id).or_default().push(Rule {
        lhs: array_id,
        rhs: vec![
            Symbol::Terminal(lbracket_id),
            Symbol::NonTerminal(elements_id),
            Symbol::Terminal(rbracket_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(6),
    });

    grammar.rules.entry(array_id).or_default().push(Rule {
        lhs: array_id,
        rhs: vec![Symbol::Terminal(lbracket_id), Symbol::Terminal(rbracket_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(7),
    });

    // members → member | member , members
    grammar.rules.entry(members_id).or_default().push(Rule {
        lhs: members_id,
        rhs: vec![Symbol::NonTerminal(member_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(8),
    });

    grammar.rules.entry(members_id).or_default().push(Rule {
        lhs: members_id,
        rhs: vec![
            Symbol::NonTerminal(member_id),
            Symbol::Terminal(comma_id),
            Symbol::NonTerminal(members_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(9),
    });

    // member → string : value
    grammar.rules.entry(member_id).or_default().push(Rule {
        lhs: member_id,
        rhs: vec![
            Symbol::Terminal(string_id),
            Symbol::Terminal(colon_id),
            Symbol::NonTerminal(value_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(10),
    });

    // elements → value | value , elements
    grammar.rules.entry(elements_id).or_default().push(Rule {
        lhs: elements_id,
        rhs: vec![Symbol::NonTerminal(value_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(11),
    });

    grammar.rules.entry(elements_id).or_default().push(Rule {
        lhs: elements_id,
        rhs: vec![
            Symbol::NonTerminal(value_id),
            Symbol::Terminal(comma_id),
            Symbol::NonTerminal(elements_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(12),
    });

    grammar.set_start_symbol(value_id);
    grammar
}

#[test]
fn test_tree_bridge_json_number() {
    let grammar = Arc::new(create_json_grammar());
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();

    match build_lr1_automaton(&grammar, &first_follow) {
        Ok(parse_table) => {
            let input = "42";
            let mut parser = GLRParser::new(parse_table, (*grammar).clone());
            let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();

            // Parse input
            while let Some(token) = lexer.next_token() {
                parser.process_token(token.symbol_id, &token.text, token.byte_offset);
            }
            parser.process_eof(input.len());

            // Get parse tree
            let subtree = parser.get_best_parse().expect("Failed to parse");

            // Convert to GLR tree
            let tree = subtree_to_tree(subtree, input.as_bytes().to_vec(), (*grammar).clone());
            let root = tree.root_node();

            // Test tree API
            // The root is the "value" non-terminal (start symbol)
            assert_eq!(root.kind(), "value");
            assert_eq!(root.byte_range(), 0..2);

            // The value contains a single child: the number terminal
            assert_eq!(root.child_count(), 1);
            let number_node = root.child(0).unwrap();
            assert_eq!(number_node.kind(), "number");
            assert_eq!(number_node.utf8_text(tree.text()).unwrap(), "42");
        }
        Err(e) => panic!("Failed to build parse table: {:?}", e),
    }
}

#[test]
fn test_tree_bridge_json_object() {
    let grammar = Arc::new(create_json_grammar());
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();

    match build_lr1_automaton(&grammar, &first_follow) {
        Ok(parse_table) => {
            let input = r#"{"key": 123}"#;
            let mut parser = GLRParser::new(parse_table, (*grammar).clone());
            let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();

            // Parse input
            while let Some(token) = lexer.next_token() {
                parser.process_token(token.symbol_id, &token.text, token.byte_offset);
            }
            parser.process_eof(input.len());

            // Get parse tree
            let subtree = parser.get_best_parse().expect("Failed to parse");

            // Convert to GLR tree
            let tree = subtree_to_tree(subtree, input.as_bytes().to_vec(), (*grammar).clone());
            let root = tree.root_node();

            // Test tree structure
            // The root is the "value" non-terminal (start symbol)
            assert_eq!(root.kind(), "value");

            // The value contains a single child: the object non-terminal
            assert_eq!(root.child_count(), 1);
            let object_node = root.child(0).unwrap();
            assert_eq!(object_node.kind(), "object");

            // Objects have children: { members } or { }
            // In this case: { members }
            assert!(object_node.child_count() >= 2); // At least { and }

            // Use cursor to traverse
            let _cursor = root.walk();

            // Check we can access text
            let text = root.utf8_text(tree.text()).unwrap();
            assert_eq!(text, r#"{"key": 123}"#);
        }
        Err(e) => panic!("Failed to build parse table: {:?}", e),
    }
}

#[test]
fn test_tree_cursor_navigation() {
    let grammar = Arc::new(create_json_grammar());
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();

    match build_lr1_automaton(&grammar, &first_follow) {
        Ok(parse_table) => {
            let input = "[1, 2, 3]";
            let mut parser = GLRParser::new(parse_table, (*grammar).clone());
            let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();

            // Parse input
            while let Some(token) = lexer.next_token() {
                parser.process_token(token.symbol_id, &token.text, token.byte_offset);
            }
            parser.process_eof(input.len());

            // Get parse tree
            let subtree = parser.get_best_parse().expect("Failed to parse");

            // Convert to GLR tree
            let tree = subtree_to_tree(subtree, input.as_bytes().to_vec(), (*grammar).clone());
            let mut cursor = tree.root_node().walk();

            // Navigate tree with cursor
            // The root is the "value" non-terminal
            assert_eq!(cursor.node().kind(), "value");

            // Navigate to the array child
            assert!(cursor.goto_first_child());
            assert_eq!(cursor.node().kind(), "array");

            // Arrays have: [ elements ]
            // Go to first child of array (lbracket)
            assert!(cursor.goto_first_child());
            assert_eq!(cursor.node().kind(), "lbracket");

            // Go to sibling (elements)
            assert!(cursor.goto_next_sibling());

            // Go back to parent
            assert!(cursor.goto_parent());
            assert_eq!(cursor.node().kind(), "array");
        }
        Err(e) => panic!("Failed to build parse table: {:?}", e),
    }
}

#[test]
fn test_node_equality_and_ids() {
    let grammar = Arc::new(create_json_grammar());
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();

    match build_lr1_automaton(&grammar, &first_follow) {
        Ok(parse_table) => {
            let input = "123";
            let mut parser = GLRParser::new(parse_table, (*grammar).clone());
            let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();

            while let Some(token) = lexer.next_token() {
                parser.process_token(token.symbol_id, &token.text, token.byte_offset);
            }
            parser.process_eof(input.len());

            let subtree = parser.get_best_parse().expect("Failed to parse");
            let tree = subtree_to_tree(subtree, input.as_bytes().to_vec(), (*grammar).clone());

            let root1 = tree.root_node();
            let root2 = tree.root_node();

            // Same nodes should be equal
            assert_eq!(root1, root2);
            assert_eq!(root1.id(), root2.id());

            // Different nodes should have different IDs
            if let Some(child) = root1.child(0) {
                assert_ne!(root1.id(), child.id());
            }
        }
        Err(e) => panic!("Failed to build parse table: {:?}", e),
    }
}
