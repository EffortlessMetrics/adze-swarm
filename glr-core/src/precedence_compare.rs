// Tree-sitter compatible precedence comparison logic
//! Tree-sitter compatible precedence comparison logic.

// Direct port of precedence comparison from Tree-sitter's C implementation

use adze_ir::{Associativity, Grammar, PrecedenceKind, RuleId, Symbol, SymbolId};
use std::collections::HashMap;

/// Precedence information for a symbol or rule
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrecedenceInfo {
    pub level: i16,
    pub associativity: Associativity,
    pub is_fragile: bool,
}

/// Static precedence resolver matching Tree-sitter's exact behavior
pub struct StaticPrecedenceResolver {
    /// Token precedences from grammar declarations
    token_precedences: HashMap<SymbolId, PrecedenceInfo>,
    /// Rule precedences (by production ID)
    rule_precedences: HashMap<RuleId, PrecedenceInfo>,
}

impl StaticPrecedenceResolver {
    /// Build precedence tables from grammar
    pub fn from_grammar(grammar: &Grammar) -> Self {
        let mut token_precedences = HashMap::new();
        let mut rule_precedences = HashMap::new();

        // Extract token precedences from precedence declarations
        for prec_decl in &grammar.precedences {
            for &symbol_id in &prec_decl.symbols {
                token_precedences.insert(
                    symbol_id,
                    PrecedenceInfo {
                        level: prec_decl.level,
                        associativity: prec_decl.associativity,
                        is_fragile: false, // Set based on grammar annotations
                    },
                );
            }
        }

        // Extract rule precedences
        for rules in grammar.rules.values() {
            for rule in rules {
                if let Some(PrecedenceKind::Static(level)) = &rule.precedence {
                    let assoc = rule.associativity.unwrap_or(Associativity::None);
                    rule_precedences.insert(
                        RuleId(rule.production_id.0),
                        PrecedenceInfo {
                            level: *level,
                            associativity: assoc,
                            is_fragile: false,
                        },
                    );

                    // Also extract token precedences from rules with precedence
                    // This is how Tree-sitter determines token precedence - from the rules that use them
                    for symbol in &rule.rhs {
                        if let Symbol::Terminal(token_id) = symbol {
                            // Only set if not already set by explicit precedence declaration
                            if !token_precedences.contains_key(token_id) {
                                token_precedences.insert(
                                    *token_id,
                                    PrecedenceInfo {
                                        level: *level,
                                        associativity: assoc,
                                        is_fragile: false,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }

        Self {
            token_precedences,
            rule_precedences,
        }
    }

    /// Get precedence info for a token
    pub fn token_precedence(&self, symbol: SymbolId) -> Option<PrecedenceInfo> {
        self.token_precedences.get(&symbol).copied()
    }

    /// Get precedence info for a rule
    pub fn rule_precedence(&self, rule: RuleId) -> Option<PrecedenceInfo> {
        self.rule_precedences.get(&rule).copied()
    }
}

/// Result of precedence comparison
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrecedenceComparison {
    /// Shift action wins
    PreferShift,
    /// Reduce action wins
    PreferReduce,
    /// Conflict is an error (non-associative)
    Error,
    /// No precedence information to decide
    None,
}

/// Compare precedences for shift/reduce conflict resolution
/// Direct port of Tree-sitter's precedence comparison logic
pub fn compare_precedences(
    shift_prec: Option<PrecedenceInfo>,
    reduce_prec: Option<PrecedenceInfo>,
) -> PrecedenceComparison {
    match (shift_prec, reduce_prec) {
        (None, _) | (_, None) => PrecedenceComparison::None,
        (Some(shift), Some(reduce)) => {
            // Higher precedence wins
            if shift.level > reduce.level {
                PrecedenceComparison::PreferShift
            } else if reduce.level > shift.level {
                PrecedenceComparison::PreferReduce
            } else {
                // Same precedence - check associativity
                match reduce.associativity {
                    Associativity::Left => PrecedenceComparison::PreferReduce,
                    Associativity::Right => PrecedenceComparison::PreferShift,
                    Associativity::None => PrecedenceComparison::Error,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::{Grammar, Precedence};

    #[test]
    fn test_precedence_comparison() {
        // Higher precedence wins
        let shift = PrecedenceInfo {
            level: 2,
            associativity: Associativity::Left,
            is_fragile: false,
        };
        let reduce = PrecedenceInfo {
            level: 1,
            associativity: Associativity::Left,
            is_fragile: false,
        };
        assert_eq!(
            compare_precedences(Some(shift), Some(reduce)),
            PrecedenceComparison::PreferShift
        );

        // Same level, left associative
        let shift = PrecedenceInfo {
            level: 1,
            associativity: Associativity::Left,
            is_fragile: false,
        };
        let reduce = PrecedenceInfo {
            level: 1,
            associativity: Associativity::Left,
            is_fragile: false,
        };
        assert_eq!(
            compare_precedences(Some(shift), Some(reduce)),
            PrecedenceComparison::PreferReduce
        );

        // Same level, right associative
        let reduce_right = PrecedenceInfo {
            level: 1,
            associativity: Associativity::Right,
            is_fragile: false,
        };
        assert_eq!(
            compare_precedences(Some(shift), Some(reduce_right)),
            PrecedenceComparison::PreferShift
        );

        // Same level, non-associative
        let reduce_none = PrecedenceInfo {
            level: 1,
            associativity: Associativity::None,
            is_fragile: false,
        };
        assert_eq!(
            compare_precedences(Some(shift), Some(reduce_none)),
            PrecedenceComparison::Error
        );
    }

    #[test]
    fn token_precedence_missing_returns_none() {
        let grammar = Grammar::new("empty".to_string());
        let resolver = StaticPrecedenceResolver::from_grammar(&grammar);
        assert!(resolver.token_precedence(SymbolId(0)).is_none());
        assert!(resolver.token_precedence(SymbolId(42)).is_none());
    }

    #[test]
    fn rule_precedence_missing_returns_none() {
        let grammar = Grammar::new("empty".to_string());
        let resolver = StaticPrecedenceResolver::from_grammar(&grammar);
        assert!(resolver.rule_precedence(RuleId(0)).is_none());
        assert!(resolver.rule_precedence(RuleId(7)).is_none());
    }

    #[test]
    fn from_grammar_empty_grammar_produces_empty_resolver() {
        let grammar = Grammar::new("empty".to_string());
        let resolver = StaticPrecedenceResolver::from_grammar(&grammar);
        assert!(resolver.token_precedence(SymbolId(0)).is_none());
        assert!(resolver.rule_precedence(RuleId(0)).is_none());
    }

    #[test]
    fn compare_precedences_none_inputs_yield_no_decision() {
        let some = PrecedenceInfo {
            level: 1,
            associativity: Associativity::Left,
            is_fragile: false,
        };
        assert_eq!(
            compare_precedences(None, Some(some)),
            PrecedenceComparison::None
        );
        assert_eq!(
            compare_precedences(Some(some), None),
            PrecedenceComparison::None
        );
        assert_eq!(compare_precedences(None, None), PrecedenceComparison::None);
    }

    #[test]
    fn compare_precedences_higher_reduce_wins() {
        let shift = PrecedenceInfo {
            level: 1,
            associativity: Associativity::Left,
            is_fragile: false,
        };
        let reduce = PrecedenceInfo {
            level: 5,
            associativity: Associativity::Left,
            is_fragile: false,
        };
        assert_eq!(
            compare_precedences(Some(shift), Some(reduce)),
            PrecedenceComparison::PreferReduce
        );
    }

    #[test]
    fn test_precedence_extraction() {
        let mut grammar = Grammar::new("test".to_string());

        // Add precedence declarations
        grammar.precedences.push(Precedence {
            level: 1,
            associativity: Associativity::Left,
            symbols: vec![SymbolId(10), SymbolId(11)],
        });

        grammar.precedences.push(Precedence {
            level: 2,
            associativity: Associativity::Right,
            symbols: vec![SymbolId(20)],
        });

        let resolver = StaticPrecedenceResolver::from_grammar(&grammar);

        // Check token precedences
        let prec10 = resolver.token_precedence(SymbolId(10)).unwrap();
        assert_eq!(prec10.level, 1);
        assert_eq!(prec10.associativity, Associativity::Left);

        let prec20 = resolver.token_precedence(SymbolId(20)).unwrap();
        assert_eq!(prec20.level, 2);
        assert_eq!(prec20.associativity, Associativity::Right);

        // Non-existent token
        assert!(resolver.token_precedence(SymbolId(99)).is_none());
    }
}
