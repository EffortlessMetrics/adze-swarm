//! FIRST/FOLLOW set computation for normalized GLR grammars.

use crate::GLRError;
use adze_ir::*;
use fixedbitset::FixedBitSet;
use indexmap::IndexMap;

const FIRST_FOLLOW_OPERATION: &str = "FIRST/FOLLOW computation";

/// FIRST/FOLLOW sets computation for GLR parsing
#[derive(Debug, Clone)]
pub struct FirstFollowSets {
    pub(crate) first: IndexMap<SymbolId, FixedBitSet>,
    pub(crate) follow: IndexMap<SymbolId, FixedBitSet>,
    pub(crate) nullable: FixedBitSet,
    #[allow(dead_code)]
    symbol_count: usize,
}

#[derive(Debug)]
struct FirstFollowBuilder<'grammar> {
    grammar: &'grammar Grammar,
    first: IndexMap<SymbolId, FixedBitSet>,
    follow: IndexMap<SymbolId, FixedBitSet>,
    nullable: FixedBitSet,
    symbol_count: usize,
}

impl<'grammar> FirstFollowBuilder<'grammar> {
    fn new(grammar: &'grammar Grammar) -> Self {
        let symbol_count = symbol_count(grammar);
        let mut first = IndexMap::new();
        let mut follow = IndexMap::new();

        for symbol_id in grammar
            .rules
            .keys()
            .chain(grammar.tokens.keys())
            .copied()
            .chain(grammar.externals.iter().map(|external| external.symbol_id))
        {
            first.insert(symbol_id, FixedBitSet::with_capacity(symbol_count));
            follow.insert(symbol_id, FixedBitSet::with_capacity(symbol_count));
        }

        Self {
            grammar,
            first,
            follow,
            nullable: FixedBitSet::with_capacity(symbol_count),
            symbol_count,
        }
    }

    fn compute(mut self) -> Result<FirstFollowSets, GLRError> {
        self.compute_first_sets()?;
        self.compute_follow_sets()?;

        Ok(FirstFollowSets {
            first: self.first,
            follow: self.follow,
            nullable: self.nullable,
            symbol_count: self.symbol_count,
        })
    }

    fn compute_first_sets(&mut self) -> Result<(), GLRError> {
        let mut changed = true;
        while changed {
            changed = false;

            for rule in self.grammar.all_rules() {
                changed |= self.absorb_rule_first_set(rule)?;
            }
        }

        Ok(())
    }

    fn absorb_rule_first_set(&mut self, rule: &Rule) -> Result<bool, GLRError> {
        let mut changed = false;
        let mut rule_nullable = true;

        for symbol in &rule.rhs {
            match symbol {
                Symbol::Terminal(id) => {
                    changed |= insert_symbol(&mut self.first, rule.lhs, *id);
                    rule_nullable = false;
                    break;
                }
                Symbol::NonTerminal(id) => {
                    changed |= union_symbol_set(&mut self.first, rule.lhs, *id);

                    if !self.nullable.contains(id.0 as usize) {
                        rule_nullable = false;
                        break;
                    }
                }
                Symbol::External(id) => {
                    changed |= insert_symbol(&mut self.first, rule.lhs, *id);
                    rule_nullable = false;
                    break;
                }
                Symbol::Epsilon => {
                    // Epsilon doesn't contribute to FIRST set but keeps the rule nullable.
                }
                Symbol::Optional(_)
                | Symbol::Repeat(_)
                | Symbol::RepeatOne(_)
                | Symbol::Choice(_)
                | Symbol::Sequence(_) => return Err(complex_symbol_error()),
            }
        }

        if rule_nullable && !self.nullable.contains(rule.lhs.0 as usize) {
            self.nullable.insert(rule.lhs.0 as usize);
            changed = true;
        }

        Ok(changed)
    }

    fn compute_follow_sets(&mut self) -> Result<(), GLRError> {
        self.seed_start_follow_set();

        let mut changed = true;
        while changed {
            changed = false;

            for rule in self.grammar.all_rules() {
                changed |= self.propagate_left_recursive_follow(rule);
                changed |= self.propagate_rule_follow_sets(rule)?;
            }
        }

        Ok(())
    }

    fn seed_start_follow_set(&mut self) {
        if let Some(start_symbol) = self.grammar.start_symbol()
            && let Some(follow_set) = self.follow.get_mut(&start_symbol)
        {
            follow_set.insert(0); // EOF symbol
        }
    }

    fn propagate_left_recursive_follow(&mut self, rule: &Rule) -> bool {
        // Special handling for rules of the form A -> A B (left recursion).
        if rule.rhs.len() >= 2
            && let (Symbol::NonTerminal(first_id), Symbol::NonTerminal(second_id)) =
                (&rule.rhs[0], &rule.rhs[1])
            && *first_id == rule.lhs
        {
            return union_symbol_set_from(&mut self.follow, rule.lhs, &self.first, *second_id);
        }

        false
    }

    fn propagate_rule_follow_sets(&mut self, rule: &Rule) -> Result<bool, GLRError> {
        let mut changed = false;

        for (i, symbol) in rule.rhs.iter().enumerate() {
            let (Symbol::NonTerminal(id) | Symbol::External(id)) = symbol else {
                continue;
            };

            let remaining = &rule.rhs[i + 1..];
            let first_of_remaining =
                first_of_sequence_static(remaining, &self.first, &self.nullable)?;
            changed |= union_fixed_set(&mut self.follow, *id, &first_of_remaining);

            if sequence_is_nullable(remaining, &self.nullable)
                && let Some(lhs_follow) = self.follow.get(&rule.lhs).cloned()
            {
                changed |= union_fixed_set(&mut self.follow, *id, &lhs_follow);
            }
        }

        Ok(changed)
    }
}

impl FirstFollowSets {
    /// Compute FIRST/FOLLOW sets for the given grammar with automatic normalization.
    ///
    /// This method automatically normalizes complex symbols (Repeat, Choice, etc.) before computation.
    ///
    /// # Examples
    ///
    /// ```
    /// use adze_glr_core::FirstFollowSets;
    /// use adze_ir::*;
    ///
    /// // Build a tiny grammar: E → a | E '+' E
    /// let mut grammar = Grammar::new("expr".into());
    /// let a = SymbolId(1);
    /// let plus = SymbolId(2);
    /// let e = SymbolId(10);
    ///
    /// grammar.tokens.insert(a, Token { name: "a".into(), pattern: TokenPattern::String("a".into()), fragile: false });
    /// grammar.tokens.insert(plus, Token { name: "+".into(), pattern: TokenPattern::String("+".into()), fragile: false });
    /// grammar.rule_names.insert(e, "E".into());
    /// grammar.rules.insert(e, vec![
    ///     Rule { lhs: e, rhs: vec![Symbol::Terminal(a)], precedence: None, associativity: None, fields: vec![], production_id: ProductionId(0) },
    ///     Rule { lhs: e, rhs: vec![Symbol::NonTerminal(e), Symbol::Terminal(plus), Symbol::NonTerminal(e)], precedence: None, associativity: None, fields: vec![], production_id: ProductionId(1) },
    /// ]);
    ///
    /// let ff = FirstFollowSets::compute_normalized(&mut grammar).unwrap();
    /// // 'a' (SymbolId 1) should be in FIRST(E)
    /// assert!(ff.first(e).unwrap().contains(a.0 as usize));
    /// ```
    #[must_use = "computation result must be checked"]
    pub fn compute_normalized(grammar: &mut Grammar) -> Result<Self, GLRError> {
        // Normalize the grammar to convert complex symbols to simple rules
        grammar.normalize();

        // Now compute FIRST/FOLLOW sets on the normalized grammar
        Self::compute(grammar)
    }

    /// Compute FIRST/FOLLOW sets for the given grammar.
    ///
    /// # Examples
    ///
    /// ```
    /// use adze_glr_core::FirstFollowSets;
    /// use adze_ir::*;
    ///
    /// let mut grammar = Grammar::new("simple".into());
    /// let a = SymbolId(1);
    /// let s = SymbolId(10);
    ///
    /// grammar.tokens.insert(a, Token { name: "a".into(), pattern: TokenPattern::String("a".into()), fragile: false });
    /// grammar.rule_names.insert(s, "S".into());
    /// grammar.rules.insert(s, vec![
    ///     Rule { lhs: s, rhs: vec![Symbol::Terminal(a)], precedence: None, associativity: None, fields: vec![], production_id: ProductionId(0) },
    /// ]);
    ///
    /// let ff = FirstFollowSets::compute(&grammar).unwrap();
    /// assert!(ff.first(s).unwrap().contains(a.0 as usize));
    /// assert!(!ff.is_nullable(s));
    /// ```
    #[must_use = "computation result must be checked"]
    pub fn compute(grammar: &Grammar) -> Result<Self, GLRError> {
        let mut normalized_grammar = grammar.clone();
        let _ = normalized_grammar.normalize();

        FirstFollowBuilder::new(&normalized_grammar).compute()
    }

    /// Get FIRST set of a sequence of symbols
    #[must_use = "computation result must be checked"]
    pub fn first_of_sequence(&self, symbols: &[Symbol]) -> Result<FixedBitSet, GLRError> {
        first_of_sequence_static(symbols, &self.first, &self.nullable)
    }

    /// Get FIRST set for a symbol
    pub fn first(&self, symbol: SymbolId) -> Option<&FixedBitSet> {
        self.first.get(&symbol)
    }

    /// Get FOLLOW set for a symbol
    pub fn follow(&self, symbol: SymbolId) -> Option<&FixedBitSet> {
        self.follow.get(&symbol)
    }

    /// Check if a symbol is nullable
    pub fn is_nullable(&self, symbol: SymbolId) -> bool {
        self.nullable.contains(symbol.0 as usize)
    }
}

fn symbol_count(grammar: &Grammar) -> usize {
    let max_rule_id = grammar.rules.keys().map(|id| id.0).max().unwrap_or(0);
    let max_token_id = grammar.tokens.keys().map(|id| id.0).max().unwrap_or(0);
    let max_external_id = grammar
        .externals
        .iter()
        .map(|external| external.symbol_id.0)
        .max()
        .unwrap_or(0);
    let max_rhs_id = grammar
        .rules
        .values()
        .flatten()
        .flat_map(|rule| rule.rhs.iter())
        .map(max_symbol_id)
        .max()
        .unwrap_or(0);

    (max_rule_id
        .max(max_token_id)
        .max(max_external_id)
        .max(max_rhs_id)
        + 2) as usize
}

fn max_symbol_id(symbol: &Symbol) -> u16 {
    match symbol {
        Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => id.0,
        Symbol::Optional(inner) | Symbol::Repeat(inner) | Symbol::RepeatOne(inner) => {
            max_symbol_id(inner)
        }
        Symbol::Choice(choices) => choices.iter().map(max_symbol_id).max().unwrap_or(0),
        Symbol::Sequence(seq) => seq.iter().map(max_symbol_id).max().unwrap_or(0),
        Symbol::Epsilon => 0,
    }
}

fn first_of_sequence_static(
    symbols: &[Symbol],
    first: &IndexMap<SymbolId, FixedBitSet>,
    nullable: &FixedBitSet,
) -> Result<FixedBitSet, GLRError> {
    let mut result = FixedBitSet::with_capacity(nullable.len());

    for symbol in symbols {
        match symbol {
            Symbol::Terminal(id) => {
                result.insert(id.0 as usize);
                break;
            }
            Symbol::Epsilon => {
                // Epsilon doesn't contribute to FIRST set, continue to next symbol
            }
            Symbol::NonTerminal(id) => {
                if let Some(symbol_first) = first.get(id) {
                    result.union_with(symbol_first);
                }

                if !nullable.contains(id.0 as usize) {
                    break;
                }
            }
            Symbol::External(id) => {
                result.insert(id.0 as usize);
                break;
            }
            Symbol::Optional(_)
            | Symbol::Repeat(_)
            | Symbol::RepeatOne(_)
            | Symbol::Choice(_)
            | Symbol::Sequence(_) => return Err(complex_symbol_error()),
        }
    }

    Ok(result)
}

fn sequence_is_nullable(symbols: &[Symbol], nullable: &FixedBitSet) -> bool {
    symbols.iter().all(|symbol| match symbol {
        Symbol::Terminal(_) | Symbol::External(_) => false,
        Symbol::NonTerminal(id) => nullable.contains(id.0 as usize),
        Symbol::Epsilon => true,
        Symbol::Optional(_)
        | Symbol::Repeat(_)
        | Symbol::RepeatOne(_)
        | Symbol::Choice(_)
        | Symbol::Sequence(_) => {
            panic!("Complex symbols should be normalized before FIRST/FOLLOW computation");
        }
    })
}

fn insert_symbol(
    sets: &mut IndexMap<SymbolId, FixedBitSet>,
    destination: SymbolId,
    symbol: SymbolId,
) -> bool {
    let Some(destination_set) = sets.get_mut(&destination) else {
        return false;
    };

    if destination_set.contains(symbol.0 as usize) {
        return false;
    }

    destination_set.insert(symbol.0 as usize);
    true
}

fn union_symbol_set(
    sets: &mut IndexMap<SymbolId, FixedBitSet>,
    destination: SymbolId,
    source: SymbolId,
) -> bool {
    let Some(source_set) = sets.get(&source).cloned() else {
        return false;
    };

    union_fixed_set(sets, destination, &source_set)
}

fn union_symbol_set_from(
    destination_sets: &mut IndexMap<SymbolId, FixedBitSet>,
    destination: SymbolId,
    source_sets: &IndexMap<SymbolId, FixedBitSet>,
    source: SymbolId,
) -> bool {
    let Some(source_set) = source_sets.get(&source) else {
        return false;
    };

    union_fixed_set(destination_sets, destination, source_set)
}

fn union_fixed_set(
    sets: &mut IndexMap<SymbolId, FixedBitSet>,
    destination: SymbolId,
    source: &FixedBitSet,
) -> bool {
    let Some(destination_set) = sets.get_mut(&destination) else {
        return false;
    };

    let old_len = destination_set.count_ones(..);
    destination_set.union_with(source);
    destination_set.count_ones(..) > old_len
}

fn complex_symbol_error() -> GLRError {
    GLRError::ComplexSymbolsNotNormalized {
        operation: FIRST_FOLLOW_OPERATION.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grammar_with_rules(
        rules: impl IntoIterator<Item = (SymbolId, &'static str, Vec<Vec<Symbol>>)>,
    ) -> Grammar {
        let mut grammar = Grammar::new("first_follow_test".to_string());

        for (lhs, name, productions) in rules {
            grammar.rule_names.insert(lhs, name.to_string());
            for (index, rhs) in productions.into_iter().enumerate() {
                grammar.add_rule(Rule {
                    lhs,
                    rhs,
                    precedence: None,
                    associativity: None,
                    fields: vec![],
                    production_id: ProductionId(index as u16),
                });
            }
        }

        grammar
    }

    fn add_token(grammar: &mut Grammar, id: SymbolId, name: &str) {
        grammar.tokens.insert(
            id,
            Token {
                name: name.to_string(),
                pattern: TokenPattern::String(name.to_string()),
                fragile: false,
            },
        );
    }

    fn assert_set_contains_exactly(set: &FixedBitSet, expected: &[SymbolId]) {
        let mut actual = set.ones().map(|id| SymbolId(id as u16)).collect::<Vec<_>>();
        let mut expected = expected.to_vec();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_nullable_prefix_allows_first_to_continue_to_next_symbol() {
        let s = SymbolId(10);
        let maybe_a = SymbolId(11);
        let a = SymbolId(1);
        let b = SymbolId(2);

        let mut grammar = grammar_with_rules([
            (
                s,
                "Program",
                vec![vec![Symbol::NonTerminal(maybe_a), Symbol::Terminal(b)]],
            ),
            (
                maybe_a,
                "MaybeA",
                vec![vec![Symbol::Terminal(a)], vec![Symbol::Epsilon]],
            ),
        ]);
        add_token(&mut grammar, a, "a");
        add_token(&mut grammar, b, "b");

        let sets = FirstFollowSets::compute(&grammar).expect("FIRST/FOLLOW computation succeeds");

        assert!(sets.is_nullable(maybe_a));
        assert!(!sets.is_nullable(s));
        assert_set_contains_exactly(sets.first(s).unwrap(), &[a, b]);
        assert_set_contains_exactly(sets.first(maybe_a).unwrap(), &[a]);
    }

    #[test]
    fn test_follow_propagates_first_of_suffix_and_lhs_follow_when_suffix_nullable() {
        let s = SymbolId(10);
        let item = SymbolId(11);
        let maybe_tail = SymbolId(12);
        let tail = SymbolId(1);

        let mut grammar = grammar_with_rules([
            (
                s,
                "Program",
                vec![vec![
                    Symbol::NonTerminal(item),
                    Symbol::NonTerminal(maybe_tail),
                ]],
            ),
            (item, "Item", vec![vec![Symbol::Epsilon]]),
            (
                maybe_tail,
                "MaybeTail",
                vec![vec![Symbol::Terminal(tail)], vec![Symbol::Epsilon]],
            ),
        ]);
        add_token(&mut grammar, tail, "tail");

        let sets = FirstFollowSets::compute(&grammar).expect("FIRST/FOLLOW computation succeeds");

        assert_set_contains_exactly(sets.follow(s).unwrap(), &[SymbolId(0)]);
        assert_set_contains_exactly(sets.follow(item).unwrap(), &[SymbolId(0), tail]);
        assert_set_contains_exactly(sets.follow(maybe_tail).unwrap(), &[SymbolId(0)]);
    }

    #[test]
    fn test_first_of_sequence_skips_epsilon_and_nullable_nonterminals() {
        let nullable = SymbolId(10);
        let a = SymbolId(1);
        let b = SymbolId(2);

        let mut grammar = grammar_with_rules([(nullable, "Program", vec![vec![Symbol::Epsilon]])]);
        add_token(&mut grammar, a, "a");
        add_token(&mut grammar, b, "b");

        let sets = FirstFollowSets::compute(&grammar).expect("FIRST/FOLLOW computation succeeds");
        let first = sets
            .first_of_sequence(&[
                Symbol::Epsilon,
                Symbol::NonTerminal(nullable),
                Symbol::Terminal(b),
            ])
            .expect("normalized sequence is accepted");

        assert!(sets.is_nullable(nullable));
        assert_set_contains_exactly(&first, &[b]);
    }

    #[test]
    fn test_complex_symbols_are_normalized_before_computation() {
        let s = SymbolId(10);
        let a = SymbolId(1);
        let b = SymbolId(2);

        let mut grammar = grammar_with_rules([(
            s,
            "Program",
            vec![vec![
                Symbol::Optional(Box::new(Symbol::Terminal(a))),
                Symbol::Choice(vec![Symbol::Terminal(a), Symbol::Terminal(b)]),
            ]],
        )]);
        add_token(&mut grammar, a, "a");
        add_token(&mut grammar, b, "b");

        let sets = FirstFollowSets::compute(&grammar).expect("normalization permits computation");

        assert!(!sets.is_nullable(s));
        assert_set_contains_exactly(sets.first(s).unwrap(), &[a, b]);
    }

    #[test]
    fn test_external_symbols_participate_in_first_and_follow_sets() {
        let s = SymbolId(10);
        let external = SymbolId(20);
        let trailing = SymbolId(1);

        let mut grammar = grammar_with_rules([(
            s,
            "Program",
            vec![vec![Symbol::External(external), Symbol::Terminal(trailing)]],
        )]);
        grammar.externals.push(ExternalToken {
            name: "indent".to_string(),
            symbol_id: external,
        });
        add_token(&mut grammar, trailing, "trailing");

        let sets = FirstFollowSets::compute(&grammar).expect("FIRST/FOLLOW computation succeeds");

        assert_set_contains_exactly(sets.first(s).unwrap(), &[external]);
        assert_set_contains_exactly(sets.follow(external).unwrap(), &[trailing]);
    }
}
