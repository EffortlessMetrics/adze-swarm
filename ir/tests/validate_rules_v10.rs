//! Comprehensive tests for `Grammar::validate()` and rule validation in adze-ir.
//!
//! 80+ tests covering:
//!   - Valid grammar construction and validation
//!   - Start symbol presence and absence
//!   - Idempotency and immutability of validate()
//!   - Grammars of various sizes (1 rule, 10 rules, many tokens)
//!   - Precedence and all Associativity variants
//!   - Inline rules, extras, externals, supertypes, conflicts
//!   - Normalize then validate, optimize then validate
//!   - Clone-then-validate consistency
//!   - Builder chaining patterns

use adze_ir::builder::GrammarBuilder;
use adze_ir::{
    Associativity, ConflictDeclaration, ConflictResolution, Grammar, GrammarError, ProductionId,
    Rule, Symbol, SymbolId, Token, TokenPattern,
};

// ============================================================================
// HELPERS
// ============================================================================

/// Build a minimal valid grammar with one token and one rule.
fn minimal_grammar(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("A", "a")
        .rule("root", vec!["A"])
        .start("root")
        .build()
}

/// Build an arithmetic-style grammar with precedence.
fn arith_grammar(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUM", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build()
}

// ============================================================================
// 1. VALID GRAMMAR → validate() RETURNS Ok
// ============================================================================

#[test]
fn test_vr_v10_valid_minimal_grammar() {
    let g = minimal_grammar("vr_v10_minimal");
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_valid_arith_grammar() {
    let g = arith_grammar("vr_v10_arith");
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_valid_multi_token_grammar() {
    let g = GrammarBuilder::new("vr_v10_multi_tok")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .rule("root", vec!["A", "B", "C"])
        .start("root")
        .build();
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_valid_two_rules() {
    let g = GrammarBuilder::new("vr_v10_two")
        .token("X", "x")
        .token("Y", "y")
        .rule("root", vec!["item"])
        .rule("item", vec!["X"])
        .rule("item", vec!["Y"])
        .start("root")
        .build();
    assert!(g.validate().is_ok());
}

// ============================================================================
// 2. GRAMMAR WITH START SYMBOL → VALID
// ============================================================================

#[test]
fn test_vr_v10_start_symbol_present() {
    let g = minimal_grammar("vr_v10_start");
    assert!(g.start_symbol().is_some());
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_start_symbol_is_first_rule() {
    let g = GrammarBuilder::new("vr_v10_start_first")
        .token("T", "t")
        .rule("program", vec!["stmt"])
        .rule("stmt", vec!["T"])
        .start("program")
        .build();
    let start = g.start_symbol().unwrap();
    let first_rule_lhs = *g.rules.keys().next().unwrap();
    assert_eq!(start, first_rule_lhs);
}

// ============================================================================
// 3. GRAMMAR WITHOUT EXPLICIT START → BEHAVIOUR
// ============================================================================

#[test]
fn test_vr_v10_no_explicit_start_still_validates() {
    // Builder without .start() — first rule used as start
    let g = GrammarBuilder::new("vr_v10_no_start")
        .token("A", "a")
        .rule("root", vec!["A"])
        .build();
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_no_start_start_symbol_uses_builder_default() {
    let g = GrammarBuilder::new("vr_v10_fallback")
        .token("A", "a")
        .rule("root", vec!["A"])
        .build();
    assert_eq!(g.start_symbol(), g.explicit_start_symbol());
    assert!(g.start_symbol().is_some());
}

// ============================================================================
// 4. validate() DOESN'T MODIFY GRAMMAR
// ============================================================================

#[test]
fn test_vr_v10_validate_no_mutation() {
    let g = arith_grammar("vr_v10_nomut");
    let before = g.clone();
    let _ = g.validate();
    assert_eq!(g, before);
}

#[test]
fn test_vr_v10_validate_preserves_rules_count() {
    let g = arith_grammar("vr_v10_preserve_rules");
    let count_before = g.rules.values().map(|v| v.len()).sum::<usize>();
    let _ = g.validate();
    let count_after = g.rules.values().map(|v| v.len()).sum::<usize>();
    assert_eq!(count_before, count_after);
}

#[test]
fn test_vr_v10_validate_preserves_tokens_count() {
    let g = arith_grammar("vr_v10_preserve_tok");
    let count_before = g.tokens.len();
    let _ = g.validate();
    assert_eq!(g.tokens.len(), count_before);
}

#[test]
fn test_vr_v10_validate_preserves_name() {
    let g = minimal_grammar("vr_v10_preserve_name");
    let _ = g.validate();
    assert_eq!(g.name, "vr_v10_preserve_name");
}

// ============================================================================
// 5. validate() IS IDEMPOTENT
// ============================================================================

#[test]
fn test_vr_v10_idempotent_ok() {
    let g = minimal_grammar("vr_v10_idem_ok");
    let r1 = g.validate();
    let r2 = g.validate();
    assert_eq!(r1.is_ok(), r2.is_ok());
}

#[test]
fn test_vr_v10_idempotent_error() {
    let mut g = Grammar::new("vr_v10_idem_err".to_string());
    let rule = Rule {
        lhs: SymbolId(0),
        rhs: vec![Symbol::Terminal(SymbolId(999))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    };
    g.add_rule(rule);
    let r1 = g.validate();
    let r2 = g.validate();
    assert!(r1.is_err());
    assert!(r2.is_err());
}

#[test]
fn test_vr_v10_idempotent_three_calls() {
    let g = arith_grammar("vr_v10_idem3");
    for _ in 0..3 {
        assert!(g.validate().is_ok());
    }
}

// ============================================================================
// 6. GRAMMAR WITH 1 RULE → VALID
// ============================================================================

#[test]
fn test_vr_v10_single_rule_ok() {
    let g = GrammarBuilder::new("vr_v10_single")
        .token("Z", "z")
        .rule("entry", vec!["Z"])
        .start("entry")
        .build();
    assert!(g.validate().is_ok());
    assert_eq!(g.rules.values().map(|v| v.len()).sum::<usize>(), 1);
}

// ============================================================================
// 7. GRAMMAR WITH 10 RULES → VALID
// ============================================================================

#[test]
fn test_vr_v10_ten_rules() {
    let mut b = GrammarBuilder::new("vr_v10_ten");
    for i in 0..10 {
        let tok_name = format!("T{i}");
        let tok_pat = format!("t{i}");
        b = b.token(
            Box::leak(tok_name.into_boxed_str()),
            Box::leak(tok_pat.into_boxed_str()),
        );
    }
    for i in 0..10 {
        let rule_name = format!("r{i}");
        let tok_name = format!("T{i}");
        b = b.rule(
            Box::leak(rule_name.into_boxed_str()),
            vec![Box::leak(tok_name.into_boxed_str())],
        );
    }
    b = b.start("r0");
    let g = b.build();
    assert!(g.validate().is_ok());
}

// ============================================================================
// 8. GRAMMAR WITH PRECEDENCE → VALID
// ============================================================================

#[test]
fn test_vr_v10_precedence_valid() {
    let g = arith_grammar("vr_v10_prec");
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_negative_precedence() {
    let g = GrammarBuilder::new("vr_v10_neg_prec")
        .token("A", "a")
        .token("+", "+")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], -5, Associativity::Left)
        .rule("expr", vec!["A"])
        .start("expr")
        .build();
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_zero_precedence() {
    let g = GrammarBuilder::new("vr_v10_zero_prec")
        .token("A", "a")
        .token("+", "+")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 0, Associativity::None)
        .rule("expr", vec!["A"])
        .start("expr")
        .build();
    assert!(g.validate().is_ok());
}

// ============================================================================
// 9. ALL ASSOCIATIVITY VARIANTS → VALID
// ============================================================================

#[test]
fn test_vr_v10_assoc_left() {
    let g = GrammarBuilder::new("vr_v10_assoc_l")
        .token("N", "n")
        .token("+", "+")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["N"])
        .start("expr")
        .build();
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_assoc_right() {
    let g = GrammarBuilder::new("vr_v10_assoc_r")
        .token("N", "n")
        .token("=", "=")
        .rule_with_precedence(
            "assign",
            vec!["assign", "=", "assign"],
            1,
            Associativity::Right,
        )
        .rule("assign", vec!["N"])
        .start("assign")
        .build();
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_assoc_none() {
    let g = GrammarBuilder::new("vr_v10_assoc_n")
        .token("N", "n")
        .token("+", "+")
        .rule_with_precedence("cmp", vec!["cmp", "+", "cmp"], 1, Associativity::None)
        .rule("cmp", vec!["N"])
        .start("cmp")
        .build();
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_mixed_assoc() {
    let g = GrammarBuilder::new("vr_v10_mixed_assoc")
        .token("N", "n")
        .token("+", "+")
        .token("*", "*")
        .token("=", "=")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Right)
        .rule_with_precedence("expr", vec!["expr", "=", "expr"], 0, Associativity::None)
        .rule("expr", vec!["N"])
        .start("expr")
        .build();
    assert!(g.validate().is_ok());
}

// ============================================================================
// 10. GRAMMAR WITH INLINE RULES → VALID
// ============================================================================

#[test]
fn test_vr_v10_inline_rules() {
    let g = GrammarBuilder::new("vr_v10_inline")
        .token("A", "a")
        .token("B", "b")
        .rule("root", vec!["helper"])
        .rule("helper", vec!["A", "B"])
        .inline("helper")
        .start("root")
        .build();
    assert!(g.validate().is_ok());
    assert!(!g.inline_rules.is_empty());
}

#[test]
fn test_vr_v10_multiple_inline_rules() {
    let g = GrammarBuilder::new("vr_v10_multi_inline")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .rule("root", vec!["h1"])
        .rule("h1", vec!["h2"])
        .rule("h2", vec!["A", "B", "C"])
        .inline("h1")
        .inline("h2")
        .start("root")
        .build();
    assert!(g.validate().is_ok());
    assert_eq!(g.inline_rules.len(), 2);
}

// ============================================================================
// 11. GRAMMAR WITH EXTRAS → VALID
// ============================================================================

#[test]
fn test_vr_v10_extras() {
    let g = GrammarBuilder::new("vr_v10_extras")
        .token("A", "a")
        .token("WS", r"[ \t]+")
        .rule("root", vec!["A"])
        .extra("WS")
        .start("root")
        .build();
    assert!(g.validate().is_ok());
    assert!(!g.extras.is_empty());
}

#[test]
fn test_vr_v10_multiple_extras() {
    let g = GrammarBuilder::new("vr_v10_multi_extras")
        .token("A", "a")
        .token("WS", r"[ \t]+")
        .token("NL", r"\n")
        .rule("root", vec!["A"])
        .extra("WS")
        .extra("NL")
        .start("root")
        .build();
    assert!(g.validate().is_ok());
    assert_eq!(g.extras.len(), 2);
}

// ============================================================================
// 12. GRAMMAR WITH EXTERNALS → VALID
// ============================================================================

#[test]
fn test_vr_v10_externals() {
    let g = GrammarBuilder::new("vr_v10_ext")
        .token("A", "a")
        .token("INDENT", "INDENT")
        .external("INDENT")
        .rule("root", vec!["A"])
        .start("root")
        .build();
    assert!(g.validate().is_ok());
    assert!(!g.externals.is_empty());
}

#[test]
fn test_vr_v10_multiple_externals() {
    let g = GrammarBuilder::new("vr_v10_multi_ext")
        .token("A", "a")
        .token("INDENT", "INDENT")
        .token("DEDENT", "DEDENT")
        .external("INDENT")
        .external("DEDENT")
        .rule("root", vec!["A"])
        .start("root")
        .build();
    assert!(g.validate().is_ok());
    assert_eq!(g.externals.len(), 2);
}

// ============================================================================
// 13. GRAMMAR WITH SUPERTYPES → VALID
// ============================================================================

#[test]
fn test_vr_v10_supertypes() {
    let g = GrammarBuilder::new("vr_v10_super")
        .token("NUM", r"\d+")
        .token("ID", r"[a-z]+")
        .rule("root", vec!["expression"])
        .rule("expression", vec!["NUM"])
        .rule("expression", vec!["ID"])
        .supertype("expression")
        .start("root")
        .build();
    assert!(g.validate().is_ok());
    assert!(!g.supertypes.is_empty());
}

#[test]
fn test_vr_v10_multiple_supertypes() {
    let g = GrammarBuilder::new("vr_v10_multi_super")
        .token("NUM", r"\d+")
        .token("ID", r"[a-z]+")
        .token(";", ";")
        .rule("root", vec!["statement"])
        .rule("statement", vec!["expression", ";"])
        .rule("expression", vec!["NUM"])
        .rule("expression", vec!["ID"])
        .supertype("statement")
        .supertype("expression")
        .start("root")
        .build();
    assert!(g.validate().is_ok());
    assert_eq!(g.supertypes.len(), 2);
}

// ============================================================================
// 14. GRAMMAR WITH CONFLICTS → VALID
// ============================================================================

#[test]
fn test_vr_v10_conflicts_field() {
    let mut g = GrammarBuilder::new("vr_v10_conflicts")
        .token("A", "a")
        .token("B", "b")
        .rule("root", vec!["A"])
        .rule("root", vec!["B"])
        .start("root")
        .build();
    // Manually set a conflict declaration
    let sym_ids: Vec<SymbolId> = g.rules.keys().copied().collect();
    if !sym_ids.is_empty() {
        g.conflicts.push(ConflictDeclaration {
            symbols: vec![sym_ids[0]],
            resolution: ConflictResolution::GLR,
        });
    }
    assert!(g.validate().is_ok());
    assert!(!g.conflicts.is_empty());
}

#[test]
fn test_vr_v10_conflict_precedence_resolution() {
    let mut g = arith_grammar("vr_v10_conflict_prec");
    let sym_ids: Vec<SymbolId> = g.rules.keys().copied().collect();
    if !sym_ids.is_empty() {
        g.conflicts.push(ConflictDeclaration {
            symbols: vec![sym_ids[0]],
            resolution: ConflictResolution::Precedence(adze_ir::PrecedenceKind::Static(1)),
        });
    }
    assert!(g.validate().is_ok());
}

// ============================================================================
// 15. AFTER NORMALIZE → validate() STILL VALID
// ============================================================================

#[test]
fn test_vr_v10_normalize_then_validate() {
    let mut g = minimal_grammar("vr_v10_norm_val");
    let _ = g.normalize();
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_normalize_arith_then_validate() {
    let mut g = arith_grammar("vr_v10_norm_arith");
    let _ = g.normalize();
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_normalize_epsilon_then_validate() {
    let mut g = GrammarBuilder::new("vr_v10_norm_eps")
        .token("A", "a")
        .rule("root", vec!["A"])
        .rule("root", vec![])
        .start("root")
        .build();
    let _ = g.normalize();
    assert!(g.validate().is_ok());
}

// ============================================================================
// 16. AFTER OPTIMIZE → validate() STILL VALID
// ============================================================================

#[test]
fn test_vr_v10_optimize_then_validate() {
    let mut g = minimal_grammar("vr_v10_opt_val");
    g.optimize();
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_optimize_arith_then_validate() {
    let mut g = arith_grammar("vr_v10_opt_arith");
    g.optimize();
    assert!(g.validate().is_ok());
}

// ============================================================================
// 17. CLONE THEN validate() → SAME RESULT
// ============================================================================

#[test]
fn test_vr_v10_clone_valid_same_result() {
    let g = arith_grammar("vr_v10_clone_ok");
    let cloned = g.clone();
    assert_eq!(g.validate().is_ok(), cloned.validate().is_ok());
}

#[test]
fn test_vr_v10_clone_invalid_same_result() {
    let mut g = Grammar::new("vr_v10_clone_err".to_string());
    g.add_rule(Rule {
        lhs: SymbolId(0),
        rhs: vec![Symbol::Terminal(SymbolId(999))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    let cloned = g.clone();
    assert!(g.validate().is_err());
    assert!(cloned.validate().is_err());
}

#[test]
fn test_vr_v10_clone_equality() {
    let g = minimal_grammar("vr_v10_clone_eq");
    let cloned = g.clone();
    assert_eq!(g, cloned);
}

// ============================================================================
// 18. GRAMMAR WITH MANY TOKENS → VALID
// ============================================================================

#[test]
fn test_vr_v10_twenty_tokens() {
    let mut b = GrammarBuilder::new("vr_v10_20tok");
    let mut tok_names: Vec<String> = Vec::new();
    for i in 0..20 {
        let name = format!("TOK{i}");
        let pat = format!("tok{i}");
        b = b.token(
            Box::leak(name.clone().into_boxed_str()),
            Box::leak(pat.into_boxed_str()),
        );
        tok_names.push(name);
    }
    // Use first token in a rule
    b = b.rule(
        "root",
        vec![Box::leak(tok_names[0].clone().into_boxed_str())],
    );
    b = b.start("root");
    let g = b.build();
    assert!(g.validate().is_ok());
    assert!(g.tokens.len() >= 20);
}

#[test]
fn test_vr_v10_fifty_tokens() {
    let mut b = GrammarBuilder::new("vr_v10_50tok");
    let mut first_tok = String::new();
    for i in 0..50 {
        let name = format!("T{i}");
        let pat = format!("t{i}");
        if i == 0 {
            first_tok = name.clone();
        }
        b = b.token(
            Box::leak(name.into_boxed_str()),
            Box::leak(pat.into_boxed_str()),
        );
    }
    b = b.rule("root", vec![Box::leak(first_tok.into_boxed_str())]);
    b = b.start("root");
    let g = b.build();
    assert!(g.validate().is_ok());
    assert!(g.tokens.len() >= 50);
}

// ============================================================================
// 19. VARIOUS GRAMMAR SIZES → ALL VALIDATE
// ============================================================================

#[test]
fn test_vr_v10_size_one_rule_one_token() {
    let g = minimal_grammar("vr_v10_s1");
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_size_five_rules() {
    let g = GrammarBuilder::new("vr_v10_s5")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .token("D", "d")
        .token("E", "e")
        .rule("root", vec!["item"])
        .rule("item", vec!["A"])
        .rule("item", vec!["B"])
        .rule("item", vec!["C"])
        .rule("item", vec!["D"])
        .start("root")
        .build();
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_size_large_grammar() {
    let mut b = GrammarBuilder::new("vr_v10_large");
    for i in 0..30 {
        let name = format!("K{i}");
        let pat = format!("k{i}");
        b = b.token(
            Box::leak(name.into_boxed_str()),
            Box::leak(pat.into_boxed_str()),
        );
    }
    for i in 0..30 {
        let rule_name = format!("n{i}");
        let tok_name = format!("K{i}");
        b = b.rule(
            Box::leak(rule_name.into_boxed_str()),
            vec![Box::leak(tok_name.into_boxed_str())],
        );
    }
    // Chain them: root -> n0
    b = b.rule("root", vec!["n0"]);
    b = b.start("root");
    let g = b.build();
    assert!(g.validate().is_ok());
}

// ============================================================================
// 20. validate() AFTER MULTIPLE BUILDER OPERATIONS
// ============================================================================

#[test]
fn test_vr_v10_builder_chaining_all_features() {
    let g = GrammarBuilder::new("vr_v10_chain")
        .token("ID", r"[a-z]+")
        .token("NUM", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .token(";", ";")
        .token("WS", r"[ \t]+")
        .rule("root", vec!["stmt"])
        .rule("stmt", vec!["expr", ";"])
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["ID"])
        .rule("expr", vec!["NUM"])
        .extra("WS")
        .start("root")
        .build();
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_builder_token_before_rule() {
    let g = GrammarBuilder::new("vr_v10_tok_first")
        .token("A", "a")
        .token("B", "b")
        .rule("root", vec!["A", "B"])
        .start("root")
        .build();
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_builder_rule_references_later_token() {
    // Define rule before referencing all tokens — builder resolves lazily
    let g = GrammarBuilder::new("vr_v10_lazy")
        .token("X", "x")
        .rule("root", vec!["X", "helper"])
        .token("Y", "y")
        .rule("helper", vec!["Y"])
        .start("root")
        .build();
    assert!(g.validate().is_ok());
}

// ============================================================================
// ADDITIONAL TESTS (21–80+)
// ============================================================================

// --- Empty grammar error ---

#[test]
fn test_vr_v10_empty_grammar_has_no_rules() {
    let g = Grammar::new("vr_v10_empty".to_string());
    assert!(g.rules.is_empty());
}

#[test]
fn test_vr_v10_empty_grammar_validate_succeeds() {
    // Grammar::validate() checks field ordering and symbol refs, not emptiness
    let g = Grammar::new("vr_v10_empty_ok".to_string());
    assert!(g.validate().is_ok());
}

// --- Unresolved symbol errors ---

#[test]
fn test_vr_v10_unresolved_terminal() {
    let mut g = Grammar::new("vr_v10_unresolved".to_string());
    g.add_rule(Rule {
        lhs: SymbolId(0),
        rhs: vec![Symbol::Terminal(SymbolId(500))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    assert!(g.validate().is_err());
}

#[test]
fn test_vr_v10_unresolved_nonterminal() {
    let mut g = Grammar::new("vr_v10_unres_nt".to_string());
    g.add_rule(Rule {
        lhs: SymbolId(0),
        rhs: vec![Symbol::NonTerminal(SymbolId(500))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    assert!(g.validate().is_err());
}

#[test]
fn test_vr_v10_unresolved_external() {
    let mut g = Grammar::new("vr_v10_unres_ext".to_string());
    g.add_rule(Rule {
        lhs: SymbolId(0),
        rhs: vec![Symbol::External(SymbolId(500))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    assert!(g.validate().is_err());
}

#[test]
fn test_vr_v10_unresolved_error_type() {
    let mut g = Grammar::new("vr_v10_err_type".to_string());
    g.add_rule(Rule {
        lhs: SymbolId(0),
        rhs: vec![Symbol::Terminal(SymbolId(777))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    match g.validate() {
        Err(GrammarError::UnresolvedSymbol(id)) => assert_eq!(id, SymbolId(777)),
        other => panic!("Expected UnresolvedSymbol, got {:?}", other),
    }
}

// --- Field ordering ---

#[test]
fn test_vr_v10_field_ordering_valid() {
    let mut g = minimal_grammar("vr_v10_field_ok");
    g.fields.insert(adze_ir::FieldId(0), "alpha".to_string());
    g.fields.insert(adze_ir::FieldId(1), "beta".to_string());
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_field_ordering_invalid() {
    let mut g = minimal_grammar("vr_v10_field_bad");
    g.fields.insert(adze_ir::FieldId(0), "zebra".to_string());
    g.fields.insert(adze_ir::FieldId(1), "alpha".to_string());
    match g.validate() {
        Err(GrammarError::InvalidFieldOrdering) => {}
        other => panic!("Expected InvalidFieldOrdering, got {:?}", other),
    }
}

#[test]
fn test_vr_v10_empty_fields_valid() {
    let g = minimal_grammar("vr_v10_no_fields");
    assert!(g.fields.is_empty());
    assert!(g.validate().is_ok());
}

// --- Epsilon rules ---

#[test]
fn test_vr_v10_epsilon_rule_valid() {
    let g = GrammarBuilder::new("vr_v10_eps")
        .token("A", "a")
        .rule("root", vec!["A"])
        .rule("root", vec![])
        .start("root")
        .build();
    assert!(g.validate().is_ok());
}

// --- Recursive rules ---

#[test]
fn test_vr_v10_left_recursive() {
    let g = GrammarBuilder::new("vr_v10_lrec")
        .token("A", "a")
        .token("+", "+")
        .rule("expr", vec!["expr", "+", "A"])
        .rule("expr", vec!["A"])
        .start("expr")
        .build();
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_right_recursive() {
    let g = GrammarBuilder::new("vr_v10_rrec")
        .token("A", "a")
        .token("+", "+")
        .rule("expr", vec!["A", "+", "expr"])
        .rule("expr", vec!["A"])
        .start("expr")
        .build();
    assert!(g.validate().is_ok());
}

// --- Fragile tokens ---

#[test]
fn test_vr_v10_fragile_token() {
    let g = GrammarBuilder::new("vr_v10_fragile")
        .fragile_token("ERR", r".*")
        .token("A", "a")
        .rule("root", vec!["A"])
        .start("root")
        .build();
    assert!(g.validate().is_ok());
}

// --- Python-like preset ---

#[test]
fn test_vr_v10_python_like_validates() {
    let g = GrammarBuilder::python_like();
    assert!(g.validate().is_ok());
}

// --- JavaScript-like preset ---

#[test]
fn test_vr_v10_javascript_like_validates() {
    let g = GrammarBuilder::javascript_like();
    assert!(g.validate().is_ok());
}

// --- Precedence declarations ---

#[test]
fn test_vr_v10_precedence_decl() {
    let g = GrammarBuilder::new("vr_v10_prec_decl")
        .token("A", "a")
        .token("+", "+")
        .token("*", "*")
        .rule("expr", vec!["A"])
        .rule("expr", vec!["expr", "+", "expr"])
        .rule("expr", vec!["expr", "*", "expr"])
        .precedence(1, Associativity::Left, vec!["+"])
        .precedence(2, Associativity::Left, vec!["*"])
        .start("expr")
        .build();
    assert!(g.validate().is_ok());
    assert_eq!(g.precedences.len(), 2);
}

// --- Grammar with all features combined ---

#[test]
fn test_vr_v10_all_features_combined() {
    let g = GrammarBuilder::new("vr_v10_all")
        .token("ID", r"[a-z]+")
        .token("NUM", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .token(";", ";")
        .token("WS", r"[ \t]+")
        .token("INDENT", "INDENT")
        .rule("root", vec!["stmt"])
        .rule("stmt", vec!["expr", ";"])
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Right)
        .rule("expr", vec!["ID"])
        .rule("expr", vec!["NUM"])
        .rule("expr", vec!["helper"])
        .rule("helper", vec!["ID"])
        .extra("WS")
        .external("INDENT")
        .inline("helper")
        .supertype("expr")
        .precedence(1, Associativity::Left, vec!["+"])
        .precedence(2, Associativity::Right, vec!["*"])
        .start("root")
        .build();
    assert!(g.validate().is_ok());
    assert!(!g.extras.is_empty());
    assert!(!g.externals.is_empty());
    assert!(!g.inline_rules.is_empty());
    assert!(!g.supertypes.is_empty());
    assert!(!g.precedences.is_empty());
}

// --- Normalize + optimize + validate pipeline ---

#[test]
fn test_vr_v10_normalize_optimize_validate() {
    let mut g = arith_grammar("vr_v10_norm_opt");
    let _ = g.normalize();
    g.optimize();
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_optimize_normalize_validate() {
    let mut g = arith_grammar("vr_v10_opt_norm");
    g.optimize();
    let _ = g.normalize();
    assert!(g.validate().is_ok());
}

// --- Grammar Debug/Display ---

#[test]
fn test_vr_v10_grammar_debug() {
    let g = minimal_grammar("vr_v10_dbg");
    let debug_str = format!("{:?}", g);
    assert!(debug_str.contains("vr_v10_dbg"));
}

// --- GrammarError Display ---

#[test]
fn test_vr_v10_grammar_error_display_unresolved() {
    let err = GrammarError::UnresolvedSymbol(SymbolId(42));
    let msg = format!("{err}");
    assert!(msg.contains("42"));
}

#[test]
fn test_vr_v10_grammar_error_display_field_ordering() {
    let err = GrammarError::InvalidFieldOrdering;
    let msg = format!("{err}");
    assert!(msg.contains("field"));
}

// --- Rule fields ---

#[test]
fn test_vr_v10_rule_with_no_fields() {
    let g = minimal_grammar("vr_v10_no_rfields");
    for rule in g.all_rules() {
        assert!(rule.fields.is_empty());
    }
    assert!(g.validate().is_ok());
}

// --- Multiple alternatives ---

#[test]
fn test_vr_v10_many_alternatives() {
    let mut b = GrammarBuilder::new("vr_v10_alts");
    for i in 0..8 {
        let name = format!("T{i}");
        let pat = format!("t{i}");
        b = b.token(
            Box::leak(name.into_boxed_str()),
            Box::leak(pat.into_boxed_str()),
        );
    }
    for i in 0..8 {
        let tok = format!("T{i}");
        b = b.rule("root", vec![Box::leak(tok.into_boxed_str())]);
    }
    b = b.start("root");
    let g = b.build();
    assert!(g.validate().is_ok());
}

// --- Grammar name preserved ---

#[test]
fn test_vr_v10_grammar_name() {
    let g = GrammarBuilder::new("vr_v10_myname")
        .token("A", "a")
        .rule("root", vec!["A"])
        .start("root")
        .build();
    assert_eq!(g.name, "vr_v10_myname");
}

// --- validate() on Default grammar ---

#[test]
fn test_vr_v10_default_grammar_validates() {
    let g = Grammar::default();
    // Default has no rules; validate() checks field ordering and symbol refs — both trivially pass
    assert!(g.validate().is_ok());
}

// --- Token pattern types ---

#[test]
fn test_vr_v10_string_token_pattern() {
    let g = GrammarBuilder::new("vr_v10_str_pat")
        .token("+", "+")
        .token("A", "a")
        .rule("root", vec!["A", "+", "A"])
        .start("root")
        .build();
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_regex_token_pattern() {
    let g = GrammarBuilder::new("vr_v10_re_pat")
        .token("ID", r"[a-zA-Z_][a-zA-Z0-9_]*")
        .rule("root", vec!["ID"])
        .start("root")
        .build();
    assert!(g.validate().is_ok());
}

// --- Rule count accessors ---

#[test]
fn test_vr_v10_all_rules_count() {
    let g = arith_grammar("vr_v10_count");
    let count = g.all_rules().count();
    assert!(count >= 3);
    assert!(g.validate().is_ok());
}

#[test]
fn test_vr_v10_get_rules_for_symbol() {
    let g = minimal_grammar("vr_v10_get_rules");
    let start = g.start_symbol().unwrap();
    let rules = g.get_rules_for_symbol(start);
    assert!(rules.is_some());
    assert!(!rules.unwrap().is_empty());
}

// --- Validate after clone and mutate ---

#[test]
fn test_vr_v10_clone_mutate_original_unchanged() {
    let g = minimal_grammar("vr_v10_clone_mut");
    let mut cloned = g.clone();
    cloned.optimize();
    // Original still validates
    assert!(g.validate().is_ok());
}

// --- find_symbol_by_name ---

#[test]
fn test_vr_v10_find_symbol_by_name() {
    let g = GrammarBuilder::new("vr_v10_find")
        .token("A", "a")
        .rule("root", vec!["A"])
        .start("root")
        .build();
    assert!(g.find_symbol_by_name("root").is_some());
}

#[test]
fn test_vr_v10_find_symbol_by_name_missing() {
    let g = minimal_grammar("vr_v10_find_miss");
    assert!(g.find_symbol_by_name("nonexistent").is_none());
}

// --- check_empty_terminals ---

#[test]
fn test_vr_v10_check_empty_terminals_ok() {
    let g = minimal_grammar("vr_v10_empty_term");
    assert!(g.check_empty_terminals().is_ok());
}

#[test]
fn test_vr_v10_check_empty_terminals_fail() {
    let mut g = Grammar::new("vr_v10_empty_term_fail".to_string());
    g.tokens.insert(
        SymbolId(1),
        Token {
            name: "BAD".to_string(),
            pattern: TokenPattern::String(String::new()),
            fragile: false,
        },
    );
    assert!(g.check_empty_terminals().is_err());
}

// --- Multiple validate calls on same grammar ---

#[test]
fn test_vr_v10_five_validates() {
    let g = arith_grammar("vr_v10_five");
    for _ in 0..5 {
        assert!(g.validate().is_ok());
    }
}

// --- Validate preserves extras, externals, inline_rules, supertypes ---

#[test]
fn test_vr_v10_validate_preserves_extras() {
    let g = GrammarBuilder::new("vr_v10_pres_ext")
        .token("A", "a")
        .token("WS", r"[ ]+")
        .rule("root", vec!["A"])
        .extra("WS")
        .start("root")
        .build();
    let extras_before = g.extras.len();
    let _ = g.validate();
    assert_eq!(g.extras.len(), extras_before);
}

#[test]
fn test_vr_v10_validate_preserves_externals() {
    let g = GrammarBuilder::new("vr_v10_pres_ext2")
        .token("A", "a")
        .token("EXT", "ext")
        .external("EXT")
        .rule("root", vec!["A"])
        .start("root")
        .build();
    let ext_before = g.externals.len();
    let _ = g.validate();
    assert_eq!(g.externals.len(), ext_before);
}

#[test]
fn test_vr_v10_validate_preserves_inline_rules() {
    let g = GrammarBuilder::new("vr_v10_pres_inl")
        .token("A", "a")
        .rule("root", vec!["helper"])
        .rule("helper", vec!["A"])
        .inline("helper")
        .start("root")
        .build();
    let inl_before = g.inline_rules.len();
    let _ = g.validate();
    assert_eq!(g.inline_rules.len(), inl_before);
}

#[test]
fn test_vr_v10_validate_preserves_supertypes() {
    let g = GrammarBuilder::new("vr_v10_pres_sup")
        .token("A", "a")
        .rule("root", vec!["expr"])
        .rule("expr", vec!["A"])
        .supertype("expr")
        .start("root")
        .build();
    let sup_before = g.supertypes.len();
    let _ = g.validate();
    assert_eq!(g.supertypes.len(), sup_before);
}

// --- Grammar with long rule RHS ---

#[test]
fn test_vr_v10_long_rhs() {
    let g = GrammarBuilder::new("vr_v10_long_rhs")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .token("D", "d")
        .token("E", "e")
        .rule("root", vec!["A", "B", "C", "D", "E"])
        .start("root")
        .build();
    assert!(g.validate().is_ok());
}

// --- Self-referential (directly recursive) rule ---

#[test]
fn test_vr_v10_self_recursive() {
    let g = GrammarBuilder::new("vr_v10_self_rec")
        .token("A", "a")
        .rule("root", vec!["root", "A"])
        .rule("root", vec!["A"])
        .start("root")
        .build();
    assert!(g.validate().is_ok());
}

// --- Multiple start candidates ---

#[test]
fn test_vr_v10_explicit_start_overrides() {
    let g = GrammarBuilder::new("vr_v10_start_override")
        .token("A", "a")
        .token("B", "b")
        .rule("alpha", vec!["A"])
        .rule("beta", vec!["B"])
        .start("beta")
        .build();
    let start = g.start_symbol().unwrap();
    let first_lhs = *g.rules.keys().next().unwrap();
    // start() moves the start rule to front
    assert_eq!(start, first_lhs);
}

// --- validate() with Associativity on rules ---

#[test]
fn test_vr_v10_rule_has_associativity() {
    let g = GrammarBuilder::new("vr_v10_rule_assoc")
        .token("N", "n")
        .token("+", "+")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["N"])
        .start("expr")
        .build();
    let prec_rule = g.all_rules().find(|r| r.associativity.is_some()).unwrap();
    assert_eq!(prec_rule.associativity, Some(Associativity::Left));
    assert!(g.validate().is_ok());
}

// --- ConflictResolution::Associativity variant ---

#[test]
fn test_vr_v10_conflict_assoc_resolution() {
    let mut g = arith_grammar("vr_v10_conflict_assoc");
    let sym_ids: Vec<SymbolId> = g.rules.keys().copied().collect();
    if !sym_ids.is_empty() {
        g.conflicts.push(ConflictDeclaration {
            symbols: vec![sym_ids[0]],
            resolution: ConflictResolution::Associativity(Associativity::Left),
        });
    }
    assert!(g.validate().is_ok());
}

// --- Grammar::new vs Default ---

#[test]
fn test_vr_v10_new_vs_default() {
    let g1 = Grammar::new("vr_v10_cmp".to_string());
    let g2 = Grammar::default();
    // Both empty but different names
    assert!(g1.validate().is_ok());
    assert!(g2.validate().is_ok());
}
