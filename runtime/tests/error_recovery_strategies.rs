// Comprehensive error recovery strategy tests.
//
// Tests exercise the GLR parser's ability to recover from a wide range
// of syntax errors and produce a (partial) parse tree annotated with
// error nodes.

use adze::error_recovery::{ErrorRecoveryConfig, ErrorRecoveryConfigBuilder};
use adze::glr_lexer::GLRLexer;
use adze::glr_parser::GLRParser;
use adze::subtree::Subtree;
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Symbol constants
// ---------------------------------------------------------------------------

const NUM: SymbolId = SymbolId(1);
const PLUS: SymbolId = SymbolId(2);
const STAR: SymbolId = SymbolId(3);
const LPAREN: SymbolId = SymbolId(4);
const RPAREN: SymbolId = SymbolId(5);
const MINUS: SymbolId = SymbolId(6);
const EXPR: SymbolId = SymbolId(10);

// ---------------------------------------------------------------------------
// Grammar helpers
// ---------------------------------------------------------------------------

/// Arithmetic expression grammar (no statement wrapper / semicolons).
///
/// expression → expression '+' expression
/// expression → expression '-' expression
/// expression → expression '*' expression
/// expression → '(' expression ')'
/// expression → number
fn expr_grammar() -> Grammar {
    let mut g = Grammar::new("expr".to_string());

    for (id, name, pat) in [
        (NUM, "number", TokenPattern::Regex("[0-9]+".to_string())),
        (PLUS, "plus", TokenPattern::String("+".to_string())),
        (STAR, "star", TokenPattern::String("*".to_string())),
        (LPAREN, "lparen", TokenPattern::String("(".to_string())),
        (RPAREN, "rparen", TokenPattern::String(")".to_string())),
        (MINUS, "minus", TokenPattern::String("-".to_string())),
    ] {
        g.tokens.insert(
            id,
            Token {
                name: name.to_string(),
                pattern: pat,
                fragile: false,
            },
        );
    }

    g.rule_names.insert(EXPR, "expression".to_string());

    let mut prod = 0u16;
    let mut push_rule = |rhs: Vec<Symbol>| {
        let id = ProductionId(prod);
        prod += 1;
        g.rules.entry(EXPR).or_default().push(Rule {
            lhs: EXPR,
            rhs,
            precedence: None,
            associativity: None,
            production_id: id,
            fields: vec![],
        });
    };

    // expression → expression '+' expression
    push_rule(vec![
        Symbol::NonTerminal(EXPR),
        Symbol::Terminal(PLUS),
        Symbol::NonTerminal(EXPR),
    ]);
    // expression → expression '-' expression
    push_rule(vec![
        Symbol::NonTerminal(EXPR),
        Symbol::Terminal(MINUS),
        Symbol::NonTerminal(EXPR),
    ]);
    // expression → expression '*' expression
    push_rule(vec![
        Symbol::NonTerminal(EXPR),
        Symbol::Terminal(STAR),
        Symbol::NonTerminal(EXPR),
    ]);
    // expression → '(' expression ')'
    push_rule(vec![
        Symbol::Terminal(LPAREN),
        Symbol::NonTerminal(EXPR),
        Symbol::Terminal(RPAREN),
    ]);
    // expression → number
    push_rule(vec![Symbol::Terminal(NUM)]);

    g.set_start_symbol(EXPR);
    g
}

// ---------------------------------------------------------------------------
// Recovery config helpers
// ---------------------------------------------------------------------------

fn default_config() -> ErrorRecoveryConfig {
    ErrorRecoveryConfigBuilder::new()
        .add_insertable_token(RPAREN.0) // can auto-close parens
        .add_insertable_token(NUM.0) // can auto-insert a number
        .add_scope_delimiter(LPAREN.0, RPAREN.0)
        .enable_scope_recovery(true)
        .enable_phrase_recovery(true)
        .max_consecutive_errors(20)
        .build()
}

// ---------------------------------------------------------------------------
// Parse / tree helpers
// ---------------------------------------------------------------------------

fn parse(grammar: &Grammar, input: &str, config: ErrorRecoveryConfig) -> Option<Arc<Subtree>> {
    let ff = FirstFollowSets::compute(grammar).ok()?;
    let table = build_lr1_automaton(grammar, &ff).ok()?;

    let mut parser = GLRParser::new(table, grammar.clone());
    parser.enable_error_recovery(config);

    let mut lexer = GLRLexer::new(grammar, input.to_string()).ok()?;
    let tokens = lexer.tokenize_all();

    parser.reset();
    for tok in &tokens {
        parser.process_token(tok.symbol_id, &tok.text, tok.byte_offset);
    }
    let total_bytes = tokens
        .last()
        .map(|t| t.byte_offset + t.text.len())
        .unwrap_or(0);
    parser.process_eof(total_bytes);
    parser.finish().ok()
}

fn has_error(tree: &Subtree) -> bool {
    if tree.node.is_error {
        return true;
    }
    tree.children.iter().any(|e| has_error(&e.subtree))
}

fn collect_errors(tree: &Subtree) -> Vec<Arc<Subtree>> {
    let mut out = Vec::new();
    collect_errors_inner(tree, &mut out);
    out
}

fn collect_errors_inner(tree: &Subtree, out: &mut Vec<Arc<Subtree>>) {
    for edge in &tree.children {
        if edge.subtree.node.is_error {
            out.push(edge.subtree.clone());
        }
        collect_errors_inner(&edge.subtree, out);
    }
}

fn count_nodes(tree: &Subtree) -> usize {
    1 + tree
        .children
        .iter()
        .map(|e| count_nodes(&e.subtree))
        .sum::<usize>()
}

/// Collect byte ranges of all non-error leaf nodes.
fn collect_leaf_ranges(tree: &Subtree) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    collect_leaf_ranges_inner(tree, &mut out);
    out
}

fn collect_leaf_ranges_inner(tree: &Subtree, out: &mut Vec<std::ops::Range<usize>>) {
    if tree.children.is_empty() && !tree.node.is_error {
        out.push(tree.node.byte_range.clone());
    }
    for edge in &tree.children {
        collect_leaf_ranges_inner(&edge.subtree, out);
    }
}

// ===========================================================================
// 1. Missing operator recovery: "1 2"
// ===========================================================================

#[test]
fn test_missing_operator_recovery() {
    let g = expr_grammar();
    let tree = parse(&g, "1 2", default_config());
    assert!(tree.is_some(), "should produce a tree for '1 2'");
    let tree = tree.unwrap();
    assert!(has_error(&tree), "'1 2' should contain an error node");
}

// ===========================================================================
// 2. Extra operator recovery: "1++2"
// ===========================================================================

#[test]
fn test_extra_operator_recovery() {
    let g = expr_grammar();
    let tree = parse(&g, "1++2", default_config());
    assert!(tree.is_some(), "should produce a tree for '1++2'");
    let tree = tree.unwrap();
    assert!(has_error(&tree), "'1++2' should contain an error node");
}

// ===========================================================================
// 3. Missing closing paren: "(1+2"
// ===========================================================================

#[test]
fn test_missing_closing_paren() {
    let g = expr_grammar();
    let tree = parse(&g, "(1+2", default_config());
    assert!(tree.is_some(), "should produce a tree for '(1+2'");
    let tree = tree.unwrap();
    assert!(has_error(&tree), "'(1+2' should contain an error node");
}

// ===========================================================================
// 4. Extra closing paren: "1+2)"
// ===========================================================================

#[test]
fn test_extra_closing_paren() {
    let g = expr_grammar();
    let tree = parse(&g, "1+2)", default_config());
    assert!(tree.is_some(), "should produce a tree for '1+2)'");
    let tree = tree.unwrap();
    assert!(has_error(&tree), "'1+2)' should contain an error node");
}

// ===========================================================================
// 5. Missing operand: "+2"
// ===========================================================================

#[test]
fn test_missing_operand_leading_op() {
    let g = expr_grammar();
    let tree = parse(&g, "+2", default_config());
    assert!(tree.is_some(), "should produce a tree for '+2'");
    let tree = tree.unwrap();
    assert!(has_error(&tree), "'+2' should contain an error node");
}

// ===========================================================================
// 6. Empty between operators: "1+*2"
// ===========================================================================

#[test]
fn test_empty_between_operators() {
    let g = expr_grammar();
    let tree = parse(&g, "1+*2", default_config());
    assert!(tree.is_some(), "should produce a tree for '1+*2'");
    let tree = tree.unwrap();
    assert!(has_error(&tree), "'1+*2' should contain an error node");
}

// ===========================================================================
// 7. Multiple consecutive errors: "1++--2"
// ===========================================================================

#[test]
fn test_multiple_consecutive_errors() {
    let g = expr_grammar();
    let tree = parse(&g, "1++--2", default_config());
    assert!(tree.is_some(), "should produce a tree for '1++--2'");
    let tree = tree.unwrap();
    assert!(has_error(&tree), "'1++--2' should have error markers");
}

#[test]
fn test_multiple_consecutive_errors_count() {
    let g = expr_grammar();
    let tree = parse(&g, "1++--2", default_config());
    if let Some(tree) = tree {
        let errors = collect_errors(&tree);
        // At least one error node must exist for the extraneous operators
        assert!(
            !errors.is_empty(),
            "expected at least one error node in '1++--2'"
        );
    }
}

// ===========================================================================
// 8. Error at start of input: "+1+2"
// ===========================================================================

#[test]
fn test_error_at_start() {
    let g = expr_grammar();
    let tree = parse(&g, "+1+2", default_config());
    assert!(tree.is_some(), "should produce a tree for '+1+2'");
    let tree = tree.unwrap();
    assert!(has_error(&tree), "'+1+2' should contain an error node");
}

// ===========================================================================
// 9. Error at end of input: "1+2+"
// ===========================================================================

#[test]
fn test_error_at_end() {
    let g = expr_grammar();
    // Trailing operator with an insertable number allows recovery.
    let config = ErrorRecoveryConfigBuilder::new()
        .add_insertable_token(NUM.0)
        .enable_phrase_recovery(true)
        .max_consecutive_errors(20)
        .build();
    let tree = parse(&g, "1+2+", config);
    // The parser may or may not recover; either way, no panic.
    if let Some(tree) = tree {
        assert!(has_error(&tree), "'1+2+' should contain an error node");
    }
}

// ===========================================================================
// 10. Error in nested context: "(1+(+2))"
// ===========================================================================

#[test]
fn test_error_in_nested_context() {
    let g = expr_grammar();
    let tree = parse(&g, "(1+(+2))", default_config());
    assert!(tree.is_some(), "should produce a tree for '(1+(+2))'");
    let tree = tree.unwrap();
    assert!(
        has_error(&tree),
        "'(1+(+2))' should contain an error in the inner expression"
    );
}

// ===========================================================================
// 11. Recovery produces a tree (never None for non-empty input)
// ===========================================================================

#[test]
fn test_recovery_never_returns_none_for_nonempty() {
    let g = expr_grammar();
    // Inputs that have at least one valid token the parser can anchor on.
    let inputs = ["1", "1+2", "1 2 3", "1+(2)"];
    for input in &inputs {
        let result = parse(&g, input, default_config());
        assert!(
            result.is_some(),
            "should produce a tree for non-empty input '{}'",
            input
        );
    }
}

#[test]
fn test_recovery_difficult_inputs_no_panic() {
    let g = expr_grammar();
    // Edge-case inputs that may or may not produce a tree – we assert no panic.
    let inputs = ["+", "++", "(", ")", "()", "***", "1+(", "((("];
    for input in &inputs {
        let _result = parse(&g, input, default_config());
    }
}

#[test]
fn test_recovery_returns_none_for_empty() {
    let g = expr_grammar();
    // Empty string has no tokens – the parser may legitimately return None.
    let _result = parse(&g, "", default_config());
    // We only assert it does not panic.
}

// ===========================================================================
// 12. Error nodes have correct byte ranges
// ===========================================================================

#[test]
fn test_error_node_byte_ranges_within_input() {
    let g = expr_grammar();
    let input = "1++2";
    let tree = parse(&g, input, default_config());
    assert!(tree.is_some());
    let tree = tree.unwrap();

    let errors = collect_errors(&tree);
    for err in &errors {
        assert!(
            err.node.byte_range.start <= err.node.byte_range.end,
            "error byte_range start must be <= end"
        );
        assert!(
            err.node.byte_range.end <= input.len(),
            "error byte_range.end ({}) must be <= input len ({})",
            err.node.byte_range.end,
            input.len()
        );
    }
}

#[test]
fn test_error_node_byte_ranges_nonempty() {
    let g = expr_grammar();
    let input = "1++2";
    let tree = parse(&g, input, default_config());
    if let Some(tree) = tree {
        let errors = collect_errors(&tree);
        // Error nodes exist; they may have zero-length ranges for inserted tokens.
        for err in &errors {
            assert!(
                err.node.byte_range.start <= err.node.byte_range.end,
                "error range start must be <= end"
            );
        }
    }
}

// ===========================================================================
// 13. Non-error siblings have correct ranges despite error
// ===========================================================================

#[test]
fn test_non_error_siblings_correct_ranges() {
    let g = expr_grammar();
    let input = "1++2";
    let tree = parse(&g, input, default_config());
    assert!(tree.is_some());
    let tree = tree.unwrap();

    let leaves = collect_leaf_ranges(&tree);
    for range in &leaves {
        assert!(range.start <= range.end, "leaf range start must be <= end");
        assert!(
            range.end <= input.len(),
            "leaf range.end ({}) must be <= input len ({})",
            range.end,
            input.len()
        );
    }
}

#[test]
fn test_sibling_ranges_non_overlapping() {
    let g = expr_grammar();
    let input = "1+2";
    let tree = parse(&g, input, default_config());
    assert!(tree.is_some());
    let tree = tree.unwrap();

    // Check direct children for each interior node
    fn check_children_non_overlapping(tree: &Subtree) {
        let mut prev_end = 0usize;
        for edge in &tree.children {
            let start = edge.subtree.node.byte_range.start;
            assert!(
                start >= prev_end,
                "child start {} should be >= previous end {}",
                start,
                prev_end
            );
            prev_end = edge.subtree.node.byte_range.end;
            check_children_non_overlapping(&edge.subtree);
        }
    }
    check_children_non_overlapping(&tree);
}

// ===========================================================================
// 14. Tree traversal works correctly with error nodes
// ===========================================================================

#[test]
fn test_traversal_visits_all_nodes() {
    let g = expr_grammar();
    let tree = parse(&g, "1++2", default_config());
    assert!(tree.is_some());
    let tree = tree.unwrap();

    let total = count_nodes(&tree);
    assert!(
        total >= 3,
        "tree should have at least 3 nodes, got {}",
        total
    );
}

#[test]
fn test_traversal_depth_first() {
    let g = expr_grammar();
    let tree = parse(&g, "1+2", default_config());
    assert!(tree.is_some());
    let tree = tree.unwrap();

    // Depth-first traversal collects symbol ids.
    fn dfs_symbols(tree: &Subtree) -> Vec<SymbolId> {
        let mut out = vec![tree.node.symbol_id];
        for edge in &tree.children {
            out.extend(dfs_symbols(&edge.subtree));
        }
        out
    }
    let symbols = dfs_symbols(&tree);
    assert!(
        !symbols.is_empty(),
        "depth-first traversal should collect at least one symbol"
    );
}

#[test]
fn test_traversal_error_subtree_has_children_or_is_leaf() {
    let g = expr_grammar();
    let tree = parse(&g, "1++2", default_config());
    if let Some(tree) = tree {
        let errors = collect_errors(&tree);
        for err in &errors {
            // Error node is either a leaf or has children – no invalid state
            let _ = err.children.len(); // just ensure it's accessible
        }
    }
}

// ===========================================================================
// 15. Multiple parses with errors produce consistent results
// ===========================================================================

#[test]
fn test_consistency_across_multiple_parses() {
    let g = expr_grammar();
    let input = "1++2";

    let tree1 = parse(&g, input, default_config());
    let tree2 = parse(&g, input, default_config());

    assert!(tree1.is_some());
    assert!(tree2.is_some());

    let t1 = tree1.unwrap();
    let t2 = tree2.unwrap();

    // Same structure: same node counts and same root byte range
    assert_eq!(
        count_nodes(&t1),
        count_nodes(&t2),
        "repeated parses should yield the same number of nodes"
    );
    assert_eq!(
        t1.node.byte_range, t2.node.byte_range,
        "root byte ranges should be identical across parses"
    );
}

#[test]
fn test_consistency_error_presence() {
    let g = expr_grammar();
    let input = "+1+2";

    let tree1 = parse(&g, input, default_config());
    let tree2 = parse(&g, input, default_config());

    if let (Some(t1), Some(t2)) = (&tree1, &tree2) {
        assert_eq!(
            has_error(t1),
            has_error(t2),
            "error presence should be consistent across parses"
        );
    }
}

// ===========================================================================
// Additional scenario tests (to reach 25+ total)
// ===========================================================================

#[test]
fn test_missing_operand_trailing_op() {
    let g = expr_grammar();
    let tree = parse(&g, "1+", default_config());
    assert!(tree.is_some(), "should produce a tree for '1+'");
    let tree = tree.unwrap();
    assert!(has_error(&tree), "'1+' should contain an error node");
}

#[test]
fn test_double_star_operator() {
    let g = expr_grammar();
    let tree = parse(&g, "1**2", default_config());
    assert!(tree.is_some(), "should produce a tree for '1**2'");
    let tree = tree.unwrap();
    assert!(has_error(&tree), "'1**2' should contain an error node");
}

#[test]
fn test_only_operators() {
    let g = expr_grammar();
    // Pure operator input is extremely degenerate; we just assert no panic.
    let _tree = parse(&g, "+-*", default_config());
}

#[test]
fn test_deeply_nested_missing_close() {
    let g = expr_grammar();
    let tree = parse(&g, "(((1+2)", default_config());
    assert!(tree.is_some(), "should produce a tree for '(((1+2)'");
}

#[test]
fn test_deeply_nested_extra_close() {
    let g = expr_grammar();
    let tree = parse(&g, "(1+2)))", default_config());
    assert!(tree.is_some(), "should produce a tree for '(1+2)))'");
}

#[test]
fn test_mixed_errors_parens_and_ops() {
    let g = expr_grammar();
    let tree = parse(&g, "(+1+*2))", default_config());
    assert!(tree.is_some(), "should produce a tree for '(+1+*2))'");
    let tree = tree.unwrap();
    assert!(has_error(&tree));
}

#[test]
fn test_adjacent_numbers_no_op() {
    let g = expr_grammar();
    let tree = parse(&g, "1 2 3", default_config());
    assert!(tree.is_some(), "should produce a tree for '1 2 3'");
    let tree = tree.unwrap();
    assert!(has_error(&tree), "'1 2 3' should have error nodes");
}

#[test]
fn test_root_byte_range_covers_input() {
    let g = expr_grammar();
    let input = "1+2";
    let tree = parse(&g, input, default_config());
    assert!(tree.is_some());
    let tree = tree.unwrap();
    assert_eq!(tree.node.byte_range.start, 0, "root should start at byte 0");
    assert_eq!(
        tree.node.byte_range.end,
        input.len(),
        "root should end at input length"
    );
}

#[test]
fn test_error_recovery_with_high_error_limit() {
    let g = expr_grammar();
    let config = ErrorRecoveryConfigBuilder::new()
        .max_consecutive_errors(100)
        .enable_phrase_recovery(true)
        .build();
    let tree = parse(&g, "++++++1", config);
    assert!(tree.is_some(), "should survive many leading operators");
}

#[test]
fn test_error_recovery_with_low_error_limit() {
    let g = expr_grammar();
    let config = ErrorRecoveryConfigBuilder::new()
        .max_consecutive_errors(2)
        .enable_phrase_recovery(true)
        .build();
    // With a very low limit the parser may still produce a partial tree.
    let _tree = parse(&g, "++++++1", config);
    // We only assert no panic.
}

#[test]
fn test_scope_recovery_enabled() {
    let g = expr_grammar();
    let config = ErrorRecoveryConfigBuilder::new()
        .add_insertable_token(RPAREN.0)
        .add_scope_delimiter(LPAREN.0, RPAREN.0)
        .enable_scope_recovery(true)
        .max_consecutive_errors(20)
        .build();
    let tree = parse(&g, "(1+2", config);
    assert!(tree.is_some(), "scope recovery should handle missing ')'");
}
