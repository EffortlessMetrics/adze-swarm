//! Conflict resolution strategies for GLR parse table construction.

use crate::{Action, FirstFollowSets, Grammar, ParseTable, ProductionId, StateId, SymbolId};
use fixedbitset::FixedBitSet;
use rustc_hash::FxHashMap;

/// Trait for resolving conflicts at runtime
pub trait RuntimeConflictResolver {
    /// Resolve a conflict between multiple actions
    /// Returns Some(action) to take that action, or None to use default fork behavior
    fn resolve(&self, state: StateId, lookahead: SymbolId, actions: &[Action]) -> Option<Action>;
}

pub struct VecWrapperResolver {
    // Cache: state -> optional vec wrapper empty production
    wrapper_states: FxHashMap<StateId, Option<ProductionId>>,
    statement_starters: FixedBitSet,
}

impl VecWrapperResolver {
    /// Creates a new resolver by analyzing the grammar for vec-wrapper patterns.
    pub fn new(grammar: &Grammar, first_follow: &FirstFollowSets) -> Self {
        // Get the maximum symbol ID to size our bitset properly
        let max_symbol_id = grammar
            .rules
            .keys()
            .chain(grammar.tokens.keys())
            .map(|id| id.0)
            .max()
            .unwrap_or(0) as usize
            + 1;

        let mut statement_starters = FixedBitSet::with_capacity(max_symbol_id);

        // Find FIRST(Statement) - you already compute this
        if let Some(stmt_id) = grammar.find_symbol_by_name("Statement")
            && let Some(first_set) = first_follow.first(stmt_id)
        {
            statement_starters.union_with(first_set);
        }

        // Also check for other common statement starters
        for name in &[
            "ExpressionStatement",
            "AssignmentStatement",
            "Primary",
            "Number",
        ] {
            if let Some(id) = grammar.find_symbol_by_name(name)
                && let Some(first_set) = first_follow.first(id)
            {
                statement_starters.union_with(first_set);
            }
        }

        Self {
            wrapper_states: FxHashMap::default(),
            statement_starters,
        }
    }

    /// Returns the empty-production ID if the state has a vec-wrapper conflict.
    pub fn get_vec_wrapper_action(
        &mut self,
        state: StateId,
        table: &ParseTable,
        grammar: &Grammar,
    ) -> Option<ProductionId> {
        // Check cache first
        if let Some(&cached) = self.wrapper_states.get(&state) {
            return cached;
        }

        // Find vec wrapper empty production in this state
        let mut result = None;

        // Look through the action table for reduce actions in this state
        if let Some(state_actions) = table.action_table.get(state.0 as usize) {
            for action_cell in state_actions.iter() {
                // Each cell now contains a Vec<Action>
                for action in action_cell {
                    match action {
                        Action::Reduce(rule_id) => {
                            // Find the corresponding rule in the grammar
                            if let Some(rule) =
                                grammar.all_rules().find(|r| r.production_id.0 == rule_id.0)
                            {
                                // Check if this is a vec wrapper empty rule
                                if let Some(rule_name) = grammar.rule_names.get(&rule.lhs)
                                    && rule_name.ends_with("_vec_contents")
                                    && rule.rhs.is_empty()
                                {
                                    result = Some(ProductionId(rule_id.0));
                                    break;
                                }
                            }
                        }
                        Action::Fork(actions) => {
                            // Check fork actions too
                            for fork_action in actions {
                                if let Action::Reduce(rule_id) = fork_action
                                    && let Some(rule) =
                                        grammar.all_rules().find(|r| r.production_id.0 == rule_id.0)
                                    && let Some(rule_name) = grammar.rule_names.get(&rule.lhs)
                                    && rule_name.ends_with("_vec_contents")
                                    && rule.rhs.is_empty()
                                {
                                    result = Some(ProductionId(rule_id.0));
                                    break;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                if result.is_some() {
                    break;
                }
            }
        }

        self.wrapper_states.insert(state, result);
        result
    }

    /// Returns `true` if the given token is NOT a statement-starter.
    pub fn should_reduce_empty(&self, token: SymbolId) -> bool {
        // Reduce empty if NOT a statement starter
        !self.statement_starters.contains(token.0 as usize)
    }
}

impl RuntimeConflictResolver for VecWrapperResolver {
    fn resolve(&self, _state: StateId, lookahead: SymbolId, actions: &[Action]) -> Option<Action> {
        debug_assert!(
            actions.len() == 2,
            "VecWrapperResolver expects exactly 2 conflicting actions"
        );

        // Look for a reduce action that's a vec_contents empty production
        let mut reduce_action = None;
        let mut shift_action = None;

        for action in actions {
            match action {
                Action::Reduce(_) => reduce_action = Some(action.clone()),
                Action::Shift(_) => shift_action = Some(action.clone()),
                _ => {}
            }
        }

        // If we have both shift and reduce actions
        if let (Some(reduce), Some(shift)) = (reduce_action, shift_action) {
            // Heuristic: if the lookahead is in FIRST(Statement), choose Shift
            // Otherwise, choose Reduce (empty vec)
            if self.statement_starters.contains(lookahead.0 as usize) {
                Some(shift)
            } else {
                Some(reduce)
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuleId;
    use adze_ir::{Rule, Symbol, Token, TokenPattern};

    /// Build a tiny grammar where rule `S → a` exists and `a` is a terminal.
    /// Returns the grammar plus the SymbolIds used.
    fn tiny_grammar() -> (Grammar, SymbolId, SymbolId) {
        let mut grammar = Grammar::new("tiny".into());
        let a = SymbolId(1);
        let s = SymbolId(10);
        grammar.tokens.insert(
            a,
            Token {
                name: "a".into(),
                pattern: TokenPattern::String("a".into()),
                fragile: false,
            },
        );
        grammar.rule_names.insert(s, "S".into());
        grammar.rules.insert(
            s,
            vec![Rule {
                lhs: s,
                rhs: vec![Symbol::Terminal(a)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            }],
        );
        (grammar, a, s)
    }

    /// Build a grammar containing a "Statement" non-terminal so the resolver
    /// populates the statement_starters bitset via the primary path.
    fn statement_grammar() -> (Grammar, SymbolId, SymbolId) {
        let mut grammar = Grammar::new("stmts".into());
        let kw = SymbolId(2); // "if" keyword — a Statement starter
        let other = SymbolId(3); // a token that is NOT a statement starter
        let stmt = SymbolId(20);

        grammar.tokens.insert(
            kw,
            Token {
                name: "if".into(),
                pattern: TokenPattern::String("if".into()),
                fragile: false,
            },
        );
        grammar.tokens.insert(
            other,
            Token {
                name: "semi".into(),
                pattern: TokenPattern::String(";".into()),
                fragile: false,
            },
        );
        grammar.rule_names.insert(stmt, "Statement".into());
        grammar.rules.insert(
            stmt,
            vec![Rule {
                lhs: stmt,
                rhs: vec![Symbol::Terminal(kw)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            }],
        );
        (grammar, kw, other)
    }

    #[test]
    fn new_handles_grammar_without_statement_named_rules() {
        let (grammar, a, _s) = tiny_grammar();
        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let resolver = VecWrapperResolver::new(&grammar, &ff);

        // No "Statement"/"Primary"/etc. rules exist, so no symbol should be a
        // statement starter — `should_reduce_empty` returns true for any token.
        assert!(resolver.should_reduce_empty(a));
        // Cache should start empty.
        assert!(resolver.wrapper_states.is_empty());
    }

    #[test]
    fn new_populates_statement_starters_from_statement_rule() {
        let (grammar, kw, other) = statement_grammar();
        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let resolver = VecWrapperResolver::new(&grammar, &ff);

        // `kw` (FIRST(Statement)) is a statement-starter, so reducing empty is
        // discouraged. `other` is not in FIRST(Statement) so reducing is fine.
        assert!(!resolver.should_reduce_empty(kw));
        assert!(resolver.should_reduce_empty(other));
    }

    #[test]
    fn new_unions_alternative_statement_starter_names() {
        // Grammar that has no "Statement" but does have one of the fallback
        // rule names checked in the constructor ("Primary"). The starter token
        // for `Primary` should land in the bitset.
        let mut grammar = Grammar::new("primary-only".into());
        let num = SymbolId(4);
        let prim = SymbolId(30);
        grammar.tokens.insert(
            num,
            Token {
                name: "num".into(),
                pattern: TokenPattern::Regex("[0-9]+".into()),
                fragile: false,
            },
        );
        grammar.rule_names.insert(prim, "Primary".into());
        grammar.rules.insert(
            prim,
            vec![Rule {
                lhs: prim,
                rhs: vec![Symbol::Terminal(num)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            }],
        );
        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let resolver = VecWrapperResolver::new(&grammar, &ff);

        assert!(!resolver.should_reduce_empty(num));
    }

    #[test]
    fn new_handles_empty_grammar_without_panic() {
        // The constructor sizes its bitset using max symbol id; for an empty
        // grammar we expect `unwrap_or(0) + 1` => capacity 1 with no panics.
        let grammar = Grammar::new("empty".into());
        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let resolver = VecWrapperResolver::new(&grammar, &ff);
        // SymbolId(0) is the only valid index in the bitset — it's clear.
        assert!(resolver.should_reduce_empty(SymbolId(0)));
    }

    #[test]
    fn should_reduce_empty_treats_out_of_range_symbols_as_non_starters() {
        // The bitset is sized to fit the highest known symbol id; querying a
        // symbol beyond that capacity must not panic and must report "not a
        // starter" (FixedBitSet::contains returns false for out-of-bounds).
        let (grammar, _kw, _other) = statement_grammar();
        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let resolver = VecWrapperResolver::new(&grammar, &ff);

        // A symbol id well above the grammar's max — should still report as
        // "reduce empty is fine".
        assert!(resolver.should_reduce_empty(SymbolId(50_000)));
    }

    #[test]
    fn get_vec_wrapper_action_returns_none_when_state_missing() {
        let (grammar, _a, _s) = tiny_grammar();
        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let mut resolver = VecWrapperResolver::new(&grammar, &ff);
        let table = ParseTable::default();

        // No states exist — must return None and cache the miss.
        assert!(
            resolver
                .get_vec_wrapper_action(StateId(0), &table, &grammar)
                .is_none()
        );
        assert_eq!(resolver.wrapper_states.get(&StateId(0)), Some(&None));
    }

    #[test]
    fn get_vec_wrapper_action_returns_none_for_non_vec_contents_rule() {
        // State 0 has a Reduce action, but the rule's LHS is named "S" — not a
        // `*_vec_contents` rule — so the resolver should not flag it.
        let (grammar, _a, _s) = tiny_grammar();
        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let mut resolver = VecWrapperResolver::new(&grammar, &ff);

        let mut table = ParseTable::default();
        // Single state, single action cell, single Reduce action.
        table
            .action_table
            .push(vec![vec![Action::Reduce(RuleId(0))]]);

        assert!(
            resolver
                .get_vec_wrapper_action(StateId(0), &table, &grammar)
                .is_none()
        );
    }

    #[test]
    fn get_vec_wrapper_action_finds_direct_reduce_for_vec_contents_rule() {
        // Grammar where the LHS is named "items_vec_contents" and the rule has
        // an empty RHS — the classic vec-wrapper empty production pattern.
        let mut grammar = Grammar::new("vec".into());
        let items = SymbolId(11);
        grammar
            .rule_names
            .insert(items, "items_vec_contents".into());
        grammar.rules.insert(
            items,
            vec![Rule {
                lhs: items,
                rhs: vec![], // empty production
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(7),
            }],
        );

        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let mut resolver = VecWrapperResolver::new(&grammar, &ff);

        let mut table = ParseTable::default();
        table
            .action_table
            .push(vec![vec![Action::Reduce(RuleId(7))]]);

        let prod = resolver.get_vec_wrapper_action(StateId(0), &table, &grammar);
        assert_eq!(prod, Some(ProductionId(7)));
        // Subsequent calls hit the cache and return the same value.
        let cached = resolver.get_vec_wrapper_action(StateId(0), &table, &grammar);
        assert_eq!(cached, Some(ProductionId(7)));
    }

    #[test]
    fn get_vec_wrapper_action_ignores_non_empty_vec_contents_rule() {
        // Even with a `*_vec_contents` name, a non-empty RHS must not match.
        let mut grammar = Grammar::new("vec-nonempty".into());
        let items = SymbolId(12);
        let tok = SymbolId(2);
        grammar.tokens.insert(
            tok,
            Token {
                name: "tok".into(),
                pattern: TokenPattern::String("x".into()),
                fragile: false,
            },
        );
        grammar
            .rule_names
            .insert(items, "items_vec_contents".into());
        grammar.rules.insert(
            items,
            vec![Rule {
                lhs: items,
                rhs: vec![Symbol::Terminal(tok)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(3),
            }],
        );

        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let mut resolver = VecWrapperResolver::new(&grammar, &ff);

        let mut table = ParseTable::default();
        table
            .action_table
            .push(vec![vec![Action::Reduce(RuleId(3))]]);

        assert_eq!(
            resolver.get_vec_wrapper_action(StateId(0), &table, &grammar),
            None
        );
    }

    #[test]
    fn get_vec_wrapper_action_inspects_fork_actions() {
        // The empty vec-contents reduce is nested inside a Fork — the resolver
        // must still discover it.
        let mut grammar = Grammar::new("fork-vec".into());
        let items = SymbolId(13);
        grammar
            .rule_names
            .insert(items, "elements_vec_contents".into());
        grammar.rules.insert(
            items,
            vec![Rule {
                lhs: items,
                rhs: vec![],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(9),
            }],
        );

        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let mut resolver = VecWrapperResolver::new(&grammar, &ff);

        let mut table = ParseTable::default();
        table.action_table.push(vec![vec![Action::Fork(vec![
            Action::Shift(StateId(1)),
            Action::Reduce(RuleId(9)),
        ])]]);

        let prod = resolver.get_vec_wrapper_action(StateId(0), &table, &grammar);
        assert_eq!(prod, Some(ProductionId(9)));
    }

    #[test]
    fn get_vec_wrapper_action_ignores_shift_and_accept_actions() {
        let (grammar, _a, _s) = tiny_grammar();
        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let mut resolver = VecWrapperResolver::new(&grammar, &ff);

        let mut table = ParseTable::default();
        // Shift, Accept, Error, Recover should all fall through the `_ => {}` arm.
        table.action_table.push(vec![
            vec![Action::Shift(StateId(2))],
            vec![Action::Accept],
            vec![Action::Error],
            vec![Action::Recover],
        ]);

        assert!(
            resolver
                .get_vec_wrapper_action(StateId(0), &table, &grammar)
                .is_none()
        );
    }

    #[test]
    fn get_vec_wrapper_action_uses_cache_on_repeat_query() {
        // After the first computation the result must be served from the
        // wrapper_states cache without consulting the table again.
        let (grammar, _a, _s) = tiny_grammar();
        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let mut resolver = VecWrapperResolver::new(&grammar, &ff);
        let table = ParseTable::default();

        let _ = resolver.get_vec_wrapper_action(StateId(5), &table, &grammar);
        // Pre-poison the cache with a sentinel to prove the cache is consulted.
        resolver
            .wrapper_states
            .insert(StateId(5), Some(ProductionId(123)));
        assert_eq!(
            resolver.get_vec_wrapper_action(StateId(5), &table, &grammar),
            Some(ProductionId(123))
        );
    }

    #[test]
    fn resolve_returns_shift_when_lookahead_is_statement_starter() {
        let (grammar, kw, _other) = statement_grammar();
        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let resolver = VecWrapperResolver::new(&grammar, &ff);

        let actions = [Action::Shift(StateId(2)), Action::Reduce(RuleId(5))];
        let chosen = resolver.resolve(StateId(0), kw, &actions);
        assert_eq!(chosen, Some(Action::Shift(StateId(2))));
    }

    #[test]
    fn resolve_returns_reduce_when_lookahead_is_not_a_starter() {
        let (grammar, _kw, other) = statement_grammar();
        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let resolver = VecWrapperResolver::new(&grammar, &ff);

        let actions = [Action::Shift(StateId(2)), Action::Reduce(RuleId(5))];
        let chosen = resolver.resolve(StateId(0), other, &actions);
        assert_eq!(chosen, Some(Action::Reduce(RuleId(5))));
    }

    #[test]
    fn resolve_returns_none_without_a_shift_reduce_pair() {
        // Two Reduce actions — no shift to compare against, must yield None.
        let (grammar, _a, _s) = tiny_grammar();
        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let resolver = VecWrapperResolver::new(&grammar, &ff);

        let actions = [Action::Reduce(RuleId(1)), Action::Reduce(RuleId(2))];
        assert!(
            resolver
                .resolve(StateId(0), SymbolId(1), &actions)
                .is_none()
        );
    }

    #[test]
    fn resolve_ignores_non_shift_non_reduce_actions() {
        // Pair of Accept + Error — neither shift nor reduce — yields None.
        let (grammar, _a, _s) = tiny_grammar();
        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let resolver = VecWrapperResolver::new(&grammar, &ff);

        let actions = [Action::Accept, Action::Error];
        assert!(
            resolver
                .resolve(StateId(0), SymbolId(1), &actions)
                .is_none()
        );
    }

    /// Sanity check that exercises `RuntimeConflictResolver` via the trait
    /// object — proves the trait impl is wired up and dispatches correctly.
    #[test]
    fn resolver_usable_as_trait_object() {
        let (grammar, _kw, other) = statement_grammar();
        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let resolver: Box<dyn RuntimeConflictResolver> =
            Box::new(VecWrapperResolver::new(&grammar, &ff));

        let actions = [Action::Shift(StateId(9)), Action::Reduce(RuleId(0))];
        // `other` is not a statement starter — so reduce wins.
        assert_eq!(
            resolver.resolve(StateId(0), other, &actions),
            Some(Action::Reduce(RuleId(0)))
        );
    }
}
