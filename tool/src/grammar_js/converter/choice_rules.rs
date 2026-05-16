use super::{GrammarJsConverter, JsRule};
use adze_ir::{Associativity, Grammar, PrecedenceKind, SymbolId};
use anyhow::Result;

impl GrammarJsConverter {
    pub(super) fn convert_choice_rule(
        &mut self,
        grammar: &mut Grammar,
        lhs: SymbolId,
        members: &[JsRule],
    ) -> Result<()> {
        eprintln!(
            "Debug: Converting CHOICE for {} with {} members",
            lhs.0,
            members.len()
        );

        for (index, member) in members.iter().enumerate() {
            eprintln!("Debug: Converting choice member {} for {}", index, lhs.0);
            self.convert_choice_member(grammar, lhs, member, index)?;
        }
        Ok(())
    }

    fn convert_choice_member(
        &mut self,
        grammar: &mut Grammar,
        lhs: SymbolId,
        member: &JsRule,
        index: usize,
    ) -> Result<()> {
        match member {
            JsRule::Prec { value, content } => self.convert_choice_member_with_precedence(
                grammar,
                lhs,
                content,
                *value,
                None,
                "precedence",
            ),
            JsRule::PrecLeft { value, content } => self.convert_choice_member_with_precedence(
                grammar,
                lhs,
                content,
                *value,
                Some(Associativity::Left),
                "left precedence",
            ),
            JsRule::PrecRight { value, content } => self.convert_choice_member_with_precedence(
                grammar,
                lhs,
                content,
                *value,
                Some(Associativity::Right),
                "right precedence",
            ),
            JsRule::Seq { members } => {
                eprintln!(
                    "Debug: CHOICE member {} is SEQ with {} members for {}",
                    index,
                    members.len(),
                    lhs.0
                );
                let (rhs, fields) = self.seq_to_rhs_and_fields(grammar, members);
                if !rhs.is_empty() {
                    eprintln!(
                        "Debug: Adding rule {} -> {:?} (from inlined SEQ)",
                        lhs.0, rhs
                    );
                    self.add_rule_with_fields(grammar, lhs, rhs, None, None, fields);
                }
                Ok(())
            }
            _ => {
                if let Some(symbol) = self.rule_to_symbol(grammar, member) {
                    eprintln!("Debug: Adding rule {} -> {:?}", lhs.0, symbol);
                    self.add_rule(grammar, lhs, vec![symbol], None, None);
                } else {
                    eprintln!(
                        "Debug: Failed to convert choice member {} for {}",
                        index, lhs.0
                    );
                }
                Ok(())
            }
        }
    }

    fn convert_choice_member_with_precedence(
        &mut self,
        grammar: &mut Grammar,
        lhs: SymbolId,
        content: &JsRule,
        value: i32,
        associativity: Option<Associativity>,
        label: &str,
    ) -> Result<()> {
        let precedence = Some(PrecedenceKind::Static(value as i16));
        if let JsRule::Seq { members } = content {
            let (rhs, fields) = self.seq_to_rhs_and_fields(grammar, members);
            if !rhs.is_empty() {
                eprintln!(
                    "Debug: Adding rule {} -> {:?} with {} {}",
                    lhs.0, rhs, label, value
                );
                self.add_rule_with_fields(grammar, lhs, rhs, precedence, associativity, fields);
                return Ok(());
            }
        }

        if let Some(symbol) = self.rule_to_symbol(grammar, content) {
            eprintln!(
                "Debug: Adding rule {} -> {:?} with {} {}",
                lhs.0, symbol, label, value
            );
            self.add_rule(grammar, lhs, vec![symbol], precedence, associativity);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::grammar_js::{GrammarJs, Rule as JsRule};
    use adze_ir::{Associativity, PrecedenceKind, Symbol};

    /// Build a grammar from a single `source_file` rule body, driving the
    /// converter via its public entry point so we exercise the helpers under
    /// realistic state.
    fn convert_source(rule: JsRule) -> adze_ir::Grammar {
        let mut grammar_js = GrammarJs::new("test".to_string());
        grammar_js.rules.insert("source_file".to_string(), rule);
        GrammarJsConverter::new(grammar_js).convert().unwrap()
    }

    fn source_file_id(grammar: &adze_ir::Grammar) -> adze_ir::SymbolId {
        grammar
            .rule_names
            .iter()
            .find_map(|(id, name)| (name == "source_file").then_some(*id))
            .expect("source_file should exist")
    }

    #[test]
    fn choice_with_two_symbol_members_emits_two_rules() {
        let mut grammar_js = GrammarJs::new("test".to_string());
        grammar_js.rules.insert(
            "source_file".to_string(),
            JsRule::Choice {
                members: vec![
                    JsRule::Symbol {
                        name: "a".to_string(),
                    },
                    JsRule::Symbol {
                        name: "b".to_string(),
                    },
                ],
            },
        );
        grammar_js.rules.insert(
            "a".to_string(),
            JsRule::Pattern {
                value: r"\d+".to_string(),
            },
        );
        grammar_js.rules.insert(
            "b".to_string(),
            JsRule::Pattern {
                value: r"[a-z]+".to_string(),
            },
        );

        let grammar = GrammarJsConverter::new(grammar_js).convert().unwrap();
        let source_file = source_file_id(&grammar);
        let rules = grammar.rules.get(&source_file).expect("rules exist");
        assert_eq!(rules.len(), 2, "each choice member yields one rule");
        assert!(rules.iter().all(|r| r.rhs.len() == 1));
        assert!(rules.iter().all(|r| r.precedence.is_none()));
        assert!(rules.iter().all(|r| r.associativity.is_none()));
    }

    #[test]
    fn choice_with_empty_members_emits_no_rules() {
        let grammar = convert_source(JsRule::Choice { members: vec![] });
        let source_file = source_file_id(&grammar);
        // No choice members means no rules added for source_file.
        assert!(
            grammar
                .rules
                .get(&source_file)
                .is_none_or(|rules| rules.is_empty())
        );
    }

    #[test]
    fn choice_member_prec_attaches_static_precedence_without_assoc() {
        let mut grammar_js = GrammarJs::new("test".to_string());
        grammar_js.rules.insert(
            "source_file".to_string(),
            JsRule::Choice {
                members: vec![JsRule::Prec {
                    value: 7,
                    content: Box::new(JsRule::Symbol {
                        name: "a".to_string(),
                    }),
                }],
            },
        );
        grammar_js.rules.insert(
            "a".to_string(),
            JsRule::Pattern {
                value: r"\d+".to_string(),
            },
        );

        let grammar = GrammarJsConverter::new(grammar_js).convert().unwrap();
        let source_file = source_file_id(&grammar);
        let rule = &grammar.rules[&source_file][0];
        assert_eq!(rule.precedence, Some(PrecedenceKind::Static(7)));
        assert_eq!(rule.associativity, None);
    }

    #[test]
    fn choice_member_prec_left_attaches_left_associativity() {
        let mut grammar_js = GrammarJs::new("test".to_string());
        grammar_js.rules.insert(
            "source_file".to_string(),
            JsRule::Choice {
                members: vec![JsRule::PrecLeft {
                    value: 3,
                    content: Box::new(JsRule::Symbol {
                        name: "a".to_string(),
                    }),
                }],
            },
        );
        grammar_js.rules.insert(
            "a".to_string(),
            JsRule::Pattern {
                value: r"\d+".to_string(),
            },
        );

        let grammar = GrammarJsConverter::new(grammar_js).convert().unwrap();
        let source_file = source_file_id(&grammar);
        let rule = &grammar.rules[&source_file][0];
        assert_eq!(rule.precedence, Some(PrecedenceKind::Static(3)));
        assert_eq!(rule.associativity, Some(Associativity::Left));
    }

    #[test]
    fn choice_member_prec_right_attaches_right_associativity() {
        let mut grammar_js = GrammarJs::new("test".to_string());
        grammar_js.rules.insert(
            "source_file".to_string(),
            JsRule::Choice {
                members: vec![JsRule::PrecRight {
                    value: 5,
                    content: Box::new(JsRule::Symbol {
                        name: "a".to_string(),
                    }),
                }],
            },
        );
        grammar_js.rules.insert(
            "a".to_string(),
            JsRule::Pattern {
                value: r"\d+".to_string(),
            },
        );

        let grammar = GrammarJsConverter::new(grammar_js).convert().unwrap();
        let source_file = source_file_id(&grammar);
        let rule = &grammar.rules[&source_file][0];
        assert_eq!(rule.precedence, Some(PrecedenceKind::Static(5)));
        assert_eq!(rule.associativity, Some(Associativity::Right));
    }

    #[test]
    fn choice_member_prec_with_seq_inlines_sequence_into_rhs() {
        let mut grammar_js = GrammarJs::new("test".to_string());
        grammar_js.rules.insert(
            "source_file".to_string(),
            JsRule::Choice {
                members: vec![JsRule::Prec {
                    value: 2,
                    content: Box::new(JsRule::Seq {
                        members: vec![
                            JsRule::Symbol {
                                name: "a".to_string(),
                            },
                            JsRule::Symbol {
                                name: "b".to_string(),
                            },
                        ],
                    }),
                }],
            },
        );
        grammar_js.rules.insert(
            "a".to_string(),
            JsRule::Pattern {
                value: r"\d+".to_string(),
            },
        );
        grammar_js.rules.insert(
            "b".to_string(),
            JsRule::Pattern {
                value: r"[a-z]+".to_string(),
            },
        );

        let grammar = GrammarJsConverter::new(grammar_js).convert().unwrap();
        let source_file = source_file_id(&grammar);
        let rule = &grammar.rules[&source_file][0];
        // The Seq members are inlined into the rhs (length 2), not wrapped.
        assert_eq!(rule.rhs.len(), 2);
        assert_eq!(rule.precedence, Some(PrecedenceKind::Static(2)));
    }

    #[test]
    fn choice_member_prec_with_empty_seq_emits_no_rule() {
        // Empty Seq -> rhs empty -> falls through; rule_to_symbol on Seq returns None.
        let grammar = convert_source(JsRule::Choice {
            members: vec![JsRule::Prec {
                value: 1,
                content: Box::new(JsRule::Seq { members: vec![] }),
            }],
        });
        let source_file = source_file_id(&grammar);
        assert!(
            grammar
                .rules
                .get(&source_file)
                .is_none_or(|rules| rules.is_empty())
        );
    }

    #[test]
    fn choice_member_seq_with_members_emits_rule() {
        let mut grammar_js = GrammarJs::new("test".to_string());
        grammar_js.rules.insert(
            "source_file".to_string(),
            JsRule::Choice {
                members: vec![JsRule::Seq {
                    members: vec![
                        JsRule::Symbol {
                            name: "a".to_string(),
                        },
                        JsRule::Symbol {
                            name: "b".to_string(),
                        },
                    ],
                }],
            },
        );
        grammar_js.rules.insert(
            "a".to_string(),
            JsRule::Pattern {
                value: r"\d+".to_string(),
            },
        );
        grammar_js.rules.insert(
            "b".to_string(),
            JsRule::Pattern {
                value: r"[a-z]+".to_string(),
            },
        );

        let grammar = GrammarJsConverter::new(grammar_js).convert().unwrap();
        let source_file = source_file_id(&grammar);
        let rule = &grammar.rules[&source_file][0];
        assert_eq!(rule.rhs.len(), 2);
        assert!(rule.precedence.is_none());
        assert!(rule.associativity.is_none());
    }

    #[test]
    fn choice_member_empty_seq_emits_no_rule() {
        let grammar = convert_source(JsRule::Choice {
            members: vec![JsRule::Seq { members: vec![] }],
        });
        let source_file = source_file_id(&grammar);
        assert!(
            grammar
                .rules
                .get(&source_file)
                .is_none_or(|rules| rules.is_empty())
        );
    }

    #[test]
    fn choice_fallthrough_with_string_member_emits_terminal_rule() {
        // String falls through to the `_` arm, which calls rule_to_symbol.
        let grammar = convert_source(JsRule::Choice {
            members: vec![JsRule::String {
                value: "x".to_string(),
            }],
        });
        let source_file = source_file_id(&grammar);
        let rule = &grammar.rules[&source_file][0];
        assert_eq!(rule.rhs.len(), 1);
        assert!(matches!(rule.rhs[0], Symbol::Terminal(_)));
    }

    #[test]
    fn choice_fallthrough_with_unresolvable_blank_emits_no_rule() {
        // JsRule::Blank falls through; rule_to_symbol returns None for Blank.
        let grammar = convert_source(JsRule::Choice {
            members: vec![JsRule::Blank],
        });
        let source_file = source_file_id(&grammar);
        assert!(
            grammar
                .rules
                .get(&source_file)
                .is_none_or(|rules| rules.is_empty())
        );
    }
}
