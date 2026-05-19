use crate::{Symbol, SymbolId};
use std::collections::{HashSet, VecDeque};

pub(super) fn collect_used_in_symbol(symbol: &Symbol, used: &mut HashSet<SymbolId>) {
    match symbol {
        Symbol::Terminal(id) | Symbol::NonTerminal(id) => {
            used.insert(*id);
        }
        Symbol::External(id) => {
            used.insert(SymbolId(id.0));
        }
        Symbol::Optional(inner) | Symbol::Repeat(inner) | Symbol::RepeatOne(inner) => {
            collect_used_in_symbol(inner, used);
        }
        Symbol::Choice(choices) => {
            for s in choices {
                collect_used_in_symbol(s, used);
            }
        }
        Symbol::Sequence(seq) => {
            for s in seq {
                collect_used_in_symbol(s, used);
            }
        }
        Symbol::Epsilon => {}
    }
}

pub(super) fn add_reachable_from_symbol(
    symbol: &Symbol,
    reachable: &mut HashSet<SymbolId>,
    queue: &mut VecDeque<SymbolId>,
) {
    match symbol {
        Symbol::Terminal(id) | Symbol::NonTerminal(id) => {
            if reachable.insert(*id) {
                queue.push_back(*id);
            }
        }
        Symbol::External(ext_id) => {
            let id = SymbolId(ext_id.0);
            if reachable.insert(id) {
                queue.push_back(id);
            }
        }
        Symbol::Optional(inner) | Symbol::Repeat(inner) | Symbol::RepeatOne(inner) => {
            add_reachable_from_symbol(inner, reachable, queue);
        }
        Symbol::Choice(choices) => {
            for s in choices {
                add_reachable_from_symbol(s, reachable, queue);
            }
        }
        Symbol::Sequence(seq) => {
            for s in seq {
                add_reachable_from_symbol(s, reachable, queue);
            }
        }
        Symbol::Epsilon => {}
    }
}

pub(super) fn is_symbol_productive(symbol: &Symbol, productive: &HashSet<SymbolId>) -> bool {
    match symbol {
        Symbol::Terminal(id) | Symbol::NonTerminal(id) => productive.contains(id),
        Symbol::External(ext_id) => productive.contains(&SymbolId(ext_id.0)),
        Symbol::Epsilon => true,
        Symbol::Optional(_) => true,
        Symbol::Repeat(_) => true,
        Symbol::RepeatOne(inner) => is_symbol_productive(inner, productive),
        Symbol::Choice(choices) => choices.iter().any(|s| is_symbol_productive(s, productive)),
        Symbol::Sequence(seq) => seq.iter().all(|s| is_symbol_productive(s, productive)),
    }
}
