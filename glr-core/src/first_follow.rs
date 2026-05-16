use crate::GLRError;
use adze_ir::*;
use fixedbitset::FixedBitSet;
use indexmap::IndexMap;

/// FIRST/FOLLOW sets computation for GLR parsing
#[derive(Debug, Clone)]
pub struct FirstFollowSets {
    pub(crate) first: IndexMap<SymbolId, FixedBitSet>,
    pub(crate) follow: IndexMap<SymbolId, FixedBitSet>,
    nullable: FixedBitSet,
    #[allow(dead_code)]
    symbol_count: usize,
}

impl FirstFollowSets {
    fn get_max_symbol_id(symbol: &Symbol) -> u16 {
        match symbol {
            Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => id.0,
            Symbol::Optional(inner) | Symbol::Repeat(inner) | Symbol::RepeatOne(inner) => {
                Self::get_max_symbol_id(inner)
            }
            Symbol::Choice(choices) => choices
                .iter()
                .map(Self::get_max_symbol_id)
                .max()
                .unwrap_or(0),
            Symbol::Sequence(seq) => seq.iter().map(Self::get_max_symbol_id).max().unwrap_or(0),
            Symbol::Epsilon => 0,
        }
    }

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
        // Clone and normalize the grammar if it contains complex symbols
        let normalized_grammar = {
            let mut cloned = grammar.clone();
            let _ = cloned.normalize(); // normalize returns Vec<Rule>, ignore it
            cloned
        };

        // Use the normalized grammar for computation
        let grammar = &normalized_grammar;
        // Find the maximum symbol ID to determine the size needed
        let max_rule_id = grammar.rules.keys().map(|id| id.0).max().unwrap_or(0);
        let max_token_id = grammar.tokens.keys().map(|id| id.0).max().unwrap_or(0);
        let max_external_id = grammar
            .externals
            .iter()
            .map(|e| e.symbol_id.0)
            .max()
            .unwrap_or(0);

        // Also check max symbol ID in all rule RHS
        let mut max_rhs_id = 0u16;
        for rules in grammar.rules.values() {
            for rule in rules {
                for symbol in &rule.rhs {
                    max_rhs_id = max_rhs_id.max(Self::get_max_symbol_id(symbol));
                }
            }
        }

        let symbol_count = (max_rule_id
            .max(max_token_id)
            .max(max_external_id)
            .max(max_rhs_id)
            + 2) as usize; // +2 to leave room for EOF and other potential symbols

        let mut first = IndexMap::new();
        let mut follow = IndexMap::new();
        let mut nullable = FixedBitSet::with_capacity(symbol_count);

        // Initialize sets
        for &symbol_id in grammar.rules.keys().chain(grammar.tokens.keys()) {
            first.insert(symbol_id, FixedBitSet::with_capacity(symbol_count));
            follow.insert(symbol_id, FixedBitSet::with_capacity(symbol_count));
        }

        // Compute FIRST sets
        let mut changed = true;
        while changed {
            changed = false;

            for rule in grammar.all_rules() {
                let lhs = rule.lhs;
                let mut rule_nullable = true;

                for symbol in &rule.rhs {
                    match symbol {
                        Symbol::Terminal(id) => {
                            if let Some(first_set) = first.get_mut(&lhs)
                                && !first_set.contains(id.0 as usize)
                            {
                                first_set.insert(id.0 as usize);
                                changed = true;
                            }
                            rule_nullable = false;
                            break;
                        }
                        Symbol::NonTerminal(id) | Symbol::External(id) => {
                            if let Some(symbol_first) = first.get(id).cloned()
                                && let Some(lhs_first) = first.get_mut(&lhs)
                            {
                                let old_len = lhs_first.count_ones(..);
                                lhs_first.union_with(&symbol_first);
                                if lhs_first.count_ones(..) > old_len {
                                    changed = true;
                                }
                            }

                            if !nullable.contains(id.0 as usize) {
                                rule_nullable = false;
                                break;
                            }
                        }
                        Symbol::Epsilon => {
                            // Epsilon doesn't contribute to FIRST set
                            // but keeps rule nullable
                        }
                        Symbol::Optional(_)
                        | Symbol::Repeat(_)
                        | Symbol::RepeatOne(_)
                        | Symbol::Choice(_)
                        | Symbol::Sequence(_) => {
                            // These should be normalized before FIRST/FOLLOW computation
                            return Err(GLRError::ComplexSymbolsNotNormalized {
                                operation: "FIRST/FOLLOW computation".to_string(),
                            });
                        }
                    }
                }

                if rule_nullable && !nullable.contains(lhs.0 as usize) {
                    nullable.insert(lhs.0 as usize);
                    changed = true;
                }
            }
        }

        // Compute FOLLOW sets
        // Initialize FOLLOW(start_symbol) with EOF
        if let Some(start_symbol) = grammar.start_symbol()
            && let Some(follow_set) = follow.get_mut(&start_symbol)
        {
            follow_set.insert(0); // EOF symbol
        }

        changed = true;
        while changed {
            changed = false;

            for rule in grammar.all_rules() {
                // Special handling for rules of the form A -> A B (left recursion)
                if rule.rhs.len() >= 2
                    && let (Symbol::NonTerminal(first_id), Symbol::NonTerminal(second_id)) =
                        (&rule.rhs[0], &rule.rhs[1])
                    && *first_id == rule.lhs
                {
                    // This is a left-recursive rule like Module_body_vec_contents -> Module_body_vec_contents Statement
                    // FIRST(Statement) should be in FOLLOW(Module_body_vec_contents)
                    if let Some(first_of_second) = first.get(second_id)
                        && let Some(follow_set) = follow.get_mut(&rule.lhs)
                    {
                        let old_len = follow_set.count_ones(..);
                        follow_set.union_with(first_of_second);
                        if follow_set.count_ones(..) > old_len {
                            changed = true;
                        }
                    }
                }

                for (i, symbol) in rule.rhs.iter().enumerate() {
                    if let Symbol::NonTerminal(id) | Symbol::External(id) = symbol {
                        // Add FIRST of remaining symbols to FOLLOW of current symbol
                        let remaining = &rule.rhs[i + 1..];
                        let first_of_remaining =
                            Self::first_of_sequence_static(remaining, &first, &nullable)?;

                        if let Some(follow_set) = follow.get_mut(id) {
                            let old_len = follow_set.count_ones(..);
                            follow_set.union_with(&first_of_remaining);
                            if follow_set.count_ones(..) > old_len {
                                changed = true;
                            }
                        }

                        // If remaining symbols are nullable, add FOLLOW of LHS
                        if Self::sequence_is_nullable(remaining, &nullable)
                            && let Some(lhs_follow) = follow.get(&rule.lhs).cloned()
                            && let Some(follow_set) = follow.get_mut(id)
                        {
                            let old_len = follow_set.count_ones(..);
                            follow_set.union_with(&lhs_follow);
                            if follow_set.count_ones(..) > old_len {
                                changed = true;
                            }
                        }
                    }
                }
            }
        }

        Ok(Self {
            first,
            follow,
            nullable,
            symbol_count,
        })
    }

    /// Get FIRST set of a sequence of symbols
    #[must_use = "computation result must be checked"]
    pub fn first_of_sequence(&self, symbols: &[Symbol]) -> Result<FixedBitSet, GLRError> {
        Self::first_of_sequence_static(symbols, &self.first, &self.nullable)
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
                Symbol::NonTerminal(id) | Symbol::External(id) => {
                    if let Some(symbol_first) = first.get(id) {
                        result.union_with(symbol_first);
                    }

                    if !nullable.contains(id.0 as usize) {
                        break;
                    }
                }
                Symbol::Optional(_)
                | Symbol::Repeat(_)
                | Symbol::RepeatOne(_)
                | Symbol::Choice(_)
                | Symbol::Sequence(_) => {
                    return Err(GLRError::ComplexSymbolsNotNormalized {
                        operation: "FIRST/FOLLOW computation".to_string(),
                    });
                }
            }
        }

        Ok(result)
    }

    fn sequence_is_nullable(symbols: &[Symbol], nullable: &FixedBitSet) -> bool {
        symbols.iter().all(|symbol| match symbol {
            Symbol::Terminal(_) => false,
            Symbol::NonTerminal(id) | Symbol::External(id) => nullable.contains(id.0 as usize),
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
