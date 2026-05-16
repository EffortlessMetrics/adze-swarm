use super::{GrammarJsConverter, JsRule};
use adze_ir::{FieldId, Grammar, SymbolId};
use anyhow::Result;

impl GrammarJsConverter {
    pub(super) fn convert_field_rule(
        &mut self,
        grammar: &mut Grammar,
        lhs: SymbolId,
        name: &str,
        content: &JsRule,
    ) -> Result<()> {
        let field_id = self.get_or_create_field(name);
        self.convert_field_symbol_dependency(grammar, content)?;

        eprintln!(
            "Debug: FIELD conversion - lhs: SymbolId({}), field: {}, content: {:?}",
            lhs.0, name, content
        );

        if let JsRule::Choice { members } = content {
            self.convert_field_choice(grammar, lhs, name, field_id, members);
        } else if let Some(symbol) = self.rule_to_symbol(grammar, content) {
            eprintln!("Debug: FIELD resolved to symbol: {:?}", symbol);
            self.add_rule_with_fields(grammar, lhs, vec![symbol], None, None, vec![(field_id, 0)]);
        }
        Ok(())
    }

    fn convert_field_symbol_dependency(
        &mut self,
        grammar: &mut Grammar,
        content: &JsRule,
    ) -> Result<()> {
        if let JsRule::Symbol { name } = content
            && let Some(&content_symbol_id) = self.symbol_names.get(name)
            && let Some(content_rule) = self.grammar_js.rules.get(name).cloned()
        {
            eprintln!("Debug: Converting nested rule {} for field", name);
            self.convert_rule_body(grammar, &content_rule, content_symbol_id)?;
        }
        Ok(())
    }

    fn convert_field_choice(
        &mut self,
        grammar: &mut Grammar,
        lhs: SymbolId,
        name: &str,
        field_id: FieldId,
        members: &[JsRule],
    ) {
        eprintln!("Debug: FIELD contains CHOICE, converting each member with field");
        for (index, member) in members.iter().enumerate() {
            eprintln!(
                "Debug: Converting choice member {} for field {}",
                index, name
            );
            if matches!(member, JsRule::Blank) {
                eprintln!("Debug: Adding empty rule for BLANK with field {}", name);
                self.add_rule(grammar, lhs, vec![], None, None);
            } else if let Some(symbol) = self.rule_to_symbol(grammar, member) {
                eprintln!(
                    "Debug: Adding rule with symbol {:?} and field {}",
                    symbol, name
                );
                self.add_rule_with_fields(
                    grammar,
                    lhs,
                    vec![symbol],
                    None,
                    None,
                    vec![(field_id, 0)],
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::grammar_js::{GrammarJs, Rule as JsRule};
    use adze_ir::Symbol;

    fn source_file_id(grammar: &adze_ir::Grammar) -> adze_ir::SymbolId {
        grammar
            .rule_names
            .iter()
            .find_map(|(id, name)| (name == "source_file").then_some(*id))
            .expect("source_file should exist")
    }

    #[test]
    fn field_with_symbol_content_records_field_at_position_zero() {
        let mut grammar_js = GrammarJs::new("test".to_string());
        // Wrap the field in a Seq so the rule_body dispatch routes to the
        // sequence path; the field on its own at the top level still exercises
        // convert_field_rule via convert_rule_body's Field arm.
        grammar_js.rules.insert(
            "source_file".to_string(),
            JsRule::Field {
                name: "head".to_string(),
                content: Box::new(JsRule::Symbol {
                    name: "ident".to_string(),
                }),
            },
        );
        grammar_js.rules.insert(
            "ident".to_string(),
            JsRule::Pattern {
                value: r"[a-z]+".to_string(),
            },
        );

        let grammar = GrammarJsConverter::new(grammar_js).convert().unwrap();
        let source_file = source_file_id(&grammar);
        let rule = &grammar.rules[&source_file][0];
        assert_eq!(rule.rhs.len(), 1);
        let head_field = grammar
            .fields
            .iter()
            .find_map(|(id, name)| (name == "head").then_some(*id))
            .expect("head field should be registered");
        assert_eq!(rule.fields, vec![(head_field, 0)]);
    }

    #[test]
    fn field_with_unresolvable_blank_emits_no_rule_but_registers_field() {
        // Blank standalone returns None from rule_to_symbol, so no rule is added.
        let mut grammar_js = GrammarJs::new("test".to_string());
        grammar_js.rules.insert(
            "source_file".to_string(),
            JsRule::Field {
                name: "absent".to_string(),
                content: Box::new(JsRule::Blank),
            },
        );

        let grammar = GrammarJsConverter::new(grammar_js).convert().unwrap();
        let source_file = source_file_id(&grammar);
        assert!(
            grammar
                .rules
                .get(&source_file)
                .is_none_or(|rules| rules.is_empty()),
            "Blank content cannot resolve to a symbol; no rule should be added"
        );
        // The field is still created via get_or_create_field before resolution.
        assert!(
            grammar.fields.iter().any(|(_, name)| name == "absent"),
            "field id should still be allocated"
        );
    }

    #[test]
    fn field_with_choice_blank_member_emits_empty_rule() {
        // Choice containing JsRule::Blank triggers the BLANK branch in
        // convert_field_choice: add an empty rule (no field metadata).
        let mut grammar_js = GrammarJs::new("test".to_string());
        grammar_js.rules.insert(
            "source_file".to_string(),
            JsRule::Field {
                name: "tail".to_string(),
                content: Box::new(JsRule::Choice {
                    members: vec![JsRule::Blank],
                }),
            },
        );

        let grammar = GrammarJsConverter::new(grammar_js).convert().unwrap();
        let source_file = source_file_id(&grammar);
        let rules = grammar.rules.get(&source_file).expect("rules exist");
        assert_eq!(rules.len(), 1);
        assert!(
            rules[0].rhs.is_empty(),
            "BLANK choice member adds an empty production"
        );
        assert!(
            rules[0].fields.is_empty(),
            "BLANK production carries no field metadata"
        );
    }

    #[test]
    fn field_with_choice_symbol_member_attaches_field_per_alternative() {
        let mut grammar_js = GrammarJs::new("test".to_string());
        grammar_js.rules.insert(
            "source_file".to_string(),
            JsRule::Field {
                name: "any".to_string(),
                content: Box::new(JsRule::Choice {
                    members: vec![
                        JsRule::Symbol {
                            name: "a".to_string(),
                        },
                        JsRule::Symbol {
                            name: "b".to_string(),
                        },
                    ],
                }),
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
        assert_eq!(rules.len(), 2, "one rule per choice alternative");
        let any_field = grammar
            .fields
            .iter()
            .find_map(|(id, name)| (name == "any").then_some(*id))
            .expect("any field should be registered");
        for rule in rules {
            assert_eq!(rule.rhs.len(), 1);
            assert_eq!(rule.fields, vec![(any_field, 0)]);
        }
    }

    #[test]
    fn field_with_choice_mixed_blank_and_symbol_yields_optional_shape() {
        // The classic optional pattern: a field that can be present or empty.
        let mut grammar_js = GrammarJs::new("test".to_string());
        grammar_js.rules.insert(
            "source_file".to_string(),
            JsRule::Field {
                name: "maybe".to_string(),
                content: Box::new(JsRule::Choice {
                    members: vec![
                        JsRule::Blank,
                        JsRule::Symbol {
                            name: "ident".to_string(),
                        },
                    ],
                }),
            },
        );
        grammar_js.rules.insert(
            "ident".to_string(),
            JsRule::Pattern {
                value: r"[a-z]+".to_string(),
            },
        );

        let grammar = GrammarJsConverter::new(grammar_js).convert().unwrap();
        let source_file = source_file_id(&grammar);
        let rules = grammar.rules.get(&source_file).expect("rules exist");
        assert_eq!(rules.len(), 2);
        let maybe_field = grammar
            .fields
            .iter()
            .find_map(|(id, name)| (name == "maybe").then_some(*id))
            .expect("maybe field should be registered");

        let empty_rule = rules
            .iter()
            .find(|r| r.rhs.is_empty())
            .expect("empty BLANK rule should exist");
        assert!(empty_rule.fields.is_empty());

        let symbol_rule = rules
            .iter()
            .find(|r| !r.rhs.is_empty())
            .expect("non-empty rule should exist");
        assert_eq!(symbol_rule.fields, vec![(maybe_field, 0)]);
    }

    #[test]
    fn field_dedups_field_id_when_name_reused() {
        // Two top-level Field rules with the same field name must share one
        // FieldId (exercises the get_or_create_field hit path inside the
        // helper across multiple call sites).
        let mut grammar_js = GrammarJs::new("test".to_string());
        grammar_js.rules.insert(
            "source_file".to_string(),
            JsRule::Field {
                name: "shared".to_string(),
                content: Box::new(JsRule::Symbol {
                    name: "a".to_string(),
                }),
            },
        );
        grammar_js.rules.insert(
            "second".to_string(),
            JsRule::Field {
                name: "shared".to_string(),
                content: Box::new(JsRule::Symbol {
                    name: "b".to_string(),
                }),
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
        let shared_count = grammar
            .fields
            .iter()
            .filter(|(_, name)| *name == "shared")
            .count();
        assert_eq!(shared_count, 1, "duplicate field names must dedupe");
    }

    #[test]
    fn field_with_nested_symbol_triggers_dependency_conversion() {
        // When the field content is a JsRule::Symbol referencing another rule,
        // convert_field_symbol_dependency recursively converts that rule body
        // so the dependency has IR rules emitted.
        let mut grammar_js = GrammarJs::new("test".to_string());
        grammar_js.rules.insert(
            "source_file".to_string(),
            JsRule::Field {
                name: "inner".to_string(),
                content: Box::new(JsRule::Symbol {
                    name: "ident".to_string(),
                }),
            },
        );
        grammar_js.rules.insert(
            "ident".to_string(),
            JsRule::Pattern {
                value: r"[a-z]+".to_string(),
            },
        );

        let grammar = GrammarJsConverter::new(grammar_js).convert().unwrap();
        let ident = grammar
            .rule_names
            .iter()
            .find_map(|(id, name)| (name == "ident").then_some(*id))
            .expect("ident symbol exists");
        // Even though the field path also performs its own conversion of
        // the dependency, ident must have at least one IR rule populated.
        let ident_rules = grammar.rules.get(&ident).expect("ident has rules");
        assert!(!ident_rules.is_empty());
        assert!(matches!(
            ident_rules[0].rhs.as_slice(),
            [Symbol::Terminal(_)]
        ));
    }
}
