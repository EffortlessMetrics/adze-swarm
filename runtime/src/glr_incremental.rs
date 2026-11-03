//! GLR-Aware Incremental Parsing
//!
//! This module provides TRUE incremental parsing capabilities for GLR parsers,
//! preserving ambiguities and efficiently handling edits to the input.
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]
//! ## Key Concepts
//!
//! ### Subtree Reuse
//! - Parse trees from unaffected regions are directly reused
//! - Only the changed region and its ancestors are reparsed
//! - Token streams are spliced to avoid re-tokenization
//!
//! ### Fork Tracking
//! - Each parse tree node remembers which fork(s) it belongs to
//! - When edits occur, we track which forks are affected
//! - Unaffected forks can reuse their subtrees entirely
//!
//! ### Ambiguity Preservation
//! - Multiple parse trees are maintained for ambiguous regions
//! - Edits may resolve or introduce new ambiguities
//! - The incremental parser preserves all valid interpretations

use crate::glr_parser::GLRParser;
use crate::subtree::Subtree;
use adze_glr_core::ParseTable;
use adze_ir::{Grammar, RuleId, SymbolId};
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(not(debug_assertions))]
macro_rules! debug_trace {
    ($($arg:tt)*) => {};
}

#[cfg(debug_assertions)]
macro_rules! debug_trace {
    ($($arg:tt)*) => {
        if std::env::var("RUST_LOG")
            .ok()
            .unwrap_or_default()
            .contains("debug")
        {
            eprintln!($($arg)*);
        }
    };
}

/// Simple edit descriptor for byte-based edits
#[derive(Debug, Clone)]
pub struct Edit {
    pub start_byte: usize,
    pub old_end_byte: usize,
    pub new_end_byte: usize,
}

impl Edit {
    pub fn new(start_byte: usize, old_end_byte: usize, new_end_byte: usize) -> Self {
        Edit {
            start_byte,
            old_end_byte,
            new_end_byte,
        }
    }

    /// Convenience helper for byte-range edits (start..old_end replaced by start..new_end).
    /// Useful in benches/tests so they don't need to know additional internal fields.
    pub fn bytes(start: usize, old_end: usize, new_end: usize) -> Self {
        Self {
            start_byte: start,
            old_end_byte: old_end,
            new_end_byte: new_end,
        }
    }
}

/// Global counter for tracking subtree reuses (for testing)
pub static SUBTREE_REUSE_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Reset the reuse counter (for testing)
pub fn reset_reuse_counter() {
    SUBTREE_REUSE_COUNT.store(0, Ordering::SeqCst);
}

/// Get the current reuse count (for testing)
pub fn get_reuse_count() -> usize {
    SUBTREE_REUSE_COUNT.load(Ordering::SeqCst)
}

/// Test-visible status for the most recent incremental parse attempt.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IncrementalParseStatus {
    /// True when incremental parsing explicitly fell back to a full reparse.
    pub full_reparse_fallback: bool,
    /// Number of forest nodes reused from the previous parse.
    pub reused_node_count: usize,
    /// Invalidated byte ranges derived from incoming edits.
    pub invalidated_ranges: Vec<Range<usize>>,
    /// Optional machine-readable reason for fallback.
    pub fallback_reason: Option<&'static str>,
}

/// Helper function to tokenize source code for arithmetic grammar
#[allow(dead_code)]
fn tokenize_source(source: &[u8], _grammar: &Grammar) -> Vec<GLRToken> {
    // Basic tokenization for arithmetic expressions
    let mut tokens = Vec::new();
    let mut position = 0;

    while position < source.len() {
        // Skip whitespace
        while position < source.len() && source[position].is_ascii_whitespace() {
            position += 1;
        }

        if position >= source.len() {
            break;
        }

        let start = position;

        // Number
        if source[position].is_ascii_digit() {
            while position < source.len() && source[position].is_ascii_digit() {
                position += 1;
            }
            tokens.push(GLRToken {
                symbol: SymbolId(1), // number
                text: source[start..position].to_vec(),
                start_byte: start,
                end_byte: position,
            });
        }
        // Plus
        else if source[position] == b'+' {
            position += 1;
            tokens.push(GLRToken {
                symbol: SymbolId(2), // plus
                text: vec![b'+'],
                start_byte: start,
                end_byte: position,
            });
        }
        // Mult
        else if source[position] == b'*' {
            position += 1;
            tokens.push(GLRToken {
                symbol: SymbolId(3), // mult
                text: vec![b'*'],
                start_byte: start,
                end_byte: position,
            });
        }
        // Minus
        else if source[position] == b'-' {
            position += 1;
            tokens.push(GLRToken {
                symbol: SymbolId(2), // treating as plus for simplicity
                text: vec![b'-'],
                start_byte: start,
                end_byte: position,
            });
        }
        // Left paren
        else if source[position] == b'(' {
            position += 1;
            tokens.push(GLRToken {
                symbol: SymbolId(4), // lparen
                text: vec![b'('],
                start_byte: start,
                end_byte: position,
            });
        }
        // Right paren
        else if source[position] == b')' {
            position += 1;
            tokens.push(GLRToken {
                symbol: SymbolId(5), // rparen
                text: vec![b')'],
                start_byte: start,
                end_byte: position,
            });
        }
        // Unknown - skip
        else {
            position += 1;
        }
    }

    tokens
}

/// Experimental reparse hook for the `parser_v4` tree substrate.
///
/// This helper is not selected by generated parser dispatch. It currently
/// returns `None` rather than performing incremental reuse.
pub fn reparse<'arena>(
    #[cfg_attr(not(feature = "incremental_glr"), allow(unused_variables))] grammar: &Grammar,
    #[cfg_attr(not(feature = "incremental_glr"), allow(unused_variables))] table: &ParseTable,
    #[cfg_attr(not(feature = "incremental_glr"), allow(unused_variables))] source: &[u8],
    #[cfg_attr(not(feature = "incremental_glr"), allow(unused_variables))]
    old_tree: &crate::parser_v4::Tree<'arena>,
    #[cfg_attr(not(feature = "incremental_glr"), allow(unused_variables))]
    edit: &crate::pure_incremental::Edit,
) -> Option<crate::parser_v4::Tree<'arena>> {
    let _ = (grammar, table, source, old_tree, edit);

    // Only enable incremental parsing if the feature is enabled
    #[cfg(feature = "incremental_glr")]
    {
        None
    }

    #[cfg(not(feature = "incremental_glr"))]
    {
        // Feature not enabled, return None to trigger fresh parse
        None
    }
}

#[derive(Debug, Clone)]
pub struct GLREdit {
    /// Byte range in the old input that was replaced
    pub old_range: Range<usize>,
    /// New text that replaces the old range
    pub new_text: Vec<u8>,
    /// Token range affected by the edit in OLD token stream
    pub old_token_range: Range<usize>,
    /// New tokens that replace the old token range
    pub new_tokens: Vec<GLRToken>,
    /// Complete old token stream (for finding reusable regions)
    pub old_tokens: Vec<GLRToken>,
    /// Old forest for subtree reuse
    pub old_forest: Option<Arc<ForestNode>>,
}

/// A token with position information
#[derive(Debug, Clone)]
pub struct GLRToken {
    pub symbol: SymbolId,
    pub text: Vec<u8>,
    pub start_byte: usize,
    pub end_byte: usize,
}

/// A parse forest node that tracks multiple interpretations
#[derive(Debug, Clone)]
pub struct ForestNode {
    /// The symbol at this node
    pub symbol: SymbolId,
    /// Alternative parse trees for this node (one per fork)
    pub alternatives: Vec<ForkAlternative>,
    /// Byte range in the input
    pub byte_range: Range<usize>,
    /// Token range in the input
    pub token_range: Range<usize>,
    /// Cached subtree (if this node can be reused)
    pub cached_subtree: Option<Arc<Subtree>>,
}

impl ForestNode {
    /// Check if this node's byte range overlaps with an edit
    pub fn overlaps_edit(&self, edit_range: &Range<usize>) -> bool {
        self.byte_range.start < edit_range.end && self.byte_range.end > edit_range.start
    }

    /// Check if this node's byte range overlaps with the given range
    pub fn overlaps(&self, other: &Range<usize>) -> bool {
        self.byte_range.start < other.end && self.byte_range.end > other.start
    }

    /// Find reusable subtrees that don't overlap the edit
    /// GLR-compatible implementation that collects valid subtrees for post-parse reuse
    pub fn find_reusable_subtrees(&self, edit_range: &Range<usize>) -> Vec<Arc<ForestNode>> {
        let mut reusable = Vec::new();

        // Only collect subtrees that are completely outside the edit range
        // This conservative approach ensures GLR forking compatibility
        for alternative in &self.alternatives {
            if !self.overlaps_edit(edit_range) {
                // Check all children in this alternative
                for child in &alternative.children {
                    if !child.overlaps(edit_range) {
                        // This child is completely unaffected by the edit
                        reusable.push(child.clone());
                    }
                }
            }
        }

        reusable
    }
}

/// An alternative parse for a forest node
#[derive(Debug, Clone)]
pub struct ForkAlternative {
    /// The fork ID this alternative belongs to
    pub fork_id: usize,
    /// The rule used (if any)
    pub rule_id: Option<RuleId>,
    /// Children for this interpretation
    pub children: Vec<Arc<ForestNode>>,
    /// The subtree for this alternative
    pub subtree: Arc<Subtree>,
}

/// Identifies reusable chunks of the parse forest based on token-level diffs
/// This replaces the old ReuseMap with a simpler chunk-based approach
#[derive(Debug)]
pub struct ChunkIdentifier {
    /// The previous forest for potential reuse
    #[allow(dead_code)]
    previous_forest: Option<Arc<ForestNode>>,
    /// Byte range of the edit
    edit_range: Range<usize>,
}

impl ChunkIdentifier {
    pub fn new(previous_forest: Option<Arc<ForestNode>>, edit: &GLREdit) -> Self {
        let edit_range = edit.old_range.clone();
        Self {
            previous_forest,
            edit_range,
        }
    }

    /// Find the largest unchanged prefix tokens before the edit
    pub fn find_prefix_boundary(&self, old_tokens: &[GLRToken], new_tokens: &[GLRToken]) -> usize {
        let mut prefix_len = 0;
        for (old_tok, new_tok) in old_tokens.iter().zip(new_tokens.iter()) {
            // Stop at the first token that overlaps or comes after the edit
            if old_tok.end_byte > self.edit_range.start {
                break;
            }
            // Tokens must match exactly
            if old_tok.symbol != new_tok.symbol || old_tok.text != new_tok.text {
                break;
            }
            prefix_len += 1;
        }
        prefix_len
    }

    /// Find the largest unchanged suffix tokens after the edit  
    pub fn find_suffix_boundary(
        &self,
        old_tokens: &[GLRToken],
        new_tokens: &[GLRToken],
        edit_delta: isize,
    ) -> usize {
        let mut suffix_len = 0;
        let old_iter = old_tokens.iter().rev();
        let new_iter = new_tokens.iter().rev();

        for (old_tok, new_tok) in old_iter.zip(new_iter) {
            // Stop at the first token that overlaps or comes before the edit
            if old_tok.start_byte < self.edit_range.end {
                break;
            }
            // Account for byte position shifts due to the edit
            let adjusted_new_start = (new_tok.start_byte as isize - edit_delta) as usize;
            if old_tok.start_byte != adjusted_new_start {
                break;
            }
            // Tokens must match exactly
            if old_tok.symbol != new_tok.symbol || old_tok.text != new_tok.text {
                break;
            }
            suffix_len += 1;
        }
        suffix_len
    }
}

// ARCHITECTURE NOTE: GSS snapshot-based recovery has been removed.
// The old approach had fundamental performance issues (3-4x slower than full reparse).
// We now use direct forest splicing: parse only the edited middle segment and
// directly splice the new subtree with preserved prefix/suffix nodes.

/// GLR-aware incremental parser
pub struct IncrementalGLRParser {
    /// The underlying GLR parser
    #[allow(dead_code)]
    parser: GLRParser,
    /// Grammar for the language
    grammar: Grammar,
    /// Parse table
    table: ParseTable,
    /// Current parse forest
    forest: Option<Arc<ForestNode>>,
    /// Previous parse forest (for incremental parsing)
    previous_forest: Option<Arc<ForestNode>>,
    /// Fork tracking information
    fork_tracker: ForkTracker,
    /// Length of unchanged prefix tokens (for chunk-based reuse)
    chunk_prefix_len: usize,
    /// Length of unchanged suffix tokens (for chunk-based reuse)
    chunk_suffix_len: usize,
    /// Current tokens being parsed (for forest reuse calculations)
    tokens: Vec<GLRToken>,
    /// Edit byte delta (new_text.len() - old_text.len())
    edit_byte_delta: isize,
    /// Status of the most recent parse attempt.
    last_parse_status: IncrementalParseStatus,
}

/// Tracks fork relationships and dependencies
#[derive(Debug)]
struct ForkTracker {
    /// Maps fork IDs to their parent forks
    fork_parents: HashMap<usize, usize>,
    /// Maps fork IDs to their merge points
    #[allow(dead_code)]
    fork_merges: HashMap<usize, Vec<usize>>,
    /// Active fork IDs
    active_forks: HashSet<usize>,
    /// Next fork ID to assign
    next_fork_id: usize,
}

impl ForkTracker {
    pub(crate) fn new() -> Self {
        Self {
            fork_parents: HashMap::new(),
            fork_merges: HashMap::new(),
            active_forks: HashSet::new(),
            next_fork_id: 0,
        }
    }

    /// Create a new fork from a parent
    pub(crate) fn create_fork(&mut self, parent: Option<usize>) -> usize {
        let fork_id = self.next_fork_id;
        self.next_fork_id += 1;

        if let Some(parent_id) = parent {
            self.fork_parents.insert(fork_id, parent_id);
        }

        self.active_forks.insert(fork_id);
        fork_id
    }

    /// Record a fork merge
    #[allow(dead_code)]
    pub(crate) fn merge_forks(&mut self, fork1: usize, fork2: usize, merge_point: usize) {
        self.fork_merges.entry(fork1).or_default().push(merge_point);
        self.fork_merges.entry(fork2).or_default().push(merge_point);
    }

    /// Get all forks affected by an edit
    #[allow(dead_code)]
    pub(crate) fn get_affected_forks(&self, _edit: &GLREdit) -> HashSet<usize> {
        // For now, conservatively mark all active forks as potentially affected
        self.active_forks.clone()
    }
}

impl IncrementalGLRParser {
    /// Create a new incremental GLR parser
    pub fn new(grammar: Grammar, table: ParseTable) -> Self {
        let parser = GLRParser::new(table.clone(), grammar.clone());

        Self {
            parser,
            grammar,
            table,
            forest: None,
            previous_forest: None,
            fork_tracker: ForkTracker::new(),
            chunk_prefix_len: 0,
            chunk_suffix_len: 0,
            tokens: vec![],
            edit_byte_delta: 0,
            last_parse_status: IncrementalParseStatus::default(),
        }
    }

    /// Create a new parser with an existing forest (for incremental parsing)
    pub fn new_with_forest(
        grammar: Grammar,
        table: ParseTable,
        previous_forest: Option<Arc<ForestNode>>,
    ) -> Self {
        let parser = GLRParser::new(table.clone(), grammar.clone());

        Self {
            parser,
            grammar,
            table,
            forest: None,
            previous_forest,
            fork_tracker: ForkTracker::new(),
            chunk_prefix_len: 0,
            chunk_suffix_len: 0,
            tokens: vec![],
            edit_byte_delta: 0,
            last_parse_status: IncrementalParseStatus::default(),
        }
    }

    /// Return status for the most recent incremental parse attempt.
    pub fn last_parse_status(&self) -> &IncrementalParseStatus {
        &self.last_parse_status
    }

    /// Parse with incremental reuse
    pub fn parse_incremental(
        &mut self,
        tokens: &[GLRToken],
        edits: &[GLREdit],
    ) -> Result<Arc<ForestNode>, String> {
        self.last_parse_status = IncrementalParseStatus {
            full_reparse_fallback: false,
            reused_node_count: 0,
            invalidated_ranges: edits.iter().map(|edit| edit.old_range.clone()).collect(),
            fallback_reason: None,
        };

        // If we have edits and a previous parse, try to reuse
        if !edits.is_empty() {
            // Check if we have an old forest to reuse from
            let has_old_forest =
                edits.iter().any(|e| e.old_forest.is_some()) || self.previous_forest.is_some();

            if has_old_forest {
                self.reparse_with_edits(tokens, edits)
            } else {
                // No previous parse, do fresh parse
                self.last_parse_status.full_reparse_fallback = true;
                self.last_parse_status.fallback_reason = Some("missing_old_forest");
                self.parse_fresh(tokens)
            }
        } else {
            // No edits, fresh parse
            self.last_parse_status.fallback_reason = Some("fresh_parse_no_edits");
            self.parse_fresh(tokens)
        }
    }

    /// Parse from scratch
    fn parse_fresh(&mut self, tokens: &[GLRToken]) -> Result<Arc<ForestNode>, String> {
        // Reset state
        self.fork_tracker = ForkTracker::new();
        // GSS snapshots removed - using direct forest splicing instead

        // Create initial fork
        let initial_fork = self.fork_tracker.create_fork(None);

        // Parse using the GLR parser
        let mut parser = GLRParser::new(self.table.clone(), self.grammar.clone());

        // Parse all tokens
        for token in tokens.iter() {
            // Convert token text to string, properly handling UTF-8
            let text = std::str::from_utf8(&token.text).map_err(|e| {
                format!("Invalid UTF-8 in token at byte {}: {}", token.start_byte, e)
            })?;
            parser.process_token(token.symbol, text, token.start_byte);
        }

        // Calculate total input length from tokens
        let total_bytes = tokens.last().map(|t| t.end_byte).unwrap_or(0);
        parser.process_eof(total_bytes);

        match parser.finish_all_alternatives() {
            Ok(trees) => {
                if trees.is_empty() {
                    return Err("Parse failed to produce any parse alternatives".to_string());
                }
                // Create a forest node with all parse alternatives
                let forest = if trees.len() == 1 {
                    // Single parse tree - no ambiguity
                    self.build_forest_from_subtree(trees[0].clone(), initial_fork, tokens)
                } else {
                    // Multiple parse trees - ambiguous grammar!
                    let mut alternatives = Vec::new();
                    for tree in trees.iter() {
                        let fork_id = self.fork_tracker.create_fork(Some(initial_fork));
                        let forest = self.subtree_to_forest_recursive(tree.clone(), fork_id);
                        alternatives.push(ForkAlternative {
                            fork_id,
                            rule_id: None,
                            children: vec![forest.clone()],
                            subtree: tree.clone(),
                        });
                    }

                    // Create a root forest node with all alternatives

                    Arc::new(ForestNode {
                        symbol: trees[0].node.symbol_id,
                        alternatives,
                        byte_range: 0..tokens.last().map(|t| t.end_byte).unwrap_or(0),
                        token_range: 0..tokens.len(),
                        cached_subtree: None,
                    })
                };

                self.forest = Some(forest.clone());
                self.previous_forest = Some(forest.clone());
                Ok(forest)
            }
            Err(e) => Err(format!("Parse error: {}", e)),
        }
    }

    /// Reparse with edits using chunk-based incremental strategy
    fn reparse_with_edits(
        &mut self,
        tokens: &[GLRToken],
        edits: &[GLREdit],
    ) -> Result<Arc<ForestNode>, String> {
        // DIRECT FOREST SPLICING STRATEGY:
        // Instead of GSS snapshots, we parse only the edited middle segment
        // and directly splice it with preserved prefix/suffix forest nodes.
        // This avoids the 3-4x performance penalty of GSS restoration.

        // Get the old forest and old tokens from the first edit
        let old_forest = edits
            .iter()
            .find_map(|e| e.old_forest.as_ref())
            .cloned()
            .or_else(|| self.previous_forest.clone());

        let old_tokens = edits
            .iter()
            .find_map(|e| {
                if e.old_tokens.is_empty() {
                    None
                } else {
                    Some(e.old_tokens.clone())
                }
            })
            .unwrap_or_default();

        if let Some(old_forest) = old_forest.clone() {
            // Create ChunkIdentifier to find reusable chunks
            let chunk_id = ChunkIdentifier::new(Some(old_forest.clone()), &edits[0]);

            // Find the prefix and suffix boundaries
            let prefix_len = chunk_id.find_prefix_boundary(&old_tokens, tokens);
            let edit_delta =
                (edits[0].new_text.len() as isize) - (edits[0].old_range.len() as isize);
            let suffix_len = chunk_id.find_suffix_boundary(&old_tokens, tokens, edit_delta);

            // Debug chunk boundaries for troubleshooting
            debug_trace!(
                "DEBUG: Chunk boundaries - prefix_len: {}, suffix_len: {}, total_tokens: {}",
                prefix_len,
                suffix_len,
                tokens.len()
            );
            debug_trace!(
                "DEBUG: Old tokens: {:?}",
                old_tokens
                    .iter()
                    .map(|t| String::from_utf8_lossy(&t.text))
                    .collect::<Vec<_>>()
            );
            debug_trace!(
                "DEBUG: New tokens: {:?}",
                tokens
                    .iter()
                    .map(|t| String::from_utf8_lossy(&t.text))
                    .collect::<Vec<_>>()
            );

            // Determine if we should use forest splicing or fall back to full parse
            // Forest splicing is beneficial when we have significant unchanged regions
            let middle_len = tokens
                .len()
                .saturating_sub(prefix_len)
                .saturating_sub(suffix_len);
            let should_splice = prefix_len > 0 || suffix_len > 0;

            // CRITICAL FIX: For potentially ambiguous grammars, be very conservative about incremental parsing.
            // Two cases to handle:
            // 1. Old forest has ambiguity - we need to preserve it
            // 2. Old forest is unambiguous but edit might introduce ambiguity
            //
            // The problem: If we only parse a small/medium segment, we lose the broader context
            // that creates ambiguity. Examples:
            // - "1-2-3" -> "1-5-3": ambiguity from entire expression structure
            // - "if a then other" -> "if a then if b then c else d": edit introduces ambiguity

            let had_ambiguity = old_forest.alternatives.len() > 1;
            let might_introduce_ambiguity = middle_len > tokens.len() / 3; // Large edit
            let middle_is_small = middle_len < tokens.len() / 2; // Less than half the tokens

            if (had_ambiguity && middle_is_small) || might_introduce_ambiguity {
                // Debug output for troubleshooting
                debug_trace!(
                    "DEBUG: Potentially ambiguous input detected - falling back to full parse"
                );
                debug_trace!(
                    "DEBUG: Old alternatives: {}, Middle len: {}, Total len: {}, Might introduce ambiguity: {}",
                    old_forest.alternatives.len(),
                    middle_len,
                    tokens.len(),
                    might_introduce_ambiguity
                );
                // Fall back to full parsing to ensure ambiguity is correctly handled
                self.last_parse_status.full_reparse_fallback = true;
                self.last_parse_status.fallback_reason = Some("ambiguity_safety_fallback");
                return self.parse_fresh(tokens);
            }

            if should_splice && middle_len < tokens.len() {
                // STEP 1: Parse ONLY the middle segment
                let middle_start = prefix_len;
                let middle_end = tokens.len() - suffix_len;
                let middle_tokens = &tokens[middle_start..middle_end];

                // For empty middle segment, create a placeholder or reuse from old forest
                let middle_forest = if middle_tokens.is_empty() {
                    // Extract the middle portion from the old forest that's between prefix and suffix
                    // For an empty edit, this represents the exact same content as before
                    if prefix_len == tokens.len() {
                        // All tokens are in prefix, return the old forest directly
                        old_forest.clone()
                    } else if suffix_len == tokens.len() {
                        // All tokens are in suffix, return the old forest directly
                        old_forest.clone()
                    } else {
                        // Create an empty forest node
                        let byte_pos = tokens
                            .get(middle_start)
                            .map(|t| t.start_byte)
                            .or_else(|| tokens.last().map(|t| t.end_byte))
                            .unwrap_or(0);
                        Arc::new(ForestNode {
                            symbol: self.grammar.start_symbol().unwrap_or(SymbolId(0)),
                            alternatives: vec![],
                            byte_range: byte_pos..byte_pos,
                            token_range: middle_start..middle_end,
                            cached_subtree: None,
                        })
                    }
                } else {
                    // Parse the middle segment
                    let mut middle_parser =
                        GLRParser::new(self.table.clone(), self.grammar.clone());

                    for token in middle_tokens {
                        middle_parser.process_token(
                            token.symbol,
                            std::str::from_utf8(&token.text).unwrap_or(""),
                            token.start_byte,
                        );
                    }

                    // Process EOF for the middle segment
                    let middle_end_byte = middle_tokens.last().map(|t| t.end_byte).unwrap_or(0);
                    middle_parser.process_eof(middle_end_byte);

                    // Get the parse result for the middle
                    match middle_parser.finish_all_alternatives() {
                        Ok(trees) if !trees.is_empty() => {
                            // Debug output for troubleshooting
                            debug_trace!(
                                "DEBUG: Middle segment parse produced {} alternatives",
                                trees.len()
                            );
                            if trees.len() == 1 {
                                // Single alternative - create fork and build forest
                                let fork_id = self.fork_tracker.create_fork(None);
                                self.build_forest_from_subtree(
                                    trees[0].clone(),
                                    fork_id,
                                    middle_tokens,
                                )
                            } else {
                                // Handle ambiguity in the middle segment
                                let mut alternatives = Vec::new();
                                for tree in trees.iter() {
                                    let fork_id = self.fork_tracker.create_fork(None);
                                    let forest =
                                        self.subtree_to_forest_recursive(tree.clone(), fork_id);
                                    alternatives.push(ForkAlternative {
                                        fork_id,
                                        rule_id: None,
                                        children: vec![forest.clone()],
                                        subtree: tree.clone(),
                                    });
                                }
                                let middle_forest = Arc::new(ForestNode {
                                    symbol: trees[0].node.symbol_id,
                                    alternatives,
                                    byte_range: middle_tokens
                                        .first()
                                        .map(|t| t.start_byte)
                                        .unwrap_or(0)
                                        ..middle_tokens.last().map(|t| t.end_byte).unwrap_or(0),
                                    token_range: middle_start..middle_end,
                                    cached_subtree: None,
                                });
                                // Debug output
                                debug_trace!(
                                    "DEBUG: Created middle forest with {} alternatives",
                                    middle_forest.alternatives.len()
                                );
                                middle_forest
                            }
                        }
                        _ => {
                            // Middle segment failed to parse - fall back to full parse
                            self.last_parse_status.full_reparse_fallback = true;
                            self.last_parse_status.fallback_reason =
                                Some("middle_segment_parse_failed");
                            return self.parse_fresh(tokens);
                        }
                    }
                };

                // STEP 2: Extract prefix and suffix nodes from old forest
                let (prefix_nodes, suffix_nodes) = self.extract_reusable_nodes(
                    &old_forest,
                    prefix_len,
                    suffix_len,
                    &old_tokens,
                    edit_delta,
                );

                // STEP 3: Splice the forests together
                // Debug output
                debug_trace!(
                    "DEBUG: About to splice - prefix: {}, suffix: {}, middle alternatives: {}",
                    prefix_nodes.len(),
                    suffix_nodes.len(),
                    middle_forest.alternatives.len()
                );
                let spliced_forest = self.splice_forests(
                    prefix_nodes.clone(),
                    middle_forest,
                    suffix_nodes.clone(),
                    tokens,
                );
                // Debug output
                debug_trace!(
                    "DEBUG: After splice - result alternatives: {}",
                    spliced_forest.alternatives.len()
                );

                // Update reuse counter with actual node count, not token count
                let reuse_count = prefix_nodes.len() + suffix_nodes.len();
                if reuse_count > 0 {
                    SUBTREE_REUSE_COUNT.fetch_add(reuse_count, Ordering::SeqCst);
                }
                self.last_parse_status.reused_node_count = reuse_count;

                self.forest = Some(spliced_forest.clone());
                self.previous_forest = Some(spliced_forest.clone());
                Ok(spliced_forest)
            } else {
                // No benefit from splicing - do a full parse
                self.last_parse_status.full_reparse_fallback = true;
                self.last_parse_status.fallback_reason = Some("no_splice_benefit");
                self.parse_fresh(tokens)
            }
        } else {
            // No old forest, do fresh parse
            self.last_parse_status.full_reparse_fallback = true;
            self.last_parse_status.fallback_reason = Some("missing_old_forest");
            self.parse_fresh(tokens)
        }
    }

    // Create a parser initialized from a GSS snapshot
    // GSS snapshot methods removed - replaced with direct forest splicing

    /// Extract reusable prefix and suffix nodes from the old forest
    fn extract_reusable_nodes(
        &self,
        old_forest: &Arc<ForestNode>,
        prefix_len: usize,
        suffix_len: usize,
        old_tokens: &[GLRToken],
        edit_delta: isize,
    ) -> (Vec<Arc<ForestNode>>, Vec<Arc<ForestNode>>) {
        // Traverse the old forest to find actual nodes that are fully contained
        // within the unchanged prefix and suffix regions

        let mut prefix_nodes = Vec::new();
        let mut suffix_nodes = Vec::new();

        // Helper function to recursively extract nodes within token ranges
        // We want to extract the deepest possible nodes that are fully contained
        fn collect_nodes_in_range(
            node: &Arc<ForestNode>,
            target_range: std::ops::Range<usize>,
            byte_offset: isize,
            collected: &mut Vec<Arc<ForestNode>>,
            depth: usize,
        ) {
            // Check if this node is fully within the target range
            if node.token_range.start >= target_range.start
                && node.token_range.end <= target_range.end
            {
                // This node is fully contained
                // Try to go deeper first to get smaller nodes
                let mut found_children = false;

                // Look at first alternative only to avoid duplicates
                if let Some(first_alt) = node.alternatives.first() {
                    for child in &first_alt.children {
                        // Check if this child is also within range
                        if child.token_range.start >= target_range.start
                            && child.token_range.end <= target_range.end
                        {
                            collect_nodes_in_range(
                                child,
                                target_range.clone(),
                                byte_offset,
                                collected,
                                depth + 1,
                            );
                            found_children = true;
                        }
                    }
                }

                // If we didn't find any children to extract, extract this node
                if !found_children {
                    let mut cloned = (**node).clone();
                    if byte_offset != 0 {
                        cloned.byte_range = ((cloned.byte_range.start as isize + byte_offset)
                            as usize)
                            ..((cloned.byte_range.end as isize + byte_offset) as usize);
                    }
                    collected.push(Arc::new(cloned));
                }
            } else if node.token_range.start < target_range.end
                && node.token_range.end > target_range.start
            {
                // This node spans the boundary - look at children
                if let Some(first_alt) = node.alternatives.first() {
                    for child in &first_alt.children {
                        collect_nodes_in_range(
                            child,
                            target_range.clone(),
                            byte_offset,
                            collected,
                            depth + 1,
                        );
                    }
                }
            }
        }

        // Extract prefix nodes (unchanged, so no byte offset)
        if prefix_len > 0 {
            collect_nodes_in_range(old_forest, 0..prefix_len, 0, &mut prefix_nodes, 0);
        }

        // Extract suffix nodes (with byte offset due to edit)
        if suffix_len > 0 && old_tokens.len() > suffix_len {
            let suffix_start = old_tokens.len() - suffix_len;
            collect_nodes_in_range(
                old_forest,
                suffix_start..old_tokens.len(),
                edit_delta,
                &mut suffix_nodes,
                0,
            );
        }

        (prefix_nodes, suffix_nodes)
    }

    /// Splice prefix, middle, and suffix forests into a single forest
    /// CRITICAL: This method must preserve all alternatives from the middle forest
    /// to maintain ambiguity after incremental edits.
    fn splice_forests(
        &mut self,
        prefix_nodes: Vec<Arc<ForestNode>>,
        middle_forest: Arc<ForestNode>,
        suffix_nodes: Vec<Arc<ForestNode>>,
        tokens: &[GLRToken],
    ) -> Arc<ForestNode> {
        // If the middle forest has no alternatives, handle the simple case first
        if middle_forest.alternatives.is_empty() {
            // Empty middle - just combine prefix and suffix if any
            let mut all_children = Vec::new();
            all_children.extend(prefix_nodes);
            all_children.extend(suffix_nodes);

            if all_children.is_empty() {
                return middle_forest;
            }

            if all_children.len() == 1
                && let Some(single_child) = all_children.first().cloned()
            {
                return single_child;
            }

            // Multiple non-middle children - create synthetic parent
            let byte_start = all_children
                .first()
                .map(|n| n.byte_range.start)
                .unwrap_or(0);
            let byte_end = all_children
                .last()
                .map(|n| n.byte_range.end)
                .unwrap_or_else(|| tokens.last().map(|t| t.end_byte).unwrap_or(0));
            let token_start = all_children
                .first()
                .map(|n| n.token_range.start)
                .unwrap_or(0);
            let token_end = all_children
                .last()
                .map(|n| n.token_range.end)
                .unwrap_or(tokens.len());

            return Arc::new(ForestNode {
                symbol: self.grammar.start_symbol().unwrap_or(SymbolId(0)),
                alternatives: vec![ForkAlternative {
                    fork_id: self.fork_tracker.create_fork(None),
                    rule_id: None,
                    children: all_children,
                    subtree: Arc::new(crate::subtree::Subtree::new(
                        crate::subtree::SubtreeNode {
                            symbol_id: self.grammar.start_symbol().unwrap_or(SymbolId(0)),
                            is_error: false,
                            byte_range: byte_start..byte_end,
                        },
                        vec![],
                    )),
                }],
                byte_range: byte_start..byte_end,
                token_range: token_start..token_end,
                cached_subtree: None,
            });
        }

        // CRITICAL FIX: Handle case where middle forest has alternatives (ambiguity)
        // We need to create a new forest node that preserves ALL alternatives from the middle,
        // but wraps them with the appropriate prefix and suffix nodes.

        // Calculate the combined byte and token ranges
        let combined_start = prefix_nodes
            .first()
            .map(|n| n.byte_range.start)
            .or_else(|| Some(middle_forest.byte_range.start))
            .unwrap_or(0);
        let combined_end = suffix_nodes
            .last()
            .map(|n| n.byte_range.end)
            .or_else(|| Some(middle_forest.byte_range.end))
            .unwrap_or_else(|| tokens.last().map(|t| t.end_byte).unwrap_or(0));

        let combined_token_start = prefix_nodes
            .first()
            .map(|n| n.token_range.start)
            .or_else(|| Some(middle_forest.token_range.start))
            .unwrap_or(0);
        let combined_token_end = suffix_nodes
            .last()
            .map(|n| n.token_range.end)
            .or_else(|| Some(middle_forest.token_range.end))
            .unwrap_or(tokens.len());

        // For each alternative in the middle forest, create a corresponding alternative
        // in the result forest that includes the prefix and suffix nodes
        let mut new_alternatives = Vec::new();

        for middle_alt in &middle_forest.alternatives {
            // Build the children list: prefix + middle_alternative_children + suffix
            let mut all_children = Vec::new();
            all_children.extend(prefix_nodes.iter().cloned());
            all_children.extend(middle_alt.children.iter().cloned());
            all_children.extend(suffix_nodes.iter().cloned());

            // Create a new alternative that preserves the middle alternative's fork_id
            // or creates a new one if needed
            let fork_id = middle_alt.fork_id;

            let new_alternative = ForkAlternative {
                fork_id,
                rule_id: middle_alt.rule_id, // Preserve rule_id from middle
                children: all_children,
                subtree: middle_alt.subtree.clone(), // This might need adjustment in a full implementation
            };

            new_alternatives.push(new_alternative);
        }

        // Create the result forest with all alternatives preserved
        Arc::new(ForestNode {
            symbol: middle_forest.symbol, // Use the middle forest's symbol as it's the "core" of the parse
            alternatives: new_alternatives,
            byte_range: combined_start..combined_end,
            token_range: combined_token_start..combined_token_end,
            cached_subtree: None,
        })
    }

    /// Inject a reusable subtree into the parser, preserving ambiguity
    fn _inject_subtree_into_parser(&self, parser: &mut GLRParser, node: Arc<ForestNode>) {
        // Convert each alternative in the ForestNode to a separate Subtree
        let subtrees: Vec<Arc<Subtree>> = if node.alternatives.is_empty() {
            // Leaf node or empty node
            let subtree_node = crate::subtree::SubtreeNode {
                symbol_id: node.symbol,
                is_error: false,
                byte_range: node.byte_range.clone(),
            };
            vec![Arc::new(Subtree::new(subtree_node, vec![]))]
        } else {
            // For each alternative, create a separate subtree
            node.alternatives
                .iter()
                .map(|alt| {
                    let subtree_node = crate::subtree::SubtreeNode {
                        symbol_id: node.symbol,
                        is_error: false,
                        byte_range: node.byte_range.clone(),
                    };

                    // Recursively convert children for this alternative
                    let children: Vec<Arc<Subtree>> = alt
                        .children
                        .iter()
                        .map(|child| self.forest_to_subtree_preserving_first_alt(child))
                        .collect();

                    Arc::new(Subtree::new(subtree_node, children))
                })
                .collect()
        };

        // Inject all alternative subtrees into the parser
        match parser.inject_ambiguous_subtrees(subtrees) {
            Ok(_) => {
                // Successfully injected the subtrees
                SUBTREE_REUSE_COUNT.fetch_add(1, Ordering::SeqCst);
            }
            Err(_) => {
                // Failed to inject - parser will re-parse this region
            }
        }
    }

    /// Helper function that creates a single subtree from a forest node
    /// Used when we need a single subtree for children but still want to be consistent
    #[allow(dead_code)]
    fn forest_to_subtree_preserving_first_alt(&self, node: &Arc<ForestNode>) -> Arc<Subtree> {
        let subtree_node = crate::subtree::SubtreeNode {
            symbol_id: node.symbol,
            is_error: false,
            byte_range: node.byte_range.clone(),
        };

        // For children, we still need to pick one alternative (limitation of Subtree structure)
        // But at least at the top level we preserve all alternatives
        let children = if let Some(alt) = node.alternatives.first() {
            alt.children
                .iter()
                .map(|child| self.forest_to_subtree_preserving_first_alt(child))
                .collect()
        } else {
            vec![]
        };

        Arc::new(Subtree::new(subtree_node, children))
    }

    /// Helper function to convert ForestNode to Subtree (legacy, only uses first alternative)
    #[allow(dead_code)]
    fn forest_to_subtree(&self, node: &Arc<ForestNode>) -> Arc<Subtree> {
        let subtree_node = crate::subtree::SubtreeNode {
            symbol_id: node.symbol,
            is_error: false,
            byte_range: node.byte_range.clone(),
        };

        // For simplicity, take the first alternative (could be improved)
        let children = if let Some(alt) = node.alternatives.first() {
            alt.children
                .iter()
                .map(|child| self.forest_to_subtree(child))
                .collect()
        } else {
            vec![]
        };

        Arc::new(Subtree::new(subtree_node, children))
    }

    /// Build a forest node from a subtree
    fn build_forest_from_subtree(
        &mut self,
        subtree: Arc<Subtree>,
        fork_id: usize,
        _tokens: &[GLRToken],
    ) -> Arc<ForestNode> {
        // Recursively build ForestNode from Subtree
        self.subtree_to_forest_recursive(subtree, fork_id)
    }

    /// Recursively convert a Subtree to a ForestNode with proper children
    fn subtree_to_forest_recursive(
        &mut self,
        subtree: Arc<Subtree>,
        fork_id: usize,
    ) -> Arc<ForestNode> {
        // Check if this subtree falls within a reusable chunk
        if let Some(ref old_forest) = self.previous_forest {
            // Check if this subtree is in the unchanged prefix chunk
            if self.chunk_prefix_len > 0 {
                let prefix_byte_boundary = if self.chunk_prefix_len < self.tokens.len() {
                    self.tokens[self.chunk_prefix_len].start_byte
                } else {
                    usize::MAX
                };

                if subtree.node.byte_range.end <= prefix_byte_boundary {
                    // This subtree is entirely within the prefix chunk - try to reuse it
                    if let Some(reused_node) = self.find_matching_node_in_forest(
                        old_forest,
                        subtree.node.symbol_id,
                        &subtree.node.byte_range,
                    ) {
                        SUBTREE_REUSE_COUNT.fetch_add(1, Ordering::SeqCst);
                        return reused_node;
                    }
                }
            }

            // Check if this subtree is in the unchanged suffix chunk
            if self.chunk_suffix_len > 0 && self.tokens.len() > self.chunk_suffix_len {
                let suffix_byte_boundary =
                    self.tokens[self.tokens.len() - self.chunk_suffix_len].start_byte;

                if subtree.node.byte_range.start >= suffix_byte_boundary {
                    // This subtree is entirely within the suffix chunk - try to reuse it
                    // Note: suffix bytes may have shifted due to the edit
                    let edit_delta = self.get_edit_byte_delta();
                    let adjusted_range = Range {
                        start: (subtree.node.byte_range.start as isize - edit_delta) as usize,
                        end: (subtree.node.byte_range.end as isize - edit_delta) as usize,
                    };

                    if let Some(reused_node) = self.find_matching_node_in_forest(
                        old_forest,
                        subtree.node.symbol_id,
                        &adjusted_range,
                    ) {
                        // Clone the node but adjust its byte range for the new position
                        let adjusted_node = Arc::new(ForestNode {
                            symbol: reused_node.symbol,
                            alternatives: reused_node.alternatives.clone(),
                            byte_range: subtree.node.byte_range.clone(),
                            token_range: reused_node.token_range.clone(),
                            cached_subtree: Some(subtree.clone()),
                        });

                        SUBTREE_REUSE_COUNT.fetch_add(1, Ordering::SeqCst);
                        return adjusted_node;
                    }
                }
            }
        }

        // If we can't reuse, build a new node
        // Convert children recursively
        let children: Vec<Arc<ForestNode>> = subtree
            .children
            .iter()
            .map(|edge| self.subtree_to_forest_recursive(edge.subtree.clone(), fork_id))
            .collect();

        // Create forest node with proper children
        let alternative = ForkAlternative {
            fork_id,
            rule_id: None,
            children,
            subtree: subtree.clone(),
        };

        // Calculate token range from byte range
        let token_range = self.find_token_range(&subtree.node.byte_range, &self.tokens);

        Arc::new(ForestNode {
            symbol: subtree.node.symbol_id,
            alternatives: vec![alternative],
            byte_range: subtree.node.byte_range.clone(),
            token_range,
            cached_subtree: Some(subtree),
        })
    }

    /// Find a matching node in the old forest for reuse
    fn find_matching_node_in_forest(
        &self,
        forest: &Arc<ForestNode>,
        symbol: SymbolId,
        byte_range: &Range<usize>,
    ) -> Option<Arc<ForestNode>> {
        // Direct match at the root
        if forest.symbol == symbol && forest.byte_range == *byte_range {
            return Some(forest.clone());
        }

        // Search in alternatives and their children
        for alt in &forest.alternatives {
            for child in &alt.children {
                if let Some(found) = self.find_matching_node_in_forest(child, symbol, byte_range) {
                    return Some(found);
                }
            }
        }

        None
    }

    /// Get the byte delta from the stored edit
    fn get_edit_byte_delta(&self) -> isize {
        self.edit_byte_delta
    }

    /// Find the token range for a byte range
    #[allow(dead_code)]
    fn find_token_range(&self, byte_range: &Range<usize>, tokens: &[GLRToken]) -> Range<usize> {
        let start = tokens
            .iter()
            .position(|t| t.start_byte >= byte_range.start)
            .unwrap_or(0);

        let end = tokens
            .iter()
            .rposition(|t| t.end_byte <= byte_range.end)
            .map(|i| i + 1)
            .unwrap_or(tokens.len());

        start..end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_identifier() {
        // Test the new ChunkIdentifier logic
        let edit = GLREdit {
            old_range: 6..7,
            new_text: b"*".to_vec(),
            old_token_range: 1..2,
            new_tokens: vec![],
            old_tokens: vec![],
            old_forest: None,
        };
        let chunk_id = ChunkIdentifier::new(None, &edit);

        // Create test tokens
        let old_tokens = vec![
            GLRToken {
                symbol: SymbolId(1),
                text: b"1".to_vec(),
                start_byte: 0,
                end_byte: 1,
            },
            GLRToken {
                symbol: SymbolId(2),
                text: b"+".to_vec(),
                start_byte: 2,
                end_byte: 3,
            },
            GLRToken {
                symbol: SymbolId(1),
                text: b"2".to_vec(),
                start_byte: 4,
                end_byte: 5,
            },
            GLRToken {
                symbol: SymbolId(2),
                text: b"-".to_vec(),
                start_byte: 6,
                end_byte: 7,
            },
            GLRToken {
                symbol: SymbolId(1),
                text: b"3".to_vec(),
                start_byte: 8,
                end_byte: 9,
            },
        ];

        let new_tokens = vec![
            GLRToken {
                symbol: SymbolId(1),
                text: b"1".to_vec(),
                start_byte: 0,
                end_byte: 1,
            },
            GLRToken {
                symbol: SymbolId(2),
                text: b"+".to_vec(),
                start_byte: 2,
                end_byte: 3,
            },
            GLRToken {
                symbol: SymbolId(1),
                text: b"2".to_vec(),
                start_byte: 4,
                end_byte: 5,
            },
            GLRToken {
                symbol: SymbolId(2),
                text: b"*".to_vec(),
                start_byte: 6,
                end_byte: 7,
            },
            GLRToken {
                symbol: SymbolId(1),
                text: b"3".to_vec(),
                start_byte: 8,
                end_byte: 9,
            },
        ];

        // Test prefix boundary detection
        let prefix_len = chunk_id.find_prefix_boundary(&old_tokens, &new_tokens);
        assert_eq!(prefix_len, 3); // First 3 tokens are unchanged and before the edit

        // Test suffix boundary detection (with proper delta)
        let edit_delta =
            (edit.new_text.len() as isize) - ((edit.old_range.end - edit.old_range.start) as isize);
        let suffix_len = chunk_id.find_suffix_boundary(&old_tokens, &new_tokens, edit_delta);
        assert_eq!(suffix_len, 1); // Last token is unchanged and after the edit
    }

    #[test]
    fn test_forest_node_overlap() {
        let node = ForestNode {
            symbol: SymbolId(1),
            alternatives: vec![],
            byte_range: 10..20,
            token_range: 2..4,
            cached_subtree: None,
        };

        // Test overlapping ranges
        assert!(node.overlaps_edit(&(5..15))); // Overlaps start
        assert!(node.overlaps_edit(&(15..25))); // Overlaps end
        assert!(node.overlaps_edit(&(12..18))); // Fully contained
        assert!(node.overlaps_edit(&(5..25))); // Fully contains

        // Test non-overlapping ranges
        assert!(!node.overlaps_edit(&(0..10))); // Before
        assert!(!node.overlaps_edit(&(20..30))); // After
    }

    #[test]
    fn test_subtree_reuse_counter() {
        reset_reuse_counter();
        assert_eq!(get_reuse_count(), 0);

        let node = ForestNode {
            symbol: SymbolId(1),
            alternatives: vec![],
            byte_range: 10..20,
            token_range: 2..4,
            cached_subtree: None,
        };

        // Find reusable subtrees (not overlapping with edit)
        // NOTE: Subtree reuse is temporarily disabled for GLR compatibility
        let _reusable = node.find_reusable_subtrees(&(30..40));
        assert_eq!(get_reuse_count(), 0); // Reuse is disabled, count stays 0

        // Find reusable subtrees (overlapping - no reuse)
        let _reusable = node.find_reusable_subtrees(&(15..25));
        assert_eq!(get_reuse_count(), 0); // Count shouldn't increase
    }

    #[test]
    fn test_fork_tracker() {
        let mut tracker = ForkTracker::new();

        // Create initial fork
        let fork0 = tracker.create_fork(None);
        assert_eq!(fork0, 0);
        assert!(tracker.active_forks.contains(&fork0));

        // Create child forks
        let fork1 = tracker.create_fork(Some(fork0));
        let fork2 = tracker.create_fork(Some(fork0));

        assert_eq!(tracker.fork_parents[&fork1], fork0);
        assert_eq!(tracker.fork_parents[&fork2], fork0);

        // Record a merge
        tracker.merge_forks(fork1, fork2, 100);
        assert!(tracker.fork_merges[&fork1].contains(&100));
        assert!(tracker.fork_merges[&fork2].contains(&100));
    }
}
