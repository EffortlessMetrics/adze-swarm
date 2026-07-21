//! Builder API for programmatically constructing grammars
//!
//! This module provides an ergonomic API for building grammars in tests
//! and integration scenarios without dealing with the internal complexity.

use crate::{
    Associativity, ConflictDeclaration, ExternalToken, Grammar, LexicalMetadata, Precedence,
    PrecedenceKind, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern,
};
use indexmap::IndexMap;

/// A builder for constructing grammars programmatically
///
/// # Examples
///
/// ```no_run
/// use adze_ir::builder::GrammarBuilder;
///
/// let grammar = GrammarBuilder::new("example")
///     .token("NUMBER", r"\d+")
///     .token("+", "+")
///     .token("-", "-")
///     .rule("expr", vec!["expr", "+", "expr"])
///     .rule("expr", vec!["expr", "-", "expr"])
///     .rule("expr", vec!["NUMBER"])
///     .start("expr")
///     .build();
/// ```
pub struct GrammarBuilder {
    name: String,
    next_symbol_id: u16,
    next_production_id: u16,
    symbol_ids: IndexMap<String, SymbolId>,
    rules: IndexMap<SymbolId, Vec<Rule>>,
    tokens: IndexMap<SymbolId, Token>,
    precedences: Vec<Precedence>,
    externals: Vec<ExternalToken>,
    extras: Vec<SymbolId>,
    start_symbol: Option<SymbolId>,
    inline_rules: Vec<SymbolId>,
    supertypes: Vec<SymbolId>,
    conflicts: Vec<ConflictDeclaration>,
    rule_names: IndexMap<SymbolId, String>,
    word_token: Option<SymbolId>,
    lexical_metadata: IndexMap<SymbolId, LexicalMetadata>,
}

impl GrammarBuilder {
    /// Create a new grammar builder with the given name
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            next_symbol_id: 1, // Reserve SymbolId(0) for EOF
            next_production_id: 0,
            symbol_ids: IndexMap::new(),
            rules: IndexMap::new(),
            tokens: IndexMap::new(),
            precedences: Vec::new(),
            externals: Vec::new(),
            extras: Vec::new(),
            start_symbol: None,
            inline_rules: Vec::new(),
            supertypes: Vec::new(),
            conflicts: Vec::new(),
            rule_names: IndexMap::new(),
            word_token: None,
            lexical_metadata: IndexMap::new(),
        }
    }

    /// Get or create a symbol ID for a given name
    fn get_or_create_symbol(&mut self, name: &str) -> SymbolId {
        if let Some(&id) = self.symbol_ids.get(name) {
            id
        } else {
            let id = SymbolId(self.next_symbol_id);
            self.next_symbol_id += 1;
            self.symbol_ids.insert(name.to_string(), id);
            // Also update rule_names if it's not a token
            if !name.chars().all(|c| c.is_uppercase() || c == '_')
                && name != "("
                && name != ")"
                && name != "{"
                && name != "}"
                && name != "["
                && name != "]"
                && name != "+"
                && name != "-"
                && name != "*"
                && name != "/"
                && name != "="
                && name != ";"
                && name != ":"
                && name != ","
            {
                self.rule_names.insert(id, name.to_string());
            }
            id
        }
    }

    /// Add a token (terminal) to the grammar
    pub fn token(mut self, name: &str, pattern: &str) -> Self {
        let symbol_id = self.get_or_create_symbol(name);

        // Determine if it's a literal or regex pattern
        let token_pattern =
            if pattern == name && !pattern.chars().any(|c| c.is_alphanumeric() || c == '_') {
                TokenPattern::String(pattern.to_string())
            } else if pattern.starts_with('/') && pattern.ends_with('/') {
                TokenPattern::Regex(pattern[1..pattern.len() - 1].to_string())
            } else if pattern.contains(|c: char| "\\[]{}()*+?|^$.".contains(c)) {
                TokenPattern::Regex(pattern.to_string())
            } else {
                TokenPattern::String(pattern.to_string())
            };

        self.tokens.insert(
            symbol_id,
            Token {
                name: name.to_string(),
                pattern: token_pattern,
                fragile: false,
            },
        );
        self
    }

    /// Add a fragile token (for error recovery)
    pub fn fragile_token(mut self, name: &str, pattern: &str) -> Self {
        let symbol_id = self.get_or_create_symbol(name);
        let token_pattern = if pattern == name {
            TokenPattern::String(pattern.to_string())
        } else {
            TokenPattern::Regex(pattern.to_string())
        };

        self.tokens.insert(
            symbol_id,
            Token {
                name: name.to_string(),
                pattern: token_pattern,
                fragile: true,
            },
        );
        self
    }

    /// Add a rule to the grammar
    ///
    /// Multiple calls with the same left-hand side will add alternative productions
    pub fn rule(mut self, lhs: &str, rhs: Vec<&str>) -> Self {
        let lhs_id = self.get_or_create_symbol(lhs);

        let rhs_symbols: Vec<Symbol> = if rhs.is_empty() {
            vec![Symbol::Epsilon]
        } else {
            rhs.iter()
                .map(|&name| {
                    let id = self.get_or_create_symbol(name);
                    // Determine if it's a terminal or non-terminal based on whether it has a token
                    if self.tokens.contains_key(&id) {
                        Symbol::Terminal(id)
                    } else {
                        Symbol::NonTerminal(id)
                    }
                })
                .collect()
        };

        let production_id = ProductionId(self.next_production_id);
        self.next_production_id += 1;

        let rule = Rule {
            lhs: lhs_id,
            rhs: rhs_symbols,
            precedence: None,
            associativity: None,
            fields: Vec::new(),
            production_id,
        };

        self.rules.entry(lhs_id).or_default().push(rule);
        self
    }

    /// Add a rule with precedence
    pub fn rule_with_precedence(
        mut self,
        lhs: &str,
        rhs: Vec<&str>,
        prec: i16,
        assoc: Associativity,
    ) -> Self {
        let lhs_id = self.get_or_create_symbol(lhs);

        let rhs_symbols: Vec<Symbol> = rhs
            .iter()
            .map(|&name| {
                let id = self.get_or_create_symbol(name);
                if self.tokens.contains_key(&id) {
                    Symbol::Terminal(id)
                } else {
                    Symbol::NonTerminal(id)
                }
            })
            .collect();

        let production_id = ProductionId(self.next_production_id);
        self.next_production_id += 1;

        let rule = Rule {
            lhs: lhs_id,
            rhs: rhs_symbols,
            precedence: Some(PrecedenceKind::Static(prec)),
            associativity: Some(assoc),
            fields: Vec::new(),
            production_id,
        };

        self.rules.entry(lhs_id).or_default().push(rule);
        self
    }

    /// Set the start symbol for the grammar  
    /// This will ensure the first rule in the grammar is for this symbol
    pub fn start(mut self, symbol: &str) -> Self {
        self.start_symbol = Some(self.get_or_create_symbol(symbol));
        self
    }

    /// Add an extra token (like whitespace)
    pub fn extra(mut self, name: &str) -> Self {
        let id = self.get_or_create_symbol(name);
        self.extras.push(id);
        self
    }

    /// Mark a rule as inline (should not create AST nodes)
    pub fn inline(mut self, name: &str) -> Self {
        let id = self.get_or_create_symbol(name);
        self.inline_rules.push(id);
        self
    }

    /// Mark a symbol as a supertype (union type node)
    pub fn supertype(mut self, name: &str) -> Self {
        let id = self.get_or_create_symbol(name);
        self.supertypes.push(id);
        self
    }

    /// Add an external scanner token
    pub fn external(mut self, name: &str) -> Self {
        let symbol_id = self.get_or_create_symbol(name);
        self.externals.push(ExternalToken {
            name: name.to_string(),
            symbol_id,
        });
        self
    }

    /// Add a precedence declaration
    pub fn precedence(mut self, level: i16, assoc: Associativity, symbols: Vec<&str>) -> Self {
        let symbol_ids: Vec<SymbolId> = symbols
            .iter()
            .map(|&s| self.get_or_create_symbol(s))
            .collect();

        self.precedences.push(Precedence {
            level,
            associativity: assoc,
            symbols: symbol_ids,
        });
        self
    }

    /// Mark the authoritative word token for keyword-versus-identifier resolution.
    pub fn word_token(mut self, name: &str) -> Self {
        let id = self.get_or_create_symbol(name);
        self.word_token = Some(id);
        self
    }

    /// Mark a token as immediate (no leading extras before matching).
    pub fn immediate_token(mut self, name: &str, pattern: &str) -> Self {
        let symbol_id = self.get_or_create_symbol(name);
        let token_pattern = if pattern == name {
            TokenPattern::String(pattern.to_string())
        } else {
            TokenPattern::Regex(pattern.to_string())
        };
        self.tokens.insert(
            symbol_id,
            Token {
                name: name.to_string(),
                pattern: token_pattern,
                fragile: false,
            },
        );
        self.lexical_metadata.insert(
            symbol_id,
            LexicalMetadata {
                immediate: true,
                lexical_priority: 0,
            },
        );
        self
    }

    /// Build the final grammar
    pub fn build(mut self) -> Grammar {
        // If a start symbol was specified, ensure its rules come first
        let mut ordered_rules = IndexMap::new();

        if let Some(start_id) = self.start_symbol
            && let Some(rules) = self.rules.shift_remove(&start_id)
        {
            ordered_rules.insert(start_id, rules);
        }

        // Add remaining rules
        for (id, rules) in self.rules {
            ordered_rules.insert(id, rules);
        }

        Grammar {
            name: self.name,
            rules: ordered_rules,
            tokens: self.tokens,
            precedences: self.precedences,
            conflicts: self.conflicts,
            externals: self.externals,
            extras: self.extras,
            fields: IndexMap::new(),
            supertypes: self.supertypes,
            inline_rules: self.inline_rules,
            alias_sequences: IndexMap::new(),
            production_ids: IndexMap::new(),
            max_alias_sequence_length: 0,
            rule_names: self.rule_names,
            symbol_registry: None,
            word_token: self.word_token,
            lexical_metadata: self.lexical_metadata,
        }
    }
}

/// Helper for creating nullable start symbol grammars (like Python)
impl GrammarBuilder {
    /// Create a Python-like grammar with nullable start symbol
    pub fn python_like() -> Grammar {
        GrammarBuilder::new("python_like")
            .token("def", "def")
            .token("pass", "pass")
            .token("IDENTIFIER", r"[a-zA-Z_][a-zA-Z0-9_]*")
            .token("(", "(")
            .token(")", ")")
            .token(":", ":")
            .token("NEWLINE", r"\n")
            .token("INDENT", "INDENT") // External scanner
            .token("DEDENT", "DEDENT") // External scanner
            .external("INDENT")
            .external("DEDENT")
            .extra("WHITESPACE")
            .token("WHITESPACE", r"[ \t]+")
            // Module can be empty (nullable)
            .rule("module", vec![])
            .rule("module", vec!["statement"])
            .rule("module", vec!["module", "statement"])
            // Statement
            .rule("statement", vec!["function_def"])
            .rule("statement", vec!["pass", "NEWLINE"])
            // Function definition
            .rule(
                "function_def",
                vec!["def", "IDENTIFIER", "(", ")", ":", "suite"],
            )
            // Suite with indentation
            .rule("suite", vec!["NEWLINE", "INDENT", "statements", "DEDENT"])
            .rule("statements", vec!["statement"])
            .rule("statements", vec!["statements", "statement"])
            .start("module")
            .build()
    }

    /// Create a JavaScript-like grammar with non-nullable start
    pub fn javascript_like() -> Grammar {
        GrammarBuilder::new("javascript_like")
            .token("function", "function")
            .token("var", "var")
            .token("return", "return")
            .token("IDENTIFIER", r"[a-zA-Z_$][a-zA-Z0-9_$]*")
            .token("NUMBER", r"\d+")
            .token(";", ";")
            .token("=", "=")
            .token("+", "+")
            .token("-", "-")
            .token("*", "*")
            .token("/", "/")
            .token("(", "(")
            .token(")", ")")
            .token("{", "{")
            .token("}", "}")
            .extra("WHITESPACE")
            .token("WHITESPACE", r"[ \t\n\r]+")
            // Program must have at least one statement (non-nullable)
            .rule("program", vec!["statement"])
            .rule("program", vec!["program", "statement"])
            // Statements
            .rule("statement", vec!["var_declaration"])
            .rule("statement", vec!["function_declaration"])
            .rule("statement", vec!["expression_statement"])
            // Variable declaration
            .rule(
                "var_declaration",
                vec!["var", "IDENTIFIER", "=", "expression", ";"],
            )
            // Function declaration
            .rule(
                "function_declaration",
                vec!["function", "IDENTIFIER", "(", ")", "block"],
            )
            // Block
            .rule("block", vec!["{", "}"])
            .rule("block", vec!["{", "statements", "}"])
            .rule("statements", vec!["statement"])
            .rule("statements", vec!["statements", "statement"])
            // Expression statement
            .rule("expression_statement", vec!["expression", ";"])
            // Expressions with precedence
            .rule_with_precedence(
                "expression",
                vec!["expression", "+", "expression"],
                1,
                Associativity::Left,
            )
            .rule_with_precedence(
                "expression",
                vec!["expression", "-", "expression"],
                1,
                Associativity::Left,
            )
            .rule_with_precedence(
                "expression",
                vec!["expression", "*", "expression"],
                2,
                Associativity::Left,
            )
            .rule_with_precedence(
                "expression",
                vec!["expression", "/", "expression"],
                2,
                Associativity::Left,
            )
            .rule("expression", vec!["IDENTIFIER"])
            .rule("expression", vec!["NUMBER"])
            .rule("expression", vec!["(", "expression", ")"])
            .start("program")
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_grammar() {
        let grammar = GrammarBuilder::new("arithmetic")
            .token("NUMBER", r"\d+")
            .token("+", "+")
            .rule("sum", vec!["NUMBER", "+", "NUMBER"])
            .start("sum")
            .build();

        assert_eq!(grammar.name, "arithmetic");
        assert_eq!(grammar.tokens.len(), 2);
        assert_eq!(grammar.rules.len(), 1);
    }

    #[test]
    fn test_python_like_nullable_start() {
        let grammar = GrammarBuilder::python_like();

        // Check that module has an empty production (nullable start)
        let module_id = grammar
            .rule_names
            .iter()
            .find(|(_, name)| name.as_str() == "module")
            .map(|(id, _)| *id)
            .unwrap();

        let module_rules = &grammar.rules[&module_id];
        assert!(
            module_rules
                .iter()
                .any(|r| r.rhs.len() == 1 && matches!(r.rhs[0], Symbol::Epsilon))
        );
    }

    #[test]
    fn test_javascript_like_non_nullable() {
        let grammar = GrammarBuilder::javascript_like();

        // Check that program does NOT have an empty production
        let program_id = grammar
            .rule_names
            .iter()
            .find(|(_, name)| name.as_str() == "program")
            .map(|(id, _)| *id)
            .unwrap();

        let program_rules = &grammar.rules[&program_id];
        assert!(
            !program_rules
                .iter()
                .any(|r| r.rhs.len() == 1 && matches!(r.rhs[0], Symbol::Epsilon))
        );
    }

    #[test]
    fn test_precedence_rules() {
        let grammar = GrammarBuilder::new("calc")
            .token("NUMBER", r"\d+")
            .token("+", "+")
            .token("*", "*")
            .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
            .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
            .rule("expr", vec!["NUMBER"])
            .start("expr")
            .build();

        let expr_id = grammar
            .rule_names
            .iter()
            .find(|(_, name)| name.as_str() == "expr")
            .map(|(id, _)| *id)
            .unwrap();

        let expr_rules = &grammar.rules[&expr_id];

        // Find the addition and multiplication rules
        let add_rule = expr_rules
            .iter()
            .find(|r| {
                r.rhs.len() == 3
                    && r.rhs.iter().any(
                        |s| matches!(s, Symbol::Terminal(id) if grammar.tokens[id].name == "+"),
                    )
            })
            .unwrap();

        let mul_rule = expr_rules
            .iter()
            .find(|r| {
                r.rhs.len() == 3
                    && r.rhs.iter().any(
                        |s| matches!(s, Symbol::Terminal(id) if grammar.tokens[id].name == "*"),
                    )
            })
            .unwrap();

        // Check precedence
        if let (Some(PrecedenceKind::Static(add_prec)), Some(PrecedenceKind::Static(mul_prec))) =
            (add_rule.precedence, mul_rule.precedence)
        {
            assert!(add_prec < mul_prec);
        } else {
            panic!("Expected precedence to be set");
        }
    }
}
