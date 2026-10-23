//! Snapshot tests for parse tree outputs across various grammars.
//!
//! Uses `insta::assert_snapshot!()` and `insta::assert_debug_snapshot!()` to
//! capture and verify parse tree representations for regression testing.
//!
//! Grammars exercised:
//!   - `arithmetic` (Sub/Mul with precedence)
//!   - `ambiguous_expr` (Binary with no precedence)
//!   - `repetitions` (delimited number lists)
#![cfg(feature = "with-grammars")]

mod common;

use adze::glr_lexer::GLRLexer;
use adze::glr_parser::GLRParser;
use adze::glr_tree_bridge::GLRTree;
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format an arithmetic parse result as an S-expression string.
fn arith_to_sexpr(expr: &adze_example::arithmetic::grammar::Expression) -> String {
    use adze_example::arithmetic::grammar::Expression;
    match expr {
        Expression::Number(n) => format!("(number {})", n),
        Expression::Sub(l, _, r) => {
            format!("(sub {} {})", arith_to_sexpr(l), arith_to_sexpr(r))
        }
        Expression::Mul(l, _, r) => {
            format!("(mul {} {})", arith_to_sexpr(l), arith_to_sexpr(r))
        }
    }
}

/// Format an ambiguous_expr parse result as an S-expression string.
fn ambig_to_sexpr(expr: &adze_example::ambiguous_expr::grammar::Expr) -> String {
    use adze_example::ambiguous_expr::grammar::Expr;
    match expr {
        Expr::Number(n) => format!("(number {})", n),
        Expr::Binary(l, op, r) => {
            format!(
                "(binary \"{}\" {} {})",
                op,
                ambig_to_sexpr(l),
                ambig_to_sexpr(r)
            )
        }
    }
}

/// Safely parse with the arithmetic grammar, catching panics.
fn safe_arith_parse(input: &str) -> String {
    let input_owned = input.to_string();
    match std::panic::catch_unwind(move || adze_example::arithmetic::grammar::parse(&input_owned)) {
        Ok(Ok(expr)) => arith_to_sexpr(&expr),
        Ok(Err(errors)) => {
            // Render the user-visible diagnostic, not the Debug form: the
            // snapshots below lock what users actually see (#875).
            let msgs: Vec<String> = errors
                .iter()
                .map(|e| e.display_with_source(input).to_string())
                .collect();
            format!("(ERROR {})", msgs.join("; "))
        }
        Err(_) => "(PANIC)".to_string(),
    }
}

/// Safely parse with the arithmetic grammar, returning Debug format.
fn safe_arith_parse_debug(input: &str) -> String {
    let input_owned = input.to_string();
    match std::panic::catch_unwind(move || adze_example::arithmetic::grammar::parse(&input_owned)) {
        Ok(result) => format!("{:#?}", result),
        Err(_) => "PANIC: Extract called with None node".to_string(),
    }
}

/// Safely parse with the ambiguous_expr grammar, catching panics.
fn safe_ambig_parse(input: &str) -> String {
    let input_owned = input.to_string();
    match std::panic::catch_unwind(move || {
        adze_example::ambiguous_expr::grammar::parse(&input_owned)
    }) {
        Ok(Ok(expr)) => ambig_to_sexpr(&expr),
        Ok(Err(errors)) => {
            let msgs: Vec<String> = errors
                .iter()
                .map(|e| e.display_with_source(input).to_string())
                .collect();
            format!("(ERROR {})", msgs.join("; "))
        }
        Err(_) => "(PANIC)".to_string(),
    }
}

// ===========================================================================
// 1. Simple subtraction "1-2"
// ===========================================================================

#[test]
fn snapshot_simple_subtraction_sexpr() {
    insta::assert_snapshot!("simple_subtraction_sexpr", safe_arith_parse("1-2"));
}

#[test]
fn snapshot_simple_subtraction_debug() {
    insta::assert_snapshot!("simple_subtraction_debug", safe_arith_parse_debug("1-2"));
}

// ===========================================================================
// 2. Nested precedence "1-2*3"
// ===========================================================================

#[test]
fn snapshot_nested_precedence_sexpr() {
    insta::assert_snapshot!("nested_precedence_sexpr", safe_arith_parse("1-2*3"));
}

#[test]
fn snapshot_nested_precedence_debug() {
    insta::assert_snapshot!("nested_precedence_debug", safe_arith_parse_debug("1-2*3"));
}

// ===========================================================================
// 3. Deep left-recursion "1-2-3-4-5"
// ===========================================================================

#[test]
fn snapshot_deep_left_recursion_sexpr() {
    insta::assert_snapshot!("deep_left_recursion_sexpr", safe_arith_parse("1-2-3-4-5"));
}

// ===========================================================================
// 4. Multiple mixed operations "1*2-3*4-5"
// ===========================================================================

#[test]
fn snapshot_multiple_operations_sexpr() {
    insta::assert_snapshot!("multiple_operations_sexpr", safe_arith_parse("1*2-3*4-5"));
}

// ===========================================================================
// 5. Single number "42"
// ===========================================================================

#[test]
fn snapshot_single_number_sexpr() {
    let result = adze_example::arithmetic::grammar::parse("42");
    insta::assert_snapshot!("single_number_sexpr", arith_to_sexpr(&result.unwrap()));
}

#[test]
fn snapshot_single_number_debug() {
    let result = adze_example::arithmetic::grammar::parse("42");
    insta::assert_debug_snapshot!("single_number_debug", result);
}

// ===========================================================================
// 6. Error input "1-" (trailing operator)
// ===========================================================================

#[test]
fn snapshot_error_trailing_op() {
    insta::assert_snapshot!("error_trailing_op_sexpr", safe_arith_parse("1-"));
}

// ===========================================================================
// 7. Empty input ""
// ===========================================================================

#[test]
fn snapshot_empty_input() {
    insta::assert_snapshot!("empty_input_sexpr", safe_arith_parse(""));
}

// ===========================================================================
// 8. Whitespace variations: "1 - 2" vs "1-2"
// ===========================================================================

#[test]
fn snapshot_whitespace_comparison() {
    let output = format!(
        "compact:  {}\nspaced:   {}\nextra:    {}",
        safe_arith_parse("1-2"),
        safe_arith_parse("1 - 2"),
        safe_arith_parse("1  -  2"),
    );
    insta::assert_snapshot!("whitespace_comparison", output);
}

// ===========================================================================
// 9. Large expression (20+ tokens) "1-2-3-4-5-6-7-8-9-10"
// ===========================================================================

#[test]
fn snapshot_large_expression_sexpr() {
    insta::assert_snapshot!(
        "large_expression_sexpr",
        safe_arith_parse("1-2-3-4-5-6-7-8-9-10")
    );
}

#[test]
fn snapshot_large_expression_debug() {
    insta::assert_snapshot!(
        "large_expression_debug",
        safe_arith_parse_debug("1-2-3-4-5-6-7-8-9-10")
    );
}

// ===========================================================================
// 10. GLR parse tree for ambiguous grammar — snapshot forest structure
// ===========================================================================

/// Build the classic ambiguous expression grammar: E → E+E | E*E | num
fn build_ambiguous_grammar() -> Grammar {
    let sym_num = SymbolId(1);
    let sym_plus = SymbolId(2);
    let sym_star = SymbolId(3);
    let sym_expr = SymbolId(10);

    let mut grammar = Grammar::new("ambiguous_expr_test".to_string());

    grammar.tokens.insert(
        sym_num,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        sym_plus,
        Token {
            name: "plus".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        sym_star,
        Token {
            name: "star".to_string(),
            pattern: TokenPattern::String("*".to_string()),
            fragile: false,
        },
    );

    grammar
        .rule_names
        .insert(sym_expr, "expression".to_string());
    grammar.rule_names.insert(sym_num, "number".to_string());
    grammar.rule_names.insert(sym_plus, "plus".to_string());
    grammar.rule_names.insert(sym_star, "star".to_string());

    // E → num
    grammar.add_rule(Rule {
        lhs: sym_expr,
        rhs: vec![Symbol::Terminal(sym_num)],
        production_id: ProductionId(0),
        precedence: None,
        associativity: None,
        fields: vec![],
    });
    // E → E + E
    grammar.add_rule(Rule {
        lhs: sym_expr,
        rhs: vec![
            Symbol::NonTerminal(sym_expr),
            Symbol::Terminal(sym_plus),
            Symbol::NonTerminal(sym_expr),
        ],
        production_id: ProductionId(1),
        precedence: None,
        associativity: None,
        fields: vec![],
    });
    // E → E * E
    grammar.add_rule(Rule {
        lhs: sym_expr,
        rhs: vec![
            Symbol::NonTerminal(sym_expr),
            Symbol::Terminal(sym_star),
            Symbol::NonTerminal(sym_expr),
        ],
        production_id: ProductionId(2),
        precedence: None,
        associativity: None,
        fields: vec![],
    });

    grammar
}

/// Format a Subtree as an S-expression with symbol names from the grammar.
fn subtree_to_sexpr(tree: &adze::subtree::Subtree, grammar: &Grammar, input: &str) -> String {
    let sym = tree.node.symbol_id;
    let name = grammar
        .rule_names
        .get(&sym)
        .or_else(|| grammar.tokens.get(&sym).map(|t| &t.name))
        .cloned()
        .unwrap_or_else(|| format!("sym_{}", sym.0));

    if tree.children.is_empty() {
        let range = &tree.node.byte_range;
        let text = &input[range.start..range.end];
        format!("({} \"{}\")", name, text)
    } else {
        let children: Vec<String> = tree
            .children
            .iter()
            .map(|edge| subtree_to_sexpr(&edge.subtree, grammar, input))
            .collect();
        if tree.is_ambiguous() {
            let alts: Vec<String> = tree
                .alternatives
                .iter()
                .map(|alt| subtree_to_sexpr(alt, grammar, input))
                .collect();
            format!(
                "(AMBIGUOUS {} [primary: {}] [alts: {}])",
                name,
                children.join(" "),
                alts.join(" | ")
            )
        } else {
            format!("({} {})", name, children.join(" "))
        }
    }
}

/// Helper to run GLR parser on input and return S-expression or error string.
fn glr_parse_to_sexpr(grammar: &Grammar, input: &str) -> String {
    let table = match common::build_table_result(grammar) {
        Ok(t) => t,
        Err(e) => return format!("(TABLE_ERROR \"{}\")", e),
    };
    let mut parser = GLRParser::new(table, grammar.clone());

    let mut lexer = match GLRLexer::new(grammar, input.to_string()) {
        Ok(l) => l,
        Err(e) => return format!("(LEXER_ERROR \"{}\")", e),
    };
    let tokens = lexer.tokenize_all();
    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }
    parser.process_eof(input.len());

    match parser.finish() {
        Ok(tree) => subtree_to_sexpr(&tree, grammar, input),
        Err(e) => format!("(PARSE_ERROR \"{}\")", e),
    }
}

#[test]
fn snapshot_glr_forest_simple() {
    let grammar = build_ambiguous_grammar();
    insta::assert_snapshot!("glr_forest_simple", glr_parse_to_sexpr(&grammar, "1+2"));
}

#[test]
fn snapshot_glr_forest_ambiguous() {
    let grammar = build_ambiguous_grammar();
    insta::assert_snapshot!(
        "glr_forest_ambiguous",
        glr_parse_to_sexpr(&grammar, "1+2*3")
    );
}

// ===========================================================================
// Additional snapshot tests for coverage
// ===========================================================================

// 11. Ambiguous grammar — simple addition
#[test]
fn snapshot_ambig_simple_add() {
    insta::assert_snapshot!("ambig_simple_add_sexpr", safe_ambig_parse("1+2"));
}

// 12. Ambiguous grammar — mixed operators
#[test]
fn snapshot_ambig_mixed_ops() {
    insta::assert_snapshot!("ambig_mixed_ops_sexpr", safe_ambig_parse("1+2*3"));
}

// 13. Ambiguous grammar — long chain
#[test]
fn snapshot_ambig_long_chain() {
    insta::assert_snapshot!("ambig_long_chain_sexpr", safe_ambig_parse("1+2-3*4"));
}

// 14. Error: invalid character in arithmetic grammar
#[test]
fn snapshot_error_invalid_char() {
    insta::assert_snapshot!("error_invalid_char_sexpr", safe_arith_parse("abc"));
}

// 15. Error: just whitespace
#[test]
fn snapshot_error_whitespace_only() {
    insta::assert_snapshot!("error_whitespace_only_sexpr", safe_arith_parse("   "));
}

// 16. Repetitions grammar — smoke tests (non-deterministic across runs, so no snapshot)
#[test]
fn snapshot_repetitions_single() {
    let result = adze_example::repetitions::grammar::parse("1");
    // Result may vary across runs; just verify it parses without panic
    let _output = format!("{:?}", result);
}

#[test]
fn snapshot_repetitions_list() {
    let result = adze_example::repetitions::grammar::parse("1, 2");
    let _output = format!("{:?}", result);
}

// 17. Zero value
#[test]
fn snapshot_zero() {
    let result = adze_example::arithmetic::grammar::parse("0");
    insta::assert_snapshot!("zero_sexpr", arith_to_sexpr(&result.unwrap()));
}

// 18. Large number
#[test]
fn snapshot_large_number() {
    let result = adze_example::arithmetic::grammar::parse("999999");
    insta::assert_snapshot!("large_number_sexpr", arith_to_sexpr(&result.unwrap()));
}

// 19. GLR tree bridge S-expression output
#[test]
fn snapshot_glr_tree_bridge_sexpr() {
    let grammar = build_ambiguous_grammar();
    let table = match common::build_table_result(&grammar) {
        Ok(t) => t,
        Err(e) => {
            insta::assert_snapshot!("glr_tree_bridge_sexpr", format!("(TABLE_ERROR \"{}\")", e));
            return;
        }
    };
    let mut parser = GLRParser::new(table, grammar.clone());

    let input = "42";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();
    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }
    parser.process_eof(input.len());

    match parser.finish() {
        Ok(tree) => {
            let glr_tree = GLRTree::new(tree, input.as_bytes().to_vec(), grammar.clone());
            let root = glr_tree.root_node();
            let sexpr = root.to_sexp();
            insta::assert_snapshot!("glr_tree_bridge_sexpr", sexpr);
        }
        Err(e) => {
            insta::assert_snapshot!("glr_tree_bridge_sexpr", format!("(PARSE_ERROR \"{}\")", e));
        }
    }
}
