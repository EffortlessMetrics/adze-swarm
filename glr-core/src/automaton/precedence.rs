use adze_ir::*;
use std::collections::BTreeMap;

/// Internal EOF sentinel used by FirstFollowSets.
/// This is NOT the actual EOF symbol - use `parse_table.eof_symbol` for that.
const EOF_SENTINEL: SymbolId = SymbolId(0);

/// Map a symbol from FOLLOW set output to actual parse table symbol.
/// Replaces the EOF sentinel (SymbolId(0)) with the actual EOF symbol.
#[inline]
pub(super) fn map_follow_symbol(sym: SymbolId, eof_symbol: SymbolId) -> SymbolId {
    if sym == EOF_SENTINEL { eof_symbol } else { sym }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Assoc {
    Left,
    Right,
    None,
}

#[derive(Copy, Clone, Debug)]
struct TokPrec {
    prec: u8,
    assoc: Assoc,
}

#[derive(Copy, Clone, Debug)]
struct RulePrec {
    prec: u8,
    assoc: Assoc,
}

pub(super) struct PrecTables {
    // table-indexed; entries 0..token_count-1 may be Some(..); others None
    tok_prec_by_index: Vec<Option<TokPrec>>,
    // production_id -> precedence and associativity
    rule_prec: Vec<RulePrec>,
}

pub(super) fn build_prec_tables(
    grammar: &Grammar,
    symbol_to_index: &BTreeMap<SymbolId, usize>,
    token_count: u32,
    production_count: u32,
) -> PrecTables {
    use adze_ir::{Associativity, PrecedenceKind};

    debug_assert!(production_count > 0, "production_count must be positive");

    let mut tok_prec_by_index = vec![None; symbol_to_index.len()];
    let tok_prec_len = tok_prec_by_index.len();

    let mut set_tok_prec = |tok_idx: usize, new: TokPrec| {
        if tok_idx >= tok_prec_by_index.len() {
            return;
        }
        tok_prec_by_index[tok_idx] = match tok_prec_by_index[tok_idx] {
            None => Some(new),
            Some(old) => Some(if new.prec > old.prec { new } else { old }),
        };
    };

    let mut rule_prec = vec![
        RulePrec {
            prec: 0,
            assoc: Assoc::None,
        };
        production_count as usize
    ];

    for rules in grammar.rules.values() {
        for rule in rules {
            let pid = rule.production_id.0 as usize;
            if pid >= production_count as usize {
                continue;
            }

            let explicit = rule.precedence.and_then(|p| {
                if let PrecedenceKind::Static(level) = p {
                    Some(level as u8)
                } else {
                    None
                }
            });

            let rule_assoc = rule
                .associativity
                .map(|assoc| match assoc {
                    Associativity::Left => Assoc::Left,
                    Associativity::Right => Assoc::Right,
                    Associativity::None => Assoc::None,
                })
                .unwrap_or(Assoc::None);

            if let Some(level) = explicit {
                let tok_idx_opt = rule.rhs.iter().rev().find_map(|sym| {
                    if let Symbol::Terminal(id) = sym {
                        symbol_to_index.get(id).copied()
                    } else {
                        None
                    }
                });

                if let Some(tok_idx) = tok_idx_opt
                    && tok_idx < tok_prec_len
                {
                    set_tok_prec(
                        tok_idx,
                        TokPrec {
                            prec: level,
                            assoc: rule_assoc,
                        },
                    );
                }
            }

            rule_prec[pid] = RulePrec {
                prec: explicit.unwrap_or(0),
                assoc: rule_assoc,
            };
        }
    }

    for rules in grammar.rules.values() {
        for rule in rules {
            let pid = rule.production_id.0 as usize;
            if pid >= production_count as usize {
                continue;
            }

            if rule_prec[pid].prec > 0 {
                continue;
            }

            let derived = rule
                .rhs
                .iter()
                .rev()
                .find_map(|sym| {
                    if let Symbol::Terminal(id) = sym {
                        symbol_to_index.get(id).and_then(|&idx| {
                            if (idx as u32) < token_count {
                                tok_prec_by_index[idx]
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    }
                })
                .unwrap_or(TokPrec {
                    prec: 0,
                    assoc: Assoc::None,
                });

            rule_prec[pid] = RulePrec {
                prec: derived.prec,
                assoc: derived.assoc,
            };
        }
    }

    PrecTables {
        tok_prec_by_index,
        rule_prec,
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum PrecDecision {
    PreferShift,
    PreferReduce,
    Error,
    NoInfo,
}

#[inline]
pub(super) fn decide_with_precedence(
    lookahead_tok_idx: usize,
    reduce_prod_id: u16,
    prec: &PrecTables,
) -> PrecDecision {
    if reduce_prod_id as usize >= prec.rule_prec.len() {
        return PrecDecision::NoInfo;
    }

    let tokp = match prec
        .tok_prec_by_index
        .get(lookahead_tok_idx)
        .and_then(|o| *o)
    {
        Some(p) => p,
        None => return PrecDecision::NoInfo,
    };
    let rulep = prec.rule_prec[reduce_prod_id as usize];

    if tokp.prec == 0 || rulep.prec == 0 {
        return PrecDecision::NoInfo;
    }

    use core::cmp::Ordering::*;
    match (tokp.prec.cmp(&rulep.prec), rulep.assoc) {
        (Greater, _) => PrecDecision::PreferShift,
        (Less, _) => PrecDecision::PreferReduce,
        (Equal, Assoc::Left) => PrecDecision::PreferReduce,
        (Equal, Assoc::Right) => PrecDecision::PreferShift,
        (Equal, Assoc::None) => PrecDecision::Error,
    }
}

#[inline]
pub(super) fn decide_reduce_reduce(a: u16, b: u16, prec: &PrecTables) -> Option<u16> {
    let pa = prec.rule_prec.get(a as usize).map(|r| r.prec).unwrap_or(0);
    let pb = prec.rule_prec.get(b as usize).map(|r| r.prec).unwrap_or(0);
    if pa > pb {
        Some(a)
    } else if pb > pa {
        Some(b)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::{
        Associativity, Grammar, PrecedenceKind, ProductionId, Rule, Symbol, SymbolId, Token,
        TokenPattern,
    };

    // ---- helpers --------------------------------------------------------

    fn make_token(name: &str) -> Token {
        Token {
            name: name.to_string(),
            pattern: TokenPattern::String(name.to_string()),
            fragile: false,
        }
    }

    fn make_rule(
        lhs: SymbolId,
        rhs: Vec<Symbol>,
        prec: Option<PrecedenceKind>,
        assoc: Option<Associativity>,
        pid: u16,
    ) -> Rule {
        Rule {
            lhs,
            rhs,
            precedence: prec,
            associativity: assoc,
            fields: vec![],
            production_id: ProductionId(pid),
        }
    }

    /// Build a tiny test PrecTables directly without going through `build_prec_tables`,
    /// so individual decision helpers can be exercised in isolation.
    fn tables_for_decision(rules: Vec<(u8, Assoc)>, tokens: Vec<(u8, Assoc)>) -> PrecTables {
        PrecTables {
            tok_prec_by_index: tokens
                .into_iter()
                .map(|(prec, assoc)| {
                    if prec == 0 {
                        None
                    } else {
                        Some(TokPrec { prec, assoc })
                    }
                })
                .collect(),
            rule_prec: rules
                .into_iter()
                .map(|(prec, assoc)| RulePrec { prec, assoc })
                .collect(),
        }
    }

    // ---- map_follow_symbol ---------------------------------------------

    #[test]
    fn map_follow_symbol_remaps_eof_sentinel() {
        let eof = SymbolId(42);
        assert_eq!(map_follow_symbol(SymbolId(0), eof), eof);
    }

    #[test]
    fn map_follow_symbol_passes_other_symbols_through() {
        let eof = SymbolId(42);
        assert_eq!(map_follow_symbol(SymbolId(7), eof), SymbolId(7));
    }

    // ---- decide_with_precedence ----------------------------------------

    #[test]
    fn decide_with_precedence_out_of_range_prod_returns_no_info() {
        let prec = tables_for_decision(vec![(1, Assoc::Left)], vec![(1, Assoc::Left)]);
        assert_eq!(
            decide_with_precedence(0, 5, &prec),
            PrecDecision::NoInfo,
            "reduce_prod_id beyond rule_prec must yield NoInfo"
        );
    }

    #[test]
    fn decide_with_precedence_out_of_range_lookahead_returns_no_info() {
        let prec = tables_for_decision(vec![(1, Assoc::Left)], vec![(1, Assoc::Left)]);
        assert_eq!(
            decide_with_precedence(9, 0, &prec),
            PrecDecision::NoInfo,
            "lookahead_tok_idx beyond tok_prec_by_index must yield NoInfo"
        );
    }

    #[test]
    fn decide_with_precedence_zero_token_prec_returns_no_info() {
        // tokp.prec == 0 path: missing entry in tok_prec_by_index slot.
        let prec = tables_for_decision(vec![(2, Assoc::Left)], vec![(0, Assoc::Left)]);
        assert_eq!(decide_with_precedence(0, 0, &prec), PrecDecision::NoInfo);
    }

    #[test]
    fn decide_with_precedence_zero_rule_prec_returns_no_info() {
        let prec = tables_for_decision(vec![(0, Assoc::Left)], vec![(2, Assoc::Left)]);
        assert_eq!(decide_with_precedence(0, 0, &prec), PrecDecision::NoInfo);
    }

    #[test]
    fn decide_with_precedence_higher_token_prefers_shift() {
        let prec = tables_for_decision(vec![(1, Assoc::Left)], vec![(3, Assoc::Left)]);
        assert_eq!(
            decide_with_precedence(0, 0, &prec),
            PrecDecision::PreferShift
        );
    }

    #[test]
    fn decide_with_precedence_lower_token_prefers_reduce() {
        let prec = tables_for_decision(vec![(5, Assoc::Left)], vec![(2, Assoc::Left)]);
        assert_eq!(
            decide_with_precedence(0, 0, &prec),
            PrecDecision::PreferReduce
        );
    }

    #[test]
    fn decide_with_precedence_equal_left_assoc_prefers_reduce() {
        let prec = tables_for_decision(vec![(2, Assoc::Left)], vec![(2, Assoc::Left)]);
        assert_eq!(
            decide_with_precedence(0, 0, &prec),
            PrecDecision::PreferReduce
        );
    }

    #[test]
    fn decide_with_precedence_equal_right_assoc_prefers_shift() {
        let prec = tables_for_decision(vec![(2, Assoc::Right)], vec![(2, Assoc::Right)]);
        assert_eq!(
            decide_with_precedence(0, 0, &prec),
            PrecDecision::PreferShift
        );
    }

    #[test]
    fn decide_with_precedence_equal_none_assoc_returns_error() {
        let prec = tables_for_decision(vec![(2, Assoc::None)], vec![(2, Assoc::None)]);
        assert_eq!(decide_with_precedence(0, 0, &prec), PrecDecision::Error);
    }

    // ---- decide_reduce_reduce ------------------------------------------

    #[test]
    fn decide_reduce_reduce_higher_a_wins() {
        let prec = tables_for_decision(vec![(3, Assoc::Left), (1, Assoc::Left)], vec![]);
        assert_eq!(decide_reduce_reduce(0, 1, &prec), Some(0));
    }

    #[test]
    fn decide_reduce_reduce_higher_b_wins() {
        let prec = tables_for_decision(vec![(1, Assoc::Left), (3, Assoc::Left)], vec![]);
        assert_eq!(decide_reduce_reduce(0, 1, &prec), Some(1));
    }

    #[test]
    fn decide_reduce_reduce_equal_preserves_conflict() {
        let prec = tables_for_decision(vec![(2, Assoc::Left), (2, Assoc::Left)], vec![]);
        assert_eq!(decide_reduce_reduce(0, 1, &prec), None);
    }

    #[test]
    fn decide_reduce_reduce_out_of_bounds_preserves_conflict() {
        // Both ids out of range -> both default precs are 0 -> tie -> no resolution.
        let prec = tables_for_decision(vec![], vec![]);
        assert_eq!(decide_reduce_reduce(7, 4, &prec), None);
    }

    // ---- build_prec_tables --------------------------------------------

    /// Build a grammar with the shape:
    ///   expr -> expr PLUS expr  (PrecedenceKind::Static(2), Associativity::Left, pid=0)
    ///   expr -> expr STAR expr  (PrecedenceKind::Static(3), Associativity::Right, pid=1)
    ///   expr -> NUM             (no explicit precedence,                          pid=2)
    fn arith_grammar() -> (Grammar, BTreeMap<SymbolId, usize>, u32, u32) {
        let expr = SymbolId(1);
        let plus = SymbolId(2);
        let star = SymbolId(3);
        let num = SymbolId(4);

        let mut grammar = Grammar::new("arith".to_string());
        grammar.tokens.insert(plus, make_token("plus"));
        grammar.tokens.insert(star, make_token("star"));
        grammar.tokens.insert(num, make_token("num"));

        grammar.rules.entry(expr).or_default().push(make_rule(
            expr,
            vec![
                Symbol::NonTerminal(expr),
                Symbol::Terminal(plus),
                Symbol::NonTerminal(expr),
            ],
            Some(PrecedenceKind::Static(2)),
            Some(Associativity::Left),
            0,
        ));
        grammar.rules.entry(expr).or_default().push(make_rule(
            expr,
            vec![
                Symbol::NonTerminal(expr),
                Symbol::Terminal(star),
                Symbol::NonTerminal(expr),
            ],
            Some(PrecedenceKind::Static(3)),
            Some(Associativity::Right),
            1,
        ));
        grammar.rules.entry(expr).or_default().push(make_rule(
            expr,
            vec![Symbol::Terminal(num)],
            None,
            None,
            2,
        ));

        // Symbol indexing: terminals are indexed first; non-terminal goes after token_count.
        let mut symbol_to_index = BTreeMap::new();
        symbol_to_index.insert(plus, 0usize);
        symbol_to_index.insert(star, 1usize);
        symbol_to_index.insert(num, 2usize);
        symbol_to_index.insert(expr, 3usize);

        let token_count: u32 = 3;
        let production_count: u32 = 3;

        (grammar, symbol_to_index, token_count, production_count)
    }

    #[test]
    fn build_prec_tables_produces_tables_with_expected_dimensions() {
        let (grammar, symbol_to_index, token_count, production_count) = arith_grammar();
        let tables = build_prec_tables(&grammar, &symbol_to_index, token_count, production_count);
        assert_eq!(tables.rule_prec.len(), production_count as usize);
        assert_eq!(tables.tok_prec_by_index.len(), symbol_to_index.len());
    }

    #[test]
    fn build_prec_tables_explicit_precedence_populates_rule_and_token_slot() {
        let (grammar, symbol_to_index, token_count, production_count) = arith_grammar();
        let tables = build_prec_tables(&grammar, &symbol_to_index, token_count, production_count);

        // Rule 0 (plus): precedence 2, left.
        assert_eq!(tables.rule_prec[0].prec, 2);
        assert_eq!(tables.rule_prec[0].assoc, Assoc::Left);

        // Rightmost terminal of rule 0 is `plus` at index 0.
        let plus_slot = tables.tok_prec_by_index[0].expect("plus must have token prec");
        assert_eq!(plus_slot.prec, 2);
        assert_eq!(plus_slot.assoc, Assoc::Left);
    }

    #[test]
    fn build_prec_tables_maps_right_associativity_for_star() {
        let (grammar, symbol_to_index, token_count, production_count) = arith_grammar();
        let tables = build_prec_tables(&grammar, &symbol_to_index, token_count, production_count);

        // Rule 1 (star): precedence 3, right.
        assert_eq!(tables.rule_prec[1].prec, 3);
        assert_eq!(tables.rule_prec[1].assoc, Assoc::Right);

        let star_slot = tables.tok_prec_by_index[1].expect("star must have token prec");
        assert_eq!(star_slot.prec, 3);
        assert_eq!(star_slot.assoc, Assoc::Right);
    }

    #[test]
    fn build_prec_tables_none_associativity_maps_to_assoc_none() {
        // A rule with explicit precedence but Associativity::None must surface as Assoc::None.
        let expr = SymbolId(1);
        let bang = SymbolId(2);

        let mut grammar = Grammar::new("none-assoc".to_string());
        grammar.tokens.insert(bang, make_token("bang"));
        grammar.rules.entry(expr).or_default().push(make_rule(
            expr,
            vec![Symbol::NonTerminal(expr), Symbol::Terminal(bang)],
            Some(PrecedenceKind::Static(4)),
            Some(Associativity::None),
            0,
        ));

        let mut symbol_to_index = BTreeMap::new();
        symbol_to_index.insert(bang, 0usize);
        symbol_to_index.insert(expr, 1usize);

        let tables = build_prec_tables(&grammar, &symbol_to_index, 1, 1);
        assert_eq!(tables.rule_prec[0].assoc, Assoc::None);
        assert_eq!(tables.rule_prec[0].prec, 4);
        let bang_slot = tables.tok_prec_by_index[0].expect("bang must have token prec");
        assert_eq!(bang_slot.assoc, Assoc::None);
    }

    #[test]
    fn build_prec_tables_rule_without_precedence_inherits_from_rightmost_terminal() {
        // Construct a grammar where one rule sets the terminal's prec and another rule
        // ending in the same terminal has no explicit precedence; the latter should
        // inherit through the rightmost-terminal lookup.
        let stmt = SymbolId(1);
        let op = SymbolId(2);

        let mut grammar = Grammar::new("inherit".to_string());
        grammar.tokens.insert(op, make_token("op"));

        // Rule 0: explicit Static(5) Left; rightmost terminal is `op`.
        grammar.rules.entry(stmt).or_default().push(make_rule(
            stmt,
            vec![Symbol::Terminal(op)],
            Some(PrecedenceKind::Static(5)),
            Some(Associativity::Left),
            0,
        ));
        // Rule 1: no explicit precedence; same rightmost terminal `op`.
        grammar.rules.entry(stmt).or_default().push(make_rule(
            stmt,
            vec![Symbol::NonTerminal(stmt), Symbol::Terminal(op)],
            None,
            None,
            1,
        ));

        let mut symbol_to_index = BTreeMap::new();
        symbol_to_index.insert(op, 0usize);
        symbol_to_index.insert(stmt, 1usize);

        let tables = build_prec_tables(&grammar, &symbol_to_index, 1, 2);
        // Rule 1 inherits from `op` token, which was set to (5, Left) by rule 0.
        assert_eq!(tables.rule_prec[1].prec, 5);
        assert_eq!(tables.rule_prec[1].assoc, Assoc::Left);
    }

    #[test]
    fn build_prec_tables_skips_rules_with_out_of_range_production_id() {
        // A rule whose production_id exceeds production_count must be silently skipped
        // (no panic, no out-of-bounds write).
        let expr = SymbolId(1);
        let plus = SymbolId(2);

        let mut grammar = Grammar::new("oob".to_string());
        grammar.tokens.insert(plus, make_token("plus"));
        grammar.rules.entry(expr).or_default().push(make_rule(
            expr,
            vec![Symbol::Terminal(plus)],
            Some(PrecedenceKind::Static(9)),
            Some(Associativity::Left),
            // production_id 7 is out of range vs. production_count = 1
            7,
        ));

        let mut symbol_to_index = BTreeMap::new();
        symbol_to_index.insert(plus, 0usize);
        symbol_to_index.insert(expr, 1usize);

        let tables = build_prec_tables(&grammar, &symbol_to_index, 1, 1);
        // Table size still respects production_count, and the single slot stays at default.
        assert_eq!(tables.rule_prec.len(), 1);
        assert_eq!(tables.rule_prec[0].prec, 0);
        assert_eq!(tables.rule_prec[0].assoc, Assoc::None);
        // Token slot is also untouched because the rule was skipped before set_tok_prec.
        assert!(tables.tok_prec_by_index[0].is_none());
    }
}
