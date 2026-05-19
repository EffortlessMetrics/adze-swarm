//! Version 4 parser implementation with GLR support.
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

// Enhanced Pure-Rust parser with external scanner support
// This module extends parser_v3 with full external scanner integration

use crate::arena_allocator::{ArenaMetrics, NodeHandle, TreeArena, TreeNode};
use crate::external_scanner::ExternalScannerRuntime;
use crate::glr_forest::{ForestNode, GLRParserState, PackedNode};
use crate::lexer::{GrammarLexer, Token as LexerToken};
use crate::scanner_registry::{DynExternalScanner, get_global_registry};
use adze_glr_core::{Action, ParseRule, ParseTable, conflict_inspection::state_has_conflicts};
use adze_ir::{Grammar, Rule, RuleId, StateId, SymbolId, TokenPattern};
use anyhow::{Result, anyhow, bail};
use std::collections::HashSet;
use std::rc::Rc;
use std::time::Instant;

const PARSE_WITH_CUSTOM_LEXER_UNSUPPORTED: &str = "Custom lexer functions are not yet supported by parser_v4 runtime. \
     Provide a grammar/tokenization path without a custom transform lexer.";
const GLR_CONFLICT_REQUIRES_TRUE_GLR: &str = "GLR conflict encountered in parser_v4: refusing ordered first-success fallback. Route conflicted tables to the GLR parser runtime.";

// Define types directly in parser_v4 (no longer dependent on parser_v3)

/// Error type for parsing operations
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("No language set")]
    NoLanguage,
    #[error("Lexer error: {0}")]
    LexerError(String),
    #[error("Parser error: {0}")]
    ParserError(String),
    #[error("Invalid action: {0}")]
    InvalidAction(String),
    #[error("Unexpected token: expected {expected:?}, got {got:?}")]
    UnexpectedToken { expected: Vec<String>, got: String },
}

/// A node in the parse tree
#[derive(Debug, Clone)]
pub struct ParseNode {
    pub symbol: SymbolId,
    pub symbol_id: SymbolId, // Keep both for compatibility
    pub start_byte: usize,
    pub end_byte: usize,
    pub field_name: Option<String>,
    pub alias_symbol_id: Option<SymbolId>,
    pub children: Vec<ParseNode>,
}

/// Parser state for incremental parsing
#[derive(Debug, Clone)]
pub struct ParserState {
    pub stack: Vec<(StateId, Option<ParseNode>)>,
    pub position: usize,
}

/// Parse tree with arena-allocated nodes
///
/// Tree borrows the parser's arena, tying its lifetime to the parser.
/// This prevents use-after-free while enabling efficient arena allocation.
///
/// # Lifetime
///
/// The `'arena` lifetime ties the tree to the parser's arena.
/// Trees cannot outlive the parser that created them.
///
/// # Example
///
/// ```ignore
/// let mut parser = Parser::new(grammar, parse_table, "example".to_string());
/// let root = parser.parse_tree("1 + 2")?;
/// ```
#[derive(Debug)]
pub struct Tree<'arena> {
    /// Root node handle
    pub(crate) root: NodeHandle,
    /// Reference to parser's arena
    pub(crate) arena: &'arena TreeArena,
    /// Number of errors encountered during parsing
    pub error_count: usize,
}

impl<'arena> Tree<'arena> {
    /// Get the root node
    ///
    /// Returns a Node<'arena> wrapping the root handle.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let root = tree.root_node();
    /// println!("Root symbol: {}", root.symbol());
    /// ```
    pub fn root_node(&self) -> crate::node::Node<'arena> {
        crate::node::Node::new(self.root, self.arena)
    }

    /// Get node by handle
    ///
    /// For advanced use cases that have a NodeHandle and need
    /// to create a Node<'arena> wrapper.
    pub fn get_node(&self, handle: NodeHandle) -> crate::node::Node<'arena> {
        crate::node::Node::new(handle, self.arena)
    }

    /// Get the number of errors in the tree
    pub fn error_count(&self) -> usize {
        self.error_count
    }
}

/// Enhanced parser with external scanner support
pub struct Parser {
    /// Arena allocator for parse tree nodes
    arena: TreeArena,
    /// The grammar being used
    grammar: Grammar,
    /// Parse table for the grammar
    parse_table: ParseTable,
    /// GLR parser state (replaces simple stacks)
    glr_state: GLRParserState,
    /// Input being parsed
    input: Vec<u8>,
    /// Current position in the input
    position: usize,
    /// External scanner instance
    external_scanner: Option<Box<dyn DynExternalScanner>>,
    /// External scanner runtime
    external_runtime: Option<ExternalScannerRuntime>,
    /// Language name for scanner registry lookup
    #[allow(dead_code)]
    language: String,
    /// Maximum parse duration in microseconds. A value of 0 disables timeouts.
    timeout_micros: u64,
}

impl Parser {
    #[inline]
    fn has_conflicted_actions(&self) -> bool {
        (0..self.parse_table.state_count)
            .any(|state| state_has_conflicts(&self.parse_table, StateId(state as u16)))
    }

    /// Get the grammar used by this parser
    pub fn grammar(&self) -> &Grammar {
        &self.grammar
    }

    /// Get the parse table used by this parser
    pub fn parse_table(&self) -> &ParseTable {
        &self.parse_table
    }

    /// Calculate priority for an action based on precedence and associativity
    #[inline]
    fn action_priority(&self, action: &Action) -> i32 {
        use Action::*;

        // Highest: Accept
        if matches!(action, Accept) {
            return 3_000_000;
        }

        // Pull dynamic precedence if this is a reduce
        let mut prec = 0i32;
        if let Reduce(rid) = action {
            // Get dynamic precedence for this rule
            if (rid.0 as usize) < self.parse_table.dynamic_prec_by_rule.len() {
                prec = self.parse_table.dynamic_prec_by_rule[rid.0 as usize] as i32;
            }

            // Get associativity from the rule: +1 left, -1 right, 0 none
            let assoc_bias = if (rid.0 as usize) < self.parse_table.rule_assoc_by_rule.len() {
                self.parse_table.rule_assoc_by_rule[rid.0 as usize] as i32
            } else {
                0
            };

            // Combine precedence and associativity
            prec = prec.saturating_add(assoc_bias);

            // Bump reduces with positive precedence above plain shift
            if prec > 0 {
                return 2_000_000 + prec;
            }
            // Neutral reduce (slightly below shift to prefer shift in S/R conflicts)
            return 1_500_000 + prec;
        }

        // Plain Shift (default TS policy prefers shift over no-prec reduce)
        if matches!(action, Shift(_)) {
            return 2_000_000;
        }

        0 // Error/other
    }

    /// Internal helper to find rule without Result wrapper
    #[allow(dead_code)]
    fn find_rule_by_production_id_internal(&self, rule_id: RuleId) -> Option<&ParseRule> {
        self.parse_table.rules.get(rule_id.0 as usize)
    }

    /// Create a new parser with the given grammar and parse table
    pub fn new(grammar: Grammar, parse_table: ParseTable, language: String) -> Self {
        // Check if grammar has external tokens
        let (external_scanner, external_runtime) = if !grammar.externals.is_empty() {
            // Get scanner from registry
            let registry = get_global_registry();
            let registry = registry.lock().unwrap_or_else(|err| err.into_inner());

            if let Some(scanner) = registry.create_scanner(&language) {
                let external_tokens: Vec<crate::SymbolId> = grammar
                    .externals
                    .iter()
                    .map(|ext| ext.symbol_id.0)
                    .collect();
                let runtime = ExternalScannerRuntime::new(external_tokens);
                (Some(scanner), Some(runtime))
            } else {
                // eprintln!(
                // "Warning: Grammar has external tokens but no scanner registered for language '{}'",
                // language
                // );
                (None, None)
            }
        } else {
            (None, None)
        };

        Self {
            arena: TreeArena::new(), // Default capacity (1024 nodes)
            grammar,
            parse_table,
            glr_state: GLRParserState::new(),
            input: Vec::new(),
            position: 0,
            external_scanner,
            external_runtime,
            language,
            timeout_micros: 0,
        }
    }

    /// Create a new parser from a TSLanguage struct
    pub fn from_language(
        language: &'static crate::pure_parser::TSLanguage,
        language_name: String,
    ) -> Self {
        Self::from_language_with_patterns(
            language,
            language_name,
            &std::collections::HashMap::new(),
        )
    }

    /// Create a new parser from a TSLanguage struct with token patterns from grammar.json
    pub fn from_language_with_patterns(
        language: &'static crate::pure_parser::TSLanguage,
        language_name: String,
        token_patterns: &std::collections::HashMap<String, TokenPattern>,
    ) -> Self {
        // Decode the grammar and parse table from the TSLanguage struct
        let grammar = crate::decoder::decode_grammar_with_patterns(language, token_patterns);
        let parse_table = crate::decoder::decode_parse_table(language);
        // #[cfg(feature = "debug")]
        // eprintln!(
        // "Parser from_language: parse_table.rules has {} rules",
        // parse_table.rules.len()
        // );

        // Check for external scanner
        let (external_scanner, external_runtime) = if language.external_token_count > 0 {
            // Get scanner from registry
            let registry = get_global_registry();
            let registry = registry.lock().unwrap_or_else(|err| err.into_inner());

            if let Some(scanner) = registry.create_scanner(&language_name) {
                // Create external tokens list from decoded grammar externals.
                let external_tokens: Vec<crate::SymbolId> = grammar
                    .externals
                    .iter()
                    .map(|ext| ext.symbol_id.0)
                    .collect();
                let runtime = ExternalScannerRuntime::new(external_tokens);
                (Some(scanner), Some(runtime))
            } else {
                // eprintln!(
                // "Warning: Grammar has external tokens but no scanner registered for language '{}'",
                // language_name
                // );
                (None, None)
            }
        } else {
            (None, None)
        };

        Self {
            arena: TreeArena::new(), // Default capacity (1024 nodes)
            grammar,
            parse_table,
            glr_state: GLRParserState::new(),
            input: Vec::new(),
            position: 0,
            external_scanner,
            external_runtime,
            language: language_name,
            timeout_micros: 0,
        }
    }

    /// Create a new parser with a custom arena capacity
    ///
    /// # Arguments
    ///
    /// * `capacity` - Initial capacity for arena (number of nodes)
    /// * `grammar` - Grammar to use for parsing
    /// * `parse_table` - Parse table for the grammar
    /// * `language` - Language name for scanner registry lookup
    ///
    /// # Example
    ///
    /// ```ignore
    /// let parser = Parser::with_arena_capacity(grammar, parse_table, "rust".to_string(), 2048);
    /// ```
    pub fn with_arena_capacity(
        grammar: Grammar,
        parse_table: ParseTable,
        language: String,
        capacity: usize,
    ) -> Self {
        // Check if grammar has external tokens
        let (external_scanner, external_runtime) = if !grammar.externals.is_empty() {
            // Get scanner from registry
            let registry = get_global_registry();
            let registry = registry.lock().unwrap_or_else(|err| err.into_inner());

            if let Some(scanner) = registry.create_scanner(&language) {
                let external_tokens: Vec<crate::SymbolId> = grammar
                    .externals
                    .iter()
                    .map(|ext| ext.symbol_id.0)
                    .collect();
                let runtime = ExternalScannerRuntime::new(external_tokens);
                (Some(scanner), Some(runtime))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        Self {
            arena: TreeArena::with_capacity(capacity),
            grammar,
            parse_table,
            glr_state: GLRParserState::new(),
            input: Vec::new(),
            position: 0,
            external_scanner,
            external_runtime,
            language,
            timeout_micros: 0,
        }
    }

    /// Sets the maximum time the parser may run in microseconds.
    pub fn set_timeout_micros(&mut self, timeout: u64) {
        self.timeout_micros = timeout;
    }

    #[cfg(test)]
    pub(crate) fn timeout_micros(&self) -> u64 {
        self.timeout_micros
    }

    /// Get current arena metrics
    ///
    /// Returns a snapshot of the arena's current state including:
    /// - Number of allocated nodes
    /// - Total capacity across all chunks
    /// - Number of chunks
    /// - Approximate memory usage in bytes
    ///
    /// # Performance
    ///
    /// O(chunks) for computing node count. Other metrics are O(1).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let parser = Parser::new(grammar, parse_table, "rust".to_string());
    /// let metrics = parser.arena_metrics();
    /// println!("Arena has {} nodes using {} bytes", metrics.len(), metrics.memory_usage());
    /// ```
    pub fn arena_metrics(&self) -> ArenaMetrics {
        self.arena.metrics()
    }

    /// Set the language for this parser from a TSLanguage struct
    pub fn set_language(
        &mut self,
        language: &'static crate::pure_parser::TSLanguage,
        language_name: String,
    ) -> Result<()> {
        // Validate version
        if language.version != 15 {
            bail!(
                "Incompatible language version. Expected 15, got {}",
                language.version
            );
        }

        // Decode the grammar and parse table from the TSLanguage struct
        self.grammar = crate::decoder::decode_grammar(language);
        self.parse_table = crate::decoder::decode_parse_table(language);
        // #[cfg(feature = "debug_parser")]
        // eprintln!(
        // "Parser set_language: parse_table.rules has {} rules",
        // self.parse_table.rules.len()
        // );
        // eprintln!(
        // "Parser set_language: parse_table.rules has {} rules",
        // self.parse_table.rules.len()
        // );
        self.language = language_name.clone();
        // Update external scanner if needed
        if language.external_token_count > 0 {
            let registry = get_global_registry();
            let registry = registry.lock().unwrap_or_else(|err| err.into_inner());

            if let Some(scanner) = registry.create_scanner(&language_name) {
                let external_tokens: Vec<crate::SymbolId> = self
                    .grammar
                    .externals
                    .iter()
                    .map(|ext| ext.symbol_id.0)
                    .collect();
                let runtime = ExternalScannerRuntime::new(external_tokens);
                self.external_scanner = Some(scanner);
                self.external_runtime = Some(runtime);
            } else {
                self.external_scanner = None;
                self.external_runtime = None;
            }
        } else {
            self.external_scanner = None;
            self.external_runtime = None;
        }

        Ok(())
    }

    /// Parse the input string with automatic lexer selection.
    ///
    /// Custom lexers are ignored in parser-v4.
    /// This method always tokenizes using grammar patterns via GrammarLexer.
    pub fn parse_with_auto_lexer<'a>(
        &'a mut self,
        input: &str,
        _language: &crate::pure_parser::TSLanguage,
    ) -> Result<Tree<'a>> {
        // Note: custom lexer is ignored - parse_internal() uses GrammarLexer
        self.parse(input)
    }

    /// Parse the input string with a custom lexer function
    pub fn parse_with_custom_lexer<'a>(
        &'a mut self,
        input: &str,
        lex_fn: unsafe extern "C" fn(
            *mut core::ffi::c_void,
            crate::pure_parser::TSLexState,
        ) -> bool,
    ) -> Result<Tree<'a>> {
        let _ = (input, lex_fn);
        bail!(PARSE_WITH_CUSTOM_LEXER_UNSUPPORTED)
    }

    /// Parse the input string and return the full parse tree
    ///
    /// This method returns the complete ParseNode tree, which can be used
    /// for extraction and AST construction.
    pub fn parse_tree(&mut self, input: &str) -> Result<ParseNode> {
        // Extract just the parse tree, ignoring error count
        let (root, _error_count) = self.parse_internal(input, true)?;
        Ok(root)
    }

    /// Parse the input string and return the parse tree plus error count.
    pub fn parse_tree_with_error_count(&mut self, input: &str) -> Result<(ParseNode, usize)> {
        self.parse_internal(input, true)
    }

    /// Parse the input string and return a native parse document.
    pub fn parse_document(&mut self, input: &str) -> Result<crate::document::AdzeDocument> {
        let (root, error_count) = self.parse_internal(input, true)?;
        Ok(crate::document::AdzeDocument::from_parse_result(
            input,
            root,
            error_count,
            &self.language,
            &self.grammar,
            &self.parse_table,
        ))
    }

    /// Parse the input string and return minimal tree metadata
    ///
    /// Parse input and return arena-allocated tree
    ///
    /// # Lifetime
    ///
    /// The returned tree borrows the parser's arena. The tree is valid
    /// until the next `parse()` call or parser drop.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut parser = Parser::new(grammar, parse_table, "example".to_string());
    /// let tree = parser.parse("1 + 2")?;
    /// let root_node = parser.parse_tree("1 + 2")?;
    /// ```
    pub fn parse<'a>(&'a mut self, input: &str) -> Result<Tree<'a>> {
        let (root_node, error_count) = self.parse_internal(input, true)?;
        self.arena.reset();
        let root = self.allocate_tree_nodes(&root_node);
        Ok(Tree {
            root,
            arena: &self.arena,
            error_count,
        })
    }

    /// Recursively allocate a `ParseNode` tree into the arena, returning the root handle.
    fn allocate_tree_nodes(&mut self, node: &ParseNode) -> NodeHandle {
        if node.children.is_empty() {
            self.arena.alloc(TreeNode::leaf(node.symbol.0 as i32))
        } else {
            let child_handles: Vec<NodeHandle> = node
                .children
                .iter()
                .map(|child| self.allocate_tree_nodes(child))
                .collect();
            self.arena.alloc(TreeNode::branch_with_symbol(
                node.symbol.0 as i32,
                child_handles,
            ))
        }
    }

    fn synthetic_missing_node(position: usize) -> ParseNode {
        ParseNode {
            symbol: SymbolId(0),
            symbol_id: SymbolId(0),
            start_byte: position,
            end_byte: position,
            field_name: None,
            alias_symbol_id: None,
            children: vec![],
        }
    }

    fn partial_error_tree(
        node_stack: &mut Vec<ParseNode>,
        lookahead: SymbolId,
        current_position: usize,
        input_len: usize,
    ) -> ParseNode {
        let had_partial_context = node_stack.len() > 1;
        let Some(mut node) = node_stack.pop() else {
            return Self::synthetic_missing_node(current_position);
        };

        if had_partial_context && lookahead.0 == 0 && current_position >= input_len {
            node.end_byte = node.end_byte.max(current_position);
            node.children
                .push(Self::synthetic_missing_node(current_position));
        }

        node
    }

    /// Internal parsing implementation shared by parse() and parse_tree()
    /// Returns (ParseNode, error_count)
    fn parse_internal(&mut self, input: &str, _return_tree: bool) -> Result<(ParseNode, usize)> {
        // eprintln!("\nStarting parse of: {:?}", input);
        if self.has_conflicted_actions() {
            bail!("{GLR_CONFLICT_REQUIRES_TRUE_GLR}");
        }

        // Store the input
        self.input = input.as_bytes().to_vec();
        self.position = 0;

        // Initialize the parser state
        let mut state_stack: Vec<StateId> = vec![StateId(0)]; // Start in state 0
        let mut symbol_stack: Vec<SymbolId> = vec![];
        let mut node_stack: Vec<ParseNode> = vec![];
        let mut error_count = 0;

        // Create lexer with grammar's actual tokens
        let tokens: Vec<(SymbolId, TokenPattern, i32)> = self
            .grammar
            .tokens
            .iter()
            .map(|(symbol_id, token)| (*symbol_id, token.pattern.clone(), 0))
            .collect();

        // Debug: print token count and check for "def"
        // eprintln!("Creating lexer with {} tokens", tokens.len());
        for (_symbol_id, _pattern, _) in tokens.iter().take(10) {
            // eprintln!("  Token {}: Symbol {} = {:?}", i, symbol_id.0, pattern);
        }
        // Check if "def" is in the token list
        for (_symbol_id, pattern, _) in &tokens {
            if let TokenPattern::String(s) = pattern
                && s == "def"
            {
                // eprintln!("Found 'def' pattern at symbol {}", symbol_id.0);
                break;
            }
        }

        let mut lexer = GrammarLexer::new(&tokens);

        // Track current position in input
        let input_bytes = input.as_bytes();
        let mut current_position = 0;

        // Main parsing loop with safety limits
        let mut loop_iterations = 0usize;
        const MAX_LOOP_ITERATIONS: usize = 1_000_000; // Prevent infinite loops
        const TIMEOUT_CHECK_INTERVAL: usize = 32; // Amortize time checks in hot loops
        let start_time = Instant::now();

        loop {
            // Periodically enforce time-based parse limits to avoid per-iteration overhead.
            if self.timeout_micros > 0
                && loop_iterations.is_multiple_of(TIMEOUT_CHECK_INTERVAL)
                && start_time.elapsed().as_micros() as u64 > self.timeout_micros
            {
                bail!(
                    "Parser timed out after {} microseconds",
                    self.timeout_micros
                );
            }
            // Safety check to prevent infinite loops
            loop_iterations += 1;
            if loop_iterations > MAX_LOOP_ITERATIONS {
                bail!(
                    "Parser exceeded maximum iteration limit ({}), possible infinite loop",
                    MAX_LOOP_ITERATIONS
                );
            }

            // Get current state
            let current_state = *state_stack.last().ok_or_else(|| {
                anyhow!("State stack is empty at parse loop iteration {loop_iterations}")
            })?;

            // Get the next token from the lexer
            let token = if current_position >= input_bytes.len() {
                // We're at EOF
                LexerToken {
                    symbol: SymbolId(0), // EOF symbol
                    text: vec![],
                    start: current_position,
                    end: current_position,
                }
            } else {
                // First try the external scanner for special tokens (indent/dedent/newline)
                self.position = current_position;
                if let Some(external_token) = self.try_external_scanner(current_state)? {
                    // CRITICAL: Prevent infinite loop on zero-length tokens
                    if external_token.end <= current_position {
                        // External scanner didn't advance, skip a byte to prevent infinite loop
                        current_position += 1;
                        continue;
                    }
                    external_token
                } else {
                    // Fall back to regular lexer
                    match lexer.next_token(input_bytes, current_position) {
                        Some(tok) => tok,
                        None => {
                            // Lexer couldn't match anything - skip a byte
                            error_count += 1;
                            current_position += 1;
                            continue;
                        }
                    }
                }
            };

            let lookahead = token.symbol;

            // Get the actions for this state and lookahead symbol (works for both regular and external tokens)
            let mut actions = self.get_parse_actions(current_state, lookahead)?;

            // Sort actions by priority (highest first) to prefer better actions
            actions.sort_by_key(|a| -self.action_priority(a));

            let _col = self
                .parse_table
                .symbol_to_index
                .get(&lookahead)
                .map(|c| format!("col {}", c))
                .unwrap_or_else(|| "no col".to_string());
            // eprintln!(
            // "State {}, Symbol {} ({}) -> Actions: {:?}",
            // current_state.0, lookahead.0, _col, actions
            // );
            // Debug: print what actions are available in state 0
            if current_state.0 == 0 && actions.is_empty() {
                // #[cfg(feature = "debug")]
                {
                    // eprintln!("  Available actions in state 0:");
                    if !self.parse_table.action_table.is_empty() {
                        for act_cell in self.parse_table.action_table[0].iter() {
                            if !act_cell.is_empty() {
                                // eprintln!("    Symbol {} -> {:?}", sym_idx, act_cell);
                            }
                        }
                    }
                    // eprintln!(
                    // "  Current token has symbol {}, looking for it in grammar...",
                    // token.symbol.0
                    // );
                    // Check what token we actually have
                    if let Some(_tok) = self.grammar.tokens.get(&token.symbol) {
                        // eprintln!("    Token is '{}' in grammar", tok.name);
                    } else {
                        // eprintln!(
                        // "    Token symbol {} not found in grammar tokens",
                        // token.symbol.0
                        // );
                    }
                }
            }

            // Handle the action(s)
            let action = if actions.is_empty() {
                Action::Error
            } else if actions.len() == 1 {
                actions[0].clone()
            } else {
                // Multiple actions - create a Fork
                Action::Fork(actions)
            };

            match action {
                Action::Shift(next_state) => {
                    // Create a leaf node for the token
                    let node = ParseNode {
                        symbol: token.symbol,
                        symbol_id: token.symbol,
                        start_byte: token.start,
                        end_byte: token.end,
                        children: vec![],
                        field_name: None,
                        alias_symbol_id: None,
                    };

                    state_stack.push(next_state);
                    symbol_stack.push(token.symbol);
                    node_stack.push(node);

                    // Advance position to the end of this token
                    current_position = token.end;
                }

                Action::Reduce(rule_id) => {
                    // Find the rule to apply
                    let rule = self.find_rule_by_production_id(rule_id)?;
                    let child_count = rule.rhs_len;

                    // Pop items from stacks
                    let mut children = Vec::new();
                    for _ in 0..child_count {
                        state_stack.pop();
                        symbol_stack.pop();
                        if let Some(child) = node_stack.pop() {
                            children.push(child);
                        }
                    }
                    children.reverse(); // Children were popped in reverse order
                    self.assign_field_names_for_production(rule_id, &mut children);
                    self.assign_aliases_for_production(rule_id, &mut children);

                    // Create a parent node
                    let start_byte = children
                        .first()
                        .map(|n| n.start_byte)
                        .unwrap_or(current_position);
                    let end_byte = children
                        .last()
                        .map(|n| n.end_byte)
                        .unwrap_or(current_position);
                    let parent_node = ParseNode {
                        symbol: rule.lhs,
                        symbol_id: rule.lhs,
                        start_byte,
                        end_byte,
                        children,
                        field_name: None,
                        alias_symbol_id: None,
                    };

                    // Get the goto state for the non-terminal
                    let goto_from_state = *state_stack.last().ok_or_else(|| {
                        anyhow!(
                            "State stack is empty after reducing rule {:?} (lhs {:?}, rhs_len {})",
                            rule_id,
                            rule.lhs,
                            rule.rhs_len
                        )
                    })?;
                    let goto_state = self.get_goto_state(goto_from_state, rule.lhs)?;

                    // Push the new state and symbol
                    state_stack.push(goto_state);
                    symbol_stack.push(rule.lhs);
                    node_stack.push(parent_node);
                }

                Action::Accept => {
                    // Parsing complete!
                    let root_node = node_stack.pop().ok_or_else(|| {
                        anyhow!(
                            "No root node on accept: node_stack is empty after parsing {} bytes",
                            input_bytes.len()
                        )
                    })?;

                    // Return the actual parse tree with error count
                    return Ok((root_node, error_count));
                }

                Action::Error => {
                    // For now, just break on error
                    // A real implementation would do error recovery
                    error_count += 1;

                    let error_node = Self::partial_error_tree(
                        &mut node_stack,
                        lookahead,
                        current_position,
                        input_bytes.len(),
                    );

                    return Ok((error_node, error_count));
                }

                Action::Recover => {
                    // Handle Recover action - treat as error for now
                    error_count += 1;

                    let recovery_node = Self::partial_error_tree(
                        &mut node_stack,
                        lookahead,
                        current_position,
                        input_bytes.len(),
                    );

                    return Ok((recovery_node, error_count));
                }

                Action::Fork(actions) => {
                    bail!(
                        "{GLR_CONFLICT_REQUIRES_TRUE_GLR} state={}, lookahead={}, actions={:?}",
                        current_state.0,
                        lookahead.0,
                        actions
                    );
                }

                _ => {
                    // Unknown action type // Expected: V for Recover
                    error_count += 1;

                    let error_node = Self::partial_error_tree(
                        &mut node_stack,
                        lookahead,
                        current_position,
                        input_bytes.len(),
                    );

                    return Ok((error_node, error_count));
                }
            }

            // Enhanced safety checks to prevent various attack vectors
            if state_stack.len() > 10000 {
                bail!("Parse stack overflow: {} states", state_stack.len());
            }
            if symbol_stack.len() > 10000 {
                bail!("Symbol stack overflow: {} symbols", symbol_stack.len());
            }
            if node_stack.len() > 10000 {
                bail!("Node stack overflow: {} nodes", node_stack.len());
            }

            // Prevent parser from getting stuck at the same position
            if current_position > input_bytes.len() {
                bail!(
                    "Parser position beyond input bounds: {} > {}",
                    current_position,
                    input_bytes.len()
                );
            }
        }
    }

    /// Parse with incremental reuse when possible
    ///
    /// This method attempts to reuse parts of the previous parse tree when parsing
    /// text that has been edited. It provides better performance for small edits
    /// by avoiding reparsing unchanged portions of the text.
    ///
    /// # Arguments
    /// * `input` - The new source text after the edit
    /// * `_old` - The previous parse tree before the edit (currently unused)
    /// * `_edit` - Description of the edit operation (currently unused)
    ///
    /// # Returns
    /// A new parse tree for the edited text, or an error if parsing fails
    ///
    /// # Note
    /// Incremental parsing is currently disabled due to lifetime constraints.
    /// This function performs a fresh parse and ignores the old tree and edit.
    /// See CLAUDE.md for details on the incremental parsing status.
    pub fn reparse<'a>(
        &'a mut self,
        input: &str,
        _old: &Tree<'a>,
        _edit: &crate::pure_incremental::Edit,
    ) -> Result<Tree<'a>> {
        // Incremental parsing is disabled - always do a fresh parse.
        // The old tree and edit parameters are kept for API compatibility.
        self.parse(input)
    }

    /// Get the parse actions for a state and symbol
    fn get_parse_actions(&self, state: StateId, symbol: SymbolId) -> Result<Vec<Action>> {
        // Look up the actions in the parse table
        let state_idx = state.0 as usize;

        // CRITICAL: Map the symbol to its column index in the action table
        let symbol_col = match self.parse_table.symbol_to_index.get(&symbol) {
            Some(&col) => col,
            None => {
                // Unknown symbol - no actions available
                // #[cfg(feature = "debug")]
                // eprintln!("Unknown symbol {} (no column mapping)", symbol.0);
                return Ok(vec![]);
            }
        };

        if state_idx >= self.parse_table.action_table.len() {
            return Ok(vec![]);
        }

        let state_actions = &self.parse_table.action_table[state_idx];
        if symbol_col >= state_actions.len() {
            return Ok(vec![]);
        }

        // Return the action cell (which is a Vec<Action>)
        Ok(state_actions[symbol_col].clone())
    }

    /// Find a rule by its production ID
    fn find_rule_by_production_id(&self, rule_id: RuleId) -> Result<&ParseRule> {
        // #[cfg(feature = "debug")]
        // eprintln!(
        // "Looking for rule ID {} in parse_table.rules (len={})",
        // rule_id.0,
        // self.parse_table.rules.len()
        // );
        if self.parse_table.rules.is_empty() {
            // #[cfg(feature = "debug")]
            // eprintln!("ERROR: parse_table.rules is empty!");
        } else {
            for _rule in self.parse_table.rules.iter().take(5) {
                // #[cfg(feature = "debug")]
                // eprintln!(
                // "  Rule {}: lhs={}, rhs_len={}",
                // _i, _rule.lhs.0, _rule.rhs_len
                // );
            }
        }
        // Get the rule from the parse table
        self.parse_table
            .rules
            .get(rule_id.0 as usize)
            .ok_or_else(|| {
                anyhow!(
                    "Rule with ID {:?} not found in parse table (table has {} rules)",
                    rule_id,
                    self.parse_table.rules.len()
                )
            })
    }

    fn assign_field_names_for_production(&self, rule_id: RuleId, children: &mut [ParseNode]) {
        if children.is_empty() || self.parse_table.field_names.is_empty() {
            return;
        }

        for (child_index, child) in children.iter_mut().enumerate() {
            let Some(field_id) = self
                .parse_table
                .field_map
                .get(&(rule_id, child_index as u16))
            else {
                continue;
            };

            if let Some(field_name) = self.parse_table.field_names.get(*field_id as usize) {
                child.field_name = Some(field_name.clone());
            }
        }
    }

    fn assign_aliases_for_production(&self, rule_id: RuleId, children: &mut [ParseNode]) {
        let Some(alias_sequence) = self.parse_table.alias_sequences.get(rule_id.0 as usize) else {
            return;
        };

        for (child_index, alias_symbol_id) in alias_sequence.iter().enumerate() {
            let Some(alias_symbol_id) = alias_symbol_id else {
                continue;
            };
            let Some(child) = children.get_mut(child_index) else {
                continue;
            };
            child.alias_symbol_id = Some(*alias_symbol_id);
        }
    }

    /// Get the goto state for a non-terminal after a reduce
    fn get_goto_state(&self, from_state: StateId, symbol: SymbolId) -> Result<StateId> {
        // For now, return a default state
        // A real implementation would look up the goto table
        // Since we don't have a proper goto table yet, we'll use a simple heuristic

        // If we have a goto table, use it
        if !self.parse_table.goto_table.is_empty() {
            let state_idx = from_state.0 as usize;
            let symbol_idx = symbol.0 as usize;

            if state_idx < self.parse_table.goto_table.len() {
                let state_gotos = &self.parse_table.goto_table[state_idx];
                if symbol_idx < state_gotos.len() {
                    // The goto table contains StateId values
                    return Ok(state_gotos[symbol_idx]);
                }
            }
        }

        // Fallback: use nonterminal_to_index to find the NT column
        let row = from_state.0 as usize;
        let col = *self
            .parse_table
            .nonterminal_to_index
            .get(&symbol)
            .ok_or_else(|| {
                anyhow!(
                    "No nonterminal-to-index mapping for symbol {:?} in goto lookup from state {}",
                    symbol,
                    from_state.0
                )
            })?;

        // Check bounds
        if row >= self.parse_table.action_table.len() {
            bail!(
                "State {} out of bounds (table has {} states)",
                row,
                self.parse_table.action_table.len()
            );
        }
        if col >= self.parse_table.action_table[row].len() {
            bail!(
                "Column {} out of bounds for state {} (row has {} columns)",
                col,
                row,
                self.parse_table.action_table[row].len()
            );
        }

        let cell = &self.parse_table.action_table[row][col];
        // #[cfg(feature = "debug")]
        // eprintln!(
        // "  Goto lookup: state {} col {} -> actions: {:?}",
        // row, col, cell
        // );

        // Prefer/require Shift action for goto
        cell.iter()
            .find_map(|a| {
                if let Action::Shift(s) = a {
                    Some(*s)
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                anyhow!(
                    "No goto (Shift) for NT {:?} in state {}",
                    symbol,
                    from_state.0
                )
            })
    }

    /// Try to scan for external tokens
    fn try_external_scanner(&mut self, current_state: StateId) -> Result<Option<LexerToken>> {
        // Compute valid external tokens for this state first (before mutable borrow)
        let valid_externals = self.compute_valid_externals(current_state)?;
        // #[cfg(feature = "debug")]
        // eprintln!(
        // "Valid externals for state {}: {:?}",
        // current_state.0, valid_externals
        // );

        if valid_externals.is_empty() {
            // #[cfg(feature = "debug")]
            // eprintln!("No valid externals for state {}", current_state.0);
            return Ok(None);
        }

        // Check if we have external scanner
        if self.external_scanner.is_none() || self.external_runtime.is_none() {
            // #[cfg(feature = "debug")]
            // eprintln!("No external scanner available");
            return Ok(None);
        }

        // Create a simple lexer adapter
        struct LexerAdapter<'a> {
            parser: &'a mut Parser,
        }

        impl<'a> crate::external_scanner::Lexer for LexerAdapter<'a> {
            fn lookahead(&self) -> Option<u8> {
                if self.parser.position < self.parser.input.len() {
                    Some(self.parser.input[self.parser.position])
                } else {
                    None
                }
            }

            fn advance(&mut self, n: usize) {
                self.parser.position =
                    std::cmp::min(self.parser.position + n, self.parser.input.len());
            }

            fn mark_end(&mut self) {
                // No-op for now
            }

            fn column(&self) -> usize {
                let mut col = 0;
                for i in (0..self.parser.position).rev() {
                    if self.parser.input[i] == b'\n' {
                        break;
                    }
                    col += 1;
                }
                col
            }

            fn is_eof(&self) -> bool {
                self.parser.position >= self.parser.input.len()
            }
        }

        // Convert the valid externals to a boolean array for the scanner
        // The scanner expects an array indexed by external token index (0-8)
        let mut valid_symbols = vec![false; self.grammar.externals.len()];
        for (idx, external) in self.grammar.externals.iter().enumerate() {
            if valid_externals.contains(&external.symbol_id) {
                valid_symbols[idx] = true;
            }
        }

        let token_start = self.position;

        // We need to temporarily take the scanner out to avoid double borrow
        let Some(mut scanner) = self.external_scanner.take() else {
            return Ok(None);
        };
        let scan_result = {
            let mut adapter = LexerAdapter { parser: self };
            scanner.scan(&mut adapter, &valid_symbols)
        };
        // Put the scanner back
        self.external_scanner = Some(scanner);

        if let Some(result) = scan_result {
            if !valid_externals.contains(&SymbolId(result.symbol)) {
                return Ok(None);
            }

            // Extract token text from the pre-scan position. Scanners may advance
            // the parser while looking ahead, so using the post-scan position here
            // would report shifted ranges for every emitted token.
            let end = token_start.saturating_add(result.length);
            let text = if end <= self.input.len() {
                self.input[token_start..end].to_vec()
            } else {
                Vec::new()
            };

            Ok(Some(LexerToken {
                symbol: SymbolId(result.symbol),
                text,
                start: token_start,
                end,
            }))
        } else {
            Ok(None)
        }
    }

    /// Compute which external tokens are valid in the given state
    fn compute_valid_externals(&self, state: StateId) -> Result<HashSet<SymbolId>> {
        let mut valid_externals = HashSet::new();

        let state_idx = state.0 as usize;

        // Check the external_scanner_states table
        if state_idx < self.parse_table.external_scanner_states.len() {
            let external_states = &self.parse_table.external_scanner_states[state_idx];

            // For each external token, check if it's valid in this state
            for (idx, external) in self.grammar.externals.iter().enumerate() {
                if idx < external_states.len() && external_states[idx] {
                    valid_externals.insert(external.symbol_id);
                }
            }
        }

        Ok(valid_externals)
    }

    /// Get action from parse table
    #[allow(dead_code)]
    fn get_action(&self, state: StateId, symbol: SymbolId) -> Result<Action> {
        let state_idx = state.0 as usize;
        if state_idx < self.parse_table.action_table.len()
            && let Some(&symbol_idx) = self.parse_table.symbol_to_index.get(&symbol)
            && symbol_idx < self.parse_table.action_table[state_idx].len()
        {
            let action_cell = &self.parse_table.action_table[state_idx][symbol_idx];
            if action_cell.is_empty() {
                return Ok(Action::Error);
            } else if action_cell.len() == 1 {
                return Ok(action_cell[0].clone());
            } else {
                // Multiple actions - create a Fork
                return Ok(Action::Fork(action_cell.clone()));
            }
        }

        // No action found - this is an error
        Ok(Action::Error)
    }

    /// Get expected symbols for error reporting
    #[allow(dead_code)]
    fn get_expected_symbols(&self, state: StateId) -> Vec<SymbolId> {
        let state_idx = state.0 as usize;
        let mut expected = Vec::new();

        if state_idx < self.parse_table.action_table.len() {
            let state_actions = &self.parse_table.action_table[state_idx];

            // Iterate through all symbols to find ones with non-error actions
            for (&symbol_id, &idx) in &self.parse_table.symbol_to_index {
                if idx < state_actions.len() {
                    let action_cell = &state_actions[idx];
                    if !action_cell.is_empty() {
                        // Include only terminals and external tokens
                        if self.grammar.tokens.contains_key(&symbol_id)
                            || self
                                .grammar
                                .externals
                                .iter()
                                .any(|ext| ext.symbol_id == symbol_id)
                        {
                            expected.push(symbol_id);
                        }
                    }
                }
            }
        }

        expected
    }

    /// Find a rule by its ID
    fn find_rule_by_id(&self, rule_id: RuleId) -> Result<&Rule> {
        // Rules are stored per symbol, need to search all of them
        for (_, rules) in &self.grammar.rules {
            for rule in rules {
                if rule.production_id.0 == rule_id.0 {
                    return Ok(rule);
                }
            }
        }
        bail!(
            "Rule with ID {:?} not found in grammar (searched all symbol rule sets)",
            rule_id
        )
    }

    // GLR-specific methods

    /// Get next token (handles external scanner)
    #[allow(dead_code)]
    fn get_next_token(&mut self, lexer: &mut GrammarLexer) -> Result<LexerToken> {
        // Try external scanner on first active head
        if !self.glr_state.active_heads.is_empty() {
            let current_state =
                StateId(self.glr_state.gss_nodes[self.glr_state.active_heads[0]].state as u16);
            if let Some(external_token) = self.try_external_scanner(current_state)? {
                return Ok(external_token);
            }
        }

        // Fall back to regular lexer
        match lexer.next_token(&self.input, self.position) {
            Some(tok) => Ok(tok),
            None => bail!(
                "Lexer failed to produce token at position {} (input length: {} bytes)",
                self.position,
                self.input.len()
            ),
        }
    }

    /// Handle shift in GLR mode
    #[allow(dead_code)]
    fn handle_glr_shift(
        &mut self,
        gss_idx: usize,
        next_state: StateId,
        token: LexerToken,
    ) -> Result<usize> {
        // Create terminal forest node
        let terminal_node =
            self.glr_state
                .get_or_create_forest_node(token.symbol, token.start, token.end, || {
                    ForestNode::Terminal {
                        symbol: token.symbol,
                        start: token.start,
                        end: token.end,
                        text: token.text.clone(),
                    }
                });

        // Fork or reuse GSS node
        let new_gss_idx = self.glr_state.fork(gss_idx, next_state.0 as usize);

        // Add link from new node to parent
        self.glr_state.gss_nodes[new_gss_idx]
            .parents
            .push(crate::glr_forest::GSSLink {
                parent: gss_idx,
                tree_node: terminal_node,
            });

        Ok(new_gss_idx)
    }

    /// Handle reduce in GLR mode
    #[allow(dead_code)]
    fn handle_glr_reduce(&mut self, gss_idx: usize, rule_id: RuleId) -> Result<Vec<usize>> {
        // Clone the rule data we need to avoid borrow checker issues
        let (rule_lhs, rule_len) = {
            let rule = self.find_rule_by_id(rule_id)?;
            (rule.lhs, rule.rhs.len())
        };
        let mut new_heads = Vec::new();

        // Perform reduction starting from this GSS node
        self.perform_glr_reduce(
            gss_idx,
            rule_lhs,
            rule_id,
            rule_len,
            Vec::new(),
            &mut new_heads,
        )?;

        Ok(new_heads)
    }

    /// Maximum recursion depth to prevent stack overflow attacks
    const MAX_RECURSION_DEPTH: usize = 1000;

    /// Recursively perform GLR reduction with stack overflow protection
    #[allow(dead_code)]
    fn perform_glr_reduce(
        &mut self,
        current_gss: usize,
        rule_lhs: SymbolId,
        rule_id: RuleId,
        remaining: usize,
        children: Vec<Rc<ForestNode>>,
        new_heads: &mut Vec<usize>,
    ) -> Result<()> {
        self.perform_glr_reduce_with_depth(
            current_gss,
            rule_lhs,
            rule_id,
            remaining,
            children,
            new_heads,
            0,
        )
    }

    /// Internal recursive function with depth tracking
    #[allow(
        dead_code,
        clippy::too_many_arguments,
        reason = "legacy Clippy suppression is kept explicit while staged cleanup narrows this surface"
    )]
    fn perform_glr_reduce_with_depth(
        &mut self,
        current_gss: usize,
        rule_lhs: SymbolId,
        rule_id: RuleId,
        remaining: usize,
        children: Vec<Rc<ForestNode>>,
        new_heads: &mut Vec<usize>,
        depth: usize,
    ) -> Result<()> {
        // Check recursion depth to prevent stack overflow
        if depth >= Self::MAX_RECURSION_DEPTH {
            bail!(
                "Maximum recursion depth exceeded in GLR reduction (depth: {})",
                depth
            );
        }

        // Validate current_gss index to prevent out-of-bounds access
        if current_gss >= self.glr_state.gss_nodes.len() {
            bail!(
                "Invalid GSS node index: {} (max: {})",
                current_gss,
                self.glr_state.gss_nodes.len()
            );
        }
        if remaining == 0 {
            // Reduction complete - create non-terminal node
            let mut children = children; // Make mutable for reverse
            children.reverse(); // Children were collected in reverse order

            let start = if children.is_empty() {
                self.position
            } else {
                match children
                    .first()
                    .expect("children verified non-empty above")
                    .as_ref()
                {
                    ForestNode::Terminal { start, .. } => *start,
                    ForestNode::NonTerminal { start, .. } => *start,
                }
            };

            let end = if children.is_empty() {
                self.position
            } else {
                match children
                    .last()
                    .expect("children verified non-empty above")
                    .as_ref()
                {
                    ForestNode::Terminal { end, .. } => *end,
                    ForestNode::NonTerminal { end, .. } => *end,
                }
            };

            let packed_node = PackedNode {
                rule_id,
                children: children.clone(),
            };

            let forest_node = self
                .glr_state
                .merge_trees(rule_lhs, start, end, packed_node);

            // Get goto state
            let current_state = self.glr_state.gss_nodes[current_gss].state;
            let goto_state = self.get_goto_for_state(current_state, rule_lhs)?;

            // Create or reuse GSS node for goto state
            let new_gss = self.glr_state.fork(current_gss, goto_state);
            self.glr_state.gss_nodes[new_gss]
                .parents
                .push(crate::glr_forest::GSSLink {
                    parent: current_gss,
                    tree_node: forest_node,
                });

            new_heads.push(new_gss);
        } else {
            // Continue reduction - follow all parent links with bounds checking
            let parents = self.glr_state.gss_nodes[current_gss].parents.clone();

            // Prevent excessive branching that could lead to exponential explosion
            if parents.len() > 100 {
                bail!(
                    "Excessive parent links in GSS node: {} (current: {})",
                    parents.len(),
                    current_gss
                );
            }

            for link in parents {
                // Validate parent index
                if link.parent >= self.glr_state.gss_nodes.len() {
                    continue; // Skip invalid parent link
                }

                let mut new_children = children.clone();
                new_children.push(link.tree_node.clone());

                // Check for reasonable children count to prevent memory exhaustion
                if new_children.len() > 10000 {
                    bail!(
                        "Excessive children count in GLR reduction: {}",
                        new_children.len()
                    );
                }

                self.perform_glr_reduce_with_depth(
                    link.parent,
                    rule_lhs,
                    rule_id,
                    remaining.saturating_sub(1), // Prevent underflow
                    new_children,
                    new_heads,
                    depth + 1,
                )?;
            }
        }

        Ok(())
    }

    /// Get goto state for a given state and symbol
    #[allow(dead_code)]
    fn get_goto_for_state(&self, state: usize, symbol: SymbolId) -> Result<usize> {
        if state < self.parse_table.goto_table.len()
            && let Some(&symbol_idx) = self.parse_table.symbol_to_index.get(&symbol)
            && symbol_idx < self.parse_table.goto_table[state].len()
        {
            let goto_state = self.parse_table.goto_table[state][symbol_idx];
            if goto_state != StateId(0) {
                // 0 typically means no transition
                return Ok(goto_state.0 as usize);
            }
        }
        bail!(
            "No goto action for symbol {:?} in state {} (goto table has {} states)",
            symbol,
            state,
            self.parse_table.goto_table.len()
        )
    }

    /// Build final tree from accepted GSS node
    #[allow(dead_code)]
    fn build_final_tree(&self, gss_idx: usize) -> Result<ForestNode> {
        // Find the path from this node to the start
        let mut current = gss_idx;
        let mut nodes = Vec::new();

        while !self.glr_state.gss_nodes[current].parents.is_empty() {
            let link = &self.glr_state.gss_nodes[current].parents[0];
            nodes.push(link.tree_node.clone());
            current = link.parent;
        }

        // The last node should be the root of the parse tree
        if let Some(root) = nodes.last() {
            Ok(root.as_ref().clone())
        } else {
            bail!(
                "No parse tree found after processing {} GSS nodes",
                nodes.len()
            )
        }
    }

    /// Get raw input bytes
    pub fn raw_input(&self) -> &[u8] {
        &self.input
    }

    /// Get current byte position
    pub fn byte_pos(&self) -> usize {
        self.position
    }

    /// Borrow the lexer as a trait object
    pub fn borrow_lexer(&mut self) -> &mut dyn crate::external_scanner::Lexer {
        self as &mut dyn crate::external_scanner::Lexer
    }

    /// Advance from scanner result
    pub fn advance_from_scanner(&mut self, length: usize) {
        // Use saturating arithmetic and bounds checking to prevent overflow
        self.position = self.position.saturating_add(length).min(self.input.len());
    }

    /// Get TS lexer pointer (for FFI compatibility)
    pub fn ts_lexer_ptr(&mut self) -> *mut std::ffi::c_void {
        self as *mut _ as *mut std::ffi::c_void
    }

    /// Reset the parser state
    ///
    /// This clears any internal state and prepares the parser for a fresh parse
    pub fn reset(&mut self) {
        self.glr_state = GLRParserState::new();
        self.input.clear();
        self.position = 0;
        // Reset external scanner state if present
        if let Some(ref mut runtime) = self.external_runtime {
            runtime.reset();
        }
    }

    /// Get the GLR parser statistics
    pub fn get_glr_stats(&self) -> &crate::glr_forest::GLRStats {
        self.glr_state.get_stats()
    }
}

/// Implement the Lexer trait for Parser so it can be used by external scanners
impl crate::external_scanner::Lexer for Parser {
    fn lookahead(&self) -> Option<u8> {
        if self.position < self.input.len() {
            Some(self.input[self.position])
        } else {
            None
        }
    }

    fn advance(&mut self, n: usize) {
        // Use saturating arithmetic to prevent overflow
        self.position = self.position.saturating_add(n).min(self.input.len());
    }

    fn mark_end(&mut self) {
        // For external scanners, mark_end is typically used to mark
        // the end of the current token. This is handled by the scanner
        // returning the length, so this is a no-op for now.
    }

    fn column(&self) -> usize {
        // Calculate column by counting back from current position to last newline
        let mut col = 0;
        for i in (0..self.position).rev() {
            if self.input[i] == b'\n' {
                break;
            }
            col += 1;
        }
        col
    }

    fn is_eof(&self) -> bool {
        self.position >= self.input.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pure_parser::TSLanguage;
    use crate::pure_parser::{ExternalScanner, TSLexState};
    use crate::scanner_registry::ExternalScannerBuilder;
    use crate::scanners::IndentationScanner;
    use std::cell::RefCell;
    use std::ffi::c_void;

    thread_local! {
        static MINIMAL_CUSTOM_LEXER_LANGUAGE: RefCell<Option<Box<TSLanguage>>> =
            const { RefCell::new(None) };
    }

    #[allow(dead_code)]
    unsafe extern "C" fn test_custom_lexer_fn(_lexer: *mut c_void, _state: TSLexState) -> bool {
        true
    }

    fn minimal_custom_lexer_language() -> &'static TSLanguage {
        MINIMAL_CUSTOM_LEXER_LANGUAGE.with(|language| {
            let mut language = language.borrow_mut();
            let language = language.get_or_insert_with(|| {
                Box::new(TSLanguage {
                    version: 15,
                    symbol_count: 0,
                    alias_count: 0,
                    token_count: 0,
                    external_token_count: 0,
                    state_count: 0,
                    large_state_count: 0,
                    production_id_count: 0,
                    field_count: 0,
                    max_alias_sequence_length: 0,
                    production_id_map: std::ptr::null(),
                    parse_table: std::ptr::null(),
                    small_parse_table: std::ptr::null(),
                    small_parse_table_map: std::ptr::null(),
                    parse_actions: std::ptr::null(),
                    symbol_names: std::ptr::null(),
                    field_names: std::ptr::null(),
                    field_map_slices: std::ptr::null(),
                    field_map_entries: std::ptr::null(),
                    symbol_metadata: std::ptr::null(),
                    public_symbol_map: std::ptr::null(),
                    alias_map: std::ptr::null(),
                    alias_sequences: std::ptr::null(),
                    lex_modes: std::ptr::null(),
                    lex_fn: Some(test_custom_lexer_fn),
                    keyword_lex_fn: None,
                    keyword_capture_token: 0,
                    external_scanner: ExternalScanner::default(),
                    primary_state_ids: std::ptr::null(),
                    production_lhs_index: std::ptr::null(),
                    production_count: 0,
                    rules: std::ptr::null(),
                    rule_count: 0,
                    eof_symbol: 0,
                })
            });
            let language_ptr = language.as_ref() as *const TSLanguage;
            unsafe { &*language_ptr }
        })
    }

    #[test]
    fn test_assign_field_names_for_production_preserves_named_children() {
        let mut field_map = std::collections::BTreeMap::new();
        field_map.insert((RuleId(2), 0), 1);
        let parse_table = ParseTable {
            field_names: vec!["_".to_string(), "value".to_string()],
            field_map,
            ..Default::default()
        };

        let parser = Parser::new(
            Grammar::new("field_name_test".to_string()),
            parse_table,
            "field_name_test".to_string(),
        );
        let mut children = vec![
            ParseNode {
                symbol: SymbolId(4),
                symbol_id: SymbolId(4),
                start_byte: 0,
                end_byte: 1,
                field_name: None,
                alias_symbol_id: None,
                children: vec![],
            },
            ParseNode {
                symbol: SymbolId(5),
                symbol_id: SymbolId(5),
                start_byte: 1,
                end_byte: 2,
                field_name: None,
                alias_symbol_id: None,
                children: vec![],
            },
        ];

        parser.assign_field_names_for_production(RuleId(2), &mut children);

        assert_eq!(children[0].field_name.as_deref(), Some("value"));
        assert_eq!(children[1].field_name, None);
    }

    #[test]
    fn test_parser_with_external_scanner() {
        // Register a scanner with a stable language key for this regression test
        let language_name = "test_parser_with_external_scanner".to_string();
        ExternalScannerBuilder::new(language_name.clone()).register_rust::<IndentationScanner>();

        // Create a simple grammar with external tokens
        let mut grammar = Grammar::new(language_name.clone());

        // Add external tokens
        grammar.externals.push(adze_ir::ExternalToken {
            name: "NEWLINE".to_string(),
            symbol_id: SymbolId(0),
        });
        grammar.externals.push(adze_ir::ExternalToken {
            name: "INDENT".to_string(),
            symbol_id: SymbolId(1),
        });
        grammar.externals.push(adze_ir::ExternalToken {
            name: "DEDENT".to_string(),
            symbol_id: SymbolId(2),
        });

        // Create a dummy parse table
        let parse_table = ParseTable {
            action_table: vec![],
            goto_table: vec![],
            symbol_metadata: vec![],
            state_count: 0,
            symbol_count: 0,
            symbol_to_index: std::collections::BTreeMap::new(),
            index_to_symbol: vec![],
            external_scanner_states: vec![vec![true, false, false]],
            rules: vec![],
            nonterminal_to_index: std::collections::BTreeMap::new(),
            goto_indexing: adze_glr_core::GotoIndexing::NonterminalMap,
            eof_symbol: SymbolId(0),
            start_symbol: SymbolId(1),
            grammar: Grammar::default(),
            initial_state: StateId(0),
            token_count: 0,
            external_token_count: 0,
            lex_modes: vec![],
            extras: vec![],
            dynamic_prec_by_rule: vec![],
            rule_assoc_by_rule: vec![],
            alias_sequences: vec![],
            field_names: vec![],
            field_map: std::collections::BTreeMap::new(),
        };

        // Create parser
        let mut parser = Parser::new(grammar, parse_table, language_name);
        parser.input = b"\n".to_vec();
        parser.position = 0;

        // Regression: ensure external scanner path is exercised
        assert!(parser.external_scanner.is_some());
        assert!(parser.external_runtime.is_some());

        let scanned = parser
            .try_external_scanner(StateId(0))
            .expect("external scanner should be available")
            .expect("external scanner should emit NEWLINE");
        assert_eq!(scanned.symbol, SymbolId(0));
        // After scanner advances, position reflects post-scan state
        assert_eq!(scanned.start, 0);
        assert_eq!(scanned.end, 1);
        assert_eq!(scanned.text, b"\n");
    }

    #[derive(Default)]
    struct InvalidEmittingScanner;

    impl crate::external_scanner::ExternalScanner for InvalidEmittingScanner {
        fn scan(
            &mut self,
            _lexer: &mut dyn crate::external_scanner::Lexer,
            _valid_symbols: &[bool],
        ) -> Option<crate::external_scanner::ScanResult> {
            // Always emits token id 1, even when parser state only allows token id 0.
            Some(crate::external_scanner::ScanResult {
                symbol: 1,
                length: 0,
            })
        }

        fn serialize(&self, _buffer: &mut Vec<u8>) {}

        fn deserialize(&mut self, _buffer: &[u8]) {}
    }

    #[derive(Default)]
    struct NewlineIndexScanner;

    impl crate::external_scanner::ExternalScanner for NewlineIndexScanner {
        fn scan(
            &mut self,
            lexer: &mut dyn crate::external_scanner::Lexer,
            valid_symbols: &[bool],
        ) -> Option<crate::external_scanner::ScanResult> {
            if valid_symbols.get(1) == Some(&true) && lexer.lookahead() == Some(b'\n') {
                lexer.advance(1);
                lexer.mark_end();
                return Some(crate::external_scanner::ScanResult {
                    symbol: 1,
                    length: 1,
                });
            }

            None
        }

        fn serialize(&self, _buffer: &mut Vec<u8>) {}

        fn deserialize(&mut self, _buffer: &[u8]) {}
    }

    fn external_newline_parser(language_name: &str) -> Parser {
        ExternalScannerBuilder::new(language_name.to_string())
            .register_rust::<NewlineIndexScanner>();

        let eof = SymbolId(0);
        let newline = SymbolId(1);
        let root = SymbolId(2);
        let mut grammar = Grammar::new(language_name.to_string());
        grammar.externals.push(adze_ir::ExternalToken {
            name: "_unused_external_zero".to_string(),
            symbol_id: eof,
        });
        grammar.externals.push(adze_ir::ExternalToken {
            name: "NEWLINE".to_string(),
            symbol_id: newline,
        });

        let mut symbol_to_index = std::collections::BTreeMap::new();
        symbol_to_index.insert(eof, 0);
        symbol_to_index.insert(newline, 1);
        symbol_to_index.insert(root, 2);

        let mut action_table = vec![vec![vec![Action::Error]; 3]; 3];
        action_table[0][1] = vec![Action::Shift(StateId(1))];
        action_table[1][0] = vec![Action::Reduce(RuleId(0))];
        action_table[2][0] = vec![Action::Accept];

        let mut goto_table = vec![vec![StateId(0); 3]; 3];
        goto_table[0][2] = StateId(2);

        let parse_table = ParseTable {
            action_table,
            goto_table,
            symbol_metadata: vec![
                adze_glr_core::SymbolMetadata {
                    name: "EOF".to_string(),
                    is_visible: false,
                    is_named: false,
                    is_supertype: false,
                    is_terminal: true,
                    is_extra: false,
                    is_fragile: false,
                    symbol_id: eof,
                },
                adze_glr_core::SymbolMetadata {
                    name: "NEWLINE".to_string(),
                    is_visible: true,
                    is_named: false,
                    is_supertype: false,
                    is_terminal: true,
                    is_extra: false,
                    is_fragile: false,
                    symbol_id: newline,
                },
                adze_glr_core::SymbolMetadata {
                    name: "Root".to_string(),
                    is_visible: true,
                    is_named: true,
                    is_supertype: false,
                    is_terminal: false,
                    is_extra: false,
                    is_fragile: false,
                    symbol_id: root,
                },
            ],
            state_count: 3,
            symbol_count: 3,
            symbol_to_index,
            index_to_symbol: vec![eof, newline, root],
            external_scanner_states: vec![
                vec![false, true],
                vec![false, false],
                vec![false, false],
            ],
            rules: vec![ParseRule {
                lhs: root,
                rhs_len: 1,
            }],
            nonterminal_to_index: std::collections::BTreeMap::new(),
            goto_indexing: adze_glr_core::GotoIndexing::NonterminalMap,
            eof_symbol: eof,
            start_symbol: root,
            grammar: grammar.clone(),
            initial_state: StateId(0),
            token_count: 1,
            external_token_count: 2,
            lex_modes: vec![],
            extras: vec![],
            dynamic_prec_by_rule: vec![],
            rule_assoc_by_rule: vec![],
            alias_sequences: vec![],
            field_names: vec![],
            field_map: std::collections::BTreeMap::new(),
        };

        Parser::new(grammar, parse_table, language_name.to_string())
    }

    #[test]
    fn test_external_scanner_rejects_token_not_in_valid_symbols() {
        let language_name = "test_parser_external_scanner_valid_symbols_contract".to_string();
        ExternalScannerBuilder::new(language_name.clone())
            .register_rust::<InvalidEmittingScanner>();

        let mut grammar = Grammar::new(language_name.clone());
        grammar.externals.push(adze_ir::ExternalToken {
            name: "NEWLINE".to_string(),
            symbol_id: SymbolId(0),
        });
        grammar.externals.push(adze_ir::ExternalToken {
            name: "INDENT".to_string(),
            symbol_id: SymbolId(1),
        });

        let parse_table = ParseTable {
            action_table: vec![],
            goto_table: vec![],
            symbol_metadata: vec![],
            state_count: 0,
            symbol_count: 0,
            symbol_to_index: std::collections::BTreeMap::new(),
            index_to_symbol: vec![],
            // Only NEWLINE is valid in this state.
            external_scanner_states: vec![vec![true, false]],
            rules: vec![],
            nonterminal_to_index: std::collections::BTreeMap::new(),
            goto_indexing: adze_glr_core::GotoIndexing::NonterminalMap,
            eof_symbol: SymbolId(0),
            start_symbol: SymbolId(1),
            grammar: Grammar::default(),
            initial_state: StateId(0),
            token_count: 0,
            external_token_count: 0,
            lex_modes: vec![],
            extras: vec![],
            dynamic_prec_by_rule: vec![],
            rule_assoc_by_rule: vec![],
            alias_sequences: vec![],
            field_names: vec![],
            field_map: std::collections::BTreeMap::new(),
        };

        let mut parser = Parser::new(grammar, parse_table, language_name);
        parser.input = b"\n".to_vec();
        parser.position = 0;

        let scanned = parser
            .try_external_scanner(StateId(0))
            .expect("external scanner dispatch should be available");
        assert!(
            scanned.is_none(),
            "scanner emitted token that is false in valid_symbols and must be rejected",
        );
    }

    #[test]
    fn test_external_scanner_parse_document_bad_input_returns_diagnostic_document() {
        let mut parser = external_newline_parser(
            "test_external_scanner_parse_document_bad_input_returns_diagnostic_document",
        );
        let source = "x";

        let document = parser
            .parse_document(source)
            .expect("external-scanner parser should return a diagnostic document for bad input");
        let diagnostic = document
            .diagnostics()
            .first()
            .expect("bad external-scanner input should produce a diagnostic");

        assert!(document.metadata().error_count > 0);
        assert!(document.tree().has_errors());
        assert!(diagnostic.start_byte <= diagnostic.end_byte);
        assert!(diagnostic.end_byte <= source.len());
        assert_eq!(
            diagnostic.point_range,
            crate::document::PointRange::from_byte_range(source, diagnostic.byte_span())
        );
    }

    #[test]
    fn test_parse_with_custom_lexer_is_unsupported() {
        let language = minimal_custom_lexer_language();
        let mut parser = Parser::from_language(language, "custom_lexer_test".to_string());

        let result = parser.parse_with_custom_lexer("abc", test_custom_lexer_fn);
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains(PARSE_WITH_CUSTOM_LEXER_UNSUPPORTED),
            "parse_with_custom_lexer should explicitly reject custom lexers in parser-v4",
        );
    }

    #[test]
    fn test_parse_with_custom_lexer_falls_back_to_grammar_lexer() {
        // Custom lexer functions are now ignored - parse_internal() uses GrammarLexer
        // which handles tokenization from the grammar's token patterns.
        // This allows grammars with custom lexers to work as long as they have
        // proper token patterns defined in the grammar.
        let language = minimal_custom_lexer_language();
        let mut parser = Parser::from_language(language, "custom_lexer_test".to_string());

        // parse() should now succeed (or fail for parsing reasons, not custom lexer rejection)
        let result = parser.parse("abc");
        // The parse may succeed or fail depending on the grammar, but it should NOT
        // fail with "Custom lexer functions are not yet supported"
        if let Err(ref e) = result {
            assert!(
                !e.to_string().contains("Custom lexer functions"),
                "parse() should not reject custom lexer, got error: {}",
                e
            );
        }

        let result = parser.parse_with_auto_lexer("abc", language);
        if let Err(ref e) = result {
            assert!(
                !e.to_string().contains("Custom lexer functions"),
                "parse_with_auto_lexer() should not reject custom lexer, got error: {}",
                e
            );
        }
    }
}
