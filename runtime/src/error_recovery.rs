//! Error recovery strategies for robust parsing.
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

// Comprehensive error recovery strategies for Adze
// This module implements various error recovery techniques to produce useful parse trees
// even when the input contains syntax errors.

use adze_glr_core::ParseTable;
use adze_ir::StateId;
use adze_ir::{Grammar, SymbolId};
use smallvec::SmallVec;
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Error recovery strategies that can be applied during parsing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryStrategy {
    /// Skip tokens until a synchronization point is found
    PanicMode,
    /// Insert a missing token
    TokenInsertion,
    /// Delete an unexpected token
    TokenDeletion,
    /// Replace an unexpected token with an expected one
    TokenSubstitution,
    /// Use phrase-level recovery to skip to next valid construct
    PhraseLevel,
    /// Use scope-based recovery (e.g., balance brackets)
    ScopeRecovery,
    /// Use indentation-based recovery for languages like Python
    #[allow(dead_code)]
    IndentationRecovery,
}

/// Action to take for error recovery
#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryAction {
    /// Insert a token to continue parsing
    InsertToken(adze_ir::SymbolId),
    /// Delete the current token
    DeleteToken,
    /// Replace the current token with another
    #[allow(dead_code)]
    ReplaceToken(adze_ir::SymbolId),
    /// Create an error node containing problematic tokens
    #[allow(dead_code)]
    CreateErrorNode(Vec<adze_ir::SymbolId>),
}

/// Error recovery configuration
#[derive(Debug, Clone)]
pub struct ErrorRecoveryConfig {
    /// Maximum number of tokens to skip during panic mode
    pub max_panic_skip: usize,
    /// Synchronization tokens for panic mode recovery (GLR-aware)
    pub sync_tokens: SmallVec<[SymbolId; 8]>,
    /// Tokens that can be auto-inserted during recovery
    pub insert_candidates: SmallVec<[SymbolId; 8]>,
    /// Tokens that can be deleted during error recovery
    pub deletable_tokens: HashSet<u16>,
    /// Maximum number of tokens to delete in a row
    pub max_token_deletions: usize,
    /// Maximum number of tokens to insert in a row
    pub max_token_insertions: usize,
    /// Maximum number of consecutive errors before giving up
    pub max_consecutive_errors: usize,
    /// Enable phrase-level recovery
    pub enable_phrase_recovery: bool,
    /// Enable scope-based recovery
    pub enable_scope_recovery: bool,
    /// Scope delimiters (open, close) pairs
    pub scope_delimiters: Vec<(u16, u16)>,
    /// Enable indentation-based recovery
    pub enable_indentation_recovery: bool,
}

impl ErrorRecoveryConfig {
    /// Check if a token can be deleted
    pub fn can_delete_token(&self, token: adze_ir::SymbolId) -> bool {
        // Check if token is explicitly marked as deletable, or if it's not a sync token
        self.deletable_tokens.contains(&token.0) || !self.sync_tokens.contains(&token)
    }

    /// Check if a token can be replaced
    pub fn can_replace_token(&self, token: adze_ir::SymbolId) -> bool {
        // Allow replacing if it's not a sync token
        !self.sync_tokens.contains(&token)
    }
}

impl Default for ErrorRecoveryConfig {
    fn default() -> Self {
        Self {
            max_panic_skip: 50,
            sync_tokens: SmallVec::new(),
            insert_candidates: SmallVec::new(),
            deletable_tokens: HashSet::new(),
            max_token_deletions: 3,
            max_token_insertions: 2,
            max_consecutive_errors: 10,
            enable_phrase_recovery: true,
            enable_scope_recovery: true,
            scope_delimiters: Vec::new(),
            enable_indentation_recovery: false,
        }
    }
}

/// Error recovery state during parsing
pub struct ErrorRecoveryState {
    /// Configuration for error recovery
    config: ErrorRecoveryConfig,
    /// Number of consecutive errors encountered (thread-safe atomic counter)
    consecutive_errors: AtomicUsize,
    /// Stack of open scopes for scope-based recovery (thread-safe)
    scope_stack: Arc<Mutex<Vec<u16>>>,
    /// Recent tokens for context-aware recovery (thread-safe)
    recent_tokens: Arc<Mutex<VecDeque<u16>>>,
    /// Indentation levels for indentation-based recovery
    #[allow(dead_code)]
    indentation_stack: Arc<Mutex<Vec<usize>>>,
    /// Error nodes created during recovery (thread-safe)
    error_nodes: Arc<Mutex<Vec<ErrorNode>>>,
}

/// Represents an error node in the parse tree
#[derive(Debug, Clone)]
pub struct ErrorNode {
    /// Start byte position of the error
    pub start_byte: usize,
    /// End byte position of the error
    pub end_byte: usize,
    /// Start position (row, column)
    #[allow(dead_code)]
    pub start_position: (usize, usize),
    /// End position (row, column)
    #[allow(dead_code)]
    pub end_position: (usize, usize),
    /// Expected symbols at this position
    pub expected: Vec<u16>,
    /// Actual symbol encountered
    pub actual: Option<u16>,
    /// Recovery strategy used
    pub recovery: RecoveryStrategy,
    /// Skipped tokens during recovery
    #[allow(dead_code)]
    pub skipped_tokens: Vec<u16>,
}

impl ErrorRecoveryState {
    pub fn new(config: ErrorRecoveryConfig) -> Self {
        Self {
            config,
            consecutive_errors: AtomicUsize::new(0),
            scope_stack: Arc::new(Mutex::new(Vec::new())),
            recent_tokens: Arc::new(Mutex::new(VecDeque::with_capacity(10))),
            indentation_stack: Arc::new(Mutex::new(vec![0])),
            error_nodes: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Record an error and determine recovery strategy
    pub fn determine_recovery_strategy(
        &mut self,
        expected: &[u16],
        actual: Option<u16>,
        _position: (usize, usize),
        _byte_offset: usize,
    ) -> RecoveryStrategy {
        let current_errors = self.consecutive_errors.fetch_add(1, Ordering::SeqCst) + 1;

        // Check if we've hit the error limit
        if current_errors > self.config.max_consecutive_errors {
            return RecoveryStrategy::PanicMode;
        }

        // Try strategies in order of preference

        // 1. Token insertion - if the missing token is insertable
        if (actual.is_none() || self.can_insert_token(expected))
            && let Some(_token) = self.find_insertable_token(expected)
        {
            self.consecutive_errors.store(0, Ordering::SeqCst); // Reset on successful recovery
            return RecoveryStrategy::TokenInsertion;
        }

        // 2. Token deletion - if current token is clearly wrong
        if let Some(token) = actual
            && self.is_clearly_wrong(token, expected)
        {
            return RecoveryStrategy::TokenDeletion;
        }

        // 3. Token substitution - if there's a clear candidate
        if let Some(token) = actual
            && self.can_substitute_token(token, expected)
        {
            return RecoveryStrategy::TokenSubstitution;
        }

        // 4. Scope recovery - if we're in a scope mismatch
        if self.config.enable_scope_recovery && self.has_scope_mismatch(actual) {
            return RecoveryStrategy::ScopeRecovery;
        }

        // 5. Phrase-level recovery - skip to next major construct
        if self.config.enable_phrase_recovery {
            return RecoveryStrategy::PhraseLevel;
        }

        // 6. Default to panic mode
        RecoveryStrategy::PanicMode
    }

    /// Record an error node
    #[expect(
        clippy::too_many_arguments,
        reason = "error state capture requires all fields in a single call; a builder would obscure the recording contract"
    )]
    pub fn record_error(
        &mut self,
        start_byte: usize,
        end_byte: usize,
        start_position: (usize, usize),
        end_position: (usize, usize),
        expected: Vec<u16>,
        actual: Option<u16>,
        recovery: RecoveryStrategy,
        skipped_tokens: Vec<u16>,
    ) {
        if let Ok(mut nodes) = self.error_nodes.lock() {
            nodes.push(ErrorNode {
                start_byte,
                end_byte,
                start_position,
                end_position,
                expected,
                actual,
                recovery,
                skipped_tokens,
            });
        }
    }

    /// Update recent tokens for context-aware recovery
    pub fn add_recent_token(&mut self, token: u16) {
        if let Ok(mut tokens) = self.recent_tokens.lock() {
            if tokens.len() >= 10 {
                tokens.pop_front();
            }
            tokens.push_back(token);
        }
    }

    /// Update scope stack for scope-based recovery
    pub fn push_scope(&mut self, token: u16) {
        if self.is_opening_delimiter(token)
            && let Ok(mut stack) = self.scope_stack.lock()
        {
            stack.push(token);
        }
    }

    /// Update scope stack when closing delimiter is found
    pub fn pop_scope(&mut self, token: u16) -> bool {
        if let Some(expected_open) = self.find_matching_open(token)
            && let Ok(mut stack) = self.scope_stack.lock()
            && stack.last() == Some(&expected_open)
        {
            stack.pop();
            return true;
        }
        false
    }

    /// Get error nodes collected during parsing
    pub fn get_error_nodes(&self) -> Vec<ErrorNode> {
        if let Ok(nodes) = self.error_nodes.lock() {
            nodes.clone()
        } else {
            Vec::new()
        }
    }

    /// Reset consecutive error count (called on successful parse)
    #[allow(dead_code)]
    pub fn reset_consecutive_errors(&mut self) {
        self.consecutive_errors.store(0, Ordering::SeqCst);
    }

    /// Clear all error nodes
    #[allow(dead_code)]
    pub fn clear_errors(&mut self) {
        if let Ok(mut nodes) = self.error_nodes.lock() {
            nodes.clear();
        }
    }

    // Helper methods

    fn can_insert_token(&self, expected: &[u16]) -> bool {
        expected
            .iter()
            .any(|s| self.config.insert_candidates.iter().any(|t| t.0 == *s))
    }

    fn find_insertable_token(&self, expected: &[u16]) -> Option<u16> {
        expected
            .iter()
            .find(|s| self.config.insert_candidates.iter().any(|t| t.0 == **s))
            .copied()
    }

    fn is_clearly_wrong(&self, token: u16, expected: &[u16]) -> bool {
        // Token is clearly wrong if it's not in expected set
        // and it's not a sync token
        !expected.contains(&token) && !self.config.sync_tokens.iter().any(|t| t.0 == token)
    }

    fn can_substitute_token(&self, _actual: u16, expected: &[u16]) -> bool {
        // In a real implementation, check if tokens are similar
        // For now, just check if there's exactly one expected token
        expected.len() == 1
    }

    fn has_scope_mismatch(&self, actual: Option<u16>) -> bool {
        if let Some(token) = actual {
            // Check if it's a closing delimiter without matching open
            self.config.scope_delimiters.iter().any(|(_, close)| {
                token == *close
                    && if let Ok(stack) = self.scope_stack.lock() {
                        !stack.iter().any(|open| {
                            self.config
                                .scope_delimiters
                                .iter()
                                .any(|(o, c)| o == open && c == close)
                        })
                    } else {
                        true // If we can't lock, assume mismatch for safety
                    }
            })
        } else {
            false
        }
    }

    fn is_opening_delimiter(&self, token: u16) -> bool {
        self.config
            .scope_delimiters
            .iter()
            .any(|(open, _)| *open == token)
    }

    fn find_matching_open(&self, close_token: u16) -> Option<u16> {
        self.config
            .scope_delimiters
            .iter()
            .find(|(_, close)| *close == close_token)
            .map(|(open, _)| *open)
    }

    // Test helper methods
    pub fn increment_error_count(&mut self) {
        self.consecutive_errors.fetch_add(1, Ordering::SeqCst);
    }

    pub fn reset_error_count(&mut self) {
        self.consecutive_errors.store(0, Ordering::SeqCst);
    }

    pub fn should_give_up(&self) -> bool {
        self.consecutive_errors.load(Ordering::SeqCst) >= self.config.max_consecutive_errors
    }

    // Legacy pop_scope method for tests
    pub fn pop_scope_test(&mut self) -> Option<u16> {
        if let Ok(mut stack) = self.scope_stack.lock() {
            stack.pop()
        } else {
            None
        }
    }

    pub fn update_recent_tokens(&mut self, token: SymbolId) {
        self.add_recent_token(token.0);
    }

    // Static helper methods for tests
    pub fn is_scope_delimiter(token: u16, delimiters: &[(u16, u16)]) -> bool {
        delimiters
            .iter()
            .any(|(open, close)| *open == token || *close == token)
    }

    pub fn is_matching_delimiter(open: u16, close: u16, delimiters: &[(u16, u16)]) -> bool {
        delimiters.iter().any(|(o, c)| *o == open && *c == close)
    }
}

/// Builder for ErrorRecoveryConfig
pub struct ErrorRecoveryConfigBuilder {
    config: ErrorRecoveryConfig,
}

impl Default for ErrorRecoveryConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl ErrorRecoveryConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: ErrorRecoveryConfig::default(),
        }
    }

    pub fn max_panic_skip(mut self, max: usize) -> Self {
        self.config.max_panic_skip = max;
        self
    }

    pub fn add_sync_token(mut self, token: u16) -> Self {
        self.config.sync_tokens.push(SymbolId(token));
        self
    }

    pub fn add_sync_token_sym(mut self, token: SymbolId) -> Self {
        self.config.sync_tokens.push(token);
        self
    }

    pub fn add_insertable_token(mut self, token: u16) -> Self {
        self.config.insert_candidates.push(SymbolId(token));
        self
    }

    pub fn add_insertable_token_sym(mut self, token: SymbolId) -> Self {
        self.config.insert_candidates.push(token);
        self
    }

    pub fn add_deletable_token(mut self, token: u16) -> Self {
        self.config.deletable_tokens.insert(token);
        self
    }

    pub fn add_scope_delimiter(mut self, open: u16, close: u16) -> Self {
        self.config.scope_delimiters.push((open, close));
        self
    }

    pub fn enable_indentation_recovery(mut self, enable: bool) -> Self {
        self.config.enable_indentation_recovery = enable;
        self
    }

    pub fn enable_scope_recovery(mut self, enable: bool) -> Self {
        self.config.enable_scope_recovery = enable;
        self
    }

    pub fn enable_phrase_recovery(mut self, enable: bool) -> Self {
        self.config.enable_phrase_recovery = enable;
        self
    }

    pub fn max_consecutive_errors(mut self, max: usize) -> Self {
        self.config.max_consecutive_errors = max;
        self
    }

    pub fn set_max_recovery_attempts(mut self, max: usize) -> Self {
        self.config.max_consecutive_errors = max;
        self
    }

    pub fn build(self) -> ErrorRecoveryConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_recovery_config() {
        let config = ErrorRecoveryConfig::default();
        assert_eq!(config.max_panic_skip, 50);
        assert_eq!(config.max_consecutive_errors, 10);
        assert!(config.enable_phrase_recovery);
        assert!(config.enable_scope_recovery);
    }

    #[test]
    fn test_recovery_state_creation() {
        let config = ErrorRecoveryConfig::default();
        let state = ErrorRecoveryState::new(config);
        assert_eq!(state.consecutive_errors.load(Ordering::SeqCst), 0);
        assert!(state.scope_stack.lock().unwrap().is_empty());
        assert_eq!(*state.indentation_stack.lock().unwrap(), vec![0]);
    }

    #[test]
    fn test_config_builder() {
        let config = ErrorRecoveryConfigBuilder::new()
            .max_panic_skip(100)
            .add_sync_token(1)
            .add_sync_token(2)
            .add_insertable_token(3)
            .add_scope_delimiter(4, 5)
            .enable_indentation_recovery(true)
            .build();

        assert_eq!(config.max_panic_skip, 100);
        assert!(config.sync_tokens.iter().any(|t| t.0 == 1));
        assert!(config.sync_tokens.iter().any(|t| t.0 == 2));
        assert!(config.insert_candidates.iter().any(|t| t.0 == 3));
        assert_eq!(config.scope_delimiters, vec![(4, 5)]);
        assert!(config.enable_indentation_recovery);
    }

    #[test]
    fn test_recovery_strategy_selection() {
        let mut config = ErrorRecoveryConfig::default();
        config.insert_candidates.push(SymbolId(10));
        config.sync_tokens.push(SymbolId(20));

        let mut state = ErrorRecoveryState::new(config);

        // Test token insertion strategy
        let strategy = state.determine_recovery_strategy(&[10, 11], None, (0, 0), 0);
        assert_eq!(strategy, RecoveryStrategy::TokenInsertion);

        // Test panic mode after too many errors
        state.consecutive_errors.store(11, Ordering::SeqCst);
        let strategy = state.determine_recovery_strategy(&[10, 11], Some(15), (0, 0), 0);
        assert_eq!(strategy, RecoveryStrategy::PanicMode);
    }

    #[test]
    fn test_scope_tracking() {
        let mut config = ErrorRecoveryConfig::default();
        config.scope_delimiters.push((1, 2)); // ( and )
        config.scope_delimiters.push((3, 4)); // { and }

        let mut state = ErrorRecoveryState::new(config);

        // Push opening delimiters
        state.push_scope(1);
        state.push_scope(3);
        assert_eq!(*state.scope_stack.lock().unwrap(), vec![1, 3]);

        // Pop matching delimiter
        assert!(state.pop_scope(4));
        assert_eq!(*state.scope_stack.lock().unwrap(), vec![1]);

        // Try to pop non-matching delimiter
        assert!(!state.pop_scope(4));
        assert_eq!(*state.scope_stack.lock().unwrap(), vec![1]);

        // Pop correct delimiter
        assert!(state.pop_scope(2));
        assert!(state.scope_stack.lock().unwrap().is_empty());
    }

    #[test]
    fn test_error_recording() {
        let config = ErrorRecoveryConfig::default();
        let mut state = ErrorRecoveryState::new(config);

        state.record_error(
            0,
            5,
            (0, 0),
            (0, 5),
            vec![1, 2, 3],
            Some(4),
            RecoveryStrategy::TokenDeletion,
            vec![4],
        );

        let errors = state.get_error_nodes();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].start_byte, 0);
        assert_eq!(errors[0].end_byte, 5);
        assert_eq!(errors[0].expected, vec![1, 2, 3]);
        assert_eq!(errors[0].actual, Some(4));
        assert_eq!(errors[0].recovery, RecoveryStrategy::TokenDeletion);
    }
}

impl ErrorRecoveryState {
    /// Suggest a recovery action for the current error state
    pub fn suggest_recovery(
        &mut self,
        state: StateId,
        unexpected_token: SymbolId,
        table: &ParseTable,
        _grammar: &Grammar,
    ) -> Option<RecoveryAction> {
        let current_errors = self.consecutive_errors.fetch_add(1, Ordering::SeqCst) + 1;

        // Check if we've hit the error limit
        if current_errors > self.config.max_consecutive_errors {
            return None;
        }

        // Record the token in recent history
        if let Ok(mut tokens) = self.recent_tokens.lock() {
            tokens.push_back(unexpected_token.0);
            if tokens.len() > 10 {
                tokens.pop_front();
            }
        }

        // Find expected tokens in this state. Table lookups are fallible:
        // a state or column missing from a mismatched/corrupt table yields
        // no expected token (or no suggestion) instead of panicking.
        let mut expected_tokens = Vec::new();
        let row = table.action_table.get(state.0 as usize)?;
        for (symbol_id, &symbol_idx) in &table.symbol_to_index {
            let Some(action) = row.get(symbol_idx) else {
                continue;
            };
            if !action.is_empty() {
                expected_tokens.push(*symbol_id);
            }
        }

        // Try different recovery strategies

        // 1. Token insertion - check if any expected token is insertable
        if let Some(insertable) = expected_tokens
            .iter()
            .find(|&&token| self.config.insert_candidates.iter().any(|t| t == &token))
        {
            self.consecutive_errors.store(0, Ordering::SeqCst); // Reset on successful recovery
            return Some(RecoveryAction::InsertToken(*insertable));
        }

        // 2. Token deletion - if this token can be safely deleted
        if self.config.can_delete_token(unexpected_token) {
            return Some(RecoveryAction::DeleteToken);
        }

        // 3. Create error node as fallback
        Some(RecoveryAction::CreateErrorNode(vec![unexpected_token]))
    }
}

#[cfg(test)]
mod tests2 {
    use super::*;

    #[test]
    fn test_recovery_strategy() {
        // Test enum equality
        assert_eq!(RecoveryStrategy::PanicMode, RecoveryStrategy::PanicMode);
        assert_ne!(
            RecoveryStrategy::PanicMode,
            RecoveryStrategy::TokenInsertion
        );
    }

    #[test]
    fn test_recovery_action() {
        let action = RecoveryAction::InsertToken(SymbolId(42));
        match action {
            RecoveryAction::InsertToken(id) => assert_eq!(id, SymbolId(42)),
            _ => panic!("Expected InsertToken"),
        }

        let delete_action = RecoveryAction::DeleteToken;
        assert!(matches!(delete_action, RecoveryAction::DeleteToken));
    }

    #[test]
    fn test_error_recovery_config_default() {
        let config = ErrorRecoveryConfig::default();

        assert_eq!(config.max_panic_skip, 50);
        assert!(config.sync_tokens.is_empty());
        assert!(config.insert_candidates.is_empty());
        assert_eq!(config.max_consecutive_errors, 10);
        assert!(config.enable_phrase_recovery);
        assert!(config.enable_scope_recovery);
        assert!(config.scope_delimiters.is_empty());
        assert!(!config.enable_indentation_recovery);
    }

    #[test]
    fn test_error_recovery_config_can_delete() {
        let mut config = ErrorRecoveryConfig::default();
        config.sync_tokens.push(SymbolId(10));
        config.sync_tokens.push(SymbolId(20));

        // Can delete non-sync tokens
        assert!(config.can_delete_token(SymbolId(5)));
        assert!(config.can_delete_token(SymbolId(15)));

        // Cannot delete sync tokens
        assert!(!config.can_delete_token(SymbolId(10)));
        assert!(!config.can_delete_token(SymbolId(20)));
    }

    #[test]
    fn test_error_recovery_config_can_replace() {
        let mut config = ErrorRecoveryConfig::default();
        config.sync_tokens.push(SymbolId(30));

        // Can replace non-sync tokens
        assert!(config.can_replace_token(SymbolId(25)));

        // Cannot replace sync tokens
        assert!(!config.can_replace_token(SymbolId(30)));
    }

    #[test]
    fn test_error_recovery_state_creation() {
        let config = ErrorRecoveryConfig::default();
        let state = ErrorRecoveryState::new(config.clone());

        assert_eq!(state.consecutive_errors.load(Ordering::SeqCst), 0);
        assert!(state.scope_stack.lock().unwrap().is_empty());
        assert!(state.recent_tokens.lock().unwrap().is_empty());
        assert!(state.error_nodes.lock().unwrap().is_empty());
    }

    #[test]
    fn test_error_recovery_state_increment_errors() {
        let config = ErrorRecoveryConfig::default();
        let mut state = ErrorRecoveryState::new(config);

        assert_eq!(state.consecutive_errors.load(Ordering::SeqCst), 0);
        state.increment_error_count();
        assert_eq!(state.consecutive_errors.load(Ordering::SeqCst), 1);
        state.increment_error_count();
        assert_eq!(state.consecutive_errors.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_error_recovery_state_reset_errors() {
        let config = ErrorRecoveryConfig::default();
        let mut state = ErrorRecoveryState::new(config);

        state.consecutive_errors.store(5, Ordering::SeqCst);
        state.reset_error_count();
        assert_eq!(state.consecutive_errors.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_error_recovery_state_should_give_up() {
        let config = ErrorRecoveryConfig {
            max_consecutive_errors: 3,
            ..Default::default()
        };
        let state = ErrorRecoveryState::new(config);

        assert!(!state.should_give_up());
        state.consecutive_errors.store(2, Ordering::SeqCst);
        assert!(!state.should_give_up());
        state.consecutive_errors.store(3, Ordering::SeqCst);
        assert!(state.should_give_up());
        state.consecutive_errors.store(4, Ordering::SeqCst);
        assert!(state.should_give_up());
    }

    #[test]
    fn test_error_recovery_state_scope_operations() {
        let config = ErrorRecoveryConfig {
            scope_delimiters: vec![(100, 101), (200, 201)],
            ..Default::default()
        };
        let mut state = ErrorRecoveryState::new(config);

        // Push scope
        state.push_scope(100);
        assert_eq!(state.scope_stack.lock().unwrap().len(), 1);
        assert_eq!(state.scope_stack.lock().unwrap()[0], 100);

        // Push another
        state.push_scope(200);
        assert_eq!(state.scope_stack.lock().unwrap().len(), 2);

        // Pop scope
        assert_eq!(state.pop_scope_test(), Some(200));
        assert_eq!(state.scope_stack.lock().unwrap().len(), 1);
        assert_eq!(state.pop_scope_test(), Some(100));
        assert_eq!(state.scope_stack.lock().unwrap().len(), 0);
        assert_eq!(state.pop_scope_test(), None);
    }

    #[test]
    fn test_error_recovery_state_update_recent_tokens() {
        let config = ErrorRecoveryConfig::default();
        let mut state = ErrorRecoveryState::new(config);

        // Add tokens
        state.update_recent_tokens(SymbolId(1));
        assert_eq!(state.recent_tokens.lock().unwrap().len(), 1);

        // Add more tokens
        for i in 2..15 {
            state.update_recent_tokens(SymbolId(i));
        }

        // Should maintain max of 10
        let tokens = state.recent_tokens.lock().unwrap();
        assert_eq!(tokens.len(), 10);
        // First token should be removed
        assert_eq!(tokens[0], 5);
        assert_eq!(tokens[9], 14);
    }

    #[test]
    fn test_recovery_heuristics() {
        // Test scope delimiter matching
        let delimiters = vec![(1, 2), (3, 4), (5, 6)];
        assert!(ErrorRecoveryState::is_scope_delimiter(1, &delimiters));
        assert!(ErrorRecoveryState::is_scope_delimiter(3, &delimiters));
        assert!(!ErrorRecoveryState::is_scope_delimiter(7, &delimiters));

        assert!(ErrorRecoveryState::is_matching_delimiter(1, 2, &delimiters));
        assert!(ErrorRecoveryState::is_matching_delimiter(5, 6, &delimiters));
        assert!(!ErrorRecoveryState::is_matching_delimiter(
            1,
            4,
            &delimiters
        ));
    }

    #[test]
    fn test_suggest_recovery_missing_state_row_returns_none() {
        // Mismatched table: symbol map is non-empty but the action table has
        // no row for the requested state. Must return None, not panic.
        let mut table = ParseTable::default();
        table.symbol_to_index.insert(SymbolId(1), 0);
        let grammar = Grammar::new("test".to_string());
        let mut state = ErrorRecoveryState::new(ErrorRecoveryConfig::default());

        let result = state.suggest_recovery(StateId(0), SymbolId(99), &table, &grammar);
        assert_eq!(result, None);
    }

    #[test]
    fn test_suggest_recovery_stale_column_index_is_skipped() {
        // Corrupt table: column index in the symbol map exceeds the row width.
        // The stale entry is skipped instead of panicking.
        let mut table = ParseTable::default();
        table.action_table = vec![vec![]];
        table.symbol_to_index.insert(SymbolId(1), 3);
        let grammar = Grammar::new("test".to_string());
        let mut state = ErrorRecoveryState::new(ErrorRecoveryConfig::default());

        let result = state.suggest_recovery(StateId(0), SymbolId(99), &table, &grammar);
        assert_eq!(result, Some(RecoveryAction::DeleteToken));
    }
}
