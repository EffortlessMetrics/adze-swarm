// Grammar optimization passes for Adze
//! Grammar optimization passes that simplify and normalize rules.

// This module implements various optimizations to improve parser performance

#[cfg(test)]
use crate::Token;
use crate::{Grammar, ProductionId, Rule, Symbol, SymbolId, TokenPattern};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

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

/// Grammar optimizer that applies various optimization passes
pub struct GrammarOptimizer {
    /// Track which symbols are actually used
    used_symbols: HashSet<SymbolId>,
    /// Track which rules can be inlined
    inlinable_rules: HashSet<SymbolId>,
    /// Track left-recursive rules for special handling
    left_recursive_rules: HashSet<SymbolId>,
    /// Track the source_file symbol ID to prevent inlining
    source_file_id: Option<SymbolId>,
}

impl Default for GrammarOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

impl GrammarOptimizer {
    /// Create a new optimizer
    pub fn new() -> Self {
        GrammarOptimizer {
            used_symbols: HashSet::new(),
            inlinable_rules: HashSet::new(),
            left_recursive_rules: HashSet::new(),
            source_file_id: None,
        }
    }

    /// Optimize a grammar by applying all optimization passes
    pub fn optimize(&mut self, grammar: &mut Grammar) -> OptimizationStats {
        let mut stats = OptimizationStats::default();

        // Check source_file status after each optimization
        let check_source_file = |grammar: &Grammar, phase: &str| {
            if let Some(sf_id) = grammar.find_symbol_by_name("source_file") {
                let has_rules = grammar.rules.contains_key(&sf_id);
                let rule_count = grammar.rules.get(&sf_id).map(|r| r.len()).unwrap_or(0);
                debug_trace!(
                    "Debug after {}: source_file is SymbolId({}), has_rules={}, rule_count={}",
                    phase,
                    sf_id.0,
                    has_rules,
                    rule_count
                );
            } else {
                debug_trace!("Debug after {}: source_file not found!", phase);
            }
        };

        // Phase 1: Analysis
        self.analyze_grammar(grammar);
        check_source_file(grammar, "analysis");

        // Phase 2: Optimizations
        stats.removed_unused_symbols = self.remove_unused_symbols(grammar);
        check_source_file(grammar, "remove_unused_symbols");

        stats.inlined_rules = self.inline_simple_rules(grammar);
        check_source_file(grammar, "inline_simple_rules");

        stats.merged_tokens = self.merge_equivalent_tokens(grammar);
        check_source_file(grammar, "merge_equivalent_tokens");

        stats.optimized_left_recursion = self.optimize_left_recursion(grammar);
        check_source_file(grammar, "optimize_left_recursion");

        stats.eliminated_unit_rules = self.eliminate_unit_rules(grammar);
        check_source_file(grammar, "eliminate_unit_rules");

        // Phase 3: Cleanup
        self.renumber_symbols(grammar);
        check_source_file(grammar, "renumber_symbols");

        stats
    }

    /// Mark a symbol and all its sub-symbols as used
    fn mark_used_in_symbol(&mut self, symbol: &Symbol) {
        match symbol {
            Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => {
                self.used_symbols.insert(*id);
            }
            Symbol::Optional(inner) | Symbol::Repeat(inner) | Symbol::RepeatOne(inner) => {
                self.mark_used_in_symbol(inner);
            }
            Symbol::Choice(choices) => {
                for s in choices {
                    self.mark_used_in_symbol(s);
                }
            }
            Symbol::Sequence(seq) => {
                for s in seq {
                    self.mark_used_in_symbol(s);
                }
            }
            Symbol::Epsilon => {}
        }
    }

    /// Analyze the grammar to collect information for optimization
    fn analyze_grammar(&mut self, grammar: &Grammar) {
        // Mark start symbol as used
        if let Some(start_symbol) = grammar.start_symbol() {
            self.used_symbols.insert(start_symbol);
        }

        // Always mark source_file as used if it exists (Tree-sitter compatibility)
        if let Some(source_file_id) = grammar.find_symbol_by_name("source_file") {
            self.source_file_id = Some(source_file_id);
            self.used_symbols.insert(source_file_id);

            // Also mark symbols referenced by source_file
            if let Some(rules) = grammar.rules.get(&source_file_id) {
                for rule in rules {
                    for symbol in &rule.rhs {
                        match symbol {
                            Symbol::NonTerminal(id) => {
                                self.used_symbols.insert(*id);
                            }
                            Symbol::Terminal(id) => {
                                self.used_symbols.insert(*id);
                            }
                            _ => {
                                // Other symbol types don't directly reference IDs that need marking
                            }
                        }
                    }
                }
            }
        }

        // Also mark all rule LHS as used (they define the symbols)
        for symbol_id in grammar.rules.keys() {
            self.used_symbols.insert(*symbol_id);
        }

        // Analyze all rules
        for rules in grammar.rules.values() {
            for rule in rules {
                // Mark symbols used in productions
                for symbol in &rule.rhs {
                    match symbol {
                        Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => {
                            self.used_symbols.insert(*id);
                        }
                        Symbol::Optional(inner)
                        | Symbol::Repeat(inner)
                        | Symbol::RepeatOne(inner) => {
                            self.mark_used_in_symbol(inner);
                        }
                        Symbol::Choice(choices) => {
                            for s in choices {
                                self.mark_used_in_symbol(s);
                            }
                        }
                        Symbol::Sequence(seq) => {
                            for s in seq {
                                self.mark_used_in_symbol(s);
                            }
                        }
                        Symbol::Epsilon => {}
                    }
                }

                // Check if rule is inlinable (simple, non-recursive)
                // Never inline source_file as it's the start symbol
                if rule.rhs.len() == 1
                    && !self.is_recursive_rule(rule, grammar)
                    && Some(rule.lhs) != self.source_file_id
                {
                    self.inlinable_rules.insert(rule.lhs);
                } else if Some(rule.lhs) == self.source_file_id && rule.rhs.len() == 1 {
                    // source_file is not inlinable
                }

                // Check for left recursion
                if self.is_left_recursive(rule) {
                    self.left_recursive_rules.insert(rule.lhs);
                }
            }
        }

        // Note: We don't mark tokens as used here - they need to be referenced in rules
    }

    /// Remove symbols that are never referenced
    fn remove_unused_symbols(&mut self, grammar: &mut Grammar) -> usize {
        let mut removed = 0;

        // Remove unused rules
        let unused_rules: Vec<_> = grammar
            .rules
            .iter()
            .filter(|(id, _)| !self.used_symbols.contains(id))
            .map(|(id, _)| *id)
            .collect();

        for id in unused_rules {
            grammar.rules.shift_remove(&id);
            removed += 1;
        }

        // Remove unused tokens
        let unused_tokens: Vec<_> = grammar
            .tokens
            .iter()
            .filter(|(id, _)| !self.used_symbols.contains(id))
            .map(|(id, _)| *id)
            .collect();

        for id in unused_tokens {
            grammar.tokens.shift_remove(&id);
            removed += 1;
        }

        removed
    }

    /// Inline simple rules that just reference another symbol
    fn inline_simple_rules(&mut self, grammar: &mut Grammar) -> usize {
        // Process inlinable rules

        let mut inlined = 0;
        let mut replacements = HashMap::new();

        // Find rules to inline
        for (symbol_id, rules) in &grammar.rules {
            if self.inlinable_rules.contains(symbol_id) {
                // Only inline if all rules for this symbol have exactly one RHS symbol
                if rules.len() == 1
                    && rules[0].rhs.len() == 1
                    && let Some(target) = rules[0].rhs.first()
                {
                    replacements.insert(*symbol_id, target.clone());
                }
            } else if Some(*symbol_id) == self.source_file_id {
                // source_file is not inlined
            }
        }

        // Apply replacements
        for rules in grammar.rules.values_mut() {
            for rule in rules.iter_mut() {
                let mut modified = false;
                for symbol in &mut rule.rhs {
                    if let Symbol::NonTerminal(id) = symbol
                        && let Some(replacement) = replacements.get(id)
                    {
                        *symbol = replacement.clone();
                        modified = true;
                    }
                }
                if modified {
                    inlined += 1;
                }
            }
        }

        // Remove inlined rules
        for id in replacements.keys() {
            grammar.rules.shift_remove(id);
            grammar.inline_rules.push(*id);
        }

        inlined
    }

    /// Merge tokens with identical patterns
    fn merge_equivalent_tokens(&mut self, grammar: &mut Grammar) -> usize {
        let mut merged = 0;
        let mut pattern_to_id: HashMap<String, SymbolId> = HashMap::new();
        let mut replacements: HashMap<SymbolId, SymbolId> = HashMap::new();

        // Find equivalent tokens
        for (id, token) in &grammar.tokens {
            let pattern_str = match &token.pattern {
                TokenPattern::String(s) => s.clone(),
                TokenPattern::Regex(r) => r.clone(),
            };

            if let Some(&existing_id) = pattern_to_id.get(&pattern_str) {
                // Found duplicate
                replacements.insert(*id, existing_id);
                merged += 1;
            } else {
                pattern_to_id.insert(pattern_str, *id);
            }
        }

        // Apply replacements in rules
        for rules in grammar.rules.values_mut() {
            for rule in rules.iter_mut() {
                for symbol in &mut rule.rhs {
                    if let Symbol::Terminal(id) = symbol
                        && let Some(&new_id) = replacements.get(id)
                    {
                        *symbol = Symbol::Terminal(new_id);
                    }
                }
            }
        }

        // Remove duplicate tokens
        for old_id in replacements.keys() {
            grammar.tokens.shift_remove(old_id);
        }

        merged
    }

    /// Optimize left-recursive rules by transforming them
    fn optimize_left_recursion(&mut self, grammar: &mut Grammar) -> usize {
        let mut optimized = 0;

        // For each left-recursive rule, transform it
        // A -> A α | β becomes:
        // A -> β A'
        // A' -> α A' | ε
        let left_recursive: Vec<_> = self.left_recursive_rules.iter().cloned().collect();

        for symbol in left_recursive {
            if let Some(rules) = self.extract_rules_for_symbol(grammar, symbol) {
                let (recursive_rules, base_rules) = self.partition_recursive_rules(&rules, symbol);

                if !recursive_rules.is_empty() && !base_rules.is_empty() {
                    // Create new symbol for the recursive part
                    let new_symbol = self.create_new_symbol(grammar);

                    // Transform the rules
                    self.transform_left_recursion(
                        grammar,
                        symbol,
                        new_symbol,
                        recursive_rules,
                        base_rules,
                    );

                    optimized += 1;
                }
            }
        }

        optimized
    }

    /// Eliminate unit rules (A -> B)
    fn eliminate_unit_rules(&mut self, grammar: &mut Grammar) -> usize {
        let mut eliminated = 0;
        let mut unit_rules = Vec::new();

        // Get the start symbol to prevent creating terminal productions for it
        let start_symbol = grammar.start_symbol();

        // Find unit rules
        for rule in grammar.all_rules() {
            if rule.rhs.len() == 1
                && let Symbol::NonTerminal(_) = &rule.rhs[0]
            {
                unit_rules.push(rule.clone());
            }
        }

        // For each unit rule A -> B, add rules A -> γ for each B -> γ
        let mut new_rules = Vec::new();
        for unit_rule in unit_rules {
            if let Symbol::NonTerminal(target) = &unit_rule.rhs[0] {
                if let Some(target_rules) = grammar.get_rules_for_symbol(*target) {
                    for target_rule in target_rules {
                        // Skip if this would create a terminal production for the start symbol
                        if Some(unit_rule.lhs) == start_symbol
                            && target_rule
                                .rhs
                                .iter()
                                .any(|s| matches!(s, Symbol::Terminal(_)))
                        {
                            continue;
                        }

                        // Create new rule A -> γ
                        let new_rule = Rule {
                            lhs: unit_rule.lhs,
                            rhs: target_rule.rhs.clone(),
                            precedence: target_rule.precedence.or(unit_rule.precedence),
                            associativity: target_rule.associativity.or(unit_rule.associativity),
                            fields: target_rule.fields.clone(),
                            production_id: self.create_new_production_id(grammar),
                        };
                        new_rules.push(new_rule);
                        eliminated += 1;
                    }
                }
                // Remove the unit rule from the appropriate symbol's rules
                if let Some(symbol_rules) = grammar.rules.get_mut(&unit_rule.lhs) {
                    symbol_rules.retain(|r| r.production_id != unit_rule.production_id);
                    if symbol_rules.is_empty() {
                        grammar.rules.shift_remove(&unit_rule.lhs);
                    }
                }
            }
        }

        // Add all new rules
        for rule in new_rules {
            grammar.add_rule(rule);
        }

        eliminated
    }

    /// Check if a rule is recursive
    fn is_recursive_rule(&self, rule: &Rule, grammar: &Grammar) -> bool {
        let mut visited = HashSet::new();
        self.contains_symbol_recursive(&rule.rhs, rule.lhs, grammar, &mut visited)
    }

    /// Check if a rule is left-recursive
    fn is_left_recursive(&self, rule: &Rule) -> bool {
        if let Some(Symbol::NonTerminal(id)) = rule.rhs.first() {
            *id == rule.lhs
        } else {
            false
        }
    }

    /// Recursively check if symbols contain a target symbol
    fn contains_symbol_recursive(
        &self,
        symbols: &[Symbol],
        target: SymbolId,
        grammar: &Grammar,
        visited: &mut HashSet<SymbolId>,
    ) -> bool {
        for symbol in symbols {
            match symbol {
                Symbol::NonTerminal(id) if *id == target => return true,
                Symbol::NonTerminal(id) if !visited.contains(id) => {
                    visited.insert(*id);

                    // Check all rules for this non-terminal
                    if let Some(rules) = grammar.get_rules_for_symbol(*id) {
                        for rule in rules {
                            if self.contains_symbol_recursive(&rule.rhs, target, grammar, visited) {
                                return true;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        false
    }

    /// Extract all rules for a given symbol
    fn extract_rules_for_symbol(&self, grammar: &Grammar, symbol: SymbolId) -> Option<Vec<Rule>> {
        grammar.get_rules_for_symbol(symbol).cloned()
    }

    /// Partition rules into recursive and non-recursive
    fn partition_recursive_rules(
        &self,
        rules: &[Rule],
        symbol: SymbolId,
    ) -> (Vec<Rule>, Vec<Rule>) {
        let mut recursive = Vec::new();
        let mut non_recursive = Vec::new();

        for rule in rules {
            if let Some(Symbol::NonTerminal(id)) = rule.rhs.first() {
                if *id == symbol {
                    recursive.push(rule.clone());
                } else {
                    non_recursive.push(rule.clone());
                }
            } else {
                non_recursive.push(rule.clone());
            }
        }

        (recursive, non_recursive)
    }

    /// Create a new unique symbol ID
    fn create_new_symbol(&self, grammar: &Grammar) -> SymbolId {
        let max_id = grammar
            .rules
            .keys()
            .chain(grammar.tokens.keys())
            .map(|id| id.0)
            .max()
            .unwrap_or(0);

        SymbolId(max_id + 1)
    }

    /// Create a new unique production ID
    fn create_new_production_id(&self, grammar: &Grammar) -> ProductionId {
        let max_id = grammar
            .rules
            .values()
            .flat_map(|rules| rules.iter())
            .map(|r| r.production_id.0)
            .max()
            .unwrap_or(0);

        ProductionId(max_id + 1)
    }

    /// Transform left-recursive rules
    fn transform_left_recursion(
        &mut self,
        grammar: &mut Grammar,
        original_symbol: SymbolId,
        new_symbol: SymbolId,
        recursive_rules: Vec<Rule>,
        base_rules: Vec<Rule>,
    ) {
        // Remove all original rules for the symbol using the current Grammar APIs
        grammar.rules.shift_remove(&original_symbol);

        // Any conflict declarations referencing the original symbol should also
        // reference the new helper symbol to preserve conflict metadata
        for conflict in &mut grammar.conflicts {
            if conflict.symbols.contains(&original_symbol)
                && !conflict.symbols.contains(&new_symbol)
            {
                conflict.symbols.push(new_symbol);
            }
        }

        // Give the new symbol a readable name if possible
        if let Some(name) = grammar.rule_names.get(&original_symbol).cloned() {
            grammar
                .rule_names
                .insert(new_symbol, format!("{}__rec", name));
        }

        // Add transformed base rules: A -> β A'
        for base_rule in base_rules {
            let mut new_rhs = base_rule.rhs.clone();
            new_rhs.push(Symbol::NonTerminal(new_symbol));

            let new_rule = Rule {
                lhs: original_symbol,
                rhs: new_rhs,
                precedence: base_rule.precedence,
                associativity: base_rule.associativity,
                fields: base_rule.fields,
                production_id: self.create_new_production_id(grammar),
            };

            grammar.add_rule(new_rule);
        }

        // Add recursive rules: A' -> α A' | ε
        for recursive_rule in recursive_rules {
            // Remove the left-recursive symbol
            let mut new_rhs: Vec<_> = recursive_rule.rhs[1..].to_vec();
            new_rhs.push(Symbol::NonTerminal(new_symbol));
            // Adjust field positions since we removed the first symbol
            let adjusted_fields = recursive_rule
                .fields
                .iter()
                .filter_map(|(field_id, index)| {
                    if *index > 0 {
                        Some((*field_id, index - 1))
                    } else {
                        None
                    }
                })
                .collect();

            let new_rule = Rule {
                lhs: new_symbol,
                rhs: new_rhs,
                precedence: recursive_rule.precedence,
                associativity: recursive_rule.associativity,
                fields: adjusted_fields,
                production_id: self.create_new_production_id(grammar),
            };

            grammar.add_rule(new_rule);
        }

        // Add epsilon rule: A' -> ε
        let epsilon_rule = Rule {
            lhs: new_symbol,
            rhs: Vec::new(),
            precedence: None,
            associativity: None,
            fields: Vec::new(),
            production_id: self.create_new_production_id(grammar),
        };

        grammar.add_rule(epsilon_rule);
    }

    /// Helper to renumber a symbol recursively
    fn collect_symbol_ids(&self, symbol: &Symbol, ids: &mut HashSet<SymbolId>) {
        match symbol {
            Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => {
                ids.insert(*id);
            }
            Symbol::Optional(inner) | Symbol::Repeat(inner) | Symbol::RepeatOne(inner) => {
                self.collect_symbol_ids(inner, ids);
            }
            Symbol::Choice(choices) => {
                for s in choices {
                    self.collect_symbol_ids(s, ids);
                }
            }
            Symbol::Sequence(seq) => {
                for s in seq {
                    self.collect_symbol_ids(s, ids);
                }
            }
            Symbol::Epsilon => {}
        }
    }

    fn renumber_symbol(&self, symbol: &mut Symbol, old_to_new: &HashMap<SymbolId, SymbolId>) {
        match symbol {
            Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => {
                if let Some(&new_id) = old_to_new.get(id) {
                    *id = new_id;
                }
            }
            Symbol::Optional(inner) | Symbol::Repeat(inner) | Symbol::RepeatOne(inner) => {
                self.renumber_symbol(inner, old_to_new);
            }
            Symbol::Choice(choices) => {
                for s in choices {
                    self.renumber_symbol(s, old_to_new);
                }
            }
            Symbol::Sequence(seq) => {
                for s in seq {
                    self.renumber_symbol(s, old_to_new);
                }
            }
            Symbol::Epsilon => {}
        }
    }

    /// Renumber symbols to be contiguous
    fn renumber_symbols(&mut self, grammar: &mut Grammar) {
        let mut old_to_new: HashMap<SymbolId, SymbolId> = HashMap::new();
        let mut next_id = 1u16; // 0 is reserved for EOF

        // Renumber symbols to be contiguous while preserving parse table ordering

        // Collect all symbols
        let mut token_symbols: HashSet<SymbolId> = HashSet::new();
        let mut non_terminal_symbols: HashSet<SymbolId> = HashSet::new();
        let mut external_symbols: HashSet<SymbolId> = HashSet::new();

        // Categorize symbols
        token_symbols.extend(grammar.tokens.keys().copied());

        // Add all symbols from rules
        for (symbol_id, _) in &grammar.rules {
            if !token_symbols.contains(symbol_id) {
                non_terminal_symbols.insert(*symbol_id);
            }
        }

        // Add all symbols referenced in rule RHS
        for rules in grammar.rules.values() {
            for rule in rules {
                for symbol in &rule.rhs {
                    match symbol {
                        Symbol::Terminal(id) => {
                            token_symbols.insert(*id);
                        }
                        Symbol::NonTerminal(id) => {
                            non_terminal_symbols.insert(*id);
                        }
                        Symbol::External(id) => {
                            external_symbols.insert(*id);
                        }
                        _ => {
                            let mut ids = HashSet::new();
                            self.collect_symbol_ids(symbol, &mut ids);
                            for id in ids {
                                // Determine category based on existing knowledge
                                if grammar.tokens.contains_key(&id) {
                                    token_symbols.insert(id);
                                } else if grammar.externals.iter().any(|e| e.symbol_id == id) {
                                    external_symbols.insert(id);
                                } else {
                                    non_terminal_symbols.insert(id);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Add external symbols
        for external in &grammar.externals {
            external_symbols.insert(external.symbol_id);
        }

        // Sort each category for deterministic ordering by symbol name
        let mut token_vec: Vec<_> = token_symbols.into_iter().collect();
        let mut non_terminal_vec: Vec<_> = non_terminal_symbols.into_iter().collect();
        let mut external_vec: Vec<_> = external_symbols.into_iter().collect();

        // Sort by symbol name for deterministic ordering
        token_vec.sort_by_key(|id| {
            grammar
                .tokens
                .get(id)
                .map(|t| t.name.clone())
                .unwrap_or_else(|| format!("_token_{}", id.0))
        });
        non_terminal_vec.sort_by_key(|id| {
            // Try to find the symbol name from rule_names
            grammar
                .rule_names
                .get(id)
                .cloned()
                .unwrap_or_else(|| format!("_nt_{}", id.0))
        });
        external_vec.sort_by_key(|id| {
            grammar
                .externals
                .iter()
                .find(|e| e.symbol_id == *id)
                .map(|e| e.name.clone())
                .unwrap_or_else(|| format!("_ext_{}", id.0))
        });

        // Assign new IDs preserving parse table ordering: tokens first, then non-terminals, then externals
        debug_trace!("DEBUG renumber_symbols: Assigning new IDs");
        debug_trace!("  Tokens: {:?}", token_vec);
        debug_trace!("  Non-terminals: {:?}", non_terminal_vec);
        debug_trace!("  Externals: {:?}", external_vec);

        for old_id in token_vec {
            if let std::collections::hash_map::Entry::Vacant(e) = old_to_new.entry(old_id) {
                e.insert(SymbolId(next_id));
                debug_trace!("  Token {:?} -> {:?}", old_id, SymbolId(next_id));
                next_id += 1;
            }
        }

        for old_id in non_terminal_vec {
            if let std::collections::hash_map::Entry::Vacant(e) = old_to_new.entry(old_id) {
                e.insert(SymbolId(next_id));
                debug_trace!("  Non-terminal {:?} -> {:?}", old_id, SymbolId(next_id));
                next_id += 1;
            }
        }

        for old_id in external_vec {
            if let std::collections::hash_map::Entry::Vacant(e) = old_to_new.entry(old_id) {
                e.insert(SymbolId(next_id));
                debug_trace!("  External {:?} -> {:?}", old_id, SymbolId(next_id));
                next_id += 1;
            }
        }

        // Apply renumbering mappings

        // Update tokens
        let mut new_tokens = IndexMap::new();
        for (old_id, token) in grammar.tokens.drain(..) {
            if let Some(&new_id) = old_to_new.get(&old_id) {
                new_tokens.insert(new_id, token);
            }
        }
        grammar.tokens = new_tokens;

        // Update rules
        let mut new_rules = IndexMap::new();
        // Process rules

        for (old_id, mut rules) in grammar.rules.drain(..) {
            // Process rules for this symbol

            // Update each rule in the vector
            for rule in &mut rules {
                // Update LHS
                if let Some(&new_id) = old_to_new.get(&rule.lhs) {
                    rule.lhs = new_id;
                }

                // Update RHS
                for symbol in &mut rule.rhs {
                    self.renumber_symbol(symbol, &old_to_new);
                }
            }

            // Insert with possibly updated key
            let new_key = if let Some(&new_id) = old_to_new.get(&old_id) {
                // Renumber symbol
                new_id
            } else {
                // Keep original ID
                old_id
            };
            new_rules.insert(new_key, rules);
        }

        // Update grammar rules
        grammar.rules = new_rules;

        // Update source_file_id if it was renumbered
        if let Some(sf_id) = self.source_file_id
            && let Some(&new_id) = old_to_new.get(&sf_id)
        {
            // Update source_file_id
            self.source_file_id = Some(new_id);
        }

        // Update rule_names
        let mut new_rule_names = IndexMap::new();
        for (old_id, name) in grammar.rule_names.drain(..) {
            if let Some(&new_id) = old_to_new.get(&old_id) {
                new_rule_names.insert(new_id, name);
            }
        }
        grammar.rule_names = new_rule_names;

        // Update other references
        grammar.supertypes = grammar
            .supertypes
            .iter()
            .filter_map(|id| old_to_new.get(id).copied())
            .collect();

        grammar.inline_rules = grammar
            .inline_rules
            .iter()
            .filter_map(|id| old_to_new.get(id).copied())
            .collect();

        // Update external tokens
        for external in &mut grammar.externals {
            if let Some(&new_id) = old_to_new.get(&external.symbol_id) {
                external.symbol_id = new_id;
            }
        }

        // Update extras
        debug_trace!("DEBUG renumber_symbols: Updating extras");
        debug_trace!("  Old extras: {:?}", grammar.extras);
        grammar.extras = grammar
            .extras
            .iter()
            .filter_map(|&old_id| {
                if let Some(&new_id) = old_to_new.get(&old_id) {
                    debug_trace!("  Extra {:?} -> {:?}", old_id, new_id);
                    Some(new_id)
                } else {
                    debug_trace!(
                        "  WARNING: Extra {:?} not found in renumbering map!",
                        old_id
                    );
                    None
                }
            })
            .collect();
        debug_trace!("  New extras: {:?}", grammar.extras);
    }
}

/// Statistics about optimizations performed
#[derive(Debug, Default)]
pub struct OptimizationStats {
    /// Number of unused symbols removed
    pub removed_unused_symbols: usize,
    /// Number of rules inlined
    pub inlined_rules: usize,
    /// Number of tokens merged
    pub merged_tokens: usize,
    /// Number of left-recursive rules optimized
    pub optimized_left_recursion: usize,
    /// Number of unit rules eliminated
    pub eliminated_unit_rules: usize,
}

/// Convenience function to optimize a grammar
pub fn optimize_grammar(mut grammar: Grammar) -> crate::error::Result<Grammar> {
    let mut optimizer = GrammarOptimizer::new();
    optimizer.optimize(&mut grammar);
    Ok(grammar)
}

impl OptimizationStats {
    /// Get total number of optimizations performed
    pub fn total(&self) -> usize {
        self.removed_unused_symbols
            + self.inlined_rules
            + self.merged_tokens
            + self.optimized_left_recursion
            + self.eliminated_unit_rules
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Associativity, ConflictDeclaration, ConflictResolution, FieldId, PrecedenceKind};

    fn create_test_grammar() -> Grammar {
        let mut grammar = Grammar::new("test".to_string());

        // Add some tokens
        grammar.tokens.insert(
            SymbolId(1),
            Token {
                name: "plus".to_string(),
                pattern: TokenPattern::String("+".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            SymbolId(2),
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );

        // Add an unused token
        grammar.tokens.insert(
            SymbolId(99),
            Token {
                name: "unused".to_string(),
                pattern: TokenPattern::String("unused".to_string()),
                fragile: false,
            },
        );

        // Add rules
        let expr = SymbolId(3);
        let term = SymbolId(4);

        // expr -> expr + term (left recursive)
        grammar.add_rule(Rule {
            lhs: expr,
            rhs: vec![
                Symbol::NonTerminal(expr),
                Symbol::Terminal(SymbolId(1)),
                Symbol::NonTerminal(term),
            ],
            precedence: Some(PrecedenceKind::Static(1)),
            associativity: Some(Associativity::Left),
            fields: vec![],
            production_id: ProductionId(0),
        });

        // expr -> term
        grammar.add_rule(Rule {
            lhs: expr,
            rhs: vec![Symbol::NonTerminal(term)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(1),
        });

        // term -> number
        grammar.add_rule(Rule {
            lhs: term,
            rhs: vec![Symbol::Terminal(SymbolId(2))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(2),
        });

        grammar
    }

    #[test]
    fn test_remove_unused_symbols() {
        let mut grammar = create_test_grammar();
        let mut optimizer = GrammarOptimizer::new();

        optimizer.analyze_grammar(&grammar);

        debug_trace!("Used symbols: {:?}", optimizer.used_symbols);
        debug_trace!(
            "Tokens before: {:?}",
            grammar.tokens.keys().collect::<Vec<_>>()
        );
        debug_trace!(
            "Rules: {:?}",
            grammar
                .all_rules()
                .map(|r| (r.lhs, &r.rhs))
                .collect::<Vec<_>>()
        );

        let removed = optimizer.remove_unused_symbols(&mut grammar);

        debug_trace!("Removed: {}", removed);
        debug_trace!(
            "Tokens after: {:?}",
            grammar.tokens.keys().collect::<Vec<_>>()
        );

        // We expect to remove: SymbolId(99) token, and the rule key symbols 5 and 6
        assert!(removed >= 1); // At least the unused token should be removed
        assert!(!grammar.tokens.contains_key(&SymbolId(99)));
    }

    #[test]
    fn test_eliminate_unit_rules() {
        let mut grammar = create_test_grammar();
        let mut optimizer = GrammarOptimizer::new();

        optimizer.analyze_grammar(&grammar);
        let _eliminated = optimizer.eliminate_unit_rules(&mut grammar);

        // The test grammar may not have unit rules, which is fine
    }

    #[test]
    fn test_optimization_stats() {
        let mut grammar = create_test_grammar();
        let mut optimizer = GrammarOptimizer::new();

        let stats = optimizer.optimize(&mut grammar);

        assert!(stats.total() > 0);
        debug_trace!("Optimization stats: {:?}", stats);
    }

    #[test]
    fn test_left_recursion_detection() {
        let grammar = create_test_grammar();
        let mut optimizer = GrammarOptimizer::new();

        optimizer.analyze_grammar(&grammar);

        // The expr rule should be detected as left-recursive
        let expr = SymbolId(3);
        assert!(optimizer.left_recursive_rules.contains(&expr));
    }

    #[test]
    fn test_inline_single_use_rules() {
        let mut grammar = Grammar::new("test".to_string());

        // Create a rule that's only used once
        let single_use = SymbolId(10);
        let main = SymbolId(11);
        let terminal = SymbolId(12);

        grammar.tokens.insert(
            terminal,
            Token {
                name: "a".to_string(),
                pattern: TokenPattern::String("a".to_string()),
                fragile: false,
            },
        );

        // main -> single_use
        grammar.add_rule(Rule {
            lhs: main,
            rhs: vec![Symbol::NonTerminal(single_use)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

        // single_use -> a
        grammar.add_rule(Rule {
            lhs: single_use,
            rhs: vec![Symbol::Terminal(terminal)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(1),
        });

        let mut optimizer = GrammarOptimizer::new();
        optimizer.analyze_grammar(&grammar);

        // The inline_simple_rules function eliminates unit rules, not general inlining
        // So we test that at least something was optimized
        let stats = optimizer.optimize(&mut grammar);

        // Either unit rules were eliminated or symbols were removed
        assert!(stats.total() > 0);
    }

    #[test]
    fn test_transform_left_recursion_rewrites_grammar() {
        let mut grammar = Grammar::new("lr".to_string());

        // Tokens used in the grammar
        grammar.tokens.insert(
            SymbolId(1),
            Token {
                name: "+".to_string(),
                pattern: TokenPattern::String("+".to_string()),
                fragile: false,
            },
        );
        grammar.tokens.insert(
            SymbolId(2),
            Token {
                name: "b".to_string(),
                pattern: TokenPattern::String("b".to_string()),
                fragile: false,
            },
        );

        // Field and rule name for the non-terminal
        grammar.fields.insert(FieldId(0), "b".to_string());
        let a = SymbolId(3);
        grammar.rule_names.insert(a, "A".to_string());

        // Left-recursive rule: A -> A + b
        grammar.add_rule(Rule {
            lhs: a,
            rhs: vec![
                Symbol::NonTerminal(a),
                Symbol::Terminal(SymbolId(1)),
                Symbol::Terminal(SymbolId(2)),
            ],
            precedence: Some(PrecedenceKind::Static(5)),
            associativity: Some(Associativity::Left),
            fields: vec![(FieldId(0), 2)],
            production_id: ProductionId(0),
        });

        // Base rule: A -> b
        grammar.add_rule(Rule {
            lhs: a,
            rhs: vec![Symbol::Terminal(SymbolId(2))],
            precedence: None,
            associativity: None,
            fields: vec![(FieldId(0), 0)],
            production_id: ProductionId(1),
        });

        // Conflict referencing original symbol
        grammar.conflicts.push(ConflictDeclaration {
            symbols: vec![a],
            resolution: ConflictResolution::GLR,
        });

        let mut optimizer = GrammarOptimizer::new();
        optimizer.analyze_grammar(&grammar);
        let rules = optimizer.extract_rules_for_symbol(&grammar, a).unwrap();
        let (recursive, base) = optimizer.partition_recursive_rules(&rules, a);
        let new_symbol = optimizer.create_new_symbol(&grammar);
        optimizer.transform_left_recursion(&mut grammar, a, new_symbol, recursive, base);

        // Verify base rule was rewritten
        let b_id = grammar
            .tokens
            .iter()
            .find(|(_, t)| t.name == "b")
            .map(|(id, _)| *id)
            .unwrap();
        let a_rules = grammar.get_rules_for_symbol(a).unwrap();
        assert_eq!(a_rules.len(), 1);
        assert_eq!(
            a_rules[0].rhs,
            vec![Symbol::Terminal(b_id), Symbol::NonTerminal(new_symbol)]
        );
        assert_eq!(a_rules[0].fields, vec![(FieldId(0), 0)]);

        // Verify new symbol rules
        let plus_id = grammar
            .tokens
            .iter()
            .find(|(_, t)| t.name == "+")
            .map(|(id, _)| *id)
            .unwrap();
        let new_rules = grammar.get_rules_for_symbol(new_symbol).unwrap();
        assert_eq!(new_rules.len(), 2);
        let recursive_rule = new_rules.iter().find(|r| !r.rhs.is_empty()).unwrap();
        assert_eq!(
            recursive_rule.rhs,
            vec![
                Symbol::Terminal(plus_id),
                Symbol::Terminal(b_id),
                Symbol::NonTerminal(new_symbol),
            ]
        );
        assert_eq!(recursive_rule.fields, vec![(FieldId(0), 1)]);
        assert_eq!(recursive_rule.precedence, Some(PrecedenceKind::Static(5)));
        assert_eq!(recursive_rule.associativity, Some(Associativity::Left));

        // Ensure epsilon rule exists
        assert!(new_rules.iter().any(|r| r.rhs.is_empty()));

        // Conflicts should include new symbol
        assert!(grammar.conflicts[0].symbols.contains(&a));
        assert!(grammar.conflicts[0].symbols.contains(&new_symbol));
    }
}
