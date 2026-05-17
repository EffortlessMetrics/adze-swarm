// Predicate evaluation for query patterns
use super::ast::Predicate;
use crate::parser_v4::ParseNode;
use regex::Regex;
use std::cell::RefCell;
use std::collections::HashMap;

/// Context for predicate evaluation
pub struct PredicateContext<'a> {
    /// Source text
    pub source: &'a str,
    /// Regex cache
    regex_cache: RefCell<HashMap<String, Regex>>,
}

impl<'a> PredicateContext<'a> {
    /// Create a new predicate context
    pub fn new(source: &'a str) -> Self {
        PredicateContext {
            source,
            regex_cache: RefCell::new(HashMap::new()),
        }
    }

    /// Get text for a node
    pub fn node_text(&self, node: &ParseNode) -> Option<&str> {
        self.source.get(node.start_byte..node.end_byte)
    }

    /// Evaluate a predicate
    pub fn evaluate(&self, predicate: &Predicate, captures: &HashMap<u32, ParseNode>) -> bool {
        match predicate {
            Predicate::Eq {
                capture1,
                capture2,
                value,
            } => self.evaluate_eq(*capture1, capture2.as_ref(), value.as_ref(), captures),

            Predicate::NotEq {
                capture1,
                capture2,
                value,
            } => {
                // Strict: missing key does NOT satisfy NotEq
                if !captures.contains_key(capture1) {
                    return false;
                }
                if let Some(c2) = capture2
                    && !captures.contains_key(c2)
                {
                    return false;
                }
                !self.evaluate_eq(*capture1, capture2.as_ref(), value.as_ref(), captures)
            }

            Predicate::Match { capture, regex } => self.evaluate_match(*capture, regex, captures),

            Predicate::NotMatch { capture, regex } => self
                .evaluate_match_result(*capture, regex, captures)
                .map(|matched| !matched)
                .unwrap_or(false),

            Predicate::AnyOf { capture, values } => {
                self.evaluate_any_of(*capture, values, captures)
            }

            Predicate::Set { .. } | Predicate::Is { .. } | Predicate::IsNot { .. } => {
                // Property predicates are handled separately
                true
            }

            Predicate::Custom { name: _, args: _ } => {
                // Custom predicates need external handlers
                // eprintln!("Warning: Custom predicate '{}' not implemented", name);
                true
            }
        }
    }

    /// Evaluate #eq? predicate
    fn evaluate_eq(
        &self,
        capture1: u32,
        capture2: Option<&u32>,
        value: Option<&String>,
        captures: &HashMap<u32, ParseNode>,
    ) -> bool {
        if let Some(node1) = captures.get(&capture1)
            && let Some(text1) = self.node_text(node1)
        {
            if let Some(capture2) = capture2 {
                // Compare with another capture
                if let Some(node2) = captures.get(capture2)
                    && let Some(text2) = self.node_text(node2)
                {
                    return text1 == text2;
                }
            } else if let Some(value) = value {
                // Compare with literal value
                return text1 == value;
            }
        }
        false
    }

    /// Evaluate #match? predicate
    fn evaluate_match(
        &self,
        capture: u32,
        regex_str: &str,
        captures: &HashMap<u32, ParseNode>,
    ) -> bool {
        self.evaluate_match_result(capture, regex_str, captures)
            .unwrap_or(false)
    }

    fn evaluate_match_result(
        &self,
        capture: u32,
        regex_str: &str,
        captures: &HashMap<u32, ParseNode>,
    ) -> Option<bool> {
        if let Some(node) = captures.get(&capture)
            && let Some(text) = self.node_text(node)
        {
            // Get or compile regex
            let mut cache = self.regex_cache.borrow_mut();
            let regex = if let Some(regex) = cache.get(regex_str).cloned() {
                regex
            } else {
                let Ok(regex) = Regex::new(regex_str) else {
                    return None;
                };
                cache.insert(regex_str.to_string(), regex);
                cache.get(regex_str).cloned()?
            };

            // Full-string match: the entire text must match the regex
            if let Some(m) = regex.find(text) {
                return Some(m.start() == 0 && m.end() == text.len());
            }
            return Some(false);
        }
        None
    }

    /// Evaluate #any-of? predicate
    fn evaluate_any_of(
        &self,
        capture: u32,
        values: &[String],
        captures: &HashMap<u32, ParseNode>,
    ) -> bool {
        if let Some(node) = captures.get(&capture)
            && let Some(text) = self.node_text(node)
        {
            return values.iter().any(|v| text == v);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::SymbolId;

    fn make_node(start: usize, end: usize) -> ParseNode {
        let symbol_id = SymbolId(0);
        ParseNode {
            symbol: symbol_id,
            symbol_id,
            children: vec![],
            start_byte: start,
            end_byte: end,
            field_name: None,
            alias_symbol_id: None,
        }
    }

    #[test]
    fn test_eq_captures() {
        let source = "hello world hello";
        let ctx = PredicateContext::new(source);

        let mut captures = HashMap::new();
        captures.insert(0, make_node(0, 5)); // "hello"
        captures.insert(1, make_node(12, 17)); // "hello"
        captures.insert(2, make_node(6, 11)); // "world"

        // Test equal captures
        let pred = Predicate::Eq {
            capture1: 0,
            capture2: Some(1),
            value: None,
        };
        assert!(ctx.evaluate(&pred, &captures));

        // Test unequal captures
        let pred = Predicate::Eq {
            capture1: 0,
            capture2: Some(2),
            value: None,
        };
        assert!(!ctx.evaluate(&pred, &captures));
    }

    #[test]
    fn test_eq_value() {
        let source = "hello world";
        let ctx = PredicateContext::new(source);

        let mut captures = HashMap::new();
        captures.insert(0, make_node(0, 5)); // "hello"

        // Test equal to value
        let pred = Predicate::Eq {
            capture1: 0,
            capture2: None,
            value: Some("hello".to_string()),
        };
        assert!(ctx.evaluate(&pred, &captures));

        // Test not equal to value
        let pred = Predicate::Eq {
            capture1: 0,
            capture2: None,
            value: Some("world".to_string()),
        };
        assert!(!ctx.evaluate(&pred, &captures));
    }

    #[test]
    fn test_not_eq_literal_and_capture() {
        let source = "hello world";
        let ctx = PredicateContext::new(source);

        let mut captures = HashMap::new();
        captures.insert(0, make_node(0, 5)); // "hello"
        captures.insert(1, make_node(6, 11)); // "world"

        let pred = Predicate::NotEq {
            capture1: 0,
            capture2: None,
            value: Some("world".to_string()),
        };
        assert!(ctx.evaluate(&pred, &captures));

        let pred = Predicate::NotEq {
            capture1: 0,
            capture2: Some(1),
            value: None,
        };
        assert!(ctx.evaluate(&pred, &captures));

        let pred = Predicate::NotEq {
            capture1: 0,
            capture2: None,
            value: Some("hello".to_string()),
        };
        assert!(!ctx.evaluate(&pred, &captures));
    }

    #[test]
    fn test_match_regex() {
        let source = "variable_123";
        let ctx = PredicateContext::new(source);

        let mut captures = HashMap::new();
        captures.insert(0, make_node(0, 12)); // "variable_123"

        // Test matching regex
        let pred = Predicate::Match {
            capture: 0,
            regex: r"^[a-z_]\w*$".to_string(),
        };
        assert!(ctx.evaluate(&pred, &captures));

        // Test non-matching regex
        let pred = Predicate::Match {
            capture: 0,
            regex: r"^\d+$".to_string(),
        };
        assert!(!ctx.evaluate(&pred, &captures));
    }

    #[test]
    fn test_not_match_regex() {
        let source = "variable_123";
        let ctx = PredicateContext::new(source);

        let mut captures = HashMap::new();
        captures.insert(0, make_node(0, 12)); // "variable_123"

        let pred = Predicate::NotMatch {
            capture: 0,
            regex: r"^\d+$".to_string(),
        };
        assert!(ctx.evaluate(&pred, &captures));

        let pred = Predicate::NotMatch {
            capture: 0,
            regex: r"^[a-z_]\w*$".to_string(),
        };
        assert!(!ctx.evaluate(&pred, &captures));
    }

    #[test]
    fn test_any_of() {
        let source = "public";
        let ctx = PredicateContext::new(source);

        let mut captures = HashMap::new();
        captures.insert(0, make_node(0, 6)); // "public"

        // Test matching any-of
        let pred = Predicate::AnyOf {
            capture: 0,
            values: vec![
                "public".to_string(),
                "private".to_string(),
                "protected".to_string(),
            ],
        };
        assert!(ctx.evaluate(&pred, &captures));

        // Test non-matching any-of
        let pred = Predicate::AnyOf {
            capture: 0,
            values: vec!["const".to_string(), "let".to_string(), "var".to_string()],
        };
        assert!(!ctx.evaluate(&pred, &captures));
    }

    #[test]
    fn test_predicates_fail_closed_for_missing_capture() {
        let ctx = PredicateContext::new("hello");
        let captures = HashMap::new();

        let predicates = [
            Predicate::Eq {
                capture1: 0,
                capture2: None,
                value: Some("hello".to_string()),
            },
            Predicate::NotEq {
                capture1: 0,
                capture2: None,
                value: Some("hello".to_string()),
            },
            Predicate::Match {
                capture: 0,
                regex: "hello".to_string(),
            },
            Predicate::NotMatch {
                capture: 0,
                regex: "hello".to_string(),
            },
            Predicate::AnyOf {
                capture: 0,
                values: vec!["hello".to_string()],
            },
        ];

        for predicate in predicates {
            assert!(!ctx.evaluate(&predicate, &captures));
        }
    }

    #[test]
    fn test_invalid_regex_fails_closed() {
        let ctx = PredicateContext::new("hello");
        let mut captures = HashMap::new();
        captures.insert(0, make_node(0, 5));

        let pred = Predicate::Match {
            capture: 0,
            regex: "(".to_string(),
        };
        assert!(!ctx.evaluate(&pred, &captures));

        let pred = Predicate::NotMatch {
            capture: 0,
            regex: "(".to_string(),
        };
        assert!(!ctx.evaluate(&pred, &captures));
    }

    #[test]
    fn test_invalid_node_range_fails_closed() {
        let ctx = PredicateContext::new("hello");
        let mut captures = HashMap::new();
        captures.insert(0, make_node(0, 50));

        let predicates = [
            Predicate::Eq {
                capture1: 0,
                capture2: None,
                value: Some("hello".to_string()),
            },
            Predicate::Match {
                capture: 0,
                regex: "hello".to_string(),
            },
            Predicate::NotMatch {
                capture: 0,
                regex: "world".to_string(),
            },
            Predicate::AnyOf {
                capture: 0,
                values: vec!["hello".to_string()],
            },
        ];

        for predicate in predicates {
            assert!(!ctx.evaluate(&predicate, &captures));
        }
    }
}
