// Query compiler - compiles query strings into Query objects
use super::ast::Query;
use super::parser::QueryParser;
use adze_ir::Grammar;

/// Compile a query string into a Query object
pub fn compile_query(source: &str, grammar: &Grammar) -> Result<Query, super::QueryError> {
    let parser = QueryParser::new(source, grammar);
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::QueryError;
    use crate::query::ast::PatternChild;
    use adze_ir::{Grammar, SymbolId, Token, TokenPattern};

    fn create_test_grammar() -> Grammar {
        use adze_ir::{Rule, Symbol};

        let mut grammar = Grammar::new("test".to_string());

        // Add some test tokens
        grammar.tokens.insert(
            SymbolId(1),
            Token {
                name: "identifier".to_string(),
                pattern: TokenPattern::Regex("[a-zA-Z]+".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            SymbolId(2),
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex("[0-9]+".to_string()),
                fragile: false,
            },
        );

        // Add actual rules
        grammar.rules.entry(SymbolId(10)).or_default().push(Rule {
            lhs: SymbolId(10),
            rhs: vec![Symbol::NonTerminal(SymbolId(1))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: adze_ir::ProductionId(0),
        });

        grammar.rules.entry(SymbolId(11)).or_default().push(Rule {
            lhs: SymbolId(11),
            rhs: vec![Symbol::NonTerminal(SymbolId(10))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: adze_ir::ProductionId(1),
        });

        // Add rule names
        grammar
            .rule_names
            .insert(SymbolId(1), "identifier".to_string());
        grammar.rule_names.insert(SymbolId(2), "number".to_string());
        grammar
            .rule_names
            .insert(SymbolId(10), "expression".to_string());
        grammar
            .rule_names
            .insert(SymbolId(11), "statement".to_string());

        // Add field names to grammar and statement rule
        use adze_ir::FieldId;
        grammar.fields.insert(FieldId(1), "value".to_string());
        if let Some(rules) = grammar.rules.get_mut(&SymbolId(11))
            && let Some(rule) = rules.get_mut(0)
        {
            rule.fields.push((FieldId(1), 0));
        }

        grammar
    }

    #[test]
    fn test_simple_query() {
        let grammar = create_test_grammar();
        let query_str = r#"(expression)"#;

        let query = compile_query(query_str, &grammar).unwrap();
        assert_eq!(query.patterns.len(), 1);
        assert_eq!(query.capture_count(), 0);
    }

    #[test]
    fn test_query_with_capture() {
        let grammar = create_test_grammar();
        let query_str = r#"(expression @expr)"#;

        let query = compile_query(query_str, &grammar).unwrap();
        assert_eq!(query.patterns.len(), 1);
        assert_eq!(query.capture_count(), 1);
        assert_eq!(query.capture_index("expr"), Some(0));
    }

    #[test]
    fn test_query_with_field() {
        let grammar = create_test_grammar();
        let query_str = r#"(statement value: (expression))"#;

        match compile_query(query_str, &grammar) {
            Ok(query) => assert_eq!(query.patterns.len(), 1),
            Err(QueryError::SyntaxError { position, message }) => {
                // println!("Query string: '{}'", query_str);
                // println!(
                //     "Error at position {}: '{}'",
                //     position,
                //     &query_str[..position]
                // );
                // println!("Rest of query: '{}'", &query_str[position..]);
                panic!(
                    "Query compilation failed at position {}: {}",
                    position, message
                );
            }
            Err(e) => panic!("Query compilation failed: {:?}", e),
        }
    }

    #[test]
    fn test_query_with_predicate() {
        let grammar = create_test_grammar();
        let query_str = r#"
            (expression @expr)
            (#eq? @expr "test")
        "#;

        match compile_query(query_str, &grammar) {
            Ok(query) => {
                assert_eq!(query.patterns.len(), 1);
                assert_eq!(query.patterns[0].predicates.len(), 1);
            }
            Err(QueryError::SyntaxError { position, message }) => {
                // println!("Query string: '{}'", query_str);
                // println!(
                // "Error at position {}: '{}'",
                // position,
                // &query_str[..position.min(query_str.len())]
                // );
                // println!(
                // "Rest of query: '{}'",
                // &query_str[position.min(query_str.len())..]
                // );
                // println!(
                // "Character at position: {:?}",
                // query_str.chars().nth(position)
                // );
                panic!(
                    "Query compilation failed at position {}: {}",
                    position, message
                );
            }
            Err(e) => panic!("Query compilation failed: {:?}", e),
        }
    }

    #[test]
    fn test_query_with_quantifiers() {
        let grammar = create_test_grammar();
        let query_str = r#"(expression (identifier)+ (number)?)"#;

        match compile_query(query_str, &grammar) {
            Ok(query) => assert_eq!(query.patterns.len(), 1),
            Err(QueryError::SyntaxError { position, message }) => {
                // println!("Query string: '{}'", query_str);
                // println!(
                //     "Error at position {}: '{}'",
                //     position,
                //     &query_str[..position]
                // );
                // println!("Rest of query: '{}'", &query_str[position..]);
                // println!(
                // "Character at position: {:?}",
                // query_str.chars().nth(position)
                // );
                panic!(
                    "Query compilation failed at position {}: {}",
                    position, message
                );
            }
            Err(e) => panic!("Query compilation failed: {:?}", e),
        }
    }

    #[test]
    fn test_query_with_anchors() {
        let grammar = create_test_grammar();
        let query = compile_query(r#"(statement . (expression) .)"#, &grammar).unwrap();
        let children = &query.patterns[0].root.children;

        assert!(matches!(children.first(), Some(PatternChild::Anchor)));
        assert!(matches!(children.get(1), Some(PatternChild::Node(_))));
        assert!(matches!(children.get(2), Some(PatternChild::Anchor)));
    }
}
