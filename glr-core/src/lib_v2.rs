//! Improved LR(1) automaton construction with proper conflict handling.

use crate::{
    Action, ActionCell, FirstFollowSets, GotoIndexing, ItemSetCollection, LexMode, ParseRule,
    ParseTable, StateId, SymbolId, SymbolMetadata, action_utils::action_eq,
};
use adze_ir::{Grammar, Symbol, TokenPattern};
use std::collections::{BTreeMap, HashMap};

/// Build LR(1) automaton with proper conflict handling for GLR parsing
pub fn build_lr1_automaton_v2(
    grammar: &Grammar,
    first_follow: &FirstFollowSets,
) -> Result<ParseTable, crate::GLRError> {
    // Build LR(1) item sets
    let collection = ItemSetCollection::build_canonical_collection(grammar, first_follow);

    // Create symbol to index mapping
    let mut symbol_to_index = BTreeMap::new();
    let mut index_to_symbol = Vec::new();

    let insert_symbol =
        |map: &mut BTreeMap<SymbolId, usize>, inv: &mut Vec<SymbolId>, id: SymbolId| {
            if !map.contains_key(&id) {
                let idx = map.len();
                map.insert(id, idx);
                inv.push(id);
            }
        };

    // Add terminal symbols
    for (symbol_id, _) in &grammar.tokens {
        insert_symbol(&mut symbol_to_index, &mut index_to_symbol, *symbol_id);
    }

    // Add non-terminal symbols
    for (symbol_id, _) in &grammar.rule_names {
        insert_symbol(&mut symbol_to_index, &mut index_to_symbol, *symbol_id);
    }

    // Add external symbols
    for external in &grammar.externals {
        insert_symbol(
            &mut symbol_to_index,
            &mut index_to_symbol,
            external.symbol_id,
        );
    }

    // Add EOF symbol
    insert_symbol(&mut symbol_to_index, &mut index_to_symbol, SymbolId(0));

    // Create parse table dimensions
    let state_count = collection.sets.len();
    let indexed_symbol_count = symbol_to_index.len();

    // ActionCell = Vec<Action>; one cell per (state, symbol)
    let mut action_table: Vec<Vec<ActionCell>> =
        vec![vec![vec![Action::Error]; indexed_symbol_count]; state_count];
    let mut goto_table = vec![vec![StateId(0); indexed_symbol_count]; state_count];

    // Track conflicts as we build the table
    let mut conflicts_by_state: HashMap<(usize, usize), Vec<Action>> = HashMap::new();

    // Fill action table with conflict detection
    for item_set in &collection.sets {
        let state_idx = item_set.id.0 as usize;

        for item in &item_set.items {
            if item.is_reduce_item(grammar) {
                if let Some(&lookahead_idx) = symbol_to_index.get(&item.lookahead) {
                    let new_action = Action::Reduce(item.rule_id);
                    add_action_with_conflict(
                        &mut action_table,
                        &mut conflicts_by_state,
                        state_idx,
                        lookahead_idx,
                        new_action,
                    );
                }
            } else if let Some(next_symbol) = item.next_symbol(grammar) {
                let symbol_id = match next_symbol {
                    Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => *id,
                    _ => continue,
                };

                if let Some(&symbol_idx) = symbol_to_index.get(&symbol_id)
                    && matches!(next_symbol, Symbol::Terminal(_))
                    && let Some(&goto_state) = collection.goto_table.get(&(item_set.id, symbol_id))
                {
                    let new_action = Action::Shift(goto_state);
                    add_action_with_conflict(
                        &mut action_table,
                        &mut conflicts_by_state,
                        state_idx,
                        symbol_idx,
                        new_action,
                    );
                }
            }
        }
    }

    // Convert conflicts to Fork actions
    for ((state_idx, symbol_idx), actions) in conflicts_by_state {
        if actions.len() > 1 {
            action_table[state_idx][symbol_idx] = vec![Action::Fork(actions)];
        } else if let Some(action) = actions.into_iter().next() {
            action_table[state_idx][symbol_idx] = vec![action];
        }
    }

    // Fill goto table
    for ((from_state, symbol), to_state) in &collection.goto_table {
        let from_idx = from_state.0 as usize;
        if let Some(&symbol_idx) = symbol_to_index.get(symbol) {
            goto_table[from_idx][symbol_idx] = *to_state;
        }
    }

    // Build symbol metadata
    let mut symbol_metadata = Vec::new();

    for (sym_id, token) in &grammar.tokens {
        symbol_metadata.push(SymbolMetadata {
            name: token.name.clone(),
            is_visible: !token.name.starts_with('_'),
            is_named: !matches!(&token.pattern, TokenPattern::String(_)),
            is_supertype: false,
            is_terminal: true,
            is_extra: false,
            is_fragile: false,
            symbol_id: *sym_id,
        });
    }

    for (symbol_id, name) in &grammar.rule_names {
        let is_supertype_val = grammar.supertypes.contains(symbol_id);
        symbol_metadata.push(SymbolMetadata {
            name: name.clone(),
            is_visible: true,
            is_named: true,
            is_supertype: is_supertype_val,
            is_terminal: false,
            is_extra: false,
            is_fragile: false,
            symbol_id: *symbol_id,
        });
    }

    for external in &grammar.externals {
        symbol_metadata.push(SymbolMetadata {
            name: external.name.clone(),
            is_visible: true,
            is_named: true,
            is_supertype: false,
            is_terminal: true,
            is_extra: false,
            is_fragile: false,
            symbol_id: external.symbol_id,
        });
    }

    symbol_metadata.push(SymbolMetadata {
        name: "_eof".to_string(),
        is_visible: false,
        is_named: false,
        is_supertype: false,
        is_terminal: true,
        is_extra: false,
        is_fragile: false,
        symbol_id: SymbolId(0),
    });

    // Build parse rules from grammar
    let rules: Vec<ParseRule> = grammar
        .all_rules()
        .map(|r| ParseRule {
            lhs: r.lhs,
            rhs_len: r.rhs.len() as u16,
        })
        .collect();

    // Nonterminal-to-index mapping
    let mut nonterminal_to_index = BTreeMap::new();
    for (i, (symbol_id, _)) in grammar.rule_names.iter().enumerate() {
        nonterminal_to_index.insert(*symbol_id, i);
    }

    let start_symbol = grammar.start_symbol().unwrap_or(SymbolId(0));

    Ok(ParseTable {
        action_table,
        goto_table,
        symbol_metadata,
        state_count,
        symbol_count: indexed_symbol_count,
        symbol_to_index,
        index_to_symbol,
        external_scanner_states: vec![vec![]; state_count],
        rules,
        nonterminal_to_index,
        goto_indexing: GotoIndexing::NonterminalMap,
        eof_symbol: SymbolId(0),
        start_symbol,
        grammar: grammar.clone(),
        initial_state: StateId(0),
        token_count: grammar.tokens.len(),
        external_token_count: grammar.externals.len(),
        lex_modes: vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0
            };
            state_count
        ],
        extras: grammar.extras.clone(),
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
    })
}

/// Add an action to the parse table, tracking conflicts
fn add_action_with_conflict(
    action_table: &mut [Vec<ActionCell>],
    conflicts_by_state: &mut HashMap<(usize, usize), Vec<Action>>,
    state_idx: usize,
    symbol_idx: usize,
    new_action: Action,
) {
    let cell = &action_table[state_idx][symbol_idx];
    let is_error_only =
        cell.len() == 1 && matches!(cell.first(), Some(Action::Error)) || cell.is_empty();

    if is_error_only {
        action_table[state_idx][symbol_idx] = vec![new_action.clone()];
    } else {
        // Conflict detected
        let entry = conflicts_by_state
            .entry((state_idx, symbol_idx))
            .or_default();

        if entry.is_empty() {
            for a in &action_table[state_idx][symbol_idx] {
                if let Action::Fork(actions) = a {
                    entry.extend(actions.clone());
                } else {
                    entry.push(a.clone());
                }
            }
        }

        if !entry.iter().any(|a| action_eq(a, &new_action)) {
            entry.push(new_action);
        }
    }
}
