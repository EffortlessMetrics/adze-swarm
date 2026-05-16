use crate::{FirstFollowSets, GLRError};
use adze_ir::*;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// LR(1) item for GLR parsing
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LRItem {
    /// Owning rule for this item/state
    pub rule_id: RuleId,
    /// Position within the rule's RHS
    pub position: usize,
    /// Lookahead symbol for LR(1) parsing
    pub lookahead: SymbolId,
}

impl LRItem {
    /// Construct an `LRItem` from its owning rule, dot position, and lookahead symbol.
    pub fn new(rule_id: RuleId, position: usize, lookahead: SymbolId) -> Self {
        Self {
            rule_id,
            position,
            lookahead,
        }
    }

    /// Check if this item is at the end of the rule (reduce item)
    pub fn is_reduce_item(&self, grammar: &Grammar) -> bool {
        if let Some(rule) = grammar
            .all_rules()
            .find(|r| r.production_id.0 == self.rule_id.0)
        {
            // Special case: epsilon rules (A -> epsilon) are reduce items at position 0
            // because epsilon doesn't need to be "consumed" - it represents empty string
            if rule.rhs.len() == 1 && matches!(rule.rhs[0], Symbol::Epsilon) {
                return true; // Always a reduce item for epsilon rules
            }

            self.position >= rule.rhs.len()
        } else {
            false
        }
    }

    /// Get the symbol after the dot (next symbol to parse)
    pub fn next_symbol<'a>(&self, grammar: &'a Grammar) -> Option<&'a Symbol> {
        if let Some(rule) = grammar
            .all_rules()
            .find(|r| r.production_id.0 == self.rule_id.0)
        {
            rule.rhs.get(self.position)
        } else {
            None
        }
    }
}

/// Set of LR(1) items representing a parser state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSet {
    /// The LR(1) item set that defines this state's closure
    pub items: BTreeSet<LRItem>,
    /// Unique identifier for this state in the canonical collection
    pub id: StateId,
}

impl ItemSet {
    /// Create a new empty item set with the given state ID
    pub fn new(id: StateId) -> Self {
        Self {
            items: BTreeSet::new(),
            id,
        }
    }

    /// Add an LR(1) item to this item set
    pub fn add_item(&mut self, item: LRItem) {
        self.items.insert(item);
    }

    /// Compute closure of this item set
    pub fn closure(
        &mut self,
        grammar: &Grammar,
        first_follow: &FirstFollowSets,
    ) -> Result<(), GLRError> {
        let _initial_size = self.items.len();

        let mut added = true;
        let mut _iteration = 0;
        while added {
            added = false;
            _iteration += 1;
            let current_items: Vec<_> = self.items.iter().cloned().collect();

            for item in current_items {
                if let Some(Symbol::NonTerminal(symbol_id)) = item.next_symbol(grammar) {
                    // Find all rules with this symbol as LHS
                    if let Some(rules) = grammar.get_rules_for_symbol(*symbol_id) {
                        for rule in rules {
                            // Compute FIRST of β α where β is the rest of the current rule
                            // and α is the lookahead
                            let mut beta = Vec::new();
                            if let Some(current_rule) = grammar
                                .all_rules()
                                .find(|r| r.production_id.0 == item.rule_id.0)
                            {
                                beta.extend_from_slice(&current_rule.rhs[item.position + 1..]);
                            }
                            beta.push(Symbol::Terminal(item.lookahead));

                            let first_beta_alpha = first_follow.first_of_sequence(&beta)?;

                            // Add new items for each symbol in FIRST(β α)
                            for lookahead_idx in first_beta_alpha.ones() {
                                let new_item = LRItem::new(
                                    RuleId(rule.production_id.0),
                                    0,
                                    SymbolId(lookahead_idx as u16),
                                );

                                if !self.items.contains(&new_item) {
                                    self.items.insert(new_item);
                                    added = true;
                                    if rule.rhs.is_empty() {
                                        // Empty production
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Closure complete
        Ok(())
    }

    /// Compute GOTO for a given symbol
    pub fn goto(
        &self,
        symbol: &Symbol,
        grammar: &Grammar,
        _first_follow: &FirstFollowSets,
    ) -> ItemSet {
        let mut new_set = ItemSet::new(StateId(0)); // ID will be assigned later

        // Add all items where the dot can advance over the given symbol
        for item in &self.items {
            if let Some(next_sym) = item.next_symbol(grammar)
                && std::mem::discriminant(next_sym) == std::mem::discriminant(symbol)
            {
                match (next_sym, symbol) {
                    (Symbol::Terminal(a), Symbol::Terminal(b))
                    | (Symbol::NonTerminal(a), Symbol::NonTerminal(b))
                    | (Symbol::External(a), Symbol::External(b))
                        if a == b =>
                    {
                        let new_item = LRItem::new(item.rule_id, item.position + 1, item.lookahead);
                        new_set.add_item(new_item);
                    }
                    _ => {}
                }
            }
        }

        // Compute closure of the new set
        let _ = new_set.closure(grammar, _first_follow);
        new_set
    }
}

/// Collection of all LR(1) item sets (parser states)
#[derive(Debug, Clone)]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub struct ItemSetCollection {
    /// All computed LR(1) item sets (parser states).
    pub sets: Vec<ItemSet>,
    /// GOTO transitions: `(from_state, symbol) -> to_state`.
    pub goto_table: IndexMap<(StateId, SymbolId), StateId>,
    /// Track which symbols in goto_table are terminals (true) vs non-terminals (false)
    pub symbol_is_terminal: IndexMap<SymbolId, bool>,
}

impl ItemSetCollection {
    /// Build canonical collection of LR(1) item sets for augmented grammar
    pub fn build_canonical_collection_augmented(
        grammar: &Grammar,
        first_follow: &FirstFollowSets,
        augmented_start: SymbolId,
        _original_start: SymbolId,
        eof_symbol: SymbolId,
    ) -> Self {
        let mut collection = Self {
            sets: Vec::new(),
            goto_table: IndexMap::new(),
            symbol_is_terminal: IndexMap::new(),
        };

        // Create initial state with the augmented start rule S' -> S $
        let mut initial_set = ItemSet::new(StateId(0));

        // Find the augmented start rule
        if let Some(augmented_rules) = grammar.get_rules_for_symbol(augmented_start) {
            for rule in augmented_rules {
                // Add S' -> • S with lookahead $ (EOF)
                let start_item = LRItem::new(
                    RuleId(rule.production_id.0),
                    0,
                    eof_symbol, // EOF symbol
                );
                initial_set.add_item(start_item);
            }
        }

        // Compute closure
        let _ = initial_set.closure(grammar, first_follow);
        debug_trace!(
            "Initial state 0 after closure has {} items:",
            initial_set.items.len()
        );

        // Track what symbols we expect transitions for
        let mut expected_terminals = std::collections::BTreeSet::new();
        let mut expected_nonterminals = std::collections::BTreeSet::new();

        for item in &initial_set.items {
            // Print each item to debug
            if let Some(rule) = grammar
                .all_rules()
                .find(|r| r.production_id.0 == item.rule_id.0)
            {
                let mut rhs_str = String::new();
                for (idx, sym) in rule.rhs.iter().enumerate() {
                    if idx == item.position {
                        rhs_str.push_str(" • ");
                    }
                    match sym {
                        Symbol::Terminal(id) => rhs_str.push_str(&format!("T({}) ", id.0)),
                        Symbol::NonTerminal(id) => rhs_str.push_str(&format!("NT({}) ", id.0)),
                        _ => rhs_str.push_str("? "),
                    }
                }
                if item.position == rule.rhs.len() {
                    rhs_str.push_str(" • ");
                }
                debug_trace!(
                    "  Item: NT({}) -> {}, lookahead={}",
                    rule.lhs.0,
                    rhs_str,
                    item.lookahead.0
                );

                // Track what symbol is next
                if item.position < rule.rhs.len() {
                    match &rule.rhs[item.position] {
                        Symbol::Terminal(t) => {
                            expected_terminals.insert(*t);
                        }
                        Symbol::NonTerminal(nt) => {
                            expected_nonterminals.insert(*nt);
                        }
                        _ => {}
                    }
                }
            }
        }

        debug_trace!("State 0 expects transitions for:");
        debug_trace!("  Terminals: {:?}", expected_terminals);
        debug_trace!("  Nonterminals: {:?}", expected_nonterminals);

        collection.sets.push(initial_set);
        let mut state_counter = 1;

        // Build all reachable states (same as before)
        let mut i = 0;
        while i < collection.sets.len() {
            let current_set = collection.sets[i].clone();

            // Debug: Print all items in this state
            for item in &current_set.items {
                if let Some(rule) = grammar
                    .all_rules()
                    .find(|r| r.production_id.0 == item.rule_id.0)
                {
                    let mut rhs_str = String::new();
                    for (idx, sym) in rule.rhs.iter().enumerate() {
                        if idx == item.position {
                            rhs_str.push_str(" • ");
                        }
                        rhs_str.push_str(&format!("{:?} ", sym));
                    }
                    if item.position == rule.rhs.len() {
                        rhs_str.push_str(" • ");
                    }
                    // "  [{}] {:?} -> {} , lookahead={}"
                }
            }

            // Find all symbols that can be shifted from this state
            let mut symbols = BTreeSet::new();
            let mut _terminal_count = 0;
            let mut _non_terminal_count = 0;
            if i == 0 {
                debug_trace!("\n=== State 0 Analysis ===");
                debug_trace!("State 0 has {} items:", current_set.items.len());
            }
            for (_idx, item) in current_set.items.iter().enumerate() {
                if i == 0 {
                    // Print the item details
                    if let Some(rule) = grammar
                        .all_rules()
                        .find(|r| r.production_id.0 == item.rule_id.0)
                    {
                        let mut item_str = String::new();
                        item_str.push_str(&format!("NT({}) -> ", rule.lhs.0));
                        for (pos, sym) in rule.rhs.iter().enumerate() {
                            if pos == item.position {
                                item_str.push_str("• ");
                            }
                            match sym {
                                Symbol::Terminal(t) => item_str.push_str(&format!("T({}) ", t.0)),
                                Symbol::NonTerminal(nt) => {
                                    item_str.push_str(&format!("NT({}) ", nt.0))
                                }
                                Symbol::External(e) => item_str.push_str(&format!("EXT({}) ", e.0)),
                                _ => item_str.push_str(&format!("{:?} ", sym)),
                            }
                        }
                        if item.position == rule.rhs.len() {
                            item_str.push_str("• ");
                        }
                        debug_trace!("  Item {}: {} (rule_id={})", _idx, item_str, item.rule_id.0);
                    }
                }

                if let Some(symbol) = item.next_symbol(grammar) {
                    match symbol {
                        Symbol::Terminal(_id) => {
                            _terminal_count += 1;
                        }
                        Symbol::NonTerminal(_id) => {
                            _non_terminal_count += 1;
                        }
                        Symbol::External(_id) => {
                            _terminal_count += 1; // Count externals as terminals
                        }
                        _ => {}
                    }
                    symbols.insert(symbol.clone());
                    if i == 0 {
                        debug_trace!("    -> next symbol: {:?}", symbol);
                    }
                }
            }

            if i == 0 {
                debug_trace!("\nState 0 summary:");
                debug_trace!("  Total symbols that can be shifted: {}", symbols.len());
                debug_trace!("  Terminals: {}", _terminal_count);
                debug_trace!("  Non-terminals: {}", _non_terminal_count);
                debug_trace!("  Symbols: {:?}\n", symbols);
            }

            // Debug: symbols.len(), _terminal_count, _non_terminal_count
            // Compute GOTO for each symbol
            for symbol in symbols {
                let goto_set = current_set.goto(&symbol, grammar, first_follow);

                if !goto_set.items.is_empty() {
                    // Check if this set already exists
                    let existing_state = collection
                        .sets
                        .iter()
                        .find(|set| set.items == goto_set.items)
                        .map(|set| set.id);

                    let target_state = if let Some(existing_id) = existing_state {
                        existing_id
                    } else {
                        // Add new state
                        let new_id = StateId(state_counter);
                        let mut new_set = goto_set;
                        new_set.id = new_id;
                        collection.sets.push(new_set);
                        state_counter += 1;
                        new_id
                    };

                    // Add to GOTO table
                    let symbol_id = match symbol {
                        Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => id,
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
                    if current_set.id.0 == 0 {
                        debug_trace!(
                            "  State 0 GOTO: symbol {:?} -> state {}",
                            symbol_id,
                            target_state.0
                        );
                    }
                    collection
                        .goto_table
                        .insert((current_set.id, symbol_id), target_state);

                    // Track whether this symbol is a terminal or non-terminal
                    let is_terminal = matches!(symbol, Symbol::Terminal(_) | Symbol::External(_));
                    collection.symbol_is_terminal.insert(symbol_id, is_terminal);
                    // "DEBUG: Added goto({}, {}) = {}"
                }
            }

            i += 1;
        }

        collection
    }

    /// Build canonical collection of LR(1) item sets.
    ///
    /// # Examples
    ///
    /// ```
    /// use adze_glr_core::{FirstFollowSets, ItemSetCollection};
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
    /// let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    /// assert!(!collection.sets.is_empty(), "should have at least one state");
    /// ```
    pub fn build_canonical_collection(grammar: &Grammar, first_follow: &FirstFollowSets) -> Self {
        let mut collection = Self {
            sets: Vec::new(),
            goto_table: IndexMap::new(),
            symbol_is_terminal: IndexMap::new(),
        };

        // Create initial state with augmented start rule
        let mut initial_set = ItemSet::new(StateId(0));

        // Find the start symbol (LHS of the first rule in grammar)
        if let Some(start_symbol) = grammar.start_symbol() {
            // Debug: grammar.rule_names.get(&start_symbol)

            // Add items for ALL rules with the start symbol as LHS
            if let Some(start_rules) = grammar.get_rules_for_symbol(start_symbol) {
                for rule in start_rules.iter() {
                    // Debug: idx, rule.lhs, rule.rhs, rule.production_id.0
                    let start_item = LRItem::new(
                        RuleId(rule.production_id.0),
                        0,
                        SymbolId(0), // EOF symbol
                    );
                    initial_set.add_item(start_item);
                    // Debug: rule.production_id.0
                }
            }

            // Compute closure
            let _ = initial_set.closure(grammar, first_follow);
        }

        // Only add initial set if it has items
        if initial_set.items.is_empty() {
            // Handle empty initial set if needed
        } else {
            for _item in &initial_set.items {
                // Debug: item.rule_id.0, item.position, item.lookahead.0
            }
        }

        collection.sets.push(initial_set);
        let mut state_counter = 1;

        // Build all reachable states
        let mut i = 0;
        while i < collection.sets.len() {
            let current_set = collection.sets[i].clone();

            // Debug: Print all items in this state
            for item in &current_set.items {
                if let Some(rule) = grammar
                    .all_rules()
                    .find(|r| r.production_id.0 == item.rule_id.0)
                {
                    let mut rhs_str = String::new();
                    for (idx, sym) in rule.rhs.iter().enumerate() {
                        if idx == item.position {
                            rhs_str.push_str(" • ");
                        }
                        rhs_str.push_str(&format!("{:?} ", sym));
                    }
                    if item.position == rule.rhs.len() {
                        rhs_str.push_str(" • ");
                    }
                    // "  [{}] {:?} -> {} , lookahead={}"
                }
            }

            // Find all symbols that can be shifted from this state
            let mut symbols = BTreeSet::new();
            let mut _terminal_count = 0;
            let mut _non_terminal_count = 0;
            if i == 0 {
                debug_trace!("\n=== State 0 Analysis ===");
                debug_trace!("State 0 has {} items:", current_set.items.len());
            }
            for (_idx, item) in current_set.items.iter().enumerate() {
                if i == 0 {
                    // Print the item details
                    if let Some(rule) = grammar
                        .all_rules()
                        .find(|r| r.production_id.0 == item.rule_id.0)
                    {
                        let mut item_str = String::new();
                        item_str.push_str(&format!("NT({}) -> ", rule.lhs.0));
                        for (pos, sym) in rule.rhs.iter().enumerate() {
                            if pos == item.position {
                                item_str.push_str("• ");
                            }
                            match sym {
                                Symbol::Terminal(t) => item_str.push_str(&format!("T({}) ", t.0)),
                                Symbol::NonTerminal(nt) => {
                                    item_str.push_str(&format!("NT({}) ", nt.0))
                                }
                                Symbol::External(e) => item_str.push_str(&format!("EXT({}) ", e.0)),
                                _ => item_str.push_str(&format!("{:?} ", sym)),
                            }
                        }
                        if item.position == rule.rhs.len() {
                            item_str.push_str("• ");
                        }
                        debug_trace!("  Item {}: {} (rule_id={})", _idx, item_str, item.rule_id.0);
                    }
                }

                if let Some(symbol) = item.next_symbol(grammar) {
                    match symbol {
                        Symbol::Terminal(_id) => {
                            _terminal_count += 1;
                        }
                        Symbol::NonTerminal(_id) => {
                            _non_terminal_count += 1;
                        }
                        Symbol::External(_id) => {
                            _terminal_count += 1; // Count externals as terminals
                        }
                        _ => {}
                    }
                    symbols.insert(symbol.clone());
                    if i == 0 {
                        debug_trace!("    -> next symbol: {:?}", symbol);
                    }
                }
            }

            if i == 0 {
                debug_trace!("\nState 0 summary:");
                debug_trace!("  Total symbols that can be shifted: {}", symbols.len());
                debug_trace!("  Terminals: {}", _terminal_count);
                debug_trace!("  Non-terminals: {}", _non_terminal_count);
                debug_trace!("  Symbols: {:?}\n", symbols);
            }

            // Debug: symbols.len(), _terminal_count, _non_terminal_count
            for item in &current_set.items {
                if let Some(symbol) = item.next_symbol(grammar) {
                    let _symbol_id = match &symbol {
                        Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => id,
                        _ => panic!("Complex symbol"),
                    };
                    // "  Item rule_id={}, position={}, next_symbol={:?} (id={})"
                }
            }

            for symbol in &symbols {
                let _symbol_id = match symbol {
                    Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => id,
                    _ => panic!("Complex symbol"),
                };
            }

            // Compute GOTO for each symbol
            for symbol in symbols {
                let goto_set = current_set.goto(&symbol, grammar, first_follow);

                if !goto_set.items.is_empty() {
                    // Check if this set already exists
                    let existing_state = collection
                        .sets
                        .iter()
                        .find(|set| set.items == goto_set.items)
                        .map(|set| set.id);

                    let target_state = if let Some(existing_id) = existing_state {
                        existing_id
                    } else {
                        // Add new state
                        let new_id = StateId(state_counter);
                        let mut new_set = goto_set;
                        new_set.id = new_id;
                        collection.sets.push(new_set);
                        state_counter += 1;
                        new_id
                    };

                    // Add to GOTO table
                    let symbol_id = match symbol {
                        Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => id,
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
                    if current_set.id.0 == 0 {
                        debug_trace!(
                            "  State 0 GOTO: symbol {:?} -> state {}",
                            symbol_id,
                            target_state.0
                        );
                    }
                    collection
                        .goto_table
                        .insert((current_set.id, symbol_id), target_state);

                    // Track whether this symbol is a terminal or non-terminal
                    let is_terminal = matches!(symbol, Symbol::Terminal(_) | Symbol::External(_));
                    collection.symbol_is_terminal.insert(symbol_id, is_terminal);
                    // "DEBUG: Added goto({}, {}) = {}"
                }
            }

            i += 1;
        }

        collection
    }
}
