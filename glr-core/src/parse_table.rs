use crate::{TableError, parse_forest};
use adze_ir::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Lexer mode for a parser state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serialization", derive(Serialize, Deserialize))]
pub struct LexMode {
    /// Internal lexer DFA state
    pub lex_state: u16,
    /// State for the external scanner (if any)
    pub external_lex_state: u16,
}

/// How GOTO table columns are indexed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serialization", derive(Serialize, Deserialize))]
pub enum GotoIndexing {
    /// Use nonterminal_to_index mapping (standard)
    NonterminalMap,
    /// Use SymbolId.0 directly as column index (some table generators)
    DirectSymbolId,
}

/// GLR-compatible parse table supporting multiple actions per state
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialization", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub struct ParseTable {
    /// ACTION table: indexed by `[state][terminal]` using symbol_to_index
    pub action_table: Vec<Vec<ActionCell>>,
    /// GOTO table: indexed by `[state][nonterminal]` using nonterminal_to_index or direct ID
    pub goto_table: Vec<Vec<StateId>>,
    /// Metadata (name, visibility, etc.) for each symbol in the grammar.
    pub symbol_metadata: Vec<SymbolMetadata>,
    /// Total number of parser states.
    pub state_count: usize,
    /// Total number of symbols (terminals + non-terminals).
    pub symbol_count: usize,
    /// Maps terminal symbols to ACTION table column indices
    pub symbol_to_index: BTreeMap<SymbolId, usize>,
    /// Index -> SymbolId, perfectly mirroring `symbol_to_index`.
    pub index_to_symbol: Vec<SymbolId>,
    /// For each state, a bitset indicating which external tokens are valid
    pub external_scanner_states: Vec<Vec<bool>>,
    /// Grammar rules for reduction
    pub rules: Vec<ParseRule>,
    /// Maps nonterminal symbols to GOTO table column indices
    pub nonterminal_to_index: BTreeMap<SymbolId, usize>,
    /// How GOTO table columns are indexed
    pub goto_indexing: GotoIndexing,
    /// EOF symbol ID
    pub eof_symbol: SymbolId,
    /// Start symbol ID
    pub start_symbol: SymbolId,
    /// Grammar metadata
    pub grammar: Grammar,
    /// Initial parser state (default: 0, Tree-sitter uses 1)
    pub initial_state: StateId,
    /// Number of tokens (regular terminals)
    pub token_count: usize,
    /// Number of external tokens (from external scanner)
    pub external_token_count: usize,
    /// Lex modes for each state (length == state_count)
    pub lex_modes: Vec<LexMode>,
    /// Terminal symbols to skip as whitespace/comments
    pub extras: Vec<SymbolId>,
    /// Dynamic precedence for each rule (optional)
    pub dynamic_prec_by_rule: Vec<i16>,
    /// Associativity for each rule: -1=Right, 0=None, +1=Left
    pub rule_assoc_by_rule: Vec<i8>,
    /// Alias sequences for rules
    pub alias_sequences: Vec<Vec<Option<SymbolId>>>,
    /// Field names
    pub field_names: Vec<String>,
    /// Map (rule, child_index) -> field_id
    pub field_map: BTreeMap<(RuleId, u16), u16>,
}

/// Parse rule for reduction
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialization", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub struct ParseRule {
    /// Left-hand side non-terminal symbol of the rule.
    pub lhs: SymbolId,
    /// Number of symbols on the right-hand side.
    pub rhs_len: u16,
}

impl Default for ParseTable {
    fn default() -> Self {
        Self {
            action_table: vec![],
            goto_table: vec![],
            symbol_metadata: vec![],
            state_count: 0,
            symbol_count: 0,
            symbol_to_index: BTreeMap::new(),
            index_to_symbol: vec![],
            external_scanner_states: vec![],
            rules: vec![],
            nonterminal_to_index: BTreeMap::new(),
            goto_indexing: GotoIndexing::NonterminalMap,
            eof_symbol: SymbolId(0),
            start_symbol: SymbolId(0),
            grammar: Grammar::new("default".to_string()),
            initial_state: StateId(0),
            token_count: 0,
            external_token_count: 0,
            lex_modes: vec![],
            extras: vec![],
            dynamic_prec_by_rule: vec![],
            rule_assoc_by_rule: vec![],
            alias_sequences: vec![],
            field_names: vec![],
            field_map: BTreeMap::new(),
        }
    }
}

impl ParseTable {
    /// Builder method to auto-detect GOTO indexing
    pub fn with_detected_goto_indexing(mut self) -> Self {
        self.detect_goto_indexing();
        self
    }

    /// Normalize EOF symbol to SymbolId(0) for consistency
    /// This ensures compatibility with various table producers
    pub fn normalize_eof_to_zero(mut self) -> Self {
        // If EOF is already 0, nothing to do
        if self.eof_symbol == SymbolId(0) {
            return self;
        }

        let old_eof = self.eof_symbol;
        // Log the normalization for debugging
        #[cfg(debug_assertions)]
        debug_trace!("Normalizing EOF from {:?} to SymbolId(0)", old_eof);

        // Get the indices for remapping
        let old_idx = self.symbol_to_index.get(&old_eof).copied();
        let zero_idx = self.symbol_to_index.get(&SymbolId(0)).copied();

        // Swap columns in ACTION table if both indices exist
        if let (Some(old_idx), Some(zero_idx)) = (old_idx, zero_idx) {
            for row in &mut self.action_table {
                if old_idx < row.len() && zero_idx < row.len() {
                    row.swap(old_idx, zero_idx);
                }
            }
            // Now: 0 → old_idx, old_eof → (removed)
            self.symbol_to_index.insert(SymbolId(0), old_idx);
            self.symbol_to_index.remove(&old_eof);

            // Update index_to_symbol if it exists
            if old_idx < self.index_to_symbol.len() {
                self.index_to_symbol[old_idx] = SymbolId(0);
            }
        } else if let Some(old_idx) = old_idx {
            // Only old EOF existed: move its column mapping to 0
            self.symbol_to_index.remove(&old_eof);
            self.symbol_to_index.insert(SymbolId(0), old_idx);

            // Update index_to_symbol if it exists
            if old_idx < self.index_to_symbol.len() {
                self.index_to_symbol[old_idx] = SymbolId(0);
            }
        } else {
            // Neither mapped: ensure EOF->0 exists so consumers don't panic
            self.symbol_to_index.insert(SymbolId(0), 0);
        }

        // Update EOF symbol
        self.eof_symbol = SymbolId(0);
        self
    }

    /// Auto-detect the GOTO indexing mode based on table contents
    pub fn detect_goto_indexing(&mut self) {
        // Try to determine if the start symbol has a valid GOTO from state 0
        let start_nt = self.start_symbol;

        // Check if start symbol has entry via nonterminal_to_index
        let col_map = self
            .nonterminal_to_index
            .get(&start_nt)
            .and_then(|&c| self.goto_table.first().and_then(|row| row.get(c)))
            .copied();

        // Check if start symbol has entry via direct symbol ID
        let col_direct = self
            .goto_table
            .first()
            .and_then(|row| row.get(start_nt.0 as usize))
            .copied();

        self.goto_indexing = match (col_map, col_direct) {
            (Some(s), _) if s.0 != 0 => GotoIndexing::NonterminalMap,
            (_, Some(s)) if s.0 != 0 => GotoIndexing::DirectSymbolId,
            // Default to nonterminal map; unit tests will catch a mismatch
            _ => GotoIndexing::NonterminalMap,
        };
    }

    /// Get the terminal boundary (tokens + external tokens)
    #[inline]
    pub fn terminal_boundary(&self) -> usize {
        self.token_count + self.external_token_count
    }

    /// Check if a symbol is a terminal
    #[inline]
    pub fn is_terminal(&self, sym: SymbolId) -> bool {
        (sym.0 as usize) < self.terminal_boundary()
    }

    /// Get valid symbols mask for a state (terminals that have actions)
    pub fn valid_symbols(&self, state: StateId) -> Vec<bool> {
        let n = self.terminal_boundary();
        let mut v = vec![false; n];
        let s = state.0 as usize;
        if s < self.action_table.len() {
            for t in 0..n.min(self.action_table[s].len()) {
                v[t] = !self.action_table[s][t].is_empty();
            }
        }
        v
    }

    /// Get actions for a state and symbol.
    ///
    /// Returns the slice of [`Action`]s for the given `(state, terminal)` pair.
    /// Returns an empty slice when no actions exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use adze_glr_core::{FirstFollowSets, build_lr1_automaton, Action};
    /// use adze_ir::*;
    ///
    /// let mut grammar = Grammar::new("act".into());
    /// let a = SymbolId(1);
    /// let s = SymbolId(10);
    /// grammar.tokens.insert(a, Token { name: "a".into(), pattern: TokenPattern::String("a".into()), fragile: false });
    /// grammar.rule_names.insert(s, "S".into());
    /// grammar.rules.insert(s, vec![
    ///     Rule { lhs: s, rhs: vec![Symbol::Terminal(a)], precedence: None, associativity: None, fields: vec![], production_id: ProductionId(0) },
    /// ]);
    ///
    /// let ff = FirstFollowSets::compute(&grammar).unwrap();
    /// let table = build_lr1_automaton(&grammar, &ff).unwrap();
    ///
    /// // Initial state should have a Shift on terminal 'a'
    /// let actions = table.actions(table.initial_state, a);
    /// assert!(actions.iter().any(|a| matches!(a, Action::Shift(_))));
    /// ```
    #[inline]
    pub fn actions(&self, state: StateId, sym: SymbolId) -> &'_ [Action] {
        let s = state.0 as usize;
        let Some(&col) = self.symbol_to_index.get(&sym) else {
            return &[];
        };
        if s >= self.action_table.len() || col >= self.action_table[s].len() {
            return &[];
        }
        &self.action_table[s][col]
    }

    /// Get goto state for a nonterminal.
    ///
    /// Returns the target state after reducing to `nt` in the given `state`,
    /// or `None` if no transition exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
    /// use adze_ir::*;
    ///
    /// let mut grammar = Grammar::new("goto".into());
    /// let a = SymbolId(1);
    /// let s = SymbolId(10);
    /// grammar.tokens.insert(a, Token { name: "a".into(), pattern: TokenPattern::String("a".into()), fragile: false });
    /// grammar.rule_names.insert(s, "S".into());
    /// grammar.rules.insert(s, vec![
    ///     Rule { lhs: s, rhs: vec![Symbol::Terminal(a)], precedence: None, associativity: None, fields: vec![], production_id: ProductionId(0) },
    /// ]);
    ///
    /// let ff = FirstFollowSets::compute(&grammar).unwrap();
    /// let table = build_lr1_automaton(&grammar, &ff).unwrap();
    ///
    /// // After shifting 'a' and reducing S→a, goto(0, S) should exist
    /// let target = table.goto(table.initial_state, s);
    /// assert!(target.is_some(), "goto(initial, S) should exist");
    /// ```
    #[inline]
    pub fn goto(&self, state: StateId, nt: SymbolId) -> Option<StateId> {
        let s = state.0 as usize;
        let &col = self.nonterminal_to_index.get(&nt)?;
        // Allow "no edge" to be represented as a sentinel (e.g., u16::MAX)
        let ns = *self.goto_table.get(s)?.get(col)?;
        (ns.0 != u16::MAX).then_some(ns)
    }

    /// Get rule information by ID
    #[inline]
    pub fn rule(&self, id: RuleId) -> (SymbolId, u16) {
        let r = &self.rules[id.0 as usize];
        (r.lhs, r.rhs_len)
    }

    /// Get EOF symbol
    #[inline]
    pub fn eof(&self) -> SymbolId {
        self.eof_symbol
    }

    /// Get start symbol
    #[inline]
    pub fn start_symbol(&self) -> SymbolId {
        self.start_symbol
    }

    /// Get grammar reference
    #[inline]
    pub fn grammar(&self) -> &Grammar {
        &self.grammar
    }

    /// Get the ERROR symbol (by convention, symbol 0 or -1 in Tree-sitter)
    #[inline]
    pub fn error_symbol(&self) -> SymbolId {
        // Tree-sitter convention: ERROR is typically symbol 0
        // We could also check for a symbol named "ERROR" in the grammar
        SymbolId(0)
    }

    /// Get valid symbols mask for a state (terminals that have actions)
    #[inline]
    pub fn valid_symbols_mask(&self, state: StateId) -> Vec<bool> {
        let n = self.terminal_boundary();
        let mut v = vec![false; n];
        let s = state.0 as usize;
        if s < self.action_table.len() {
            for t in 0..n.min(self.action_table[s].len()) {
                v[t] = !self.action_table[s][t].is_empty();
            }
        }
        v
    }

    /// Get lex mode for a state
    #[inline]
    pub fn lex_mode(&self, state: StateId) -> LexMode {
        let idx = state.0 as usize;
        if idx < self.lex_modes.len() {
            self.lex_modes[idx]
        } else {
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            }
        }
    }

    /// Check if a symbol is an extra (whitespace/comment)
    #[inline]
    pub fn is_extra(&self, sym: SymbolId) -> bool {
        self.extras.contains(&sym)
    }

    /// Validate parse table invariants
    ///
    /// This method checks critical invariants that must hold for correct parsing:
    /// - EOF symbol is not internal ERROR sentinel
    /// - EOF symbol is a proper sentinel:
    ///   Tree-sitter-style `0` or `>= token_count + external_token_count`
    /// - EOF symbol is present in symbol_to_index mapping
    /// - EOF symbol is present in symbol_to_index mapping
    #[must_use = "validation result must be checked"]
    pub fn validate(&self) -> Result<(), TableError> {
        let terminal_boundary = self.token_count + self.external_token_count;

        debug_assert_ne!(
            self.eof_symbol,
            parse_forest::ERROR_SYMBOL,
            "EOF symbol cannot be the ERROR sentinel"
        );

        if self.eof_symbol == parse_forest::ERROR_SYMBOL {
            return Err(TableError::EofIsError);
        }

        let eof_is_zero_sentinel = self.eof_symbol == SymbolId(0);

        // Check EOF is a terminal sentinel. Tree-sitter-compatible tables use
        // symbol 0 for EOF; other producers may place EOF beyond all terminal
        // and external token symbols.
        if !eof_is_zero_sentinel && (self.eof_symbol.0 as usize) < terminal_boundary {
            return Err(TableError::EofNotSentinel {
                eof: self.eof_symbol.0,
                token_count: self.token_count as u32,
                external_count: self.external_token_count as u32,
            });
        }

        // Check EOF is in symbol_to_index
        if !self.symbol_to_index.contains_key(&self.eof_symbol) {
            return Err(TableError::EofMissingFromIndex);
        }

        // Validate terminal partitions
        let tb = self.terminal_boundary();

        // All extras must be regular terminals
        debug_assert!(
            self.extras
                .iter()
                .all(|&sym| (sym.0 as usize) < self.token_count),
            "all extras must be within [0..token_count)"
        );

        // Regular terminals must not be external
        for sym_id in 0..self.token_count {
            let sym = SymbolId(sym_id as u16);
            debug_assert!(self.is_terminal(sym), "0..token_count are terminals");
            // Regular terminals are not external - we verify this by the band
            debug_assert!(
                (sym.0 as usize) < self.token_count,
                "regular terminals are in [0..token_count)"
            );
        }

        // External tokens must be in their band
        for sym_id in self.token_count..tb {
            let sym = SymbolId(sym_id as u16);
            debug_assert!(self.is_terminal(sym), "external tokens are terminals");
            // External tokens are in the external band by definition
            debug_assert!(
                (sym.0 as usize) >= self.token_count && (sym.0 as usize) < tb,
                "external tokens are in [token_count..terminal_boundary)"
            );
        }

        debug_assert!(
            self.symbol_to_index.contains_key(&self.eof_symbol),
            "EOF must exist in ACTION map"
        );

        Ok(())
    }

    /// Remap GOTO table from NonterminalMap layout to DirectSymbolId layout.
    /// No-op if already DirectSymbolId.
    pub fn remap_goto_to_direct_symbol_id(mut self) -> Self {
        if matches!(self.goto_indexing, GotoIndexing::DirectSymbolId) {
            return self;
        }
        // Establish the max symbol id we need to size rows
        let max_sym = self
            .nonterminal_to_index
            .keys()
            .map(|s| s.0 as usize)
            .max()
            .unwrap_or(0);
        let new_width = max_sym + 1;

        for row in &mut self.goto_table {
            // Defensive check: ensure all column indices are valid
            debug_assert!(
                self.nonterminal_to_index.values().all(|&c| c < row.len()),
                "nonterminal_to_index contains a column >= row width"
            );

            let mut new_row = vec![StateId(0); new_width];
            // Move each mapped nonterminal from its old column into the col = symbol id
            for (sym, &old_col) in &self.nonterminal_to_index {
                if old_col < row.len() {
                    new_row[sym.0 as usize] = row[old_col];
                }
            }
            *row = new_row;
        }
        self.goto_indexing = GotoIndexing::DirectSymbolId;
        self
    }

    /// Remap GOTO table from DirectSymbolId layout to NonterminalMap layout.
    /// No-op if already NonterminalMap.
    pub fn remap_goto_to_nonterminal_map(mut self) -> Self {
        if matches!(self.goto_indexing, GotoIndexing::NonterminalMap) {
            return self;
        }
        // Compute width for the map layout
        let width = self
            .nonterminal_to_index
            .values()
            .copied()
            .max()
            .unwrap_or(0)
            + 1;
        for row in &mut self.goto_table {
            // Defensive check: ensure source indices are valid
            debug_assert!(
                self.nonterminal_to_index
                    .keys()
                    .all(|s| (s.0 as usize) < row.len()),
                "nonterminal_to_index contains a symbol id >= row width"
            );

            let mut new_row = vec![StateId(0); width];
            for (sym, &col) in &self.nonterminal_to_index {
                let src = sym.0 as usize;
                if src < row.len() && col < new_row.len() {
                    new_row[col] = row[src];
                }
            }
            *row = new_row;
        }
        self.goto_indexing = GotoIndexing::NonterminalMap;
        self
    }
}

/// Actions in GLR parse table (supporting multiple actions per state)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub enum Action {
    /// Shift the current token and transition to the given state.
    Shift(StateId),
    /// Reduce by the given grammar rule.
    Reduce(RuleId),
    /// Accept the input (parsing complete).
    Accept,
    /// No valid action (syntax error).
    Error,
    /// Tree-sitter error recovery — insert missing node.
    Recover,
    /// GLR fork point — multiple valid actions to explore.
    Fork(Vec<Action>),
}

/// Action cell that can hold multiple actions for GLR
pub type ActionCell = Vec<Action>;

/// Symbol metadata for the parse table
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialization", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub struct SymbolMetadata {
    /// Human-readable symbol name.
    pub name: String,
    /// Whether the symbol is visible in the syntax tree.
    pub is_visible: bool,
    /// Whether the symbol is a named node (vs anonymous).
    pub is_named: bool,
    /// Whether the symbol is a supertype node.
    pub is_supertype: bool,
    // Additional fields required by API contracts
    /// Whether the symbol is a terminal (leaf token).
    pub is_terminal: bool,
    /// Whether the symbol is an extra (e.g., whitespace, comments).
    pub is_extra: bool,
    /// Whether the symbol is fragile (invalidated by edits).
    pub is_fragile: bool,
    /// Unique identifier for this symbol.
    pub symbol_id: SymbolId,
}
