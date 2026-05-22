//! Tests for precedence and associativity handling in grammars.
//!
//! Categories (8 × 8 = 64):
//!   1. prec_basic_*     — basic precedence construction and properties
//!   2. prec_left_*      — left associativity
//!   3. prec_right_*     — right associativity
//!   4. prec_none_*      — non-associative rules
//!   5. prec_conflict_*  — conflict declarations
//!   6. prec_mixed_*     — mixed precedence levels
//!   7. prec_serialize_* — serialization round-trips
//!   8. prec_edge_*      — edge cases

use adze_ir::builder::GrammarBuilder;
use adze_ir::{
    Associativity, ConflictDeclaration, ConflictResolution, Grammar, Precedence, PrecedenceKind,
    ProductionId, Rule, SymbolId,
};

// ───────────────────────────────────────────────────────────────────────────
// Helpers
// ───────────────────────────────────────────────────────────────────────────

fn arith_grammar() -> Grammar {
    GrammarBuilder::new("arith")
        .token("NUMBER", r"\d+")
        .token("+", "+")
        .token("-", "-")
        .token("*", "*")
        .token("/", "/")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "-", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "/", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build()
}

fn assign_grammar() -> Grammar {
    GrammarBuilder::new("assign")
        .token("ID", r"[a-z]+")
        .token("=", "=")
        .rule_with_precedence("expr", vec!["expr", "=", "expr"], 1, Associativity::Right)
        .rule("expr", vec!["ID"])
        .start("expr")
        .build()
}

fn comparison_grammar() -> Grammar {
    GrammarBuilder::new("cmp")
        .token("NUM", r"\d+")
        .token("<", "<")
        .rule_with_precedence("expr", vec!["expr", "<", "expr"], 1, Associativity::None)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build()
}

// ───────────────────────────────────────────────────────────────────────────
// 1. prec_basic — basic precedence construction and properties
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn prec_basic_static_positive() {
    let pk = PrecedenceKind::Static(5);
    match pk {
        PrecedenceKind::Static(v) => assert_eq!(v, 5),
        _ => panic!("expected Static"),
    }
}

#[test]
fn prec_basic_static_negative() {
    let pk = PrecedenceKind::Static(-10);
    assert_eq!(pk, PrecedenceKind::Static(-10));
}

#[test]
fn prec_basic_dynamic_positive() {
    let pk = PrecedenceKind::Dynamic(42);
    match pk {
        PrecedenceKind::Dynamic(v) => assert_eq!(v, 42),
        _ => panic!("expected Dynamic"),
    }
}

#[test]
fn prec_basic_static_ne_dynamic_same_level() {
    assert_ne!(PrecedenceKind::Static(3), PrecedenceKind::Dynamic(3));
}

#[test]
fn prec_basic_copy_semantics() {
    let a = PrecedenceKind::Static(7);
    let b = a; // Copy, not move
    assert_eq!(a, b);
}

#[test]
fn prec_basic_rule_has_precedence() {
    let g = arith_grammar();
    let rules: Vec<&Rule> = g.all_rules().collect();
    let with_prec: Vec<_> = rules.iter().filter(|r| r.precedence.is_some()).collect();
    assert_eq!(with_prec.len(), 4);
}

#[test]
fn prec_basic_rule_without_precedence() {
    let g = arith_grammar();
    let plain: Vec<&Rule> = g.all_rules().filter(|r| r.precedence.is_none()).collect();
    assert_eq!(plain.len(), 1); // expr -> NUMBER
}

#[test]
fn prec_basic_precedence_level_stored() {
    let g = arith_grammar();
    let first = g.all_rules().next().unwrap();
    assert_eq!(first.precedence, Some(PrecedenceKind::Static(1)));
}

// ───────────────────────────────────────────────────────────────────────────
// 2. prec_left — left associativity
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn prec_left_variant_equality() {
    assert_eq!(Associativity::Left, Associativity::Left);
}

#[test]
fn prec_left_ne_right() {
    assert_ne!(Associativity::Left, Associativity::Right);
}

#[test]
fn prec_left_rule_has_left_assoc() {
    let g = arith_grammar();
    let left_rules: Vec<_> = g
        .all_rules()
        .filter(|r| r.associativity == Some(Associativity::Left))
        .collect();
    assert_eq!(left_rules.len(), 4);
}

#[test]
fn prec_left_add_rule_rhs_length() {
    let g = arith_grammar();
    let add_rule = g.all_rules().next().unwrap();
    assert_eq!(add_rule.rhs.len(), 3); // expr + expr
}

#[test]
fn prec_left_multiple_operators_same_level() {
    let g = arith_grammar();
    let level1: Vec<_> = g
        .all_rules()
        .filter(|r| r.precedence == Some(PrecedenceKind::Static(1)))
        .collect();
    assert_eq!(level1.len(), 2); // + and -
}

#[test]
fn prec_left_debug_format() {
    let dbg = format!("{:?}", Associativity::Left);
    assert!(dbg.contains("Left"));
}

#[test]
fn prec_left_copy_semantics() {
    let a = Associativity::Left;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn prec_left_builder_single_rule() {
    let g = GrammarBuilder::new("test")
        .token("N", r"\d+")
        .token("+", "+")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["N"])
        .start("expr")
        .build();
    let rule = g.all_rules().next().unwrap();
    assert_eq!(rule.associativity, Some(Associativity::Left));
}

// ───────────────────────────────────────────────────────────────────────────
// 3. prec_right — right associativity
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn prec_right_variant_equality() {
    assert_eq!(Associativity::Right, Associativity::Right);
}

#[test]
fn prec_right_ne_none() {
    assert_ne!(Associativity::Right, Associativity::None);
}

#[test]
fn prec_right_assign_rule() {
    let g = assign_grammar();
    let assign = g
        .all_rules()
        .find(|r| r.associativity == Some(Associativity::Right))
        .unwrap();
    assert_eq!(assign.precedence, Some(PrecedenceKind::Static(1)));
}

#[test]
fn prec_right_assign_rhs_is_three_symbols() {
    let g = assign_grammar();
    let assign = g
        .all_rules()
        .find(|r| r.associativity == Some(Associativity::Right))
        .unwrap();
    assert_eq!(assign.rhs.len(), 3);
}

#[test]
fn prec_right_debug_format() {
    let dbg = format!("{:?}", Associativity::Right);
    assert!(dbg.contains("Right"));
}

#[test]
fn prec_right_copy_semantics() {
    let a = Associativity::Right;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn prec_right_builder_creates_rule() {
    let g = GrammarBuilder::new("r")
        .token("X", "x")
        .token("^", "^")
        .rule_with_precedence("pow", vec!["X", "^", "pow"], 2, Associativity::Right)
        .rule("pow", vec!["X"])
        .start("pow")
        .build();
    let count = g
        .all_rules()
        .filter(|r| r.associativity == Some(Associativity::Right))
        .count();
    assert_eq!(count, 1);
}

#[test]
fn prec_right_lhs_matches_start() {
    let g = assign_grammar();
    let start = g.start_symbol().unwrap();
    let assign = g
        .all_rules()
        .find(|r| r.associativity == Some(Associativity::Right))
        .unwrap();
    assert_eq!(assign.lhs, start);
}

// ───────────────────────────────────────────────────────────────────────────
// 4. prec_none — non-associative rules
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn prec_none_variant_equality() {
    assert_eq!(Associativity::None, Associativity::None);
}

#[test]
fn prec_none_ne_left() {
    assert_ne!(Associativity::None, Associativity::Left);
}

#[test]
fn prec_none_comparison_rule() {
    let g = comparison_grammar();
    let cmp = g
        .all_rules()
        .find(|r| r.associativity == Some(Associativity::None))
        .unwrap();
    assert_eq!(cmp.precedence, Some(PrecedenceKind::Static(1)));
}

#[test]
fn prec_none_debug_format() {
    let dbg = format!("{:?}", Associativity::None);
    assert!(dbg.contains("None"));
}

#[test]
fn prec_none_copy_semantics() {
    let a = Associativity::None;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn prec_none_rule_count() {
    let g = comparison_grammar();
    let none_rules: Vec<_> = g
        .all_rules()
        .filter(|r| r.associativity == Some(Associativity::None))
        .collect();
    assert_eq!(none_rules.len(), 1);
}

#[test]
fn prec_none_plain_rule_has_no_assoc() {
    let g = comparison_grammar();
    let plain = g.all_rules().find(|r| r.associativity.is_none()).unwrap();
    assert!(plain.precedence.is_none());
}

#[test]
fn prec_none_builder_preserves_rhs() {
    let g = comparison_grammar();
    let cmp = g
        .all_rules()
        .find(|r| r.associativity == Some(Associativity::None))
        .unwrap();
    assert_eq!(cmp.rhs.len(), 3); // expr < expr
}

// ───────────────────────────────────────────────────────────────────────────
// 5. prec_conflict — conflict declarations
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn prec_conflict_glr_resolution() {
    let mut g = arith_grammar();
    g.conflicts.push(ConflictDeclaration {
        symbols: vec![SymbolId(1), SymbolId(2)],
        resolution: ConflictResolution::GLR,
    });
    assert_eq!(g.conflicts.len(), 1);
}

#[test]
fn prec_conflict_precedence_resolution() {
    let mut g = arith_grammar();
    g.conflicts.push(ConflictDeclaration {
        symbols: vec![SymbolId(1)],
        resolution: ConflictResolution::Precedence(PrecedenceKind::Static(2)),
    });
    assert_eq!(
        g.conflicts[0].resolution,
        ConflictResolution::Precedence(PrecedenceKind::Static(2))
    );
}

#[test]
fn prec_conflict_associativity_resolution() {
    let mut g = arith_grammar();
    g.conflicts.push(ConflictDeclaration {
        symbols: vec![SymbolId(3)],
        resolution: ConflictResolution::Associativity(Associativity::Left),
    });
    assert_eq!(
        g.conflicts[0].resolution,
        ConflictResolution::Associativity(Associativity::Left)
    );
}

#[test]
fn prec_conflict_multiple_declarations() {
    let mut g = arith_grammar();
    for i in 0..5 {
        g.conflicts.push(ConflictDeclaration {
            symbols: vec![SymbolId(i)],
            resolution: ConflictResolution::GLR,
        });
    }
    assert_eq!(g.conflicts.len(), 5);
}

#[test]
fn prec_conflict_symbols_preserved() {
    let mut g = arith_grammar();
    let syms = vec![SymbolId(10), SymbolId(20), SymbolId(30)];
    g.conflicts.push(ConflictDeclaration {
        symbols: syms.clone(),
        resolution: ConflictResolution::GLR,
    });
    assert_eq!(g.conflicts[0].symbols, syms);
}

#[test]
fn prec_conflict_dynamic_precedence_resolution() {
    let mut g = arith_grammar();
    g.conflicts.push(ConflictDeclaration {
        symbols: vec![SymbolId(1)],
        resolution: ConflictResolution::Precedence(PrecedenceKind::Dynamic(5)),
    });
    assert_eq!(
        g.conflicts[0].resolution,
        ConflictResolution::Precedence(PrecedenceKind::Dynamic(5))
    );
}

#[test]
fn prec_conflict_resolution_equality() {
    let a = ConflictResolution::GLR;
    let b = ConflictResolution::GLR;
    assert_eq!(a, b);
}

#[test]
fn prec_conflict_resolution_ne_variants() {
    assert_ne!(
        ConflictResolution::GLR,
        ConflictResolution::Associativity(Associativity::Left)
    );
}

// ───────────────────────────────────────────────────────────────────────────
// 6. prec_mixed — mixed precedence levels
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn prec_mixed_two_levels() {
    let g = arith_grammar();
    let levels: std::collections::HashSet<_> = g
        .all_rules()
        .filter_map(|r| r.precedence)
        .map(|p| match p {
            PrecedenceKind::Static(l) => l,
            PrecedenceKind::Dynamic(l) => l,
        })
        .collect();
    assert_eq!(levels.len(), 2); // 1 and 2
}

#[test]
fn prec_mixed_higher_level_is_mul() {
    let g = arith_grammar();
    let level2: Vec<_> = g
        .all_rules()
        .filter(|r| r.precedence == Some(PrecedenceKind::Static(2)))
        .collect();
    assert_eq!(level2.len(), 2); // * and /
}

#[test]
fn prec_mixed_left_and_right() {
    let g = GrammarBuilder::new("mixed")
        .token("N", r"\d+")
        .token("+", "+")
        .token("=", "=")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "=", "expr"], 0, Associativity::Right)
        .rule("expr", vec!["N"])
        .start("expr")
        .build();
    let left_count = g
        .all_rules()
        .filter(|r| r.associativity == Some(Associativity::Left))
        .count();
    let right_count = g
        .all_rules()
        .filter(|r| r.associativity == Some(Associativity::Right))
        .count();
    assert_eq!(left_count, 1);
    assert_eq!(right_count, 1);
}

#[test]
fn prec_mixed_three_levels() {
    let g = GrammarBuilder::new("three")
        .token("N", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .token("^", "^")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "^", "expr"], 3, Associativity::Right)
        .rule("expr", vec!["N"])
        .start("expr")
        .build();
    let levels: std::collections::HashSet<_> = g
        .all_rules()
        .filter_map(|r| match r.precedence {
            Some(PrecedenceKind::Static(l)) => Some(l),
            _ => Option::None,
        })
        .collect();
    assert_eq!(levels.len(), 3);
}

#[test]
fn prec_mixed_declaration_via_builder() {
    let g = GrammarBuilder::new("decl")
        .token("+", "+")
        .token("*", "*")
        .precedence(1, Associativity::Left, vec!["+"])
        .precedence(2, Associativity::Left, vec!["*"])
        .build();
    assert_eq!(g.precedences.len(), 2);
    assert_eq!(g.precedences[0].level, 1);
    assert_eq!(g.precedences[1].level, 2);
}

#[test]
fn prec_mixed_declaration_symbols() {
    let g = GrammarBuilder::new("sym")
        .token("+", "+")
        .token("-", "-")
        .precedence(1, Associativity::Left, vec!["+", "-"])
        .build();
    assert_eq!(g.precedences[0].symbols.len(), 2);
}

#[test]
fn prec_mixed_zero_and_positive() {
    let g = GrammarBuilder::new("zp")
        .token("N", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 0, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["N"])
        .start("expr")
        .build();
    let levels: Vec<_> = g
        .all_rules()
        .filter_map(|r| match r.precedence {
            Some(PrecedenceKind::Static(l)) => Some(l),
            _ => Option::None,
        })
        .collect();
    assert!(levels.contains(&0));
    assert!(levels.contains(&1));
}

#[test]
fn prec_mixed_negative_and_positive() {
    let g = GrammarBuilder::new("np")
        .token("N", r"\d+")
        .token("+", "+")
        .token("?", "?")
        .rule_with_precedence("expr", vec!["expr", "?", "expr"], -1, Associativity::Right)
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["N"])
        .start("expr")
        .build();
    let neg = g
        .all_rules()
        .find(|r| r.precedence == Some(PrecedenceKind::Static(-1)))
        .unwrap();
    assert_eq!(neg.associativity, Some(Associativity::Right));
}

// ───────────────────────────────────────────────────────────────────────────
// 7. prec_serialize — serialization round-trips
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn prec_serialize_precedence_kind_json() {
    let pk = PrecedenceKind::Static(3);
    let json = serde_json::to_string(&pk).unwrap();
    let pk2: PrecedenceKind = serde_json::from_str(&json).unwrap();
    assert_eq!(pk, pk2);
}

#[test]
fn prec_serialize_dynamic_kind_json() {
    let pk = PrecedenceKind::Dynamic(-7);
    let json = serde_json::to_string(&pk).unwrap();
    let pk2: PrecedenceKind = serde_json::from_str(&json).unwrap();
    assert_eq!(pk, pk2);
}

#[test]
fn prec_serialize_associativity_json() {
    for assoc in [
        Associativity::Left,
        Associativity::Right,
        Associativity::None,
    ] {
        let json = serde_json::to_string(&assoc).unwrap();
        let assoc2: Associativity = serde_json::from_str(&json).unwrap();
        assert_eq!(assoc, assoc2);
    }
}

#[test]
fn prec_serialize_precedence_struct_json() {
    let p = Precedence {
        level: 5,
        associativity: Associativity::Left,
        symbols: vec![SymbolId(1), SymbolId(2)],
    };
    let json = serde_json::to_string(&p).unwrap();
    let p2: Precedence = serde_json::from_str(&json).unwrap();
    assert_eq!(p, p2);
}

#[test]
fn prec_serialize_conflict_resolution_json() {
    let cr = ConflictResolution::Precedence(PrecedenceKind::Static(10));
    let json = serde_json::to_string(&cr).unwrap();
    let cr2: ConflictResolution = serde_json::from_str(&json).unwrap();
    assert_eq!(cr, cr2);
}

#[test]
fn prec_serialize_grammar_roundtrip() {
    let g = arith_grammar();
    let json = serde_json::to_string(&g).unwrap();
    let g2: Grammar = serde_json::from_str(&json).unwrap();
    assert_eq!(g.precedences.len(), g2.precedences.len());
    // Rule count should be preserved
    assert_eq!(g.all_rules().count(), g2.all_rules().count());
}

#[test]
fn prec_serialize_grammar_with_conflicts() {
    let mut g = arith_grammar();
    g.conflicts.push(ConflictDeclaration {
        symbols: vec![SymbolId(1), SymbolId(2)],
        resolution: ConflictResolution::GLR,
    });
    let json = serde_json::to_string(&g).unwrap();
    let g2: Grammar = serde_json::from_str(&json).unwrap();
    assert_eq!(g2.conflicts.len(), 1);
}

#[test]
fn prec_serialize_conflict_declaration_json() {
    let cd = ConflictDeclaration {
        symbols: vec![SymbolId(5), SymbolId(6)],
        resolution: ConflictResolution::Associativity(Associativity::Right),
    };
    let json = serde_json::to_string(&cd).unwrap();
    let cd2: ConflictDeclaration = serde_json::from_str(&json).unwrap();
    assert_eq!(cd, cd2);
}

// ───────────────────────────────────────────────────────────────────────────
// 8. prec_edge — edge cases
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn prec_edge_i16_max() {
    let pk = PrecedenceKind::Static(i16::MAX);
    assert_eq!(pk, PrecedenceKind::Static(i16::MAX));
}

#[test]
fn prec_edge_i16_min() {
    let pk = PrecedenceKind::Static(i16::MIN);
    assert_eq!(pk, PrecedenceKind::Static(i16::MIN));
}

#[test]
fn prec_edge_zero_precedence() {
    let g = GrammarBuilder::new("zero")
        .token("A", "a")
        .token("+", "+")
        .rule_with_precedence("s", vec!["s", "+", "s"], 0, Associativity::Left)
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let r = g
        .all_rules()
        .find(|r| r.precedence == Some(PrecedenceKind::Static(0)))
        .unwrap();
    assert_eq!(r.associativity, Some(Associativity::Left));
}

#[test]
fn prec_edge_grammar_no_precedences() {
    let g = GrammarBuilder::new("plain")
        .token("X", "x")
        .rule("s", vec!["X"])
        .start("s")
        .build();
    assert!(g.precedences.is_empty());
    let prec_rules: Vec<_> = g.all_rules().filter(|r| r.precedence.is_some()).collect();
    assert!(prec_rules.is_empty());
}

#[test]
fn prec_edge_empty_conflict_symbols() {
    let mut g = Grammar::default();
    g.conflicts.push(ConflictDeclaration {
        symbols: vec![],
        resolution: ConflictResolution::GLR,
    });
    assert!(g.conflicts[0].symbols.is_empty());
}

#[test]
fn prec_edge_same_prec_different_assoc() {
    let g = GrammarBuilder::new("same")
        .token("N", r"\d+")
        .token("+", "+")
        .token("=", "=")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "=", "expr"], 1, Associativity::Right)
        .rule("expr", vec!["N"])
        .start("expr")
        .build();
    let assocs: Vec<_> = g
        .all_rules()
        .filter(|r| r.precedence == Some(PrecedenceKind::Static(1)))
        .map(|r| r.associativity.unwrap())
        .collect();
    assert!(assocs.contains(&Associativity::Left));
    assert!(assocs.contains(&Associativity::Right));
}

#[test]
fn prec_edge_many_rules_same_prec() {
    let mut b = GrammarBuilder::new("many")
        .token("N", r"\d+")
        .token("+", "+")
        .token("-", "-")
        .token("*", "*")
        .token("/", "/");
    for op in ["+", "-", "*", "/"] {
        b = b.rule_with_precedence("expr", vec!["expr", op, "expr"], 1, Associativity::Left);
    }
    let g = b.rule("expr", vec!["N"]).start("expr").build();
    let level1_count = g
        .all_rules()
        .filter(|r| r.precedence == Some(PrecedenceKind::Static(1)))
        .count();
    assert_eq!(level1_count, 4);
}

#[test]
fn prec_edge_production_ids_unique() {
    let g = arith_grammar();
    let ids: Vec<ProductionId> = g.all_rules().map(|r| r.production_id).collect();
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(ids.len(), unique.len());
}
