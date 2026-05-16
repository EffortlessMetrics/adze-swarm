// Simple grammar converter for demonstration
//! Conversion utilities between grammar representations.

// This would be expanded with actual grammar extraction logic

use adze_ir::{
    Associativity, FieldId, Grammar, PrecedenceKind, ProductionId, Rule, Symbol, SymbolId, Token,
    TokenPattern,
};

/// Simplified grammar converter
pub struct GrammarConverter;

impl GrammarConverter {
    /// Create a sample grammar for testing
    pub fn create_sample_grammar() -> Grammar {
        let mut grammar = Grammar::new("sample".to_string());

        // Define some basic tokens
        let id_symbol = SymbolId(1);
        let num_symbol = SymbolId(2);
        let plus_symbol = SymbolId(3);
        let expr_symbol = SymbolId(4);

        // Add tokens
        grammar.tokens.insert(
            id_symbol,
            Token {
                name: "identifier".to_string(),
                pattern: TokenPattern::Regex(r"[a-zA-Z_][a-zA-Z0-9_]*".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            num_symbol,
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            plus_symbol,
            Token {
                name: "plus".to_string(),
                pattern: TokenPattern::String("+".to_string()),
                fragile: false,
            },
        );

        // Add rules
        // expr -> identifier
        grammar.rules.entry(expr_symbol).or_default().push(Rule {
            lhs: expr_symbol,
            rhs: vec![Symbol::Terminal(id_symbol)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

        // expr -> number
        grammar.rules.entry(expr_symbol).or_default().push(Rule {
            lhs: expr_symbol,
            rhs: vec![Symbol::Terminal(num_symbol)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(1),
        });

        // expr -> expr + expr
        grammar.rules.entry(expr_symbol).or_default().push(Rule {
            lhs: expr_symbol,
            rhs: vec![
                Symbol::NonTerminal(expr_symbol),
                Symbol::Terminal(plus_symbol),
                Symbol::NonTerminal(expr_symbol),
            ],
            precedence: Some(PrecedenceKind::Static(1)),
            associativity: Some(Associativity::Left),
            fields: vec![(FieldId(1), 0), (FieldId(2), 2)], // left, right
            production_id: ProductionId(2),
        });

        // Add field names
        grammar.fields.insert(FieldId(1), "left".to_string());
        grammar.fields.insert(FieldId(2), "right".to_string());

        grammar
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_sample_grammar_uses_sample_name() {
        let grammar = GrammarConverter::create_sample_grammar();
        assert_eq!(grammar.name, "sample");
    }

    #[test]
    fn create_sample_grammar_has_three_tokens() {
        let grammar = GrammarConverter::create_sample_grammar();
        assert_eq!(grammar.tokens.len(), 3);
    }

    #[test]
    fn create_sample_grammar_registers_identifier_token() {
        let grammar = GrammarConverter::create_sample_grammar();
        let token = grammar
            .tokens
            .get(&SymbolId(1))
            .expect("identifier token missing");
        assert_eq!(token.name, "identifier");
        assert!(!token.fragile);
        match &token.pattern {
            TokenPattern::Regex(re) => assert_eq!(re, r"[a-zA-Z_][a-zA-Z0-9_]*"),
            other => panic!("expected Regex pattern, got {:?}", other),
        }
    }

    #[test]
    fn create_sample_grammar_registers_number_token() {
        let grammar = GrammarConverter::create_sample_grammar();
        let token = grammar
            .tokens
            .get(&SymbolId(2))
            .expect("number token missing");
        assert_eq!(token.name, "number");
        match &token.pattern {
            TokenPattern::Regex(re) => assert_eq!(re, r"\d+"),
            other => panic!("expected Regex pattern, got {:?}", other),
        }
    }

    #[test]
    fn create_sample_grammar_registers_plus_token_as_string() {
        let grammar = GrammarConverter::create_sample_grammar();
        let token = grammar
            .tokens
            .get(&SymbolId(3))
            .expect("plus token missing");
        assert_eq!(token.name, "plus");
        match &token.pattern {
            TokenPattern::String(s) => assert_eq!(s, "+"),
            other => panic!("expected String pattern, got {:?}", other),
        }
    }

    #[test]
    fn create_sample_grammar_has_three_expr_rules() {
        let grammar = GrammarConverter::create_sample_grammar();
        let rules = grammar.rules.get(&SymbolId(4)).expect("expr rules missing");
        assert_eq!(rules.len(), 3);
    }

    #[test]
    fn create_sample_grammar_first_rule_is_terminal_identifier() {
        let grammar = GrammarConverter::create_sample_grammar();
        let rules = grammar.rules.get(&SymbolId(4)).expect("expr rules missing");
        assert_eq!(rules[0].lhs, SymbolId(4));
        assert_eq!(rules[0].rhs, vec![Symbol::Terminal(SymbolId(1))]);
        assert_eq!(rules[0].production_id, ProductionId(0));
        assert!(rules[0].precedence.is_none());
        assert!(rules[0].associativity.is_none());
        assert!(rules[0].fields.is_empty());
    }

    #[test]
    fn create_sample_grammar_second_rule_is_terminal_number() {
        let grammar = GrammarConverter::create_sample_grammar();
        let rules = grammar.rules.get(&SymbolId(4)).expect("expr rules missing");
        assert_eq!(rules[1].rhs, vec![Symbol::Terminal(SymbolId(2))]);
        assert_eq!(rules[1].production_id, ProductionId(1));
    }

    #[test]
    fn create_sample_grammar_third_rule_is_binary_plus() {
        let grammar = GrammarConverter::create_sample_grammar();
        let rules = grammar.rules.get(&SymbolId(4)).expect("expr rules missing");
        let binary = &rules[2];
        assert_eq!(
            binary.rhs,
            vec![
                Symbol::NonTerminal(SymbolId(4)),
                Symbol::Terminal(SymbolId(3)),
                Symbol::NonTerminal(SymbolId(4)),
            ]
        );
        assert_eq!(binary.precedence, Some(PrecedenceKind::Static(1)));
        assert_eq!(binary.associativity, Some(Associativity::Left));
        assert_eq!(binary.production_id, ProductionId(2));
        assert_eq!(binary.fields, vec![(FieldId(1), 0), (FieldId(2), 2)]);
    }

    #[test]
    fn create_sample_grammar_registers_left_and_right_fields() {
        let grammar = GrammarConverter::create_sample_grammar();
        assert_eq!(grammar.fields.len(), 2);
        assert_eq!(
            grammar.fields.get(&FieldId(1)).map(String::as_str),
            Some("left")
        );
        assert_eq!(
            grammar.fields.get(&FieldId(2)).map(String::as_str),
            Some("right")
        );
    }
}
