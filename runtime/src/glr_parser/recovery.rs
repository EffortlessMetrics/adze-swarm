//! GLR-aware error recovery helpers.

use super::{GLRParser, ParseStack, RecoveryEvent};
use crate::subtree::{Subtree, SubtreeNode};
use adze_glr_core::Action;
use adze_ir::SymbolId;
use std::sync::Arc;

impl GLRParser {
    /// Check if any active stack has an action for the given token.
    #[inline]
    pub(super) fn any_stack_has_action(&self, lookahead: SymbolId) -> bool {
        let Some(&col) = self.table.symbol_to_index.get(&lookahead) else {
            return false;
        };
        self.stacks.iter().any(|stack| {
            let s = stack.current_state();
            !self.table.action_table[s.0 as usize][col].is_empty()
        })
    }

    pub(super) fn any_stack_has_action_in(
        &self,
        stacks: &[ParseStack],
        lookahead: SymbolId,
    ) -> bool {
        let Some(&col) = self.table.symbol_to_index.get(&lookahead) else {
            return false;
        };
        stacks.iter().any(|stack| {
            let s = stack.current_state();
            !self.table.action_table[s.0 as usize][col].is_empty()
        })
    }

    /// Check if any stack can shift or reduce for the given terminal symbol.
    ///
    /// This only checks terminals, not nonterminals, which use `goto_table`.
    #[inline]
    fn can_shift_or_reduce(&self, sym: SymbolId) -> bool {
        let Some(&col) = self.table.symbol_to_index.get(&sym) else {
            debug_glr!("  Terminal {:?} not in symbol_to_index map", sym);
            return false;
        };
        let result = self.stacks.iter().any(|stack| {
            let s = stack.current_state();
            if s.0 as usize >= self.table.action_table.len() {
                debug_glr!("  State {} is out of bounds!", s.0);
                return false;
            }
            if col >= self.table.action_table[s.0 as usize].len() {
                debug_glr!("  Column {} is out of bounds for state {}!", col, s.0);
                return false;
            }
            let cell = &self.table.action_table[s.0 as usize][col];
            if !cell.is_empty() {
                debug_glr!(
                    "  State {} has action for symbol {:?}: {:?}",
                    s.0,
                    sym,
                    cell
                );
            } else {
                debug_glr!("  State {} has NO action for symbol {:?}", s.0, sym);
            }
            !cell.is_empty()
        });
        if !result {
            debug_glr!(
                "  No stack has action for symbol {:?} (checked {} stacks)",
                sym,
                self.stacks.len()
            );
        }
        result
    }

    /// Insert a synthetic token with zero width into the input stream.
    fn insert_token_zero_width(&mut self, sym: SymbolId) {
        self.pending_synthetic_tokens.push_back(sym);
    }

    /// Perform shifts for a synthetic token across all GLR stacks.
    pub(super) fn shift_synthetic_token(&mut self, sym: SymbolId) {
        let mut new_stacks = Vec::new();

        for stack in self.stacks.drain(..) {
            let state = stack.current_state();
            let mut shifted = false;

            if let Some(&symbol_idx) = self.table.symbol_to_index.get(&sym) {
                let action_cell = &self.table.action_table[state.0 as usize][symbol_idx];

                for action in action_cell {
                    if let Action::Shift(new_state) = action {
                        let mut new_stack = stack.clone();
                        new_stack.push(
                            *new_state,
                            Arc::new(Subtree::new(
                                SubtreeNode {
                                    symbol_id: sym,
                                    is_error: true,
                                    byte_range: self.input_length..self.input_length,
                                },
                                Vec::new(),
                            )),
                        );
                        new_stacks.push(new_stack);
                        shifted = true;
                        break;
                    }
                }
            }

            if !shifted {
                new_stacks.push(stack);
            }
        }

        self.stacks = new_stacks;
    }

    /// Pop symbols from stacks towards sync tokens.
    fn pop_towards_sync(&mut self, lookahead: SymbolId) -> Option<usize> {
        let config = self.error_recovery.as_ref()?;

        let mut target_set = config.sync_tokens.clone().into_vec();
        target_set.push(lookahead);

        const POP_BOUND: usize = 8;
        let mut max_popped = 0usize;
        let mut progress = false;

        let mut modified_stacks = Vec::new();
        for stack in self.stacks.iter() {
            let mut test_stack = stack.clone();
            let mut pops = 0usize;

            while pops < POP_BOUND && test_stack.states.len() > 1 {
                let state = test_stack.current_state();

                let has_action = target_set.iter().any(|&sym| {
                    self.table.symbol_to_index.get(&sym).is_some_and(|&col| {
                        !self.table.action_table[state.0 as usize][col].is_empty()
                    })
                });

                if has_action {
                    progress = true;
                    max_popped = max_popped.max(pops);
                    modified_stacks.push(test_stack);
                    break;
                }

                if test_stack.states.len() > 1 {
                    test_stack.states.pop();
                    test_stack.nodes.pop();
                    pops += 1;
                } else {
                    break;
                }
            }
        }

        if progress {
            if !modified_stacks.is_empty() {
                self.stacks = modified_stacks;
            }
            Some(max_popped)
        } else {
            None
        }
    }

    /// Main GLR-aware recovery driver.
    pub(super) fn try_recover(&mut self, lookahead: SymbolId, eof: bool) -> Option<RecoveryEvent> {
        if self.error_recovery.is_none() || self.stacks.is_empty() {
            debug_glr!(
                "try_recover: no recovery (config={:?}, stacks={})",
                self.error_recovery.is_some(),
                self.stacks.len()
            );
            return None;
        }

        let recovery = self.error_recovery.clone()?;
        let max_insertions = recovery.max_token_insertions;
        if self.inserted_in_row < max_insertions {
            let candidates = recovery.insert_candidates.clone();
            debug_glr!(
                "recovery: checking {} insert candidates with {} stacks (eof={})",
                candidates.len(),
                self.stacks.len(),
                eof
            );
            #[allow(unused_variables)]
            for stack in &self.stacks {
                debug_glr!("  Stack in state {}", stack.current_state().0);
            }
            for tok in candidates {
                debug_glr!("recovery: checking if {:?} would help (eof={})", tok, eof);
                if self.can_shift_or_reduce(tok) {
                    debug_glr!("recovery: INSERT {:?} would help", tok);
                    self.insert_token_zero_width(tok);
                    self.inserted_in_row += 1;
                    self.deleted_in_row = 0;

                    let stacks = std::mem::take(&mut self.stacks);
                    let stacks = self.reduce_until_saturated(stacks, tok, self.input_length);
                    self.stacks = stacks;
                    self.shift_synthetic_token(tok);

                    return Some(RecoveryEvent::Insert(tok));
                }
            }
        }

        if let Some(popped) = self.pop_towards_sync(lookahead) {
            debug_glr!("recovery: POP {} symbols towards sync", popped);
            self.deleted_in_row = 0;
            self.inserted_in_row = 0;

            let stacks = std::mem::take(&mut self.stacks);
            let stacks = self.reduce_until_saturated(stacks, lookahead, self.input_length);
            self.stacks = stacks;

            if self.any_stack_has_action(lookahead) {
                return Some(RecoveryEvent::Pop(popped));
            }
        }

        let max_deletions = recovery.max_token_deletions;
        if !eof && self.deleted_in_row < max_deletions {
            debug_glr!("recovery: DELETE {:?}", lookahead);
            self.deleted_in_row += 1;
            self.inserted_in_row = 0;

            return Some(RecoveryEvent::Delete(lookahead));
        }

        None
    }
}
