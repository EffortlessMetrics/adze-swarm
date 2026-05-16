#![cfg_attr(feature = "strict_docs", allow(missing_docs))]
//! NODE_TYPES JSON metadata generation for Tree-sitter grammars.

use adze_ir::{Grammar, Symbol, TokenPattern};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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

/// Tree-sitter NODE_TYPES.json generator
pub struct NodeTypesGenerator<'a> {
    grammar: &'a Grammar,
}

#[derive(Debug, Serialize, Deserialize)]
struct NodeType {
    #[serde(rename = "type")]
    type_name: String,
    named: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    fields: Option<HashMap<String, FieldInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<ChildrenInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subtypes: Option<Vec<SubtypeRef>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FieldInfo {
    multiple: bool,
    required: bool,
    types: Vec<TypeRef>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChildrenInfo {
    multiple: bool,
    required: bool,
    types: Vec<TypeRef>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TypeRef {
    #[serde(rename = "type")]
    type_name: String,
    named: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct SubtypeRef {
    #[serde(rename = "type")]
    type_name: String,
    named: bool,
}

impl<'a> NodeTypesGenerator<'a> {
    pub fn new(grammar: &'a Grammar) -> Self {
        Self { grammar }
    }

    /// Generate NODE_TYPES.json content
    #[must_use = "generation result must be checked"]
    pub fn generate(&self) -> Result<String, String> {
        let mut node_types = Vec::new();
        let mut symbol_names: HashMap<_, _> = HashMap::new();

        debug_trace!(
            "Debug: NodeTypesGenerator - grammar has {} rules",
            self.grammar.rules.len()
        );

        // First, collect all symbol names
        for (symbol_id, _rule) in &self.grammar.rules {
            if let Some(rule_name) = self.get_rule_name(*symbol_id) {
                debug_trace!(
                    "Debug: Adding rule name '{}' for symbol {}",
                    rule_name,
                    symbol_id.0
                );
                symbol_names.insert(*symbol_id, rule_name);
            }
        }

        // Add token names
        for (symbol_id, token) in &self.grammar.tokens {
            symbol_names.insert(*symbol_id, token.name.clone());
        }

        // Process rules to create node types
        let mut processed = HashSet::new();

        debug_trace!(
            "Debug: Processing {} rules for node types",
            self.grammar.rules.len()
        );

        // Find supertypes (rules that have other rules as alternatives)
        let _supertypes: HashMap<adze_ir::SymbolId, Vec<adze_ir::SymbolId>> = HashMap::new();

        // Analyze rule relationships to find choice patterns
        for (symbol_id, rules) in &self.grammar.rules {
            if processed.contains(symbol_id) {
                continue;
            }

            debug_trace!(
                "Debug: Processing symbol {} with {} rules",
                symbol_id.0,
                rules.len()
            );

            // Get the rule name
            if let Some(name) = self.get_rule_name(*symbol_id) {
                // Skip internal rules (starting with _)
                let is_internal = name.starts_with('_');

                // Collect fields from all rules for this symbol
                let mut fields = HashMap::new();
                for rule in rules {
                    for (field_id, position) in &rule.fields {
                        if let Some(field_name) = self.grammar.fields.get(field_id)
                            && let Some(symbol) = rule.rhs.get(*position)
                        {
                            let type_ref = self.symbol_to_type_ref(symbol, &symbol_names);
                            fields.insert(
                                field_name.clone(),
                                FieldInfo {
                                    multiple: false, // TODO: Detect repetition
                                    required: true,  // TODO: Detect optionality
                                    types: vec![type_ref],
                                },
                            );
                        }
                    }
                }

                // Add the node type if it's not internal
                if !is_internal {
                    node_types.push(NodeType {
                        type_name: name.clone(),
                        named: true,
                        fields: if fields.is_empty() {
                            None
                        } else {
                            Some(fields)
                        },
                        children: None,
                        subtypes: None,
                    });
                }
            }

            processed.insert(*symbol_id);
        }

        // Add tokens as unnamed nodes
        for (_, token) in &self.grammar.tokens {
            let (type_name, named) = match &token.pattern {
                TokenPattern::String(s) => (s.clone(), false),
                TokenPattern::Regex(_) => (token.name.clone(), true),
            };

            if !named {
                node_types.push(NodeType {
                    type_name,
                    named,
                    fields: None,
                    children: None,
                    subtypes: None,
                });
            }
        }

        // Sort for consistent output
        node_types.sort_by(|a, b| a.type_name.cmp(&b.type_name));

        // Serialize to JSON
        serde_json::to_string_pretty(&node_types)
            .map_err(|e| format!("Failed to serialize NODE_TYPES: {}", e))
    }

    fn get_rule_name(&self, symbol_id: adze_ir::SymbolId) -> Option<String> {
        // Check if this is a token first
        if let Some(token) = self.grammar.tokens.get(&symbol_id) {
            return Some(token.name.clone());
        }

        // Look up rule name
        if let Some(rule_name) = self.grammar.rule_names.get(&symbol_id) {
            return Some(rule_name.clone());
        }

        // Fallback
        Some(format!("rule_{}", symbol_id.0))
    }

    fn symbol_to_type_ref(
        &self,
        symbol: &Symbol,
        symbol_names: &HashMap<adze_ir::SymbolId, String>,
    ) -> TypeRef {
        match symbol {
            Symbol::Terminal(id) => {
                if let Some(token) = self.grammar.tokens.get(id) {
                    match &token.pattern {
                        TokenPattern::String(s) => TypeRef {
                            type_name: s.clone(),
                            named: false,
                        },
                        TokenPattern::Regex(_) => TypeRef {
                            type_name: token.name.clone(),
                            named: true,
                        },
                    }
                } else {
                    TypeRef {
                        type_name: "unknown".to_string(),
                        named: false,
                    }
                }
            }
            Symbol::NonTerminal(id) => TypeRef {
                type_name: symbol_names
                    .get(id)
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string()),
                named: true,
            },
            Symbol::External(_) => TypeRef {
                type_name: "external".to_string(),
                named: true,
            },
            Symbol::Optional(inner) => self.symbol_to_type_ref(inner, symbol_names),
            Symbol::Repeat(inner) | Symbol::RepeatOne(inner) => {
                let inner_ref = self.symbol_to_type_ref(inner, symbol_names);
                TypeRef {
                    type_name: inner_ref.type_name,
                    named: inner_ref.named,
                }
            }
            Symbol::Choice(choices) => {
                // For now, just use the first choice
                if let Some(first) = choices.first() {
                    self.symbol_to_type_ref(first, symbol_names)
                } else {
                    TypeRef {
                        type_name: "empty".to_string(),
                        named: false,
                    }
                }
            }
            Symbol::Sequence(seq) => {
                // For sequences, we might want to create a composite type
                if let Some(first) = seq.first() {
                    self.symbol_to_type_ref(first, symbol_names)
                } else {
                    TypeRef {
                        type_name: "empty".to_string(),
                        named: false,
                    }
                }
            }
            Symbol::Epsilon => TypeRef {
                type_name: "empty".to_string(),
                named: false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::{ProductionId, Rule, SymbolId, Token};

    #[test]
    fn test_simple_node_types() {
        let mut grammar = Grammar::new("test".to_string());

        // Add a number token
        let number_token = Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        };
        let number_token_id = SymbolId(0);
        grammar.tokens.insert(number_token_id, number_token);

        // Add a simple rule
        let rule = Rule {
            lhs: SymbolId(1),
            rhs: vec![Symbol::Terminal(number_token_id)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        };
        grammar.add_rule(rule);

        let generator = NodeTypesGenerator::new(&grammar);
        let result = generator.generate().unwrap();

        let node_types: Vec<NodeType> = serde_json::from_str(&result).unwrap();
        assert!(!node_types.is_empty());
    }

    // ---- Coverage additions for previously-untested branches ----

    use adze_ir::FieldId;
    use std::collections::HashMap;

    fn make_grammar_with_named_rule(
        name: &str,
        rule_id: SymbolId,
        rhs: Vec<Symbol>,
        fields: Vec<(FieldId, usize)>,
    ) -> Grammar {
        let mut grammar = Grammar::new("g".to_string());
        grammar.add_rule(Rule {
            lhs: rule_id,
            rhs,
            precedence: None,
            associativity: None,
            fields,
            production_id: ProductionId(0),
        });
        grammar.rule_names.insert(rule_id, name.to_string());
        grammar
    }

    #[test]
    fn generate_sorts_node_types_alphabetically() {
        // Two rules with deliberately reversed alphabetical order.
        let mut grammar = Grammar::new("g".to_string());
        grammar.add_rule(Rule {
            lhs: SymbolId(1),
            rhs: vec![],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });
        grammar.add_rule(Rule {
            lhs: SymbolId(2),
            rhs: vec![],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(1),
        });
        grammar.rule_names.insert(SymbolId(1), "zebra".to_string());
        grammar.rule_names.insert(SymbolId(2), "apple".to_string());

        let json = NodeTypesGenerator::new(&grammar).generate().unwrap();
        let parsed: Vec<NodeType> = serde_json::from_str(&json).unwrap();
        let names: Vec<&str> = parsed.iter().map(|nt| nt.type_name.as_str()).collect();
        assert_eq!(names, vec!["apple", "zebra"]);
        // Default named=true for rules.
        assert!(parsed.iter().all(|nt| nt.named));
    }

    #[test]
    fn generate_skips_internal_rules_starting_with_underscore() {
        let grammar = make_grammar_with_named_rule("_hidden", SymbolId(7), vec![], vec![]);
        let json = NodeTypesGenerator::new(&grammar).generate().unwrap();
        let parsed: Vec<NodeType> = serde_json::from_str(&json).unwrap();
        assert!(
            parsed.iter().all(|nt| nt.type_name != "_hidden"),
            "internal rules must not appear: {:?}",
            parsed.iter().map(|nt| &nt.type_name).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn generate_includes_string_tokens_as_unnamed_nodes() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.tokens.insert(
            SymbolId(1),
            Token {
                name: "plus".to_string(),
                pattern: TokenPattern::String("+".to_string()),
                fragile: false,
            },
        );
        // Regex tokens are skipped because they are "named" — only string tokens add unnamed entries.
        grammar.tokens.insert(
            SymbolId(2),
            Token {
                name: "ident".to_string(),
                pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
                fragile: false,
            },
        );

        let json = NodeTypesGenerator::new(&grammar).generate().unwrap();
        let parsed: Vec<NodeType> = serde_json::from_str(&json).unwrap();
        // The string token contributes an unnamed node, the regex token does not.
        let plus = parsed
            .iter()
            .find(|nt| nt.type_name == "+")
            .expect("string token must appear as anonymous node");
        assert!(!plus.named);
        assert!(!parsed.iter().any(|nt| nt.type_name == "ident"));
    }

    #[test]
    fn generate_attaches_fields_from_rule_metadata() {
        let mut grammar = make_grammar_with_named_rule(
            "binary",
            SymbolId(10),
            vec![Symbol::Terminal(SymbolId(1))],
            vec![(FieldId(0), 0)],
        );
        grammar.fields.insert(FieldId(0), "lhs".to_string());
        grammar.tokens.insert(
            SymbolId(1),
            Token {
                name: "num".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );

        let json = NodeTypesGenerator::new(&grammar).generate().unwrap();
        let parsed: Vec<NodeType> = serde_json::from_str(&json).unwrap();
        let binary = parsed
            .iter()
            .find(|nt| nt.type_name == "binary")
            .expect("binary rule must be present");
        let fields = binary.fields.as_ref().expect("fields populated");
        let lhs = fields.get("lhs").expect("lhs field present");
        assert!(lhs.required);
        assert!(!lhs.multiple);
        assert_eq!(lhs.types.len(), 1);
        // Field type ref resolves to a named regex token by token name.
        assert_eq!(lhs.types[0].type_name, "num");
        assert!(lhs.types[0].named);
    }

    #[test]
    fn generate_omits_empty_fields_block() {
        // Rule with no fields -> fields key serialized as absent.
        let grammar = make_grammar_with_named_rule("plain", SymbolId(1), vec![], vec![]);
        let json = NodeTypesGenerator::new(&grammar).generate().unwrap();
        assert!(!json.contains("\"fields\""), "json was: {json}");
    }

    #[test]
    fn get_rule_name_returns_token_name_then_rule_name_then_fallback() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.tokens.insert(
            SymbolId(1),
            Token {
                name: "tok".to_string(),
                pattern: TokenPattern::String("a".to_string()),
                fragile: false,
            },
        );
        grammar
            .rule_names
            .insert(SymbolId(2), "named_rule".to_string());

        let generator = NodeTypesGenerator::new(&grammar);
        assert_eq!(generator.get_rule_name(SymbolId(1)).as_deref(), Some("tok"));
        assert_eq!(
            generator.get_rule_name(SymbolId(2)).as_deref(),
            Some("named_rule"),
        );
        // Unknown id falls back to "rule_<n>".
        assert_eq!(
            generator.get_rule_name(SymbolId(99)).as_deref(),
            Some("rule_99"),
        );
    }

    #[test]
    fn symbol_to_type_ref_handles_all_symbol_variants() {
        let mut grammar = Grammar::new("g".to_string());
        // A string token (anonymous) and a regex token (named).
        grammar.tokens.insert(
            SymbolId(1),
            Token {
                name: "plus".to_string(),
                pattern: TokenPattern::String("+".to_string()),
                fragile: false,
            },
        );
        grammar.tokens.insert(
            SymbolId(2),
            Token {
                name: "ident".to_string(),
                pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
                fragile: false,
            },
        );

        let generator = NodeTypesGenerator::new(&grammar);

        let mut names: HashMap<SymbolId, String> = HashMap::new();
        names.insert(SymbolId(10), "expr".to_string());

        // Terminal -> string token: anonymous.
        let t = generator.symbol_to_type_ref(&Symbol::Terminal(SymbolId(1)), &names);
        assert_eq!(t.type_name, "+");
        assert!(!t.named);

        // Terminal -> regex token: named, uses token name.
        let t = generator.symbol_to_type_ref(&Symbol::Terminal(SymbolId(2)), &names);
        assert_eq!(t.type_name, "ident");
        assert!(t.named);

        // Terminal -> unknown id: "unknown", unnamed.
        let t = generator.symbol_to_type_ref(&Symbol::Terminal(SymbolId(999)), &names);
        assert_eq!(t.type_name, "unknown");
        assert!(!t.named);

        // NonTerminal -> resolved name.
        let nt = generator.symbol_to_type_ref(&Symbol::NonTerminal(SymbolId(10)), &names);
        assert_eq!(nt.type_name, "expr");
        assert!(nt.named);

        // NonTerminal -> missing entry falls back to "unknown" but stays named.
        let nt = generator.symbol_to_type_ref(&Symbol::NonTerminal(SymbolId(11)), &names);
        assert_eq!(nt.type_name, "unknown");
        assert!(nt.named);

        // External -> hard-coded "external", named.
        let ext = generator.symbol_to_type_ref(&Symbol::External(SymbolId(0)), &names);
        assert_eq!(ext.type_name, "external");
        assert!(ext.named);

        // Optional / Repeat / RepeatOne unwrap their inner symbol.
        let opt = generator.symbol_to_type_ref(
            &Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(1)))),
            &names,
        );
        assert_eq!(opt.type_name, "+");
        let rep = generator.symbol_to_type_ref(
            &Symbol::Repeat(Box::new(Symbol::Terminal(SymbolId(2)))),
            &names,
        );
        assert_eq!(rep.type_name, "ident");
        let rep1 = generator.symbol_to_type_ref(
            &Symbol::RepeatOne(Box::new(Symbol::NonTerminal(SymbolId(10)))),
            &names,
        );
        assert_eq!(rep1.type_name, "expr");

        // Choice with at least one element uses the first.
        let ch = generator.symbol_to_type_ref(
            &Symbol::Choice(vec![
                Symbol::Terminal(SymbolId(2)),
                Symbol::Terminal(SymbolId(1)),
            ]),
            &names,
        );
        assert_eq!(ch.type_name, "ident");
        // Empty Choice -> "empty"/unnamed.
        let ch_empty = generator.symbol_to_type_ref(&Symbol::Choice(vec![]), &names);
        assert_eq!(ch_empty.type_name, "empty");
        assert!(!ch_empty.named);

        // Sequence with first uses first; empty Sequence -> "empty".
        let seq = generator.symbol_to_type_ref(
            &Symbol::Sequence(vec![Symbol::Terminal(SymbolId(1))]),
            &names,
        );
        assert_eq!(seq.type_name, "+");
        let seq_empty = generator.symbol_to_type_ref(&Symbol::Sequence(vec![]), &names);
        assert_eq!(seq_empty.type_name, "empty");
        assert!(!seq_empty.named);

        // Epsilon -> "empty"/unnamed.
        let eps = generator.symbol_to_type_ref(&Symbol::Epsilon, &names);
        assert_eq!(eps.type_name, "empty");
        assert!(!eps.named);
    }

    #[test]
    fn generate_emits_valid_json_when_grammar_has_only_regex_tokens() {
        // Regex tokens are skipped from the unnamed-node loop; the JSON must still parse and be empty.
        let mut grammar = Grammar::new("g".to_string());
        grammar.tokens.insert(
            SymbolId(1),
            Token {
                name: "ident".to_string(),
                pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
                fragile: false,
            },
        );
        let json = NodeTypesGenerator::new(&grammar).generate().unwrap();
        let parsed: Vec<NodeType> = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_empty(), "expected empty array, got {:?}", parsed);
    }
}
