use super::{GrammarJsConverter, JsRule};
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
        // wrapper. Include the rule name so equal regex text keeps distinct identities.
        let rule_name = self
            .symbol_names
            .iter()
            .find(|(_, id)| **id == lhs)
            .map(|(name, _)| name.as_str())
            .unwrap_or("pattern");
        let token_name = format!("_/{rule_name}/");
        let token_id =
            self.get_or_create_token(grammar, &token_name, TokenPattern::Regex(value.to_string()))?;
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
