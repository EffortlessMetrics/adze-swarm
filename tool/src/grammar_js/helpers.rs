use super::Rule;
use anyhow::{Result, bail};

/// Common helper functions used in Tree-sitter grammars
pub struct HelperFunctions;

impl HelperFunctions {
    /// Check if a function name is a known helper pattern
    pub fn is_helper_function(name: &str) -> bool {
        matches!(
            name,
            "commaSep"
                | "commaSep1"
                | "sep"
                | "sep1"
                | "sepBy"
                | "sepBy1"
                | "list"
                | "list1"
                | "delimited"
                | "parens"
                | "brackets"
                | "braces"
        )
    }

    /// Evaluate a helper function call
    pub fn evaluate_helper(name: &str, args: Vec<Rule>) -> Result<Rule> {
        match name {
            "commaSep" => {
                // commaSep(rule) => optional(seq(rule, repeat(seq(',', rule))))
                if args.len() != 1 {
                    bail!("commaSep expects 1 argument, got {}", args.len());
                }
                let rule = args.into_iter().next().unwrap();
                Ok(Rule::Optional {
                    value: Box::new(Rule::Seq {
                        members: vec![
                            rule.clone(),
                            Rule::Repeat {
                                content: Box::new(Rule::Seq {
                                    members: vec![
                                        Rule::String {
                                            value: ",".to_string(),
                                        },
                                        rule,
                                    ],
                                }),
                            },
                        ],
                    }),
                })
            }

            "commaSep1" => {
                // commaSep1(rule) => seq(rule, repeat(seq(',', rule)))
                if args.len() != 1 {
                    bail!("commaSep1 expects 1 argument, got {}", args.len());
                }
                let rule = args.into_iter().next().unwrap();
                Ok(Rule::Seq {
                    members: vec![
                        rule.clone(),
                        Rule::Repeat {
                            content: Box::new(Rule::Seq {
                                members: vec![
                                    Rule::String {
                                        value: ",".to_string(),
                                    },
                                    rule,
                                ],
                            }),
                        },
                    ],
                })
            }

            "sep" => {
                // sep(rule, separator) => optional(seq(rule, repeat(seq(separator, rule))))
                if args.len() != 2 {
                    bail!("sep expects 2 arguments, got {}", args.len());
                }
                let mut iter = args.into_iter();
                let rule = iter.next().unwrap();
                let separator = iter.next().unwrap();

                Ok(Rule::Optional {
                    value: Box::new(Rule::Seq {
                        members: vec![
                            rule.clone(),
                            Rule::Repeat {
                                content: Box::new(Rule::Seq {
                                    members: vec![separator, rule],
                                }),
                            },
                        ],
                    }),
                })
            }

            "sep1" => {
                // sep1(rule, separator) => seq(rule, repeat(seq(separator, rule)))
                if args.len() != 2 {
                    bail!("sep1 expects 2 arguments, got {}", args.len());
                }
                let mut iter = args.into_iter();
                let rule = iter.next().unwrap();
                let separator = iter.next().unwrap();

                Ok(Rule::Seq {
                    members: vec![
                        rule.clone(),
                        Rule::Repeat {
                            content: Box::new(Rule::Seq {
                                members: vec![separator, rule],
                            }),
                        },
                    ],
                })
            }

            "parens" => {
                // parens(rule) => seq('(', rule, ')')
                if args.len() != 1 {
                    bail!("parens expects 1 argument, got {}", args.len());
                }
                let rule = args.into_iter().next().unwrap();
                Ok(Rule::Seq {
                    members: vec![
                        Rule::String {
                            value: "(".to_string(),
                        },
                        rule,
                        Rule::String {
                            value: ")".to_string(),
                        },
                    ],
                })
            }

            "brackets" => {
                // brackets(rule) => seq('[', rule, ']')
                if args.len() != 1 {
                    bail!("brackets expects 1 argument, got {}", args.len());
                }
                let rule = args.into_iter().next().unwrap();
                Ok(Rule::Seq {
                    members: vec![
                        Rule::String {
                            value: "[".to_string(),
                        },
                        rule,
                        Rule::String {
                            value: "]".to_string(),
                        },
                    ],
                })
            }

            "braces" => {
                // braces(rule) => seq('{', rule, '}')
                if args.len() != 1 {
                    bail!("braces expects 1 argument, got {}", args.len());
                }
                let rule = args.into_iter().next().unwrap();
                Ok(Rule::Seq {
                    members: vec![
                        Rule::String {
                            value: "{".to_string(),
                        },
                        rule,
                        Rule::String {
                            value: "}".to_string(),
                        },
                    ],
                })
            }

            _ => bail!("Unknown helper function: {}", name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym(name: &str) -> Rule {
        Rule::Symbol {
            name: name.to_string(),
        }
    }

    fn s(value: &str) -> Rule {
        Rule::String {
            value: value.to_string(),
        }
    }

    /// Recognised names should match exactly.
    #[test]
    fn is_helper_function_recognises_known_helpers() {
        for name in [
            "commaSep",
            "commaSep1",
            "sep",
            "sep1",
            "sepBy",
            "sepBy1",
            "list",
            "list1",
            "delimited",
            "parens",
            "brackets",
            "braces",
        ] {
            assert!(
                HelperFunctions::is_helper_function(name),
                "expected helper: {name}"
            );
        }
    }

    /// Anything outside the known set, including empty and case variants, is rejected.
    #[test]
    fn is_helper_function_rejects_unknown_and_case_variants() {
        for name in [
            "", "commasep", "COMMASEP", "Sep", "field", "unknown", "parens ",
        ] {
            assert!(
                !HelperFunctions::is_helper_function(name),
                "did not expect helper: {name:?}"
            );
        }
    }

    /// commaSep(rule) => optional(seq(rule, repeat(seq(',', rule)))).
    #[test]
    fn evaluate_helper_comma_sep_expands_to_optional_seq() {
        let rule = sym("expr");
        let out = HelperFunctions::evaluate_helper("commaSep", vec![rule.clone()]).unwrap();

        let Rule::Optional { value } = out else {
            panic!("expected Optional, got {out:?}");
        };
        let Rule::Seq { members } = *value else {
            panic!("expected Seq inside Optional");
        };
        assert_eq!(members.len(), 2);
        assert!(matches!(&members[0], Rule::Symbol { name } if name == "expr"));
        let Rule::Repeat { content } = &members[1] else {
            panic!("expected Repeat as second member");
        };
        let Rule::Seq { members: inner } = content.as_ref() else {
            panic!("expected Seq inside Repeat");
        };
        assert_eq!(inner.len(), 2);
        assert!(matches!(&inner[0], Rule::String { value } if value == ","));
        assert!(matches!(&inner[1], Rule::Symbol { name } if name == "expr"));
    }

    /// commaSep1(rule) drops the outer Optional wrapper.
    #[test]
    fn evaluate_helper_comma_sep1_omits_optional_wrapper() {
        let out = HelperFunctions::evaluate_helper("commaSep1", vec![sym("expr")]).unwrap();
        let Rule::Seq { members } = out else {
            panic!("expected Seq, got {out:?}");
        };
        assert_eq!(members.len(), 2);
        assert!(matches!(&members[0], Rule::Symbol { name } if name == "expr"));
        let Rule::Repeat { content } = &members[1] else {
            panic!("expected Repeat as second member");
        };
        let Rule::Seq { members: inner } = content.as_ref() else {
            panic!("expected Seq inside Repeat");
        };
        assert!(matches!(&inner[0], Rule::String { value } if value == ","));
    }

    /// sep(rule, separator) uses caller-supplied separator and an Optional wrapper.
    #[test]
    fn evaluate_helper_sep_uses_custom_separator() {
        let out = HelperFunctions::evaluate_helper("sep", vec![sym("item"), s(";")]).unwrap();
        let Rule::Optional { value } = out else {
            panic!("expected Optional, got {out:?}");
        };
        let Rule::Seq { members } = *value else {
            panic!("expected Seq inside Optional");
        };
        let Rule::Repeat { content } = &members[1] else {
            panic!("expected Repeat as second member");
        };
        let Rule::Seq { members: inner } = content.as_ref() else {
            panic!("expected Seq inside Repeat");
        };
        // Separator must be the caller-provided ";" (not a literal comma).
        assert!(matches!(&inner[0], Rule::String { value } if value == ";"));
        assert!(matches!(&inner[1], Rule::Symbol { name } if name == "item"));
    }

    /// sep1(rule, separator) omits the Optional wrapper.
    #[test]
    fn evaluate_helper_sep1_is_required_with_custom_separator() {
        let out = HelperFunctions::evaluate_helper("sep1", vec![sym("item"), s("|")]).unwrap();
        let Rule::Seq { members } = out else {
            panic!("expected Seq, got {out:?}");
        };
        assert_eq!(members.len(), 2);
        let Rule::Repeat { content } = &members[1] else {
            panic!("expected Repeat as second member");
        };
        let Rule::Seq { members: inner } = content.as_ref() else {
            panic!("expected Seq inside Repeat");
        };
        assert!(matches!(&inner[0], Rule::String { value } if value == "|"));
    }

    /// parens(rule) => seq('(', rule, ')').
    #[test]
    fn evaluate_helper_parens_wraps_in_round_brackets() {
        let out = HelperFunctions::evaluate_helper("parens", vec![sym("inner")]).unwrap();
        let Rule::Seq { members } = out else {
            panic!("expected Seq, got {out:?}");
        };
        assert_eq!(members.len(), 3);
        assert!(matches!(&members[0], Rule::String { value } if value == "("));
        assert!(matches!(&members[1], Rule::Symbol { name } if name == "inner"));
        assert!(matches!(&members[2], Rule::String { value } if value == ")"));
    }

    /// brackets(rule) => seq('[', rule, ']').
    #[test]
    fn evaluate_helper_brackets_wraps_in_square_brackets() {
        let out = HelperFunctions::evaluate_helper("brackets", vec![sym("inner")]).unwrap();
        let Rule::Seq { members } = out else {
            panic!("expected Seq, got {out:?}");
        };
        assert_eq!(members.len(), 3);
        assert!(matches!(&members[0], Rule::String { value } if value == "["));
        assert!(matches!(&members[2], Rule::String { value } if value == "]"));
    }

    /// braces(rule) => seq('{', rule, '}').
    #[test]
    fn evaluate_helper_braces_wraps_in_curly_braces() {
        let out = HelperFunctions::evaluate_helper("braces", vec![sym("inner")]).unwrap();
        let Rule::Seq { members } = out else {
            panic!("expected Seq, got {out:?}");
        };
        assert_eq!(members.len(), 3);
        assert!(matches!(&members[0], Rule::String { value } if value == "{"));
        assert!(matches!(&members[2], Rule::String { value } if value == "}"));
    }

    /// Single-argument helpers reject empty argument lists with arity errors.
    #[test]
    fn evaluate_helper_single_arg_helpers_reject_empty_args() {
        for name in ["commaSep", "commaSep1", "parens", "brackets", "braces"] {
            let err = HelperFunctions::evaluate_helper(name, vec![])
                .expect_err(&format!("{name} should require an argument"));
            let msg = err.to_string();
            assert!(
                msg.contains(name) && msg.contains("got 0"),
                "unexpected error for {name}: {msg}"
            );
        }
    }

    /// Single-argument helpers reject extra arguments.
    #[test]
    fn evaluate_helper_single_arg_helpers_reject_too_many_args() {
        let extra = vec![sym("a"), sym("b")];
        for name in ["commaSep", "commaSep1", "parens", "brackets", "braces"] {
            let err = HelperFunctions::evaluate_helper(name, extra.clone())
                .expect_err(&format!("{name} should reject 2 args"));
            assert!(
                err.to_string().contains("got 2"),
                "unexpected error for {name}: {err}"
            );
        }
    }

    /// Two-argument helpers reject mis-arity in either direction.
    #[test]
    fn evaluate_helper_two_arg_helpers_enforce_arity() {
        for name in ["sep", "sep1"] {
            let too_few = HelperFunctions::evaluate_helper(name, vec![sym("x")])
                .expect_err(&format!("{name} should require 2 args"));
            assert!(
                too_few.to_string().contains("got 1"),
                "unexpected error for {name}: {too_few}"
            );

            let too_many = HelperFunctions::evaluate_helper(name, vec![sym("x"), s(","), sym("y")])
                .expect_err(&format!("{name} should reject 3 args"));
            assert!(
                too_many.to_string().contains("got 3"),
                "unexpected error for {name}: {too_many}"
            );
        }
    }

    /// Unknown helper names produce a descriptive error.
    #[test]
    fn evaluate_helper_unknown_name_errors() {
        let err = HelperFunctions::evaluate_helper("not_a_helper", vec![sym("x")]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Unknown helper function"));
        assert!(msg.contains("not_a_helper"));
    }

    /// Names recognised by `is_helper_function` but not implemented in
    /// `evaluate_helper` (sepBy/sepBy1/list/list1/delimited) fall through to
    /// the unknown-name branch. This documents the current behaviour.
    #[test]
    fn evaluate_helper_recognised_but_unimplemented_names_error() {
        for name in ["sepBy", "sepBy1", "list", "list1", "delimited"] {
            assert!(HelperFunctions::is_helper_function(name));
            let err = HelperFunctions::evaluate_helper(name, vec![sym("x")]).unwrap_err();
            assert!(
                err.to_string().contains("Unknown helper function"),
                "{name} should fall through: {err}"
            );
        }
    }

    /// The rule argument should be cloned, not aliased — confirm by passing a
    /// complex Seq and ensuring both occurrences in the expansion compare equal.
    #[test]
    fn evaluate_helper_clones_rule_for_repeat_body() {
        let complex = Rule::Seq {
            members: vec![sym("a"), sym("b")],
        };
        let out = HelperFunctions::evaluate_helper("commaSep1", vec![complex.clone()]).unwrap();
        let Rule::Seq { members } = out else {
            panic!("expected Seq");
        };
        // First member is the original rule.
        let first_json = serde_json::to_string(&members[0]).unwrap();
        let expected_json = serde_json::to_string(&complex).unwrap();
        assert_eq!(first_json, expected_json);
        // Repeat body's second element is also the original rule.
        let Rule::Repeat { content } = &members[1] else {
            panic!("expected Repeat");
        };
        let Rule::Seq { members: inner } = content.as_ref() else {
            panic!("expected inner Seq");
        };
        let inner_json = serde_json::to_string(&inner[1]).unwrap();
        assert_eq!(inner_json, expected_json);
    }

    /// commaSep accepts any Rule shape, including Choice; structure is preserved.
    #[test]
    fn evaluate_helper_accepts_arbitrary_rule_shapes() {
        let choice = Rule::Choice {
            members: vec![s("a"), s("b")],
        };
        let out = HelperFunctions::evaluate_helper("parens", vec![choice]).unwrap();
        let Rule::Seq { members } = out else {
            panic!("expected Seq");
        };
        assert!(matches!(&members[1], Rule::Choice { members } if members.len() == 2));
    }
}
