//! Converter from Grammar.js to Adze IR

use super::{GrammarJs, Rule as JsRule};
use adze_ir::{
    Associativity, ConflictDeclaration, ConflictResolution, ExternalToken, FieldId, Grammar,
    PrecedenceKind, ProductionId, Rule, RuleId, Symbol, SymbolId, Token, TokenPattern,
    validate_token_pattern,
};
use anyhow::{Context, Result};
use indexmap::IndexMap;
use indexmap::IndexMap as OrderedMap;
use std::collections::HashMap;

mod choice_rules;
mod field_rules;
mod rule_body;

#[cfg(not(debug_assertions))]
macro_rules! eprintln {
    ($($arg:tt)*) => {};
}

#[cfg(debug_assertions)]
macro_rules! eprintln {
    ($($arg:tt)*) => {
        if std::env::var("RUST_LOG")
            .ok()
            .unwrap_or_default()
            .contains("debug")
        {
            std::eprintln!($($arg)*);
        }
    };
}

/// Converts a Grammar.js structure to Adze IR
pub struct GrammarJsConverter {
    grammar_js: GrammarJs,
    symbol_names: OrderedMap<String, SymbolId>,
    token_symbols: HashMap<SymbolId, SymbolId>, // Maps token-backed rule symbols to their token IDs
    next_symbol_id: usize,
    next_production_id: usize,
    next_field_id: usize,
    fields: IndexMap<FieldId, String>,
}

impl GrammarJsConverter {
    pub fn new(grammar_js: GrammarJs) -> Self {
        Self {
            grammar_js,
            symbol_names: OrderedMap::new(),
            token_symbols: HashMap::new(),
            next_symbol_id: 1, // Start at 1 to reserve SymbolId(0) for EOF
            next_production_id: 0,
            next_field_id: 0,
            fields: IndexMap::new(),
        }
    }

    /// Convert Grammar.js to Adze Grammar IR
    pub fn convert(mut self) -> Result<Grammar> {
        eprintln!(
            "DEBUG converter.convert: Starting conversion for grammar '{}'",
            self.grammar_js.name
        );
        eprintln!(
            "DEBUG converter.convert: Grammar.js has {} rules",
            self.grammar_js.rules.len()
        );

        let mut grammar = Grammar {
            name: self.grammar_js.name.clone(),
            rules: IndexMap::new(),
            tokens: IndexMap::new(),
            precedences: Vec::new(),
            conflicts: Vec::new(),
            externals: Vec::new(),
            extras: Vec::new(),
            fields: IndexMap::new(),
            supertypes: Vec::new(),
            inline_rules: Vec::new(),
            alias_sequences: IndexMap::new(),
            production_ids: IndexMap::new(),
            max_alias_sequence_length: 0,
            rule_names: IndexMap::new(),
            symbol_registry: None,
            word_token: None,
            lexical_metadata: IndexMap::new(),
        };

        // First pass: collect all symbols (rules and tokens)
        self.collect_symbols(&mut grammar)?;

        if let Some(word_rule) = &self.grammar_js.word {
            if let Some(&symbol_id) = self.symbol_names.get(word_rule) {
                grammar.word_token = Some(symbol_id);
            } else {
                anyhow::bail!(
                    "grammar word token '{}' does not reference an existing rule",
                    word_rule
                );
            }
        }

        // Convert rules to IR rules
        self.convert_rules(&mut grammar)?;

        // Handle inline rules
        for inline in &self.grammar_js.inline {
            if let Some(&symbol_id) = self.symbol_names.get(inline) {
                grammar.inline_rules.push(symbol_id);
            }
        }

        // Handle externals
        for external in &self.grammar_js.externals {
            if let Some(&symbol_id) = self.symbol_names.get(&external.name) {
                grammar.externals.push(ExternalToken {
                    name: external.name.clone(),
                    symbol_id,
                });
            }
        }

        // Handle conflicts
        for conflict_set in &self.grammar_js.conflicts {
            let mut symbols = Vec::new();
            for rule in conflict_set {
                if let Some(&symbol_id) = self.symbol_names.get(rule) {
                    symbols.push(symbol_id);
                }
            }
            if !symbols.is_empty() {
                grammar.conflicts.push(ConflictDeclaration {
                    symbols,
                    resolution: ConflictResolution::GLR, // Default to GLR handling
                });
            }
        }

        // Handle supertypes
        for supertype in &self.grammar_js.supertypes {
            if let Some(&symbol_id) = self.symbol_names.get(supertype) {
                grammar.supertypes.push(symbol_id);
            }
        }

        // Handle extras
        eprintln!(
            "DEBUG converter: Processing extras, count = {}",
            self.grammar_js.extras.len()
        );
        for extra in &self.grammar_js.extras {
            eprintln!("  Processing extra: {:?}", extra);
            if let Some(symbol_id) = self.find_extra_symbol(extra, &grammar) {
                eprintln!("    Found symbol_id: {:?}", symbol_id);
                grammar.extras.push(symbol_id);
            } else {
                eprintln!("    WARNING: Could not find symbol for extra");
            }
        }

        // Copy fields
        grammar.fields = self.fields.clone();

        eprintln!(
            "DEBUG converter.convert: Final grammar has {} rules",
            grammar.rules.len()
        );
        eprintln!(
            "DEBUG converter.convert: Final grammar has {} tokens",
            grammar.tokens.len()
        );
        eprintln!("DEBUG converter.convert: Final grammar rule_names:");
        for (symbol_id, name) in &grammar.rule_names {
            eprintln!("  SymbolId({}) -> '{}'", symbol_id.0, name);
        }

        // Check what the start symbol will be
        if let Some(start_symbol) = grammar.start_symbol() {
            eprintln!(
                "DEBUG converter.convert: Start symbol is SymbolId({}) -> '{}'",
                start_symbol.0,
                grammar
                    .rule_names
                    .get(&start_symbol)
                    .unwrap_or(&"???".to_string())
            );
        } else {
            eprintln!("DEBUG converter.convert: No start symbol found!");
        }

        Ok(grammar)
    }

    fn collect_symbols(&mut self, grammar: &mut Grammar) -> Result<()> {
        // Add all rule names as non-terminals
        for rule_name in self.grammar_js.rules.keys() {
            let symbol_id = SymbolId(self.next_symbol_id.try_into().unwrap());
            eprintln!(
                "Debug: Collecting symbol '{}' as SymbolId({})",
                rule_name, self.next_symbol_id
            );
            if rule_name == "source_file" {
                eprintln!(
                    "Debug: FOUND source_file! Adding to symbol_names and rule_names as SymbolId({})",
                    symbol_id.0
                );
            }
            self.symbol_names.insert(rule_name.clone(), symbol_id);
            grammar.rule_names.insert(symbol_id, rule_name.clone());
            self.next_symbol_id += 1;
        }

        // Add common terminal tokens
        // NOTE: Commented out because these default tokens interfere with custom patterns
        // and cause incorrect lexer generation
        // self.add_terminal_token(grammar, "_STRING", r#""[^"]*""#)?;
        // self.add_terminal_token(grammar, "_NUMBER", r"-?\d+(\.\d+)?")?;
        // self.add_terminal_token(grammar, "_IDENTIFIER", r"[a-zA-Z_]\w*")?;

        // Add whitespace token if in extras
        let has_whitespace = self.grammar_js.extras.iter().any(|extra| {
            if let JsRule::Pattern { value } = extra {
                value.contains(r"\s")
            } else {
                false
            }
        });

        if has_whitespace {
            self.add_terminal_token(grammar, "_WHITESPACE", r"\s+")?;
        }

        // Add external symbols
        for external in &self.grammar_js.externals {
            let symbol_id = SymbolId(self.next_symbol_id.try_into().unwrap());
            self.symbol_names.insert(external.name.clone(), symbol_id);
            self.next_symbol_id += 1;
        }

        Ok(())
    }

    fn add_terminal_token(
        &mut self,
        grammar: &mut Grammar,
        name: &str,
        pattern: &str,
    ) -> Result<()> {
        let symbol_id = SymbolId(self.next_symbol_id.try_into().unwrap());
        self.symbol_names.insert(name.to_string(), symbol_id);

        grammar.tokens.insert(
            symbol_id,
            Token {
                name: name.to_string(),
                pattern: TokenPattern::Regex(pattern.to_string()),
                fragile: false,
            },
        );

        self.next_symbol_id += 1;
        Ok(())
    }

    fn convert_rules(&mut self, grammar: &mut Grammar) -> Result<()> {
        // Clone to avoid borrow issues
        let rules: Vec<(String, JsRule)> = self
            .grammar_js
            .rules
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        eprintln!("Debug: Converting {} grammar.js rules", rules.len());

        for (rule_name, rule_body) in rules {
            let lhs_symbol = *self
                .symbol_names
                .get(&rule_name)
                .context(format!("Symbol {} not found", rule_name))?;

            eprintln!(
                "Debug: Converting rule '{}' (symbol {})",
                rule_name, lhs_symbol.0
            );
            if rule_name == "source_file" {
                eprintln!("Debug: Converting source_file rule!");
                eprintln!("Debug: source_file rule body: {:?}", rule_body);
            }
            eprintln!(
                "Debug: Rule body type: {:?}",
                std::mem::discriminant(&rule_body)
            );
            self.convert_rule_body(grammar, &rule_body, lhs_symbol)?;
        }

        eprintln!(
            "Debug: After conversion, grammar has {} IR rules",
            grammar.rules.len()
        );

        // Check which symbols are referenced but have no rules
        eprintln!("Debug: Checking for symbols without rules...");
        for (name, &symbol_id) in &self.symbol_names {
            if !grammar.rules.contains_key(&symbol_id) || grammar.rules[&symbol_id].is_empty() {
                eprintln!(
                    "  WARNING: Symbol '{}' (SymbolId({})) has no rules!",
                    name, symbol_id.0
                );
            }
        }

        Ok(())
    }

    fn get_or_create_string_token(&mut self, grammar: &mut Grammar, value: &str) -> SymbolId {
        // Check if we already have this token
        for (id, token) in &grammar.tokens {
            if let TokenPattern::String(s) = &token.pattern
                && s == value
            {
                return *id;
            }
        }

        // Create new token
        let id = SymbolId(self.next_symbol_id.try_into().unwrap());
        self.next_symbol_id += 1;
        let token = Token {
            name: format!("\"{}\"", value),
            pattern: TokenPattern::String(value.to_string()),
            fragile: false,
        };
        grammar.tokens.insert(id, token);
        id
    }

    fn get_or_create_pattern_token(&mut self, grammar: &mut Grammar, pattern: &str) -> SymbolId {
        // Check if we already have this token
        for (id, token) in &grammar.tokens {
            if let TokenPattern::Regex(p) = &token.pattern
                && p == pattern
            {
                return *id;
            }
        }

        // Create new token
        let id = SymbolId(self.next_symbol_id.try_into().unwrap());
        self.next_symbol_id += 1;
        let token = Token {
            name: format!("/{}/", pattern),
            pattern: TokenPattern::Regex(pattern.to_string()),
            fragile: false,
        };
        grammar.tokens.insert(id, token);
        id
    }

    fn find_extra_symbol(&self, rule: &JsRule, grammar: &Grammar) -> Option<SymbolId> {
        eprintln!("DEBUG find_extra_symbol: rule = {:?}", rule);
        match rule {
            JsRule::Symbol { name } => {
                eprintln!("  Looking for symbol '{}'", name);

                // First check if it's directly a token
                if let Some(&symbol_id) = self.symbol_names.get(name) {
                    eprintln!("    Found symbol '{}' with id {:?}", name, symbol_id);

                    // Check if this is actually a token in the grammar
                    if grammar.tokens.contains_key(&symbol_id) {
                        eprintln!("    Symbol is a token, returning {:?}", symbol_id);
                        return Some(symbol_id);
                    }

                    // If it's a rule, we need to check if it's a simple wrapper around a token
                    // For extras like Whitespace that wrap a token pattern
                    if let Some(rules) = grammar.rules.get(&symbol_id) {
                        eprintln!("    Symbol is a rule with {} alternatives", rules.len());
                        // If there's exactly one rule and it's a simple sequence with one token
                        if rules.len() == 1
                            && rules[0].rhs.len() == 1
                            && let Symbol::Terminal(token_id) = &rules[0].rhs[0]
                        {
                            eprintln!("    Rule wraps token {:?}, using that for extra", token_id);
                            return Some(*token_id);
                        }
                    }
                }

                // Fallback: return the symbol itself
                let result = self.symbol_names.get(name).copied();
                eprintln!("  Symbol '{}' -> {:?}", name, result);
                result
            }
            JsRule::Pattern { value } => {
                // Look for a token with matching pattern
                eprintln!("  Looking for pattern '{}' in tokens", value);
                // Special handling for whitespace patterns
                if value.contains(r"\s") {
                    // Look for the whitespace token we added
                    if let Some(&id) = self.symbol_names.get("_WHITESPACE") {
                        eprintln!("    Found whitespace token with id {:?}", id);
                        return Some(id);
                    }
                }
                eprintln!("  Pattern '{}' not found in tokens", value);
                None
            }
            _ => {
                eprintln!("  Unhandled rule type");
                None
            }
        }
    }

    fn rule_to_symbol(&mut self, grammar: &mut Grammar, rule: &JsRule) -> Option<Symbol> {
        match rule {
            JsRule::Symbol { name } => {
                eprintln!("Debug: rule_to_symbol for Symbol '{}'", name);
                if let Some(&id) = self.symbol_names.get(name) {
                    eprintln!("Debug:   Found symbol ID {}", id.0);
                    // Check if this symbol is actually a token-backed wrapper.
                    if let Some(token_id) = self.token_for_wrapped_rule(grammar, id, name) {
                        eprintln!(
                            "Debug:   Symbol {} is token-backed, returning Terminal({})",
                            id.0, token_id.0
                        );
                        Some(Symbol::Terminal(token_id))
                    } else {
                        eprintln!(
                            "Debug:   Symbol {} is not a pattern, returning NonTerminal",
                            id.0
                        );
                        Some(Symbol::NonTerminal(id))
                    }
                } else {
                    eprintln!("Debug:   Symbol '{}' not found in symbol_names", name);
                    None
                }
            }
            JsRule::String { value } => {
                // Create inline token
                Some(Symbol::Terminal(
                    self.get_or_create_string_token(grammar, value),
                ))
            }
            JsRule::Pattern { value } => {
                // Create pattern token
                Some(Symbol::Terminal(
                    self.get_or_create_pattern_token(grammar, value),
                ))
            }
            JsRule::Field { content, .. } => {
                // For fields, return the symbol of the content
                self.rule_to_symbol(grammar, content)
            }
            JsRule::Prec { content, .. }
            | JsRule::PrecLeft { content, .. }
            | JsRule::PrecRight { content, .. } => {
                // For precedence rules, return the symbol of the content
                self.rule_to_symbol(grammar, content)
            }
            JsRule::Choice { members } => {
                // Create an auxiliary non-terminal for the choice so it can be
                // used as a single symbol in a SEQ. This is necessary because
                // a CHOICE inside a FIELD (e.g. Vec<Declaration> expands to
                // FIELD("declarations", CHOICE([BLANK, SYMBOL(vec_contents)])))
                // needs to be representable as one symbol on the RHS.
                let aux_id = SymbolId(self.next_symbol_id.try_into().unwrap());
                self.next_symbol_id += 1;
                grammar
                    .rule_names
                    .insert(aux_id, format!("_choice_aux_{}", aux_id.0));
                for member in members {
                    match member {
                        JsRule::Blank => {
                            let pid =
                                adze_ir::ProductionId(self.next_production_id.try_into().unwrap());
                            self.next_production_id += 1;
                            grammar.add_rule(adze_ir::Rule {
                                lhs: aux_id,
                                rhs: vec![Symbol::Epsilon],
                                precedence: None,
                                associativity: None,
                                fields: vec![],
                                production_id: pid,
                            });
                        }
                        _ => {
                            let _ = self.convert_rule_body(grammar, member, aux_id);
                        }
                    }
                }
                Some(Symbol::NonTerminal(aux_id))
            }
            JsRule::Blank => Some(Symbol::Epsilon),
            JsRule::Repeat { .. } | JsRule::Repeat1 { .. } => {
                let aux_id = SymbolId(self.next_symbol_id.try_into().unwrap());
                self.next_symbol_id += 1;
                grammar
                    .rule_names
                    .insert(aux_id, format!("_repeat_aux_{}", aux_id.0));
                let _ = self.convert_rule_body(grammar, rule, aux_id);
                Some(Symbol::NonTerminal(aux_id))
            }
            _ => None, // Other types not yet handled
        }
    }

    fn seq_to_rhs_and_fields(
        &mut self,
        grammar: &mut Grammar,
        members: &[JsRule],
    ) -> (Vec<Symbol>, Vec<(FieldId, usize)>) {
        let mut rhs = Vec::new();
        let mut fields = Vec::new();

        for member in members {
            match member {
                JsRule::Field { name, content } => {
                    let field_id = self.get_or_create_field(name);
                    if let Some(symbol) = self.rule_to_symbol(grammar, content) {
                        let position = rhs.len();
                        rhs.push(symbol);
                        if !is_generated_tuple_field_name(name) {
                            fields.push((field_id, position));
                        }
                    } else {
                        eprintln!("Debug: Failed to convert FIELD member {name}");
                    }
                }
                _ => {
                    if let Some(symbol) = self.rule_to_symbol(grammar, member) {
                        rhs.push(symbol);
                    } else {
                        eprintln!("Debug: Failed to convert SEQ member");
                    }
                }
            }
        }

        (rhs, fields)
    }

    fn token_for_wrapped_rule(
        &mut self,
        grammar: &mut Grammar,
        id: SymbolId,
        name: &str,
    ) -> Option<SymbolId> {
        if let Some(&token_id) = self.token_symbols.get(&id) {
            return Some(token_id);
        }

        let rule = self.grammar_js.rules.get(name)?.clone();
        let token_id = match rule {
            JsRule::String { value } => self
                .get_or_create_token(grammar, &value, TokenPattern::String(value.clone()))
                .ok()?,
            JsRule::Pattern { value } => {
                let token_name = hidden_pattern_token_name(&value);
                self.get_or_create_token(grammar, &token_name, TokenPattern::Regex(value.clone()))
                    .ok()?
            }
            _ => return None,
        };
        self.token_symbols.insert(id, token_id);
        Some(token_id)
    }

    fn add_rule(
        &mut self,
        grammar: &mut Grammar,
        lhs: SymbolId,
        rhs: Vec<Symbol>,
        precedence: Option<PrecedenceKind>,
        associativity: Option<Associativity>,
    ) {
        self.add_rule_with_fields(grammar, lhs, rhs, precedence, associativity, Vec::new());
    }

    fn add_rule_with_fields(
        &mut self,
        grammar: &mut Grammar,
        lhs: SymbolId,
        rhs: Vec<Symbol>,
        precedence: Option<PrecedenceKind>,
        associativity: Option<Associativity>,
        fields: Vec<(FieldId, usize)>,
    ) {
        eprintln!("Debug: Adding rule for SymbolId({}) -> {:?}", lhs.0, rhs);

        // Check if an identical rule already exists
        let duplicate_exists = grammar.rules.get(&lhs).is_some_and(|existing_rules| {
            existing_rules.iter().any(|r| {
                r.rhs == rhs
                    && r.precedence == precedence
                    && r.associativity == associativity
                    && r.fields == fields
            })
        });

        if duplicate_exists {
            eprintln!(
                "Debug: Skipping duplicate rule for SymbolId({}) -> {:?}",
                lhs.0, rhs
            );
            return;
        }

        let rule = Rule {
            lhs,
            rhs,
            precedence,
            associativity,
            fields,
            production_id: ProductionId(self.next_production_id.try_into().unwrap()),
        };
        self.next_production_id += 1;

        // Calculate rule_id before modifying grammar.rules
        let total_rules = grammar
            .rules
            .values()
            .map(|rules| rules.len())
            .sum::<usize>();
        let rule_id = RuleId(total_rules.try_into().unwrap());
        grammar.production_ids.insert(rule_id, rule.production_id);

        // Now add the rule
        grammar.rules.entry(lhs).or_default().push(rule);
    }

    fn get_or_create_field(&mut self, name: &str) -> FieldId {
        // Check if field already exists
        for (field_id, field_name) in &self.fields {
            if field_name == name {
                return *field_id;
            }
        }

        // Create new field
        let field_id = FieldId(self.next_field_id.try_into().unwrap());
        self.fields.insert(field_id, name.to_string());
        self.next_field_id += 1;
        field_id
    }

    fn get_or_create_token(
        &mut self,
        grammar: &mut Grammar,
        name: &str,
        pattern: TokenPattern,
    ) -> Result<SymbolId> {
        validate_token_pattern(name, &pattern)
            .with_context(|| format!("invalid lexical pattern for token '{name}'"))?;

        // Reuse only an existing token. Never promote a rule/non-terminal SymbolId
        // into a token — that collapses identities and can erase GLR conflicts.
        if let Some(&symbol_id) = self.symbol_names.get(name)
            && grammar.tokens.contains_key(&symbol_id)
        {
            return Ok(symbol_id);
        }

        let mut map_key = name.to_string();
        if self.symbol_names.contains_key(&map_key) {
            map_key = format!("__tok_{name}");
        }

        let symbol_id = SymbolId(self.next_symbol_id.try_into().unwrap());
        self.symbol_names.insert(map_key, symbol_id);
        self.next_symbol_id += 1;

        let token = Token {
            name: name.to_string(),
            pattern,
            fragile: false,
        };
        grammar.tokens.insert(symbol_id, token);

        Ok(symbol_id)
    }
}

fn hidden_pattern_token_name(pattern: &str) -> String {
    format!("_/{pattern}/")
}

fn is_generated_tuple_field_name(name: &str) -> bool {
    let Some((prefix, suffix)) = name.rsplit_once('_') else {
        return false;
    };

    !prefix.is_empty()
        && prefix
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_uppercase())
        && suffix.chars().all(|ch| ch.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_conversion() {
        let mut grammar_js = GrammarJs::new("test".to_string());

        grammar_js.rules.insert(
            "expression".to_string(),
            JsRule::Choice {
                members: vec![
                    JsRule::Symbol {
                        name: "number".to_string(),
                    },
                    JsRule::Symbol {
                        name: "identifier".to_string(),
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

        grammar_js.rules.insert(
            "identifier".to_string(),
            JsRule::Pattern {
                value: r"[a-zA-Z]+".to_string(),
            },
        );

        let converter = GrammarJsConverter::new(grammar_js);
        let grammar = converter.convert().unwrap();

        assert_eq!(grammar.name, "test");
        assert!(!grammar.rules.is_empty());
        assert!(!grammar.tokens.is_empty());
    }

    #[test]
    fn string_wrapper_symbols_lower_to_terminals_when_referenced() {
        let mut grammar_js = GrammarJs::new("string_wrapper".to_string());

        grammar_js.rules.insert(
            "source_file".to_string(),
            JsRule::Seq {
                members: vec![JsRule::Symbol {
                    name: "keyword_if".to_string(),
                }],
            },
        );
        grammar_js.rules.insert(
            "keyword_if".to_string(),
            JsRule::String {
                value: "if".to_string(),
            },
        );

        let grammar = GrammarJsConverter::new(grammar_js).convert().unwrap();
        let source_file = grammar
            .rule_names
            .iter()
            .find_map(|(id, name)| (name == "source_file").then_some(*id))
            .expect("source_file symbol should exist");
        let keyword_if = grammar
            .rule_names
            .iter()
            .find_map(|(id, name)| (name == "keyword_if").then_some(*id))
            .expect("keyword_if symbol should exist");
        let if_token = grammar
            .tokens
            .iter()
            .find_map(|(id, token)| (token.name == "if").then_some(*id))
            .expect("literal token should exist");

        let source_rule = grammar
            .rules
            .get(&source_file)
            .and_then(|rules| rules.first())
            .expect("source_file rule should exist");

        assert_eq!(source_rule.rhs, vec![Symbol::Terminal(if_token)]);
        assert!(
            !source_rule.rhs.contains(&Symbol::NonTerminal(keyword_if)),
            "string leaf wrappers must not hide token lookahead behind nonterminals"
        );
    }

    #[test]
    fn fielded_seq_preserves_fields_on_lowered_token_symbols() {
        let mut grammar_js = GrammarJs::new("fielded_seq".to_string());

        grammar_js.rules.insert(
            "source_file".to_string(),
            JsRule::Seq {
                members: vec![JsRule::Symbol {
                    name: "pair".to_string(),
                }],
            },
        );
        grammar_js.rules.insert(
            "pair".to_string(),
            JsRule::Seq {
                members: vec![
                    JsRule::Field {
                        name: "left".to_string(),
                        content: Box::new(JsRule::Symbol {
                            name: "pair_left".to_string(),
                        }),
                    },
                    JsRule::Field {
                        name: "right".to_string(),
                        content: Box::new(JsRule::Symbol {
                            name: "pair_right".to_string(),
                        }),
                    },
                ],
            },
        );
        grammar_js.rules.insert(
            "pair_left".to_string(),
            JsRule::Pattern {
                value: r"\d+".to_string(),
            },
        );
        grammar_js.rules.insert(
            "pair_right".to_string(),
            JsRule::String {
                value: "+".to_string(),
            },
        );

        let grammar = GrammarJsConverter::new(grammar_js).convert().unwrap();
        let pair = grammar
            .rule_names
            .iter()
            .find_map(|(id, name)| (name == "pair").then_some(*id))
            .expect("pair symbol should exist");
        let pair_left = grammar
            .rule_names
            .iter()
            .find_map(|(id, name)| (name == "pair_left").then_some(*id))
            .expect("pair_left symbol should exist");
        let pair_right = grammar
            .rule_names
            .iter()
            .find_map(|(id, name)| (name == "pair_right").then_some(*id))
            .expect("pair_right symbol should exist");
        let left = grammar
            .fields
            .iter()
            .find_map(|(id, name)| (name == "left").then_some(*id))
            .expect("left field should exist");
        let right = grammar
            .fields
            .iter()
            .find_map(|(id, name)| (name == "right").then_some(*id))
            .expect("right field should exist");

        let pair_rule = grammar
            .rules
            .get(&pair)
            .and_then(|rules| rules.first())
            .expect("pair rule should exist");

        assert!(
            matches!(
                pair_rule.rhs.as_slice(),
                [Symbol::Terminal(_), Symbol::Terminal(_)]
            ),
            "token-backed field wrapper references should keep parser productions terminal-backed"
        );
        assert_eq!(pair_rule.fields, vec![(left, 0), (right, 1)]);

        let left_rule = grammar
            .rules
            .get(&pair_left)
            .and_then(|rules| rules.first())
            .expect("pair_left rule should exist");
        assert!(
            matches!(left_rule.rhs.as_slice(), [Symbol::Terminal(_)]),
            "field wrapper rules should still lower to terminal-backed productions"
        );
        let right_rule = grammar
            .rules
            .get(&pair_right)
            .and_then(|rules| rules.first())
            .expect("pair_right rule should exist");
        assert!(
            matches!(right_rule.rhs.as_slice(), [Symbol::Terminal(_)]),
            "field wrapper rules should still lower to terminal-backed productions"
        );
    }

    #[test]
    fn fielded_seq_skips_generated_tuple_field_metadata() {
        let mut grammar_js = GrammarJs::new("generated_tuple_fields".to_string());

        grammar_js.rules.insert(
            "source_file".to_string(),
            JsRule::Seq {
                members: vec![JsRule::Symbol {
                    name: "expr".to_string(),
                }],
            },
        );
        grammar_js.rules.insert(
            "expr".to_string(),
            JsRule::Seq {
                members: vec![
                    JsRule::Field {
                        name: "Expr_Add_0".to_string(),
                        content: Box::new(JsRule::Pattern {
                            value: r"\d+".to_string(),
                        }),
                    },
                    JsRule::Field {
                        name: "Expr_Add_1".to_string(),
                        content: Box::new(JsRule::String {
                            value: "+".to_string(),
                        }),
                    },
                ],
            },
        );

        let grammar = GrammarJsConverter::new(grammar_js).convert().unwrap();
        let expr = grammar
            .rule_names
            .iter()
            .find_map(|(id, name)| (name == "expr").then_some(*id))
            .expect("expr symbol should exist");
        let expr_rule = grammar
            .rules
            .get(&expr)
            .and_then(|rules| rules.first())
            .expect("expr rule should exist");

        assert!(
            expr_rule.fields.is_empty(),
            "generated tuple field names are extraction scaffolding and should not alter existing generated AST tree shape"
        );
    }

    #[test]
    fn pattern_wrapper_tokens_keep_human_readable_hidden_names() {
        let mut grammar_js = GrammarJs::new("pattern_wrapper".to_string());

        grammar_js.rules.insert(
            "source_file".to_string(),
            JsRule::Seq {
                members: vec![JsRule::Symbol {
                    name: "identifier".to_string(),
                }],
            },
        );
        grammar_js.rules.insert(
            "identifier".to_string(),
            JsRule::Pattern {
                value: r"[a-z]+".to_string(),
            },
        );

        let grammar = GrammarJsConverter::new(grammar_js).convert().unwrap();
        let token = grammar
            .tokens
            .values()
            .find(|token| matches!(&token.pattern, TokenPattern::Regex(pattern) if pattern == r"[a-z]+"))
            .expect("wrapped pattern token should exist");

        assert_eq!(
            token.name, "_/[a-z]+/",
            "wrapped pattern tokens should remain hidden while preserving a diagnostic name"
        );
    }
}
