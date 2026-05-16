use crate::GLRError;
use adze_ir::*;
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct SymbolPartitions {
    pub(super) nonterminal_symbols: BTreeSet<SymbolId>,
    pub(super) external_symbols: BTreeSet<SymbolId>,
    pub(super) rhs_terminals: BTreeSet<SymbolId>,
    pub(super) max_symbol: u16,
    pub(super) eof_symbol: SymbolId,
}

impl SymbolPartitions {
    pub(super) fn collect(grammar: &Grammar) -> Result<Self, GLRError> {
        let nonterminal_symbols: BTreeSet<SymbolId> = grammar.rules.keys().copied().collect();
        let external_symbols: BTreeSet<SymbolId> =
            grammar.externals.iter().map(|e| e.symbol_id).collect();
        let mut rhs_terminals: BTreeSet<SymbolId> = BTreeSet::new();
        for rule in grammar.all_rules() {
            for sym in &rule.rhs {
                if let Symbol::Terminal(id) = sym {
                    rhs_terminals.insert(*id);
                }
            }
        }

        let max_symbol = grammar
            .tokens
            .keys()
            .chain(grammar.rule_names.keys())
            .chain(nonterminal_symbols.iter())
            .chain(external_symbols.iter())
            .chain(rhs_terminals.iter())
            .map(|s| s.0)
            .max()
            .unwrap_or(0);
        let eof_symbol = SymbolId(max_symbol.checked_add(1).ok_or_else(|| {
            GLRError::StateMachine(
                "EOF symbol would overflow u16: grammar has too many symbols".into(),
            )
        })?);

        Ok(Self {
            nonterminal_symbols,
            external_symbols,
            rhs_terminals,
            max_symbol,
            eof_symbol,
        })
    }
}

pub(super) struct SymbolIndex {
    pub(super) symbol_to_index: BTreeMap<SymbolId, usize>,
    pub(super) internal_tokens: Vec<SymbolId>,
    pub(super) ext_tokens: Vec<SymbolId>,
}

pub(super) fn build_symbol_index(
    grammar: &Grammar,
    partitions: &SymbolPartitions,
) -> Result<SymbolIndex, GLRError> {
    let mut symbol_to_index = BTreeMap::new();
    symbol_to_index.insert(partitions.eof_symbol, 0);

    let mut internal_terminals: BTreeSet<SymbolId> = grammar.tokens.keys().copied().collect();
    internal_terminals.extend(partitions.rhs_terminals.iter().copied());
    internal_terminals.remove(&partitions.eof_symbol);
    for id in &partitions.external_symbols {
        internal_terminals.remove(id);
    }
    for id in &partitions.nonterminal_symbols {
        internal_terminals.remove(id);
    }

    let mut internal_tokens: Vec<SymbolId> = internal_terminals.into_iter().collect();
    internal_tokens.sort_by_key(|s| s.0);
    for &id in &internal_tokens {
        if !symbol_to_index.contains_key(&id) {
            let idx = symbol_to_index.len();
            symbol_to_index.insert(id, idx);
        }
    }

    let mut ext_tokens: Vec<SymbolId> = partitions.external_symbols.iter().copied().collect();
    ext_tokens.sort_by_key(|s| s.0);
    for &id in &ext_tokens {
        if !symbol_to_index.contains_key(&id) {
            let idx = symbol_to_index.len();
            symbol_to_index.insert(id, idx);
        }
    }

    let mut non_terminals: Vec<SymbolId> = partitions.nonterminal_symbols.iter().copied().collect();
    non_terminals.sort_by_key(|s| s.0);
    for id in non_terminals {
        if !symbol_to_index.contains_key(&id) {
            let idx = symbol_to_index.len();
            symbol_to_index.insert(id, idx);
        }
    }

    let mut other_symbols: Vec<SymbolId> = grammar
        .rule_names
        .keys()
        .cloned()
        .filter(|id| !symbol_to_index.contains_key(id))
        .collect();
    other_symbols.sort_by_key(|s| s.0);
    if !other_symbols.is_empty() {
        return Err(GLRError::StateMachine(format!(
            "Unexpected symbols outside terminal/nonterminal partitions: {:?}",
            other_symbols
        )));
    }

    Ok(SymbolIndex {
        symbol_to_index,
        internal_tokens,
        ext_tokens,
    })
}

pub(super) fn build_reverse_symbol_index(
    symbol_to_index: &BTreeMap<SymbolId, usize>,
) -> Vec<SymbolId> {
    let mut index_to_symbol = vec![SymbolId(u16::MAX); symbol_to_index.len()];
    for (sym, &idx) in symbol_to_index {
        index_to_symbol[idx] = *sym;
    }
    index_to_symbol
}

pub(super) fn build_nonterminal_to_index(
    symbol_to_index: &BTreeMap<SymbolId, usize>,
    nonterminal_symbols: &BTreeSet<SymbolId>,
) -> BTreeMap<SymbolId, usize> {
    let mut nonterminal_to_index = BTreeMap::new();
    for (&symbol_id, &idx) in symbol_to_index {
        if nonterminal_symbols.contains(&symbol_id) {
            nonterminal_to_index.insert(symbol_id, idx);
        }
    }
    nonterminal_to_index
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(name: &str) -> Token {
        Token {
            name: name.to_string(),
            pattern: TokenPattern::Regex("x".to_string()),
            fragile: false,
        }
    }

    fn rule_with_rhs(lhs: SymbolId, rhs: Vec<Symbol>, production_id: ProductionId) -> Rule {
        Rule {
            lhs,
            rhs,
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id,
        }
    }

    #[test]
    fn partitions_collect_empty_grammar_has_eof_at_one() {
        // An empty grammar has max_symbol = 0 (the unwrap_or fallback), so eof = SymbolId(1).
        let grammar = Grammar::new("empty".to_string());
        let parts = SymbolPartitions::collect(&grammar).expect("collect should succeed");
        assert!(parts.nonterminal_symbols.is_empty());
        assert!(parts.external_symbols.is_empty());
        assert!(parts.rhs_terminals.is_empty());
        assert_eq!(parts.max_symbol, 0);
        assert_eq!(parts.eof_symbol, SymbolId(1));
    }

    #[test]
    fn partitions_collect_picks_max_symbol_across_pools() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.tokens.insert(SymbolId(3), token("t"));
        grammar.rules.insert(
            SymbolId(10),
            vec![rule_with_rhs(
                SymbolId(10),
                vec![Symbol::Terminal(SymbolId(3))],
                ProductionId(0),
            )],
        );
        grammar.rule_names.insert(SymbolId(10), "r".to_string());
        grammar.externals.push(ExternalToken {
            name: "e".to_string(),
            symbol_id: SymbolId(7),
        });

        let parts = SymbolPartitions::collect(&grammar).expect("collect should succeed");
        assert_eq!(parts.max_symbol, 10);
        assert_eq!(parts.eof_symbol, SymbolId(11));
        assert!(parts.nonterminal_symbols.contains(&SymbolId(10)));
        assert!(parts.external_symbols.contains(&SymbolId(7)));
        assert!(parts.rhs_terminals.contains(&SymbolId(3)));
    }

    #[test]
    fn partitions_collect_ignores_nonterminal_symbols_in_rhs_terminals() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.rules.insert(
            SymbolId(2),
            vec![rule_with_rhs(
                SymbolId(2),
                vec![
                    Symbol::NonTerminal(SymbolId(2)),
                    Symbol::External(SymbolId(5)),
                ],
                ProductionId(0),
            )],
        );

        let parts = SymbolPartitions::collect(&grammar).expect("collect should succeed");
        // Only Symbol::Terminal contributions show up in rhs_terminals.
        assert!(parts.rhs_terminals.is_empty());
    }

    #[test]
    fn partitions_collect_errors_when_eof_overflows_u16() {
        let mut grammar = Grammar::new("g".to_string());
        // max_symbol becomes u16::MAX => eof = MAX + 1 => overflow.
        grammar.tokens.insert(SymbolId(u16::MAX), token("max"));

        match SymbolPartitions::collect(&grammar) {
            Err(GLRError::StateMachine(msg)) => assert!(
                msg.contains("overflow"),
                "expected overflow message, got: {msg}"
            ),
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("collect should fail on u16 overflow"),
        }
    }

    #[test]
    fn build_symbol_index_places_eof_at_zero() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.tokens.insert(SymbolId(1), token("a"));
        let parts = SymbolPartitions::collect(&grammar).expect("collect");

        let idx = build_symbol_index(&grammar, &parts).expect("build index");
        assert_eq!(idx.symbol_to_index.get(&parts.eof_symbol), Some(&0));
    }

    #[test]
    fn build_symbol_index_orders_internal_then_external_then_nonterminal() {
        let mut grammar = Grammar::new("g".to_string());
        // Internal terminals from tokens: SymbolId(3), SymbolId(5).
        grammar.tokens.insert(SymbolId(3), token("a"));
        grammar.tokens.insert(SymbolId(5), token("b"));
        // Nonterminal: SymbolId(10).
        grammar.rules.insert(
            SymbolId(10),
            vec![rule_with_rhs(SymbolId(10), vec![], ProductionId(0))],
        );
        // External: SymbolId(20).
        grammar.externals.push(ExternalToken {
            name: "ext".to_string(),
            symbol_id: SymbolId(20),
        });

        let parts = SymbolPartitions::collect(&grammar).expect("collect");
        let idx = build_symbol_index(&grammar, &parts).expect("build index");

        // EOF at 0, then internal terminals sorted (3, 5), then externals (20), then NT (10).
        assert_eq!(idx.symbol_to_index.get(&parts.eof_symbol), Some(&0));
        assert_eq!(idx.symbol_to_index.get(&SymbolId(3)), Some(&1));
        assert_eq!(idx.symbol_to_index.get(&SymbolId(5)), Some(&2));
        assert_eq!(idx.symbol_to_index.get(&SymbolId(20)), Some(&3));
        assert_eq!(idx.symbol_to_index.get(&SymbolId(10)), Some(&4));

        assert_eq!(idx.internal_tokens, vec![SymbolId(3), SymbolId(5)]);
        assert_eq!(idx.ext_tokens, vec![SymbolId(20)]);
    }

    #[test]
    fn build_symbol_index_removes_externals_and_nonterminals_from_internals() {
        let mut grammar = Grammar::new("g".to_string());
        // SymbolId(4) appears both as a token AND as an external — externals win.
        grammar.tokens.insert(SymbolId(4), token("dup"));
        grammar.externals.push(ExternalToken {
            name: "dup".to_string(),
            symbol_id: SymbolId(4),
        });
        // SymbolId(6) is a nonterminal that happens to also have a token entry.
        grammar.tokens.insert(SymbolId(6), token("nt_token"));
        grammar.rules.insert(
            SymbolId(6),
            vec![rule_with_rhs(SymbolId(6), vec![], ProductionId(0))],
        );

        let parts = SymbolPartitions::collect(&grammar).expect("collect");
        let idx = build_symbol_index(&grammar, &parts).expect("build index");

        // Internal terminals are empty: both candidates were removed.
        assert!(idx.internal_tokens.is_empty());
        assert_eq!(idx.ext_tokens, vec![SymbolId(4)]);
        // SymbolId(6) lives in symbol_to_index as a nonterminal entry.
        assert!(idx.symbol_to_index.contains_key(&SymbolId(6)));
    }

    #[test]
    fn build_symbol_index_picks_up_rhs_terminals_not_in_tokens() {
        let mut grammar = Grammar::new("g".to_string());
        // SymbolId(8) is only referenced via Symbol::Terminal on an RHS.
        grammar.rules.insert(
            SymbolId(2),
            vec![rule_with_rhs(
                SymbolId(2),
                vec![Symbol::Terminal(SymbolId(8))],
                ProductionId(0),
            )],
        );

        let parts = SymbolPartitions::collect(&grammar).expect("collect");
        let idx = build_symbol_index(&grammar, &parts).expect("build index");

        assert!(idx.internal_tokens.contains(&SymbolId(8)));
        assert!(idx.symbol_to_index.contains_key(&SymbolId(8)));
    }

    #[test]
    fn build_symbol_index_errors_on_rule_name_outside_partitions() {
        // rule_names entry that is NOT a nonterminal, terminal, or external triggers
        // the StateMachine error path.
        let mut grammar = Grammar::new("g".to_string());
        grammar
            .rule_names
            .insert(SymbolId(50), "orphan".to_string());

        let parts = SymbolPartitions::collect(&grammar).expect("collect");
        match build_symbol_index(&grammar, &parts) {
            Err(GLRError::StateMachine(msg)) => assert!(
                msg.contains("Unexpected symbols"),
                "expected unexpected-symbols message, got: {msg}"
            ),
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("expected StateMachine error for orphan rule name"),
        }
    }

    #[test]
    fn build_reverse_symbol_index_empty_map_yields_empty_vec() {
        let map: BTreeMap<SymbolId, usize> = BTreeMap::new();
        let result = build_reverse_symbol_index(&map);
        assert!(result.is_empty());
    }

    #[test]
    fn build_reverse_symbol_index_inverts_mapping() {
        let mut map: BTreeMap<SymbolId, usize> = BTreeMap::new();
        map.insert(SymbolId(7), 0);
        map.insert(SymbolId(3), 2);
        map.insert(SymbolId(11), 1);

        let result = build_reverse_symbol_index(&map);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], SymbolId(7));
        assert_eq!(result[1], SymbolId(11));
        assert_eq!(result[2], SymbolId(3));
    }

    #[test]
    fn build_reverse_symbol_index_fills_gaps_with_max_sentinel() {
        // Length comes from map.len(); indices outside the range stay at the sentinel.
        let mut map: BTreeMap<SymbolId, usize> = BTreeMap::new();
        map.insert(SymbolId(1), 0);
        map.insert(SymbolId(2), 2); // skip index 1 inside a len-3 vector
        // Boost len to 3 with another mapping at index 1 missing -> sentinel.
        // Map has 2 entries -> len 2, so the index-2 write would panic. Use three entries.
        map.insert(SymbolId(3), 1);
        let result = build_reverse_symbol_index(&map);
        assert_eq!(result[0], SymbolId(1));
        assert_eq!(result[1], SymbolId(3));
        assert_eq!(result[2], SymbolId(2));
    }

    #[test]
    fn build_nonterminal_to_index_empty_inputs_yield_empty_map() {
        let map: BTreeMap<SymbolId, usize> = BTreeMap::new();
        let nts: BTreeSet<SymbolId> = BTreeSet::new();
        let result = build_nonterminal_to_index(&map, &nts);
        assert!(result.is_empty());
    }

    #[test]
    fn build_nonterminal_to_index_filters_out_non_nonterminals() {
        let mut map: BTreeMap<SymbolId, usize> = BTreeMap::new();
        map.insert(SymbolId(1), 0); // terminal
        map.insert(SymbolId(2), 1); // nonterminal
        map.insert(SymbolId(3), 2); // terminal
        map.insert(SymbolId(4), 3); // nonterminal

        let mut nts: BTreeSet<SymbolId> = BTreeSet::new();
        nts.insert(SymbolId(2));
        nts.insert(SymbolId(4));

        let result = build_nonterminal_to_index(&map, &nts);
        assert_eq!(result.len(), 2);
        assert_eq!(result.get(&SymbolId(2)), Some(&1));
        assert_eq!(result.get(&SymbolId(4)), Some(&3));
        assert!(!result.contains_key(&SymbolId(1)));
        assert!(!result.contains_key(&SymbolId(3)));
    }

    #[test]
    fn build_nonterminal_to_index_ignores_unmapped_nonterminals() {
        // A nonterminal listed but not in symbol_to_index is silently skipped.
        let mut map: BTreeMap<SymbolId, usize> = BTreeMap::new();
        map.insert(SymbolId(1), 0);

        let mut nts: BTreeSet<SymbolId> = BTreeSet::new();
        nts.insert(SymbolId(99)); // not in map

        let result = build_nonterminal_to_index(&map, &nts);
        assert!(result.is_empty());
    }
}
