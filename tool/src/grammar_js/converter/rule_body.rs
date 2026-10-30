use super::GrammarJsConverter;
use super::JsRule;
use crate::grammar_js::helpers::hidden_pattern_token_name;
use adze_ir::{
    Associativity, Grammar, PrecedenceKind, Symbol, SymbolId, TOKEN_WRAPPER_PRIORITY, TokenPattern,
};
use anyhow::Result;

impl GrammarJsConverter {
    pub(super) fn convert_rule_body(
        &mut self,
        grammar: &mut Grammar,
        rule: &JsRule,
        lhs: SymbolId,
    ) -> Result<()> {
        match rule {
            JsRule::String { value } => self.convert_string_rule(grammar, lhs, value),
            JsRule::Pattern { value } => self.convert_pattern_rule(grammar, lhs, value),
            JsRule::Symbol { name } => self.convert_symbol_rule(grammar, lhs, name),
            JsRule::Seq { members } => self.convert_sequence_rule(grammar, lhs, members),
            JsRule::Choice { members } => self.convert_choice_rule(grammar, lhs, members),
            JsRule::Optional { value } => self.convert_optional_rule(grammar, lhs, value),
            JsRule::Repeat { content } => self.convert_repeat_rule(grammar, lhs, content, true),
            JsRule::Repeat1 { content } => self.convert_repeat_rule(grammar, lhs, content, false),
            JsRule::Field { name, content } => self.convert_field_rule(grammar, lhs, name, content),
            JsRule::Prec { value, content } => self.convert_precedence_rule(
                grammar,
                lhs,
                content,
                Some(PrecedenceKind::Static(*value as i16)),
                None,
            ),
            JsRule::PrecLeft { value, content } => self.convert_precedence_rule(
                grammar,
                lhs,
                content,
                Some(PrecedenceKind::Static(*value as i16)),
                Some(Associativity::Left),
            ),
            JsRule::PrecRight { value, content } => self.convert_precedence_rule(
                grammar,
                lhs,
                content,
                Some(PrecedenceKind::Static(*value as i16)),
                Some(Associativity::Right),
            ),
            JsRule::ImmediateToken { content } => {
                self.convert_rule_body(grammar, content, lhs)?;
                if let Some(&token_id) = self.token_symbols.get(&lhs) {
                    grammar.mark_token_immediate(token_id);
                }
                Ok(())
            }
            JsRule::Token { content } => {
                self.convert_rule_body(grammar, content, lhs)?;
                if let Some(&token_id) = self.token_symbols.get(&lhs) {
                    grammar.boost_token_lexical_priority(token_id, TOKEN_WRAPPER_PRIORITY);
                }
                Ok(())
            }
            _ => {
                // For other rule types, add a simple rule.
                self.add_rule(grammar, lhs, vec![], None, None);
                Ok(())
            }
        }
    }

    fn convert_string_rule(
        &mut self,
        grammar: &mut Grammar,
        lhs: SymbolId,
        value: &str,
    ) -> Result<()> {
        let token_id =
            self.get_or_create_token(grammar, value, TokenPattern::String(value.to_string()))?;
        self.token_symbols.insert(lhs, token_id);
        self.add_rule(grammar, lhs, vec![Symbol::Terminal(token_id)], None, None);
        Ok(())
    }

    fn convert_pattern_rule(
        &mut self,
        grammar: &mut Grammar,
        lhs: SymbolId,
        value: &str,
    ) -> Result<()> {
        // Keep a dedicated token SymbolId so the owning rule stays a non-terminal
        // wrapper. Registry keys are rule-scoped so equal regex text keeps distinct
        // identities, while ABI/diagnostic names stay pattern-shaped.
        let rule_name = self
            .symbol_names
            .iter()
            .find(|(_, id)| **id == lhs)
            .map(|(name, _)| name.as_str())
            .unwrap_or("pattern");
        let token_name = hidden_pattern_token_name(value);
        let registry_key = format!("__pattern::{rule_name}");
        let token_id = self.get_or_create_token_named(
            grammar,
            &registry_key,
            &token_name,
            TokenPattern::Regex(value.to_string()),
        )?;
        self.token_symbols.insert(lhs, token_id);
        self.add_rule(grammar, lhs, vec![Symbol::Terminal(token_id)], None, None);
        Ok(())
    }

    fn convert_symbol_rule(
        &mut self,
        grammar: &mut Grammar,
        lhs: SymbolId,
        name: &str,
    ) -> Result<()> {
        let lhs_name = self
            .symbol_names
            .iter()
            .find(|(_, id)| **id == lhs)
            .map(|(n, _)| n.as_str())
            .unwrap_or("?");
        eprintln!("Debug: Converting SYMBOL rule: {} -> {}", lhs_name, name);

        if let Some(&symbol_id) = self.symbol_names.get(name) {
            eprintln!("Debug: Found symbol {} with ID {}", name, symbol_id.0);
            eprintln!(
                "Debug: Creating rule SymbolId({}) -> [NonTerminal(SymbolId({}))]",
                lhs.0, symbol_id.0
            );
            self.add_rule(
                grammar,
                lhs,
                vec![Symbol::NonTerminal(symbol_id)],
                None,
                None,
            );
        } else {
            eprintln!("Debug: Symbol {} not found in symbol_names!", name);
        }
        Ok(())
    }

    fn convert_sequence_rule(
        &mut self,
        grammar: &mut Grammar,
        lhs: SymbolId,
        members: &[JsRule],
    ) -> Result<()> {
        let (rhs, fields) = self.seq_to_rhs_and_fields(grammar, members);
        self.add_rule_with_fields(grammar, lhs, rhs, None, None, fields);
        Ok(())
    }

    fn convert_optional_rule(
        &mut self,
        grammar: &mut Grammar,
        lhs: SymbolId,
        value: &JsRule,
    ) -> Result<()> {
        self.convert_rule_body(grammar, value, lhs)?;
        self.add_rule(grammar, lhs, vec![], None, None);
        Ok(())
    }

    fn convert_repeat_rule(
        &mut self,
        grammar: &mut Grammar,
        lhs: SymbolId,
        content: &JsRule,
        include_empty: bool,
    ) -> Result<()> {
        if include_empty {
            self.add_rule(grammar, lhs, vec![], None, None);
        } else {
            self.convert_rule_body(grammar, content, lhs)?;
        }

        if let Some(symbol) = self.rule_to_symbol(grammar, content) {
            self.add_rule(
                grammar,
                lhs,
                vec![Symbol::NonTerminal(lhs), symbol],
                None,
                None,
            );
        }
        Ok(())
    }

    pub(super) fn convert_precedence_rule(
        &mut self,
        grammar: &mut Grammar,
        lhs: SymbolId,
        content: &JsRule,
        precedence: Option<PrecedenceKind>,
        associativity: Option<Associativity>,
    ) -> Result<()> {
        match content {
            JsRule::Seq { members } => {
                let (rhs, fields) = self.seq_to_rhs_and_fields(grammar, members);
                self.add_rule_with_fields(grammar, lhs, rhs, precedence, associativity, fields);
            }
            _ => {
                if let Some(symbol) = self.rule_to_symbol(grammar, content) {
                    self.add_rule(grammar, lhs, vec![symbol], precedence, associativity);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::grammar_js::{GrammarJs, Rule as JsRule};
    use adze_ir::Symbol;

    fn delimited_list(repeat1: bool) -> adze_ir::Grammar {
        let tail = JsRule::Seq {
            members: vec![
                JsRule::String {
                    value: ",".to_string(),
                },
                JsRule::Field {
                    name: "numbers_vec_element".to_string(),
                    content: Box::new(JsRule::Symbol {
                        name: "number".to_string(),
                    }),
                },
            ],
        };
        let mut grammar_js = GrammarJs::new("delimited".to_string());
        grammar_js.rules.insert(
            "source_file".to_string(),
            JsRule::Seq {
                members: vec![
                    JsRule::Field {
                        name: "numbers_vec_element".to_string(),
                        content: Box::new(JsRule::Symbol {
                            name: "number".to_string(),
                        }),
                    },
                    if repeat1 {
                        JsRule::Repeat1 {
                            content: Box::new(tail),
                        }
                    } else {
                        JsRule::Repeat {
                            content: Box::new(tail),
                        }
                    },
                ],
            },
        );
        grammar_js.rules.insert(
            "number".to_string(),
            JsRule::Pattern {
                value: r"\d+".to_string(),
            },
        );
        GrammarJsConverter::new(grammar_js).convert().unwrap()
    }

    fn aux_id(grammar: &adze_ir::Grammar, prefix: &str) -> adze_ir::SymbolId {
        grammar
            .rule_names
            .iter()
            .find_map(|(id, name)| name.starts_with(prefix).then_some(*id))
            .unwrap_or_else(|| panic!("{prefix} aux should exist"))
    }

    #[test]
    fn repeat_of_seq_keeps_delimiter_tail_1095() {
        let grammar = delimited_list(false);
        let repeat_aux = aux_id(&grammar, "_repeat_aux_");
        let rules = &grammar.rules[&repeat_aux];
        assert!(
            rules.iter().any(|r| r.rhs.is_empty()),
            "REPEAT tail keeps empty alternative: {rules:?}"
        );
        let recursive = rules
            .iter()
            .find(|r| r.rhs.len() == 2 && r.rhs[0] == Symbol::NonTerminal(repeat_aux))
            .unwrap_or_else(|| panic!("REPEAT of SEQ must recurse via tail: {rules:?}"));
        let Symbol::NonTerminal(seq_aux) = recursive.rhs[1] else {
            panic!("tail symbol should be seq aux: {recursive:?}");
        };
        let seq_rules = &grammar.rules[&seq_aux];
        assert_eq!(seq_rules.len(), 1);
        assert_eq!(seq_rules[0].rhs.len(), 2, "seq tail is [comma, element]");
        assert!(matches!(seq_rules[0].rhs[0], Symbol::Terminal(_)));
        assert!(matches!(seq_rules[0].rhs[1], Symbol::Terminal(_)));
        let element_field = grammar
            .fields
            .iter()
            .find_map(|(id, name)| (name == "numbers_vec_element").then_some(*id))
            .expect("element field");
        assert_eq!(seq_rules[0].fields, vec![(element_field, 1)]);
    }

    #[test]
    fn repeat1_of_seq_keeps_delimiter_tail_1095() {
        let grammar = delimited_list(true);
        let repeat_aux = aux_id(&grammar, "_repeat_aux_");
        let rules = &grammar.rules[&repeat_aux];
        assert!(
            rules.iter().any(|r| r.rhs.len() == 2),
            "REPEAT1 tail has a first delimiter+element production: {rules:?}"
        );
        assert!(
            rules
                .iter()
                .any(|r| { r.rhs.len() == 2 && r.rhs[0] == Symbol::NonTerminal(repeat_aux) }),
            "REPEAT1 of SEQ must recurse beyond one tail element: {rules:?}"
        );
    }
}
