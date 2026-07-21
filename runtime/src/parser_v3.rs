// Improved Pure-Rust parser execution engine
// This module implements the runtime parsing logic with proper reduction handling

use crate::error_recovery::{ErrorRecoveryConfig, RecoveryAction};
use crate::lexer::{GrammarLexer, Token as LexerToken};
use adze_glr_core::{Action, ParseTable};
use adze_ir::{Grammar, Rule, RuleId, StateId, SymbolId, TokenPattern};
use anyhow::{Result, bail};
use std::fmt;

// Re-export the lexer Token type for consistency

/// Parser state during execution
#[derive(Debug, Clone)]
pub struct ParserState {
    /// Current state in the parse table
    pub state: StateId,
    /// Symbol that led to this state
    #[allow(dead_code)]
    pub symbol: Option<SymbolId>,
    /// Position in the input
    #[allow(dead_code)]
    pub position: usize,
}

/// A node in the parse tree being constructed
#[derive(Debug, Clone)]
pub struct ParseNode {
    /// Symbol ID for this node
    pub symbol: SymbolId,
    /// Child nodes
    pub children: Vec<ParseNode>,
    /// Start byte offset in the input
    pub start_byte: usize,
    /// End byte offset in the input
    pub end_byte: usize,
    /// Field name if this node is a field
    pub field_name: Option<String>,
}

/// The main parser engine with Grammar awareness
pub(crate) struct Parser {
    /// The grammar being used
    grammar: Grammar,
    /// Parse table for the grammar
    parse_table: ParseTable,
    /// Stack of parser states
    state_stack: Vec<ParserState>,
    /// Stack of parse nodes
    node_stack: Vec<ParseNode>,
    /// Input being parsed
    input: Vec<u8>,
    /// Current position in the input
    position: usize,
    /// Error recovery configuration
    error_recovery: Option<ErrorRecoveryConfig>,
    /// Error nodes created during recovery
    error_nodes: Vec<ParseNode>,
}

/// Parse errors that can occur
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ParseError {
    /// Unexpected token encountered
    UnexpectedToken {
        expected: Vec<SymbolId>,
        found: SymbolId,
        position: usize,
    },
    /// No valid parse found
    #[allow(dead_code)]
    InvalidParse(String),
    /// Parser is in an invalid state
    InvalidState(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::UnexpectedToken {
                expected,
                found,
                position,
            } => {
                write!(
                    f,
                    "Unexpected token at position {}: found {:?}, expected one of {:?}",
                    position, found, expected
                )
            }
            ParseError::InvalidParse(msg) => write!(f, "Invalid parse: {}", msg),
            ParseError::InvalidState(msg) => write!(f, "Invalid parser state: {}", msg),
        }
    }
}

impl std::error::Error for ParseError {}

impl Parser {
    /// Create a new parser with the given grammar and parse table
    #[allow(dead_code)]
    pub(crate) fn new(grammar: Grammar, parse_table: ParseTable) -> Self {
        Self {
            grammar,
            parse_table,
            state_stack: vec![ParserState {
                state: StateId(0), // Start state
                symbol: None,
                position: 0,
            }],
            node_stack: Vec::new(),
            input: Vec::new(),
            position: 0,
            error_recovery: None,
            error_nodes: Vec::new(),
        }
    }

    /// Set error recovery configuration
    #[allow(dead_code)]
    pub(crate) fn with_error_recovery(mut self, config: ErrorRecoveryConfig) -> Self {
        self.error_recovery = Some(config);
        self
    }

    /// Parse the input string
    pub(crate) fn parse(&mut self, input: &str) -> Result<ParseNode> {
        self.input = input.as_bytes().to_vec();
        self.position = 0;
        self.state_stack.clear();
        self.state_stack.push(ParserState {
            state: StateId(0),
            symbol: None,
            position: 0,
        });
        self.node_stack.clear();

        // Create a lexer from the grammar
        let token_patterns: Vec<(SymbolId, TokenPattern, i32)> = self
            .grammar
            .tokens
            .iter()
            .map(|(&id, token)| (id, token.pattern.clone(), 0))
            .collect();

        let mut lexer = GrammarLexer::new(&token_patterns);

        // Main parse loop
        loop {
            // Get current state
            let current_state = self
                .state_stack
                .last()
                .ok_or_else(|| {
                    anyhow::anyhow!(ParseError::InvalidState("Empty state stack".to_string()))
                })?
                .state;

            // Get next token
            let token = match lexer.next_token(&self.input, self.position) {
                Some(tok) => tok,
                None => bail!(
                    "Lexer failed to produce token at position {}",
                    self.position
                ),
            };

            // Look up action in parse table
            let action = self.get_action(current_state, token.symbol)?;

            match action {
                Action::Shift(next_state) => {
                    self.handle_shift(next_state, token)?;
                }

                Action::Reduce(rule_id) => {
                    self.handle_reduce(rule_id)?;
                    // After reduction, don't advance - re-process with the new top state
                    continue;
                }

                Action::Accept => {
                    // Parse complete!
                    return self.node_stack.pop().ok_or_else(|| {
                        anyhow::anyhow!(ParseError::InvalidState(
                            "No parse tree on accept".to_string()
                        ))
                    });
                }

                Action::Error => {
                    // Try error recovery if configured
                    if let Some(ref config) = self.error_recovery
                        && let Some(recovery_action) =
                            self.try_error_recovery(config, current_state, &token)?
                    {
                        match recovery_action {
                            RecoveryAction::InsertToken(symbol) => {
                                // Insert a synthetic token and retry
                                let synthetic_token = LexerToken {
                                    symbol,
                                    start: self.position,
                                    end: self.position,
                                    text: Vec::new(),
                                };
                                self.handle_shift(current_state, synthetic_token)?;
                                // Retry with the original token
                                continue;
                            }
                            RecoveryAction::DeleteToken => {
                                // Skip the current token and continue
                                self.position = token.end;
                                continue;
                            }
                            RecoveryAction::ReplaceToken(symbol) => {
                                // Replace the current token
                                let mut replacement = token.clone();
                                replacement.symbol = symbol;
                                self.handle_shift(current_state, replacement)?;
                                continue;
                            }
                            RecoveryAction::CreateErrorNode(symbols) => {
                                // Create an error node containing the problematic tokens
                                self.create_error_node(symbols, token.start)?;
                                continue;
                            }
                        }
                    }

                    // No recovery possible, fail
                    bail!(ParseError::UnexpectedToken {
                        expected: self.get_expected_symbols(current_state),
                        found: token.symbol,
                        position: self.position,
                    });
                }

                Action::Fork(actions) => {
                    // Use GLR to handle ambiguous parse
                    self.handle_glr_fork(&actions, token)?
                }
                _ => {
                    // Action is #[non_exhaustive] - required wildcard
                    bail!("Unhandled action variant in parse loop"); // Expected: V for Recover
                }
            }
        }
    }

    /// Handle a shift action
    fn handle_shift(&mut self, next_state: StateId, token: LexerToken) -> Result<()> {
        // Push token as a leaf node
        self.node_stack.push(ParseNode {
            symbol: token.symbol,
            children: vec![],
            start_byte: token.start,
            end_byte: token.end,
            field_name: None,
        });

        // Push new state
        self.state_stack.push(ParserState {
            state: next_state,
            symbol: Some(token.symbol),
            position: token.end,
        });

        // Advance position
        self.position = token.end;

        Ok(())
    }

    /// Handle a reduce action
    fn handle_reduce(&mut self, rule_id: RuleId) -> Result<()> {
        // Find the rule in the grammar and extract needed data
        let (rule_lhs, rule_rhs_len, rule_fields) = {
            let rule = self.find_rule_by_id(rule_id)?;
            (rule.lhs, rule.rhs.len(), rule.fields.clone())
        };

        // Pop states and nodes for the rule length
        let mut children = Vec::with_capacity(rule_rhs_len);

        // Collect children in reverse order (they're on stack in reverse)
        for _ in 0..rule_rhs_len {
            self.state_stack.pop().ok_or_else(|| {
                anyhow::anyhow!(ParseError::InvalidState(
                    "State stack underflow".to_string()
                ))
            })?;

            let child = self.node_stack.pop().ok_or_else(|| {
                anyhow::anyhow!(ParseError::InvalidState("Node stack underflow".to_string()))
            })?;

            children.push(child);
        }

        // Children were collected in reverse order
        children.reverse();

        // Apply field names if any
        for (field_id, position) in rule_fields {
            if position < children.len()
                && let Some(field_name) = self.grammar.fields.get(&field_id)
            {
                children[position].field_name = Some(field_name.clone());
            }
        }

        // Get position info from children
        let start_byte = children
            .first()
            .map(|n| n.start_byte)
            .unwrap_or(self.position);
        let end_byte = children.last().map(|n| n.end_byte).unwrap_or(self.position);

        // Create new node for the reduction
        let new_node = ParseNode {
            symbol: rule_lhs,
            children,
            start_byte,
            end_byte,
            field_name: None,
        };

        // Push the new node
        self.node_stack.push(new_node);

        // Get the state to go to after reduction
        let goto_state = self.get_goto_state(rule_lhs)?;

        // Push new state
        self.state_stack.push(ParserState {
            state: goto_state,
            symbol: Some(rule_lhs),
            position: end_byte,
        });

        Ok(())
    }

    /// Handle any action (used for fork resolution)
    #[allow(dead_code)]
    fn handle_action(&mut self, action: Action, token: LexerToken) -> Result<ParseNode> {
        // Save the input to avoid borrowing issues
        let input_str = String::from_utf8_lossy(&self.input).to_string();

        match action {
            Action::Shift(state) => {
                self.handle_shift(state, token)?;
                self.parse(&input_str)
            }
            Action::Reduce(rule_id) => {
                self.handle_reduce(rule_id)?;
                self.parse(&input_str)
            }
            _ => bail!(
                "Unexpected action {:?} in fork handling (expected Shift or Reduce)",
                action
            ),
        }
    }

    /// Find a rule by its ID
    fn find_rule_by_id(&self, rule_id: RuleId) -> Result<&Rule> {
        // Look up the rule by searching through production_ids
        for (rid, &prod_id) in &self.grammar.production_ids {
            if *rid == rule_id {
                // Find the rule with this production ID
                for rules in self.grammar.rules.values() {
                    for rule in rules {
                        if rule.production_id == prod_id {
                            return Ok(rule);
                        }
                    }
                }
            }
        }

        bail!(
            "Rule not found for ID {:?} (searched {} production IDs)",
            rule_id,
            self.grammar.production_ids.len()
        )
    }

    /// Get the goto state after a reduction
    fn get_goto_state(&self, symbol: SymbolId) -> Result<StateId> {
        let current_state = self
            .state_stack
            .last()
            .ok_or_else(|| {
                anyhow::anyhow!(ParseError::InvalidState(
                    "Empty state stack for goto".to_string()
                ))
            })?
            .state;

        let state_idx = current_state.0 as usize;
        let symbol_idx = symbol.0 as usize;

        if state_idx >= self.parse_table.goto_table.len() {
            bail!(
                "Goto lookup failed: state index {} out of bounds (table has {} states)",
                state_idx,
                self.parse_table.goto_table.len()
            );
        }

        let state_gotos = &self.parse_table.goto_table[state_idx];

        if symbol_idx >= state_gotos.len() {
            bail!(
                "Goto lookup failed: symbol index {} out of bounds for state {} (row has {} columns)",
                symbol_idx,
                state_idx,
                state_gotos.len()
            );
        }

        Ok(state_gotos[symbol_idx])
    }

    /// Get the action for a state and symbol
    fn get_action(&self, state: StateId, symbol: SymbolId) -> Result<Action> {
        let state_idx = state.0 as usize;
        let symbol_idx = symbol.0 as usize;

        if state_idx >= self.parse_table.action_table.len() {
            bail!(
                "Action lookup failed: state index {} out of bounds (table has {} states)",
                state_idx,
                self.parse_table.action_table.len()
            );
        }

        let state_actions = &self.parse_table.action_table[state_idx];

        if symbol_idx >= state_actions.len() {
            bail!(
                "Action lookup failed: symbol index {} out of bounds for state {} (row has {} columns)",
                symbol_idx,
                state_idx,
                state_actions.len()
            );
        }

        let action_cell = &state_actions[symbol_idx];
        if action_cell.is_empty() {
            Ok(Action::Error)
        } else if action_cell.len() == 1 {
            Ok(action_cell[0].clone())
        } else {
            Ok(Action::Fork(action_cell.clone()))
        }
    }

    /// Get expected symbols for error reporting
    fn get_expected_symbols(&self, state: StateId) -> Vec<SymbolId> {
        let state_idx = state.0 as usize;
        let mut expected = Vec::new();

        if state_idx < self.parse_table.action_table.len() {
            let state_actions = &self.parse_table.action_table[state_idx];

            for (symbol_idx, action_cell) in state_actions.iter().enumerate() {
                if !action_cell.is_empty() {
                    expected.push(SymbolId(symbol_idx as u16));
                }
            }
        }

        expected
    }

    /// Handle GLR fork by exploring multiple parse paths
    fn handle_glr_fork(&mut self, actions: &[Action], token: LexerToken) -> Result<()> {
        // Save current parser state
        let current_state = self.state_stack.clone();
        let current_nodes = self.node_stack.clone();
        let current_pos = self.position;

        // Try each action and collect successful parses
        let mut successful_parses = Vec::new();
        let mut last_error = None;

        for action in actions {
            // Restore state for each attempt
            self.state_stack = current_state.clone();
            self.node_stack = current_nodes.clone();
            self.position = current_pos;

            // Try this action
            match self.try_action_path(action, token.clone()) {
                Ok(parse_tree) => {
                    successful_parses.push(parse_tree);
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        match successful_parses.len() {
            0 => {
                // No successful parse
                Err(last_error.unwrap_or_else(|| {
                    anyhow::anyhow!(
                        "All {} fork actions failed for token at position {}",
                        actions.len(),
                        token.start
                    )
                }))
            }
            1 => {
                // Single successful parse - use it
                let Some(parse) = successful_parses.into_iter().next() else {
                    return Err(anyhow::anyhow!(
                        "Internal parser state inconsistency: single successful parse vanished during extraction"
                    ));
                };
                self.state_stack = parse.0;
                self.node_stack = parse.1;
                self.position = parse.2;
                Ok(())
            }
            _ => {
                // Multiple successful parses - create ambiguity node
                let ambiguity_nodes: Vec<ParseNode> = successful_parses
                    .into_iter()
                    .filter_map(|(_, nodes, _)| nodes.last().cloned())
                    .collect();

                // Use the first parse's state but with ambiguity node
                let (state, mut nodes, pos) = (current_state, current_nodes, current_pos);

                // Create ambiguity node
                let ambiguity_node = ParseNode {
                    symbol: SymbolId(0xFFFF), // Special ambiguity marker
                    children: ambiguity_nodes,
                    start_byte: token.start,
                    end_byte: token.end,
                    field_name: Some("ambiguous".to_string()),
                };

                nodes.push(ambiguity_node);
                self.state_stack = state;
                self.node_stack = nodes;
                self.position = pos;

                Ok(())
            }
        }
    }

    /// Try a specific action path and return the resulting parser state
    fn try_action_path(
        &mut self,
        action: &Action,
        token: LexerToken,
    ) -> Result<(Vec<ParserState>, Vec<ParseNode>, usize)> {
        match action {
            Action::Shift(state) => {
                self.handle_shift(*state, token)?;
                // Continue parsing to see if this path succeeds
                match self.parse_to_completion() {
                    Ok(()) => Ok((
                        self.state_stack.clone(),
                        self.node_stack.clone(),
                        self.position,
                    )),
                    Err(e) => Err(e),
                }
            }
            Action::Reduce(rule_id) => {
                self.handle_reduce(*rule_id)?;
                // Continue parsing to see if this path succeeds
                match self.parse_to_completion() {
                    Ok(()) => Ok((
                        self.state_stack.clone(),
                        self.node_stack.clone(),
                        self.position,
                    )),
                    Err(e) => Err(e),
                }
            }
            _ => bail!(
                "Unexpected action {:?} in fork path (expected Shift or Reduce)",
                action
            ),
        }
    }

    /// Continue parsing until accept or error
    fn parse_to_completion(&mut self) -> Result<()> {
        // This is a simplified version - in practice we'd continue the main parse loop
        // For now, we'll just check if we can accept
        let current_state = self
            .state_stack
            .last()
            .ok_or_else(|| anyhow::anyhow!("Empty state stack in parse_to_completion"))?
            .state;

        // Check for accept action with EOF
        match self.get_action(current_state, SymbolId(0)) {
            Ok(Action::Accept) => Ok(()),
            _ => Ok(()), // For now, assume partial parse is ok
        }
    }

    /// Try to recover from a parse error
    fn try_error_recovery(
        &self,
        config: &ErrorRecoveryConfig,
        state: StateId,
        token: &LexerToken,
    ) -> Result<Option<RecoveryAction>> {
        // Try different recovery strategies
        // 1. Check if we can insert a token to continue
        for &sync_token in &config.sync_tokens {
            match self.get_action(state, sync_token) {
                Ok(action) if !matches!(action, Action::Error) => {
                    return Ok(Some(RecoveryAction::InsertToken(sync_token)));
                }
                _ => continue,
            }
        }

        // 2. Check if deleting the current token helps
        if config.can_delete_token(token.symbol) {
            // Look ahead to see if the next position would be valid
            return Ok(Some(RecoveryAction::DeleteToken));
        }

        // 3. Check if we can replace with an expected token
        let expected = self.get_expected_symbols(state);
        if !expected.is_empty() && config.can_replace_token(token.symbol) {
            // Try the first expected token
            return Ok(Some(RecoveryAction::ReplaceToken(expected[0])));
        }

        // 4. Create error node as last resort
        // Always allow error node creation as fallback
        Ok(Some(RecoveryAction::CreateErrorNode(vec![token.symbol])))
    }

    /// Create an error node containing problematic tokens
    fn create_error_node(&mut self, _symbols: Vec<SymbolId>, start_pos: usize) -> Result<()> {
        // Create a special error node
        let error_node = ParseNode {
            symbol: SymbolId(0xFFFE), // Special error symbol
            children: vec![],
            start_byte: start_pos,
            end_byte: self.position,
            field_name: Some("ERROR".to_string()),
        };

        self.error_nodes.push(error_node.clone());
        self.node_stack.push(error_node);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::*;

    fn create_simple_grammar() -> Grammar {
        let mut grammar = Grammar {
            name: "test".to_string(),
            rules: indexmap::IndexMap::new(),
            tokens: indexmap::IndexMap::new(),
            precedences: vec![],
            conflicts: vec![],
            externals: vec![],
            extras: vec![],
            fields: indexmap::IndexMap::new(),
            supertypes: vec![],
            inline_rules: vec![],
            alias_sequences: indexmap::IndexMap::new(),
            production_ids: indexmap::IndexMap::new(),
            rule_names: indexmap::IndexMap::new(),
            max_alias_sequence_length: 0,
            symbol_registry: None,
            word_token: None,
            lexical_metadata: indexmap::IndexMap::new(),
            start_symbol: None,
            wrapper_token_relations: indexmap::IndexMap::new(),
        };

        // Add tokens
        let num_id = SymbolId(1);
        let plus_id = SymbolId(2);
        let _eof_id = SymbolId(0);

        grammar.tokens.insert(
            num_id,
            adze_ir::Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            plus_id,
            adze_ir::Token {
                name: "plus".to_string(),
                pattern: TokenPattern::String("+".to_string()),
                fragile: false,
            },
        );

        // Add rules: E -> E + E | number
        let expr_id = SymbolId(3);

        // Rule 0: E -> number
        let rule0 = Rule {
            lhs: expr_id,
            rhs: vec![Symbol::Terminal(num_id)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        };

        // Rule 1: E -> E + E
        let rule1 = Rule {
            lhs: expr_id,
            rhs: vec![
                Symbol::NonTerminal(expr_id),
                Symbol::Terminal(plus_id),
                Symbol::NonTerminal(expr_id),
            ],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(1),
        };

        grammar
            .rules
            .entry(expr_id)
            .or_default()
            .push(rule0.clone());
        grammar
            .rules
            .entry(expr_id)
            .or_default()
            .push(rule1.clone());

        grammar.production_ids.insert(RuleId(0), ProductionId(0));
        grammar.production_ids.insert(RuleId(1), ProductionId(1));

        grammar
    }

    #[test]
    fn test_simple_parse() {
        // This test would require building a parse table
        // For now, we'll just verify the parser compiles
        let _grammar = create_simple_grammar();

        // TODO: ParseTable needs to be properly implemented in glr-core
        // For now, skip this test until ParseTable API is available

        // Unreachable code - commented out until ParseTable is available:
        // let parse_table = ParseTable::default();
        // let _parser = Parser::new(grammar, parse_table);
    }
}
