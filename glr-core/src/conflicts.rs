use crate::{
    Action, FirstFollowSets, ItemSetCollection, PrecedenceComparison, StaticPrecedenceResolver,
    compare_precedences,
};
use adze_ir::*;
use indexmap::IndexMap;

/// Conflict detection and resolution
#[derive(Debug, Clone)]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub struct ConflictResolver {
    /// All detected parse table conflicts.
    pub conflicts: Vec<Conflict>,
}

/// Conflict information for GLR parsing
#[derive(Debug, Clone)]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub struct Conflict {
    /// Parser state where the conflict occurs.
    pub state: StateId,
    /// Lookahead symbol that triggers the conflict.
    pub symbol: SymbolId,
    /// Conflicting actions for this state/symbol pair.
    pub actions: Vec<Action>,
    /// Classification of the conflict.
    pub conflict_type: ConflictType,
}

/// Type of parser conflict
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub enum ConflictType {
    /// Conflict between a shift action and a reduce action.
    ShiftReduce,
    /// Conflict between two different reduce actions.
    ReduceReduce,
}

impl ConflictResolver {
    /// Detect conflicts in the parse table.
    ///
    /// Scans every item set and reports shift/reduce or reduce/reduce conflicts.
    ///
    /// # Examples
    ///
    /// ```
    /// use adze_glr_core::{ConflictResolver, ConflictType, FirstFollowSets, ItemSetCollection};
    /// use adze_ir::*;
    ///
    /// // E → a | E E  (inherently ambiguous)
    /// let mut grammar = Grammar::new("ambig".into());
    /// let a = SymbolId(1);
    /// let e = SymbolId(10);
    /// grammar.tokens.insert(a, Token { name: "a".into(), pattern: TokenPattern::String("a".into()), fragile: false });
    /// grammar.rule_names.insert(e, "E".into());
    /// grammar.rules.insert(e, vec![
    ///     Rule { lhs: e, rhs: vec![Symbol::Terminal(a)], precedence: None, associativity: None, fields: vec![], production_id: ProductionId(0) },
    ///     Rule { lhs: e, rhs: vec![Symbol::NonTerminal(e), Symbol::NonTerminal(e)], precedence: None, associativity: None, fields: vec![], production_id: ProductionId(1) },
    /// ]);
    ///
    /// let ff = FirstFollowSets::compute(&grammar).unwrap();
    /// let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    /// let resolver = ConflictResolver::detect_conflicts(&collection, &grammar, &ff);
    /// // An ambiguous grammar like E → a | E E should have conflicts
    /// assert!(!resolver.conflicts.is_empty(), "should detect conflicts");
    /// ```
    pub fn detect_conflicts(
        item_sets: &ItemSetCollection,
        grammar: &Grammar,
        _first_follow: &FirstFollowSets,
    ) -> Self {
        let mut conflicts = Vec::new();

        for item_set in &item_sets.sets {
            let mut actions_by_symbol: IndexMap<SymbolId, Vec<Action>> = IndexMap::new();

            // Collect all possible actions for each symbol in this state
            for item in &item_set.items {
                if item.is_reduce_item(grammar) {
                    // Check if this is a reduction to the start symbol with EOF lookahead
                    let mut is_accept = false;

                    // Find the rule that corresponds to this rule ID
                    if let Some(start_symbol) = grammar.start_symbol() {
                        // Look through all rules to find the one with this rule ID
                        for rule in grammar.all_rules() {
                            if rule.production_id.0 == item.rule_id.0 {
                                // Check if this rule reduces to the start symbol and we have EOF lookahead
                                is_accept =
                                    rule.lhs == start_symbol && item.lookahead == SymbolId(0);
                                break;
                            }
                        }
                    }

                    let action = if is_accept {
                        Action::Accept
                    } else {
                        Action::Reduce(item.rule_id)
                    };

                    actions_by_symbol
                        .entry(item.lookahead)
                        .or_default()
                        .push(action);
                } else if let Some(symbol) = item.next_symbol(grammar) {
                    // Shift action
                    let symbol_id = match symbol {
                        Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => {
                            *id
                        }
                        Symbol::Optional(_)
                        | Symbol::Repeat(_)
                        | Symbol::RepeatOne(_)
                        | Symbol::Choice(_)
                        | Symbol::Sequence(_)
                        | Symbol::Epsilon => {
                            panic!(
                                "Complex symbols should be normalized before LR item generation"
                            );
                        }
                    };

                    if let Some(target_state) = item_sets.goto_table.get(&(item_set.id, symbol_id))
                    {
                        let action = Action::Shift(*target_state);
                        actions_by_symbol.entry(symbol_id).or_default().push(action);
                    }
                }
            }

            // Check for conflicts
            for (symbol_id, actions) in actions_by_symbol {
                if actions.len() > 1 {
                    let conflict_type = if actions.iter().any(|a| matches!(a, Action::Shift(_)))
                        && actions.iter().any(|a| matches!(a, Action::Reduce(_)))
                    {
                        ConflictType::ShiftReduce
                    } else {
                        ConflictType::ReduceReduce
                    };

                    conflicts.push(Conflict {
                        state: item_set.id,
                        symbol: symbol_id,
                        actions,
                        conflict_type,
                    });
                }
            }
        }

        Self { conflicts }
    }

    /// Resolve conflicts using precedence and associativity rules
    pub fn resolve_conflicts(&mut self, grammar: &Grammar) {
        // Clone conflicts to avoid borrowing issues
        let mut conflicts_to_resolve = self.conflicts.clone();
        for conflict in &mut conflicts_to_resolve {
            // Apply Tree-sitter's exact conflict resolution logic
            self.resolve_single_conflict(conflict, grammar);
        }
        self.conflicts = conflicts_to_resolve;
    }

    fn resolve_single_conflict(&self, conflict: &mut Conflict, grammar: &Grammar) {
        // Implement Tree-sitter's exact precedence and associativity resolution
        // This is where we port the C logic for conflict resolution

        match conflict.conflict_type {
            ConflictType::ShiftReduce => {
                // Apply precedence rules between shift and reduce
                // Higher precedence wins, same precedence uses associativity
                self.resolve_shift_reduce_conflict(conflict, grammar);
            }
            ConflictType::ReduceReduce => {
                // Apply precedence rules between multiple reduces
                // Usually choose the rule that appears first in the grammar
                self.resolve_reduce_reduce_conflict(conflict, grammar);
            }
        }
    }

    fn resolve_shift_reduce_conflict(&self, conflict: &mut Conflict, grammar: &Grammar) {
        // Use Tree-sitter's exact precedence comparison logic
        let precedence_resolver = StaticPrecedenceResolver::from_grammar(grammar);

        let mut shift_action = None;
        let mut reduce_action = None;

        // Find shift and reduce actions
        for action in &conflict.actions {
            match action {
                Action::Shift(_) => shift_action = Some(action.clone()),
                Action::Reduce(_) => reduce_action = Some(action.clone()),
                _ => {}
            }
        }

        match (shift_action, reduce_action) {
            (Some(shift), Some(reduce)) => {
                // Get precedence info for shift token
                let shift_prec = precedence_resolver.token_precedence(conflict.symbol);

                // Get precedence info for reduce rule
                let reduce_prec = if let Action::Reduce(rule_id) = &reduce {
                    precedence_resolver.rule_precedence(*rule_id)
                } else {
                    None
                };

                // Compare precedences
                // PRECEDENCE RESOLUTION: When precedence can definitively resolve the conflict,
                // we eliminate the lower-precedence action (not just re-order).
                // This ensures correct parsing for unambiguous grammars.
                match compare_precedences(shift_prec, reduce_prec) {
                    PrecedenceComparison::PreferShift => {
                        // Shift wins - eliminate reduce action
                        conflict.actions = vec![shift];
                    }
                    PrecedenceComparison::PreferReduce => {
                        // Reduce wins - eliminate shift action
                        conflict.actions = vec![reduce];
                    }
                    PrecedenceComparison::Error => {
                        // Non-associative conflict - this is an error
                        // Keep Fork to signal ambiguity for error reporting
                        conflict.actions = vec![Action::Fork(vec![shift, reduce])];
                    }
                    PrecedenceComparison::None => {
                        // No precedence info - use GLR fork to explore all paths
                        conflict.actions = vec![Action::Fork(vec![shift, reduce])];
                    }
                }
            }
            _ => {
                // Should not happen in a shift/reduce conflict
                // Keep original actions
            }
        }
    }

    fn resolve_reduce_reduce_conflict(&self, conflict: &mut Conflict, _grammar: &Grammar) {
        // Choose the rule that appears first in the grammar
        // This is Tree-sitter's default behavior for reduce/reduce conflicts

        let mut best_action = None;
        let mut best_rule_id = u16::MAX;

        for action in &conflict.actions {
            if let Action::Reduce(rule_id) = action
                && rule_id.0 < best_rule_id
            {
                best_rule_id = rule_id.0;
                best_action = Some(action.clone());
            }
        }

        if let Some(action) = best_action {
            conflict.actions = vec![action];
        }
    }
}
