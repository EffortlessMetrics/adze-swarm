// Conflict visualization and debugging tools for GLR parsing

//! Human-readable visualization and debugging tools for parse conflicts.

use crate::{Action, Conflict, ConflictType, ItemSet, ItemSetCollection, LRItem, RuleId, SymbolId};
use adze_ir::{Grammar, Symbol};
use std::fmt::Write;

/// Visualize conflicts in a human-readable format
pub struct ConflictVisualizer<'a> {
    grammar: &'a Grammar,
    conflicts: &'a [Conflict],
    item_sets: Option<&'a ItemSetCollection>,
}

impl<'a> ConflictVisualizer<'a> {
    pub fn new(grammar: &'a Grammar, conflicts: &'a [Conflict]) -> Self {
        Self {
            grammar,
            conflicts,
            item_sets: None,
        }
    }

    /// Add item sets for more detailed visualization
    pub fn with_item_sets(mut self, item_sets: &'a ItemSetCollection) -> Self {
        self.item_sets = Some(item_sets);
        self
    }

    /// Generate a report of all conflicts
    pub fn generate_report(&self) -> String {
        let mut report = String::new();

        writeln!(&mut report, "=== GLR Conflict Report ===").unwrap();
        writeln!(&mut report, "Total conflicts: {}", self.conflicts.len()).unwrap();

        let shift_reduce = self
            .conflicts
            .iter()
            .filter(|c| c.conflict_type == ConflictType::ShiftReduce)
            .count();
        let reduce_reduce = self
            .conflicts
            .iter()
            .filter(|c| c.conflict_type == ConflictType::ReduceReduce)
            .count();

        writeln!(&mut report, "  Shift/Reduce: {}", shift_reduce).unwrap();
        writeln!(&mut report, "  Reduce/Reduce: {}", reduce_reduce).unwrap();
        writeln!(&mut report).unwrap();

        for (i, conflict) in self.conflicts.iter().enumerate() {
            writeln!(&mut report, "Conflict #{}", i + 1).unwrap();
            self.format_conflict(&mut report, conflict);
            writeln!(&mut report).unwrap();
        }

        report
    }

    /// Format a single conflict
    fn format_conflict(&self, output: &mut String, conflict: &Conflict) {
        writeln!(output, "  Type: {:?}", conflict.conflict_type).unwrap();
        writeln!(output, "  State: {}", conflict.state.0).unwrap();
        writeln!(
            output,
            "  Symbol: {} ({})",
            conflict.symbol.0,
            self.symbol_name(conflict.symbol)
        )
        .unwrap();

        // Show the conflicting actions
        writeln!(output, "  Actions:").unwrap();
        for action in &conflict.actions {
            match action {
                Action::Shift(state) => {
                    writeln!(output, "    - Shift to state {}", state.0).unwrap();
                }
                Action::Reduce(rule_id) => {
                    writeln!(
                        output,
                        "    - Reduce by rule {}: {}",
                        rule_id.0,
                        self.format_rule(*rule_id)
                    )
                    .unwrap();
                }
                Action::Fork(actions) => {
                    writeln!(output, "    - Fork into {} actions", actions.len()).unwrap();
                }
                _ => {}
            }
        }

        // If we have item sets, show the conflicting items
        if let Some(item_sets) = self.item_sets
            && let Some(item_set) = item_sets.sets.iter().find(|s| s.id == conflict.state)
        {
            self.format_conflicting_items(output, item_set, conflict);
        }
    }

    /// Format the conflicting items in a state
    fn format_conflicting_items(
        &self,
        output: &mut String,
        item_set: &ItemSet,
        conflict: &Conflict,
    ) {
        writeln!(output, "  Items in state:").unwrap();

        for item in &item_set.items {
            if self.item_involves_conflict(item, conflict) {
                writeln!(output, "    {}", self.format_item(item)).unwrap();
            }
        }
    }

    /// Check if an item is involved in the conflict
    fn item_involves_conflict(&self, item: &LRItem, conflict: &Conflict) -> bool {
        // Check if this is a reduce item with the conflicting lookahead
        if item.is_reduce_item(self.grammar) && item.lookahead == conflict.symbol {
            return true;
        }

        // Check if this item can shift the conflicting symbol
        if let Some(next_symbol) = item.next_symbol(self.grammar) {
            match next_symbol {
                Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => {
                    if *id == conflict.symbol {
                        return true;
                    }
                }
                Symbol::Optional(_)
                | Symbol::Repeat(_)
                | Symbol::RepeatOne(_)
                | Symbol::Choice(_)
                | Symbol::Sequence(_)
                | Symbol::Epsilon => {
                    // Complex symbols should have been normalized
                    return false;
                }
            }
        }

        false
    }

    /// Format an LR item
    fn format_item(&self, item: &LRItem) -> String {
        let rule_str = self.format_rule_with_dot(item.rule_id, item.position);
        format!("[{}, {}]", rule_str, self.symbol_name(item.lookahead))
    }

    /// Format a rule with a dot at the given position
    fn format_rule_with_dot(&self, rule_id: RuleId, position: usize) -> String {
        if let Some(rule) = self.grammar.rule_by_id(rule_id) {
            let mut result = format!("{} ->", self.symbol_name(rule.lhs));

            for (i, symbol) in rule.rhs.iter().enumerate() {
                if i == position {
                    result.push_str(" •");
                }
                result.push(' ');
                result.push_str(&self.format_symbol(symbol));
            }

            if position >= rule.rhs.len() {
                result.push_str(" •");
            }

            return result;
        }

        format!("Rule {}", rule_id.0)
    }

    /// Format a rule
    fn format_rule(&self, rule_id: RuleId) -> String {
        if let Some(rule) = self.grammar.rule_by_id(rule_id) {
            let rhs: Vec<String> = rule.rhs.iter().map(|s| self.format_symbol(s)).collect();
            return format!("{} -> {}", self.symbol_name(rule.lhs), rhs.join(" "));
        }

        format!("Rule {}", rule_id.0)
    }

    /// Format a symbol
    fn format_symbol(&self, symbol: &Symbol) -> String {
        match symbol {
            Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => {
                self.symbol_name(*id)
            }
            Symbol::Optional(inner) => format!("{}?", self.format_symbol(inner)),
            Symbol::Repeat(inner) => format!("{}*", self.format_symbol(inner)),
            Symbol::RepeatOne(inner) => format!("{}+", self.format_symbol(inner)),
            Symbol::Choice(choices) => {
                let formatted: Vec<_> = choices.iter().map(|s| self.format_symbol(s)).collect();
                format!("({})", formatted.join(" | "))
            }
            Symbol::Sequence(seq) => {
                let formatted: Vec<_> = seq.iter().map(|s| self.format_symbol(s)).collect();
                formatted.join(" ")
            }
            Symbol::Epsilon => "ε".to_string(),
        }
    }

    /// Get the name of a symbol
    fn symbol_name(&self, symbol_id: SymbolId) -> String {
        // Check tokens
        if let Some(token) = self.grammar.tokens.get(&symbol_id) {
            return token.name.clone();
        }

        // Check if it's a rule
        if self.grammar.rules.contains_key(&symbol_id) {
            return format!("rule_{}", symbol_id.0);
        }

        // Check externals
        if let Some(external) = self
            .grammar
            .externals
            .iter()
            .find(|e| e.symbol_id == symbol_id)
        {
            return external.name.clone();
        }

        // Fallback
        format!("symbol_{}", symbol_id.0)
    }
}

/// Generate a DOT graph visualization of the parse states and conflicts
pub fn generate_dot_graph(
    item_sets: &ItemSetCollection,
    conflicts: &[Conflict],
    grammar: &Grammar,
) -> String {
    let mut dot = String::new();

    writeln!(&mut dot, "digraph parse_automaton {{").unwrap();
    writeln!(&mut dot, "  rankdir=LR;").unwrap();
    writeln!(&mut dot, "  node [shape=box];").unwrap();

    // Add states
    for item_set in &item_sets.sets {
        let has_conflict = conflicts.iter().any(|c| c.state == item_set.id);
        let color = if has_conflict { "red" } else { "black" };

        writeln!(
            &mut dot,
            "  state{} [label=\"State {}\\n{} items\", color={}];",
            item_set.id.0,
            item_set.id.0,
            item_set.items.len(),
            color
        )
        .unwrap();
    }

    // Add transitions
    for ((from_state, symbol), to_state) in &item_sets.goto_table {
        let symbol_name = if let Some(token) = grammar.tokens.get(symbol) {
            &token.name
        } else {
            "?"
        };

        writeln!(
            &mut dot,
            "  state{} -> state{} [label=\"{}\"];",
            from_state.0, to_state.0, symbol_name
        )
        .unwrap();
    }

    writeln!(&mut dot, "}}").unwrap();

    dot
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::{RuleId, StateId};
    // use crate::{ConflictResolver, FirstFollowSets};

    #[test]
    fn test_conflict_report_generation() {
        let grammar = Grammar::new("test".to_string());
        let conflicts = vec![Conflict {
            state: StateId(5),
            symbol: SymbolId(10),
            actions: vec![Action::Shift(StateId(7)), Action::Reduce(RuleId(2))],
            conflict_type: ConflictType::ShiftReduce,
        }];

        let visualizer = ConflictVisualizer::new(&grammar, &conflicts);
        let report = visualizer.generate_report();

        assert!(report.contains("Total conflicts: 1"));
        assert!(report.contains("Shift/Reduce: 1"));
        assert!(report.contains("State: 5"));
    }
}
