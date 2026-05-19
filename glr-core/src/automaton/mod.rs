pub(crate) mod actions;
mod augmentation;
mod metadata;
mod precedence;
mod symbols;

use self::actions::{add_action_with_conflict, normalize_action_table};
use self::augmentation::augment_grammar;
use self::metadata::{build_external_scanner_states, build_symbol_metadata};
use self::precedence::{
    PrecDecision, build_prec_tables, decide_reduce_reduce, decide_with_precedence,
    map_follow_symbol,
};
use self::symbols::{
    SymbolPartitions, build_nonterminal_to_index, build_reverse_symbol_index, build_symbol_index,
};
use crate::debug_trace;
use crate::*;
use adze_ir::*;
use std::collections::BTreeMap;

pub fn build_lr1_automaton(
    grammar: &Grammar,
    first_follow: &FirstFollowSets,
) -> Result<ParseTable, GLRError> {
    // Debug: Print some rules to see their structure
    let mut rule_count = 0;
    for rule in grammar.all_rules() {
        if rule_count >= 10 {
            break;
        }
        let mut rhs_str = String::new();
        for sym in &rule.rhs {
            match sym {
                Symbol::Terminal(id) => rhs_str.push_str(&format!("T({}) ", id.0)),
                Symbol::NonTerminal(id) => rhs_str.push_str(&format!("NT({}) ", id.0)),
                _ => rhs_str.push_str("? "),
            }
        }
        rule_count += 1;
    }

    let partitions = SymbolPartitions::collect(grammar)?;
    let nonterminal_symbols = &partitions.nonterminal_symbols;
    let eof_symbol = partitions.eof_symbol;

    let augmented = augment_grammar(grammar, partitions.max_symbol)?;
    let augmented_grammar = augmented.grammar;
    let original_start = augmented.original_start;
    let augmented_start = augmented.augmented_start;

    // Build canonical collection of LR(1) item sets with augmented grammar
    let collection = ItemSetCollection::build_canonical_collection_augmented(
        &augmented_grammar,
        first_follow,
        augmented_start,
        original_start,
        eof_symbol,
    );

    let symbol_index = build_symbol_index(grammar, &partitions)?;
    let symbol_to_index = symbol_index.symbol_to_index;
    let internal_tokens = symbol_index.internal_tokens;
    let ext_tokens = symbol_index.ext_tokens;

    // Calculate the final symbol count after adding all symbols including EOF
    let indexed_symbol_count = symbol_to_index.len();

    // Create parse table with proper dimensions
    let state_count = collection.sets.len();
    let symbol_count = indexed_symbol_count; // Keep for compatibility

    let mut action_table = vec![vec![Vec::new(); indexed_symbol_count]; state_count];
    let mut goto_table = vec![vec![StateId(0); indexed_symbol_count]; state_count];

    // Track conflicts as we build the table
    let mut conflicts_by_state: BTreeMap<(usize, usize), Vec<Action>> = BTreeMap::new();

    // Build rules for reduction and collect precedence info
    let mut rules = Vec::new();
    let mut dynamic_prec_by_rule = Vec::new();
    let mut rule_assoc_by_rule = Vec::new();
    let mut production_to_rule_id = BTreeMap::new();

    for (rule_id, rule) in grammar.all_rules().enumerate() {
        production_to_rule_id.insert(rule.production_id.0, rule_id as u16);
        rules.push(ParseRule {
            lhs: rule.lhs,
            rhs_len: rule.rhs.len() as u16,
        });

        // Extract precedence value for this rule
        let prec = match rule.precedence {
            Some(adze_ir::PrecedenceKind::Static(p)) => p,
            Some(adze_ir::PrecedenceKind::Dynamic(p)) => p,
            None => 0,
        };
        dynamic_prec_by_rule.push(prec);

        // Extract associativity for this rule
        let assoc = match rule.associativity {
            Some(adze_ir::Associativity::Left) => 1,
            Some(adze_ir::Associativity::Right) => -1,
            _ => 0,
        };
        rule_assoc_by_rule.push(assoc);
    }

    // Debug: Print goto table entries
    debug_trace!(
        "DEBUG: Collection goto table has {} entries",
        collection.goto_table.len()
    );
    debug_trace!(
        "DEBUG: Augmented grammar has {} tokens",
        augmented_grammar.tokens.len()
    );

    // Debug: Print what tokens are in the augmented grammar
    debug_trace!("=== Symbol Classification Debug ===");
    debug_trace!(
        "Tokens in augmented_grammar: {:?}",
        augmented_grammar
            .tokens
            .keys()
            .map(|k| k.0)
            .collect::<Vec<_>>()
    );
    debug_trace!(
        "Externals in augmented_grammar: {:?}",
        augmented_grammar
            .externals
            .iter()
            .map(|e| e.symbol_id.0)
            .collect::<Vec<_>>()
    );
    debug_trace!("Original grammar tokens: {}", grammar.tokens.len());
    debug_trace!(
        "Collection goto_table size: {}",
        collection.goto_table.len()
    );

    // Debug state 0 specifically
    let state0_gotos: Vec<_> = collection
        .goto_table
        .iter()
        .filter(|((from, _), _)| from.0 == 0)
        .collect();
    debug_trace!("State 0 has {} goto entries", state0_gotos.len());
    for ((_, _symbol), _to_state) in &state0_gotos {
        debug_trace!("  Symbol {} -> State {}", _symbol.0, _to_state.0);
    }

    // First, add shift actions from goto table for terminals
    // This must be done BEFORE reduce actions to enable shift/reduce conflict detection
    let mut _terminal_count = 0;
    let mut _non_terminal_count = 0;

    for ((from_state, symbol), to_state) in &collection.goto_table {
        // Check if this symbol is a terminal using the tracking from collection
        let is_terminal = collection
            .symbol_is_terminal
            .get(symbol)
            .copied()
            .unwrap_or(*symbol == eof_symbol); // EOF is a terminal

        if from_state.0 == 0 {
            debug_trace!(
                "State 0 goto entry: symbol {} -> state {}, is_terminal={} (in tokens={}, in externals={}, is EOF={})",
                symbol.0,
                to_state.0,
                is_terminal,
                augmented_grammar.tokens.contains_key(symbol),
                augmented_grammar
                    .externals
                    .iter()
                    .any(|e| e.symbol_id == *symbol),
                symbol.0 == 0
            );
        }

        if is_terminal {
            _terminal_count += 1;
            if let Some(&symbol_idx) = symbol_to_index.get(symbol) {
                let state_idx = from_state.0 as usize;
                if state_idx < action_table.len() && symbol_idx < action_table[state_idx].len() {
                    // Add as a shift action
                    let new_action = Action::Shift(*to_state);
                    if state_idx == 0 {
                        debug_trace!(
                            "DEBUG: Adding shift action to state 0: symbol {} (idx={}) -> state {}",
                            symbol.0,
                            symbol_idx,
                            to_state.0
                        );
                    }
                    add_action_with_conflict(
                        &mut action_table,
                        &mut conflicts_by_state,
                        state_idx,
                        symbol_idx,
                        new_action,
                    );
                } else if state_idx == 0 {
                    debug_trace!(
                        "DEBUG: SKIPPING shift for state 0: bounds check failed - state_idx={}, symbol_idx={}, action_table.len={}, inner_len={}",
                        state_idx,
                        symbol_idx,
                        action_table.len(),
                        if state_idx < action_table.len() {
                            action_table[state_idx].len()
                        } else {
                            0
                        }
                    );
                }
            } else if from_state.0 == 0 {
                debug_trace!(
                    "DEBUG: Terminal {} not in symbol_to_index for state 0",
                    symbol.0
                );
            }
        } else {
            _non_terminal_count += 1;
        }
    }

    // Handle "extras" (like comments, whitespace, and external tokens marked as extras).
    // In every state, for every "extra" token, if there isn't already a specific
    // action, add a self-looping SHIFT action. This allows extras to appear
    // anywhere in the grammar without changing the parser's state.
    for state_idx in 0..state_count {
        for extra_symbol_id in &augmented_grammar.extras {
            if let Some(&symbol_idx) = symbol_to_index.get(extra_symbol_id) {
                // Check if an action already exists for this extra token in this state.
                // Only add self-loop if no action exists yet (empty cell means no action)
                if action_table[state_idx][symbol_idx].is_empty() {
                    // Add a self-looping shift that stays in the same state
                    action_table[state_idx][symbol_idx]
                        .push(Action::Shift(StateId(state_idx as u16)));
                }
            }
        }
    }

    // Now fill action table with reduce actions
    for item_set in &collection.sets {
        let state_idx = item_set.id.0 as usize;

        for item in &item_set.items {
            if item.is_reduce_item(&augmented_grammar) {
                // Check if this is a reduce by the augmented start rule
                if let Some(rule) = augmented_grammar
                    .all_rules()
                    .find(|r| r.production_id.0 == item.rule_id.0)
                    && rule.lhs == augmented_start
                {
                    if item.lookahead == eof_symbol {
                        // This is S' -> S • with lookahead $, add accept action
                        if let Some(&eof_idx) = symbol_to_index.get(&eof_symbol) {
                            add_action_with_conflict(
                                &mut action_table,
                                &mut conflicts_by_state,
                                state_idx,
                                eof_idx,
                                Action::Accept,
                            );
                        }
                    }
                    // NEVER add a regular reduce action for the augmented start rule
                    continue;
                }

                // Regular reduce action
                if let Some(&rule_id) = production_to_rule_id.get(&item.rule_id.0) {
                    let rule = &rules[rule_id as usize];
                    let is_empty_production = rule.rhs_len == 0;

                    // For empty productions, we need to add reduce actions for all symbols in FOLLOW set
                    let lookaheads_to_check: Vec<SymbolId> = if is_empty_production {
                        // Get FOLLOW set for the LHS of this rule
                        if let Some(follow_set) = first_follow.follow(rule.lhs) {
                            // Map FOLLOW set symbols to actual parse table symbols.
                            // This replaces EOF_SENTINEL (SymbolId(0)) with the actual eof_symbol.
                            follow_set
                                .ones()
                                .map(|idx| map_follow_symbol(SymbolId(idx as u16), eof_symbol))
                                .collect()
                        } else {
                            vec![item.lookahead]
                        }
                    } else {
                        vec![item.lookahead]
                    };

                    for lookahead in lookaheads_to_check {
                        if let Some(&lookahead_idx) = symbol_to_index.get(&lookahead) {
                            let new_action = Action::Reduce(RuleId(rule_id));

                            // Always add reduce actions - let conflict resolution handle precedence
                            add_action_with_conflict(
                                &mut action_table,
                                &mut conflicts_by_state,
                                state_idx,
                                lookahead_idx,
                                new_action,
                            );
                        }
                    }
                }
            }
            // Note: Shift actions were already added before this loop
        }
    }

    // Shift actions were already added before reduce actions

    // Build precedence tables once
    let production_count = augmented_grammar.all_rules().count() as u32;
    // token_count includes EOF (symbol 0 in table) plus all regular tokens.
    let token_count = (internal_tokens.len() + 1) as u32;
    let prec_tables = build_prec_tables(
        &augmented_grammar,
        &symbol_to_index,
        token_count,
        production_count,
    );

    // Calculate the first non-terminal index
    // Terminals are: EOF + internal tokens + external tokens.
    // So first non-terminal is at the terminal boundary.
    let first_nonterminal_idx = internal_tokens.len() + ext_tokens.len() + 1;

    // Resolve conflicts using precedence
    for ((state_idx, symbol_idx), _actions) in conflicts_by_state {
        // Guard rail: validate indices
        debug_assert!(state_idx < action_table.len(), "state_idx out of bounds");
        debug_assert!(
            symbol_idx < action_table[0].len(),
            "symbol_idx out of bounds"
        );

        // Only resolve on terminal columns (never on gotos).
        // Terminals occupy indices [0, first_nonterminal_idx).
        if symbol_idx >= first_nonterminal_idx {
            continue; // Skip non-terminal columns
        }

        let cell = &mut action_table[state_idx][symbol_idx];

        // Guard rail: skip empty cells
        if cell.is_empty() {
            continue;
        }

        // If ACCEPT is present, keep it alone (canonical LR(1) accept)
        if cell.iter().any(|a| matches!(a, Action::Accept)) {
            *cell = vec![Action::Accept];
            continue;
        }

        // Extract first shift and the set of reduces in the cell
        let first_shift = cell.iter().find_map(|a| {
            if let Action::Shift(s) = a {
                Some(*s)
            } else {
                None
            }
        });
        let mut reduces: Vec<u16> = cell
            .iter()
            .filter_map(|a| {
                if let Action::Reduce(pid) = a {
                    Some(pid.0)
                } else {
                    None
                }
            })
            .collect();

        // If there are multiple reduces, resolve them first (rule precedence)
        if reduces.len() > 1 {
            let winner = reduces[1..].iter().try_fold(reduces[0], |acc, &r| {
                decide_reduce_reduce(acc, r, &prec_tables)
            });

            if let Some(winner) = winner {
                reduces.clear();
                reduces.push(winner);
                // keep the non-reduce actions (shift/accept) as-is for now
                cell.retain(|a| {
                    matches!(a, Action::Shift(_))
                        || matches!(a, Action::Reduce(pid) if pid.0 == winner)
                });
            }
        }

        // Now we have at most one reduce and at most one shift
        if let (Some(s), Some(r)) = (first_shift, reduces.first().copied()) {
            match decide_with_precedence(symbol_idx, r, &prec_tables) {
                PrecDecision::PreferShift => *cell = vec![Action::Shift(s)],
                PrecDecision::PreferReduce => *cell = vec![Action::Reduce(RuleId(r))],
                PrecDecision::Error => {
                    // Non-associative at equal precedence: forbid combination at this lookahead.
                    // For GLR you can either force a parse error here or keep both and let runtime err.
                    // Common Yacc behavior is to make it a syntax error:
                    // *cell = vec![Action::Error];  // Uncomment if you want to make it a hard error
                    // For now, keep both for GLR
                }
                PrecDecision::NoInfo => {
                    // For GLR: when no precedence information is available, keep both actions
                    // This preserves conflicts for GLR runtime to handle via forking
                    // Don't resolve the conflict - let GLR handle it at runtime
                }
            }
        }
    }

    // Add non-terminal goto entries to the goto table
    for ((from_state, symbol), _to_state) in &collection.goto_table {
        // Check if this symbol is a non-terminal using the tracking from collection
        let is_terminal = collection
            .symbol_is_terminal
            .get(symbol)
            .copied()
            .unwrap_or(*symbol == eof_symbol); // EOF is a terminal

        if !is_terminal && let Some(&symbol_idx) = symbol_to_index.get(symbol) {
            let state_idx = from_state.0 as usize;
            if state_idx < goto_table.len() && symbol_idx < goto_table[state_idx].len() {
                // "DEBUG: Setting goto for state {} non-terminal {} (id={}) -> state {}"
            }
        }
    }

    // Fill goto table from collection's goto_table (kept for compatibility)
    for ((from_state, symbol), to_state) in &collection.goto_table {
        let from_idx = from_state.0 as usize;
        if let Some(&symbol_idx) = symbol_to_index.get(symbol) {
            goto_table[from_idx][symbol_idx] = *to_state;
        }
    }

    // Post-process is no longer needed with proper augmentation
    // The accept action is added when we see S' -> S • with EOF lookahead

    // But we still need to handle the original grammar's symbol mapping
    if let Some(_start_symbol) = grammar.start_symbol() {
        // Find all states and check if they need EOF reduce actions
        for (state_idx, _item_set) in collection.sets.iter().enumerate() {
            // Skip this post-processing - handled by augmentation
            let needs_eof_reduce = false;
            let reduce_rule_id: Option<RuleId> = None;

            // If we found a reduce item that needs EOF action, ensure it's in the action table
            if needs_eof_reduce
                && let Some(rule_id) = reduce_rule_id
                && let Some(&eof_idx) = symbol_to_index.get(&SymbolId(0))
            {
                // Check if EOF action already exists
                if action_table[state_idx][eof_idx].is_empty() {
                    action_table[state_idx][eof_idx].push(Action::Reduce(rule_id));
                }
            }
        }
    }

    let symbol_metadata = build_symbol_metadata(grammar);
    let external_scanner_states = build_external_scanner_states(
        &augmented_grammar,
        state_count,
        &symbol_to_index,
        &action_table,
    );

    let nonterminal_to_index = build_nonterminal_to_index(&symbol_to_index, nonterminal_symbols);

    let _rule_count = rules.len();

    // Calculate proper counts for EOF symbol
    // token_count includes EOF (Symbol 0 in table) + all internal tokens
    let token_count = internal_tokens.len() + 1;
    let external_token_count = ext_tokens.len();

    // Normalize action table for deterministic output
    normalize_action_table(&mut action_table);

    let index_to_symbol = build_reverse_symbol_index(&symbol_to_index);

    let mut table = ParseTable {
        action_table,
        goto_table,
        symbol_metadata,
        state_count,
        symbol_count,
        symbol_to_index,
        index_to_symbol,
        external_scanner_states,
        rules,
        nonterminal_to_index,
        goto_indexing: GotoIndexing::NonterminalMap, // Will be auto-detected
        eof_symbol,
        start_symbol: original_start,
        grammar: grammar.clone(),
        initial_state: StateId(0), // Default initial state
        token_count,
        external_token_count,
        lex_modes: vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0
            };
            state_count
        ],
        extras: vec![],             // TODO: Get from grammar metadata
        dynamic_prec_by_rule,       // Now properly populated from grammar rules
        rule_assoc_by_rule,         // Now properly populated from grammar rules
        alias_sequences: vec![],    // TODO: Get from grammar
        field_names: vec![],        // TODO: Get from grammar
        field_map: BTreeMap::new(), // TODO: Get from grammar
    };

    // Auto-detect GOTO indexing mode
    table.detect_goto_indexing();

    Ok(table)
}
