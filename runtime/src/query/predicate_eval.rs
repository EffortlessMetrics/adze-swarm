// Predicate evaluation for query patterns
use super::ast::Predicate;
use crate::parser_v4::ParseNode;
use regex::Regex;
use std::cell::RefCell;
use std::collections::HashMap;

/// Compile `pattern` so the regex operation itself requires a full-string
/// match. `\A`/`\z` are absolute anchors, unaffected by multiline flags
/// inside the pattern, and the non-capturing group keeps alternation and
/// lazy quantifiers inside the anchors so the engine can backtrack into a
/// longer alternative (for example `a|ab` on `"ab"`).
pub(crate) fn compile_full_string_regex(pattern: &str) -> Result<Regex, regex::Error> {
    // Validate the raw pattern before wrapping: interpolation alone can
    // balance malformed syntax (for example `a)|(b` becomes the valid
    // `\A(?:a)|(b)\z`, whose alternatives escape the anchors), turning a
    // user error into a silently wrong matcher.
    Regex::new(pattern)?;
    Regex::new(&format!(r"\A(?:{pattern})\z"))
}

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

    /// All occurrence texts for a capture slot.
    ///
    /// Returns `None` when the slot has no occurrences or any occurrence
    /// text is unavailable (missing capture, out-of-bounds or reversed
    /// range, or a range that does not fall on UTF-8 boundaries).
    /// Predicates evaluate every occurrence under universal semantics: a
    /// constraint holds only when it holds for all of them, so a failing
    /// nonfinal occurrence can never be hidden by a passing final one.
    fn capture_texts(
        &self,
        capture: u32,
        captures: &HashMap<u32, Vec<ParseNode>>,
    ) -> Option<Vec<&str>> {
        let nodes = captures.get(&capture).filter(|nodes| !nodes.is_empty())?;
        nodes.iter().map(|node| self.node_text(node)).collect()
    }

    /// Evaluate a predicate. Invalid or unavailable operands fail closed:
    /// they are distinct from a valid `false` comparison, and negation
    /// (`NotEq`/`NotMatch`) never turns them into `true`.
    ///
    /// Multi-occurrence captures evaluate universally: every occurrence
    /// must satisfy the constraint.
    pub fn evaluate(&self, predicate: &Predicate, captures: &HashMap<u32, Vec<ParseNode>>) -> bool {
        match predicate {
            Predicate::Eq {
                capture1,
                capture2,
                value,
            } => self
                .evaluate_eq_result(*capture1, capture2.as_ref(), value.as_ref(), captures)
                .unwrap_or(false),

            Predicate::NotEq {
                capture1,
                capture2,
                value,
            } => self
                .evaluate_eq_result(*capture1, capture2.as_ref(), value.as_ref(), captures)
                .map(|equal| !equal)
                .unwrap_or(false),

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

    /// Evaluate an equality comparison as `Some(equal)` only when every
    /// operand is available and valid; `None` is invalid/unavailable and
    /// must fail closed for both `Eq` and `NotEq`. With repeated captures,
    /// every occurrence text must agree.
    fn evaluate_eq_result(
        &self,
        capture1: u32,
        capture2: Option<&u32>,
        value: Option<&String>,
        captures: &HashMap<u32, Vec<ParseNode>>,
    ) -> Option<bool> {
        let texts1 = self.capture_texts(capture1, captures)?;
        if let Some(capture2) = capture2 {
            let texts2 = self.capture_texts(*capture2, captures)?;
            let mut all = texts1;
            all.extend(texts2);
            Some(all.windows(2).all(|pair| pair[0] == pair[1]))
        } else {
            let value = value?;
            Some(texts1.iter().all(|text| *text == value))
        }
    }

    /// Evaluate #match? predicate
    fn evaluate_match(
        &self,
        capture: u32,
        regex_str: &str,
        captures: &HashMap<u32, Vec<ParseNode>>,
    ) -> bool {
        self.evaluate_match_result(capture, regex_str, captures)
            .unwrap_or(false)
    }

    fn evaluate_match_result(
        &self,
        capture: u32,
        regex_str: &str,
        captures: &HashMap<u32, Vec<ParseNode>>,
    ) -> Option<bool> {
        let texts = self.capture_texts(capture, captures)?;
        let mut cache = self.regex_cache.borrow_mut();
        let regex = if let Some(regex) = cache.get(regex_str) {
            regex.clone()
        } else {
            // Manually constructed patterns bypass compile-time validation;
            // an invalid pattern is unavailable (`None`), never a match.
            let regex = compile_full_string_regex(regex_str).ok()?;
            cache.insert(regex_str.to_string(), regex.clone());
            regex
        };
        Some(texts.iter().all(|text| regex.is_match(text)))
    }

    /// Evaluate #any-of? predicate: every occurrence must name a listed value.
    fn evaluate_any_of(
        &self,
        capture: u32,
        values: &[String],
        captures: &HashMap<u32, Vec<ParseNode>>,
    ) -> bool {
        match self.capture_texts(capture, captures) {
            Some(texts) => texts
                .iter()
                .all(|text| values.iter().any(|value| value == text)),
            None => false,
        }
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
        captures.insert(0, vec![make_node(0, 5)]); // "hello"
        captures.insert(1, vec![make_node(12, 17)]); // "hello"
        captures.insert(2, vec![make_node(6, 11)]); // "world"

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
        captures.insert(0, vec![make_node(0, 5)]); // "hello"

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
        captures.insert(0, vec![make_node(0, 5)]); // "hello"
        captures.insert(1, vec![make_node(6, 11)]); // "world"

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
        captures.insert(0, vec![make_node(0, 12)]); // "variable_123"

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
        captures.insert(0, vec![make_node(0, 12)]); // "variable_123"

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
        captures.insert(0, vec![make_node(0, 6)]); // "public"

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
        captures.insert(0, vec![make_node(0, 5)]);

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
    fn test_not_eq_invalid_ranges_fail_closed() {
        // (source, start, end): out-of-bounds, reversed, non-UTF-8 boundary.
        for (source, start, end) in [("a", 0, 2), ("ab", 2, 1), ("λ", 0, 1)] {
            let ctx = PredicateContext::new(source);
            let mut captures = HashMap::new();
            captures.insert(0, vec![make_node(start, end)]);
            for predicate in [
                Predicate::Eq {
                    capture1: 0,
                    capture2: None,
                    value: Some("different".to_string()),
                },
                Predicate::NotEq {
                    capture1: 0,
                    capture2: None,
                    value: Some("different".to_string()),
                },
                Predicate::Match {
                    capture: 0,
                    regex: ".*".to_string(),
                },
                Predicate::NotMatch {
                    capture: 0,
                    regex: "different".to_string(),
                },
                Predicate::AnyOf {
                    capture: 0,
                    values: vec!["different".to_string()],
                },
            ] {
                assert!(
                    !ctx.evaluate(&predicate, &captures),
                    "{predicate:?} must fail closed for range {start}..{end} in {source:?}"
                );
            }
        }
    }

    #[test]
    fn test_capture_to_capture_invalid_operand_fails_closed() {
        let ctx = PredicateContext::new("ab");
        // Second operand out of bounds.
        let mut captures = HashMap::new();
        captures.insert(0, vec![make_node(0, 1)]);
        captures.insert(1, vec![make_node(0, 3)]);
        for predicate in [
            Predicate::Eq {
                capture1: 0,
                capture2: Some(1),
                value: None,
            },
            Predicate::NotEq {
                capture1: 0,
                capture2: Some(1),
                value: None,
            },
        ] {
            assert!(!ctx.evaluate(&predicate, &captures));
        }
        // First operand missing, second valid.
        let mut captures = HashMap::new();
        captures.insert(1, vec![make_node(0, 1)]);
        for predicate in [
            Predicate::Eq {
                capture1: 0,
                capture2: Some(1),
                value: None,
            },
            Predicate::NotEq {
                capture1: 0,
                capture2: Some(1),
                value: None,
            },
        ] {
            assert!(!ctx.evaluate(&predicate, &captures));
        }
        // Eq with no second operand at all is unavailable, not true for NotEq.
        let mut captures = HashMap::new();
        captures.insert(0, vec![make_node(0, 1)]);
        assert!(!ctx.evaluate(
            &Predicate::NotEq {
                capture1: 0,
                capture2: None,
                value: None,
            },
            &captures
        ));
    }

    #[test]
    fn test_eq_and_not_eq_are_inverses_on_valid_strings() {
        let ctx = PredicateContext::new("aa ab");
        let mut captures = HashMap::new();
        captures.insert(0, vec![make_node(0, 1)]); // "a"
        captures.insert(1, vec![make_node(1, 2)]); // "a" (equal text, other range)
        captures.insert(2, vec![make_node(3, 5)]); // "ab"
        for (capture2, expected_eq) in [(1, true), (2, false)] {
            let eq = Predicate::Eq {
                capture1: 0,
                capture2: Some(capture2),
                value: None,
            };
            let not_eq = Predicate::NotEq {
                capture1: 0,
                capture2: Some(capture2),
                value: None,
            };
            assert_eq!(ctx.evaluate(&eq, &captures), expected_eq);
            assert_eq!(ctx.evaluate(&not_eq, &captures), !expected_eq);
        }
    }

    #[test]
    fn test_full_string_regex_fixtures() {
        // (text, pattern, expected Match result); NotMatch must be its inverse.
        let cases: &[(&str, &str, bool)] = &[
            ("ab", "a|ab", true),
            ("a", "a|ab", true),
            ("b", "a|ab", false),
            ("ab", ".*?", true),
            ("", ".*?", true),
            ("ab", ".+?", true),
            ("abc", "^[a-z]+$", true),
            ("abc1", "^[a-z]+$", false),
            ("abc", "[a-z]+", true),
            ("abc1", "[a-z]+", false),
            ("", "a*", true),
            ("", "a+", false),
            ("", "", true),
            ("a", "", false),
            ("abc", "(?m)^[a-z]+$", true),
            ("a\nb", "(?s).*", true),
            ("a\nb", "(?m)^b$", false),
            ("abc\ndef", "(?m)(?:^[a-z]+$\\n?)+", true),
            ("λλ", "λ+", true),
            ("λλa", "λ+", false),
            ("αβγ", r"\p{Greek}+", true),
            ("λ", ".", true),
            ("λλ", ".", false),
        ];
        for (text, pattern, expected) in cases {
            let ctx = PredicateContext::new(text);
            let mut captures = HashMap::new();
            captures.insert(0, vec![make_node(0, text.len())]);
            let matched = Predicate::Match {
                capture: 0,
                regex: pattern.to_string(),
            };
            let not_matched = Predicate::NotMatch {
                capture: 0,
                regex: pattern.to_string(),
            };
            assert_eq!(
                ctx.evaluate(&matched, &captures),
                *expected,
                "#match? {pattern:?} on {text:?}"
            );
            assert_eq!(
                ctx.evaluate(&not_matched, &captures),
                !expected,
                "#not-match? {pattern:?} on {text:?}"
            );
        }
    }

    #[test]
    fn test_invalid_node_range_fails_closed() {
        let ctx = PredicateContext::new("hello");
        let mut captures = HashMap::new();
        captures.insert(0, vec![make_node(0, 50)]);

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

    #[test]
    fn test_full_string_regex_rejects_pattern_balanced_only_by_wrapping() {
        // `a)|(b` is invalid standalone, but interpolation alone balances it
        // into `\A(?:a)|(b)\z`, whose alternatives escape the anchors. The
        // raw pattern must be validated before wrapping.
        assert!(compile_full_string_regex("a)|(b").is_err());
        // Balanced patterns still compile and stay fully anchored.
        let anchored = compile_full_string_regex("a|ab").expect("balanced alternation");
        assert!(anchored.is_match("ab"));
        assert!(!anchored.is_match("abc"));
    }

    #[test]
    fn test_malformed_match_pattern_fails_closed_on_both_surfaces() {
        let source = "a";
        let ctx = PredicateContext::new(source);
        let mut captures = HashMap::new();
        captures.insert(0, vec![make_node(0, 1)]);

        // Manually constructed predicate: an invalid pattern is unavailable,
        // never a match, and negation never turns it into a match.
        let matched = Predicate::Match {
            capture: 0,
            regex: "a)|(b".to_string(),
        };
        assert!(!ctx.evaluate(&matched, &captures));
        let not_matched = Predicate::NotMatch {
            capture: 0,
            regex: "a)|(b".to_string(),
        };
        assert!(!ctx.evaluate(&not_matched, &captures));
    }

    #[test]
    fn test_repeated_captures_evaluate_universally() {
        // "ab ab": occurrences at [0..2] and [3..5].
        let source = "ab ab";
        let ctx = PredicateContext::new(source);
        let mut captures = HashMap::new();
        captures.insert(0, vec![make_node(0, 2), make_node(3, 5)]);

        // Both occurrences satisfy the pattern.
        let matched = Predicate::Match {
            capture: 0,
            regex: "ab".to_string(),
        };
        assert!(ctx.evaluate(&matched, &captures));
        let not_matched = Predicate::NotMatch {
            capture: 0,
            regex: "ab".to_string(),
        };
        assert!(!ctx.evaluate(&not_matched, &captures));

        // Equality across occurrences: both are "ab".
        let eq = Predicate::Eq {
            capture1: 0,
            capture2: None,
            value: Some("ab".to_string()),
        };
        assert!(ctx.evaluate(&eq, &captures));
    }

    #[test]
    fn test_failing_nonfinal_occurrence_is_not_hidden() {
        // "ab cd": first occurrence matches, second does not. The failing
        // nonfinal... (here final) occurrence must fail the predicate even
        // though an earlier occurrence passes.
        let source = "ab cd";
        let ctx = PredicateContext::new(source);
        let mut captures = HashMap::new();
        captures.insert(0, vec![make_node(0, 2), make_node(3, 5)]);

        let matched = Predicate::Match {
            capture: 0,
            regex: "ab".to_string(),
        };
        assert!(
            !ctx.evaluate(&matched, &captures),
            "a failing occurrence must fail the predicate even when another passes"
        );
        // Negation of a non-universally-satisfied constraint holds.
        let not_matched = Predicate::NotMatch {
            capture: 0,
            regex: "ab".to_string(),
        };
        assert!(ctx.evaluate(&not_matched, &captures));

        // And the reverse: failing FIRST occurrence is not hidden by a
        // passing final occurrence.
        let mut reversed = HashMap::new();
        reversed.insert(0, vec![make_node(3, 5), make_node(0, 2)]);
        assert!(!ctx.evaluate(&matched, &reversed));
    }

    #[test]
    fn test_empty_capture_slot_fails_closed() {
        let ctx = PredicateContext::new("hello");
        let mut captures: HashMap<u32, Vec<ParseNode>> = HashMap::new();
        captures.insert(0, Vec::new());

        for predicate in [
            Predicate::Match {
                capture: 0,
                regex: "hello".to_string(),
            },
            Predicate::NotMatch {
                capture: 0,
                regex: "hello".to_string(),
            },
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
            Predicate::AnyOf {
                capture: 0,
                values: vec!["hello".to_string()],
            },
        ] {
            assert!(
                !ctx.evaluate(&predicate, &captures),
                "empty slot must fail closed: {predicate:?}"
            );
        }
    }
}
