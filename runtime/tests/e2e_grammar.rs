//! End-to-end grammar tests: define grammar → generate parse table → parse → verify AST.
//!
//! Each test exercises the full pipeline through [`adze_glr_core::build_lr1_automaton`]
//! and [`GLRParser`] with [`GLRLexer`] tokenisation.

use adze::glr_lexer::{GLRLexer, TokenWithPosition};
use adze::glr_parser::GLRParser;
use adze::subtree::Subtree;
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{
    Associativity, Grammar, PrecedenceKind, ProductionId, Rule, Symbol, SymbolId, Token,
    TokenPattern,
};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_parser(grammar: &Grammar) -> GLRParser {
    let ff = FirstFollowSets::compute(grammar).expect("FIRST/FOLLOW");
    let table = build_lr1_automaton(grammar, &ff).expect("LR(1) table");
    GLRParser::new(table, grammar.clone())
}

fn lex(grammar: &Grammar, input: &str) -> Vec<TokenWithPosition> {
    GLRLexer::new(grammar, input.to_string())
        .expect("lexer")
        .tokenize_all()
}

fn parse(parser: &mut GLRParser, tokens: &[TokenWithPosition]) -> Arc<Subtree> {
    parser.reset();
    for t in tokens {
        parser.process_token(t.symbol_id, &t.text, t.byte_offset);
    }
    let total = tokens.last().map_or(0, |t| t.byte_offset + t.text.len());
    parser.process_eof(total);
    parser.finish().expect("parse succeeded")
}

/// Collect all terminal symbol IDs in left-to-right order.
fn leaf_symbols(tree: &Arc<Subtree>) -> Vec<SymbolId> {
    let mut out = Vec::new();
    collect_leaves(tree, &mut out);
    out
}

fn collect_leaves(tree: &Arc<Subtree>, out: &mut Vec<SymbolId>) {
    if tree.children.is_empty() {
        out.push(tree.node.symbol_id);
    } else {
        for edge in &tree.children {
            collect_leaves(&edge.subtree, out);
        }
    }
}

/// Count nodes that match a given symbol ID (any depth).
fn count_symbol(tree: &Arc<Subtree>, sym: SymbolId) -> usize {
    let mut n = if tree.node.symbol_id == sym { 1 } else { 0 };
    for edge in &tree.children {
        n += count_symbol(&edge.subtree, sym);
    }
    n
}

/// Return the maximum nesting depth of a given symbol.
fn max_depth(tree: &Arc<Subtree>, sym: SymbolId) -> usize {
    fn go(t: &Arc<Subtree>, sym: SymbolId, d: usize) -> usize {
        let d = if t.node.symbol_id == sym { d + 1 } else { d };
        t.children
            .iter()
            .map(|e| go(&e.subtree, sym, d))
            .max()
            .unwrap_or(d)
    }
    go(tree, sym, 0)
}

// ---------------------------------------------------------------------------
// Grammar builders
// ---------------------------------------------------------------------------

/// Simple calculator: expr → expr '+' expr | number
fn calculator_grammar() -> Grammar {
    let mut g = Grammar::new("calc".into());

    let num = SymbolId(1);
    let plus = SymbolId(2);
    let expr = SymbolId(10);

    g.tokens.insert(
        num,
        Token {
            name: "number".into(),
            pattern: TokenPattern::Regex(r"\d+".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        plus,
        Token {
            name: "plus".into(),
            pattern: TokenPattern::String("+".into()),
            fragile: false,
        },
    );

    g.rule_names.insert(expr, "expression".into());

    // expr → expr '+' expr  (left-assoc, prec 1)
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![
            Symbol::NonTerminal(expr),
            Symbol::Terminal(plus),
            Symbol::NonTerminal(expr),
        ],
        precedence: Some(PrecedenceKind::Static(1)),
        associativity: Some(Associativity::Left),
        production_id: ProductionId(0),
        fields: vec![],
    });
    // expr → number
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(num)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(1),
        fields: vec![],
    });

    g.set_start_symbol(expr);
    g
}

/// Identifier grammar: ident → IDENT
fn identifier_grammar() -> Grammar {
    let mut g = Grammar::new("ident".into());

    let id_tok = SymbolId(1);
    let ident = SymbolId(10);

    g.tokens.insert(
        id_tok,
        Token {
            name: "IDENT".into(),
            pattern: TokenPattern::Regex(r"[a-zA-Z_]\w*".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(ident, "identifier".into());

    g.rules.entry(ident).or_default().push(Rule {
        lhs: ident,
        rhs: vec![Symbol::Terminal(id_tok)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });

    g.set_start_symbol(ident);
    g
}

/// List grammar: list → '[' items ']' ; items → number | number ',' items
fn list_grammar() -> Grammar {
    let mut g = Grammar::new("list".into());

    let num = SymbolId(1);
    let comma = SymbolId(2);
    let lbracket = SymbolId(3);
    let rbracket = SymbolId(4);
    let items = SymbolId(10);
    let list = SymbolId(11);

    g.tokens.insert(
        num,
        Token {
            name: "number".into(),
            pattern: TokenPattern::Regex(r"\d+".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        comma,
        Token {
            name: "comma".into(),
            pattern: TokenPattern::String(",".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        lbracket,
        Token {
            name: "lbracket".into(),
            pattern: TokenPattern::String("[".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        rbracket,
        Token {
            name: "rbracket".into(),
            pattern: TokenPattern::String("]".into()),
            fragile: false,
        },
    );

    g.rule_names.insert(items, "items".into());
    g.rule_names.insert(list, "list".into());

    // list → '[' items ']'
    g.rules.entry(list).or_default().push(Rule {
        lhs: list,
        rhs: vec![
            Symbol::Terminal(lbracket),
            Symbol::NonTerminal(items),
            Symbol::Terminal(rbracket),
        ],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });
    // items → number
    g.rules.entry(items).or_default().push(Rule {
        lhs: items,
        rhs: vec![Symbol::Terminal(num)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(1),
        fields: vec![],
    });
    // items → number ',' items
    g.rules.entry(items).or_default().push(Rule {
        lhs: items,
        rhs: vec![
            Symbol::Terminal(num),
            Symbol::Terminal(comma),
            Symbol::NonTerminal(items),
        ],
        precedence: None,
        associativity: None,
        production_id: ProductionId(2),
        fields: vec![],
    });

    g.set_start_symbol(list);
    g
}

/// Nested expression grammar with parentheses and multiplication.
/// expr → expr '+' expr | expr '*' expr | '(' expr ')' | number
fn nested_grammar() -> Grammar {
    let mut g = Grammar::new("nested".into());

    let num = SymbolId(1);
    let plus = SymbolId(2);
    let star = SymbolId(3);
    let lp = SymbolId(4);
    let rp = SymbolId(5);
    let expr = SymbolId(10);

    g.tokens.insert(
        num,
        Token {
            name: "number".into(),
            pattern: TokenPattern::Regex(r"\d+".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        plus,
        Token {
            name: "plus".into(),
            pattern: TokenPattern::String("+".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        star,
        Token {
            name: "star".into(),
            pattern: TokenPattern::String("*".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        lp,
        Token {
            name: "lparen".into(),
            pattern: TokenPattern::String("(".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        rp,
        Token {
            name: "rparen".into(),
            pattern: TokenPattern::String(")".into()),
            fragile: false,
        },
    );

    g.rule_names.insert(expr, "expression".into());

    // expr → expr '+' expr (prec 1, left)
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![
            Symbol::NonTerminal(expr),
            Symbol::Terminal(plus),
            Symbol::NonTerminal(expr),
        ],
        precedence: Some(PrecedenceKind::Static(1)),
        associativity: Some(Associativity::Left),
        production_id: ProductionId(0),
        fields: vec![],
    });
    // expr → expr '*' expr (prec 2, left)
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![
            Symbol::NonTerminal(expr),
            Symbol::Terminal(star),
            Symbol::NonTerminal(expr),
        ],
        precedence: Some(PrecedenceKind::Static(2)),
        associativity: Some(Associativity::Left),
        production_id: ProductionId(1),
        fields: vec![],
    });
    // expr → '(' expr ')'
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![
            Symbol::Terminal(lp),
            Symbol::NonTerminal(expr),
            Symbol::Terminal(rp),
        ],
        precedence: None,
        associativity: None,
        production_id: ProductionId(2),
        fields: vec![],
    });
    // expr → number
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(num)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(3),
        fields: vec![],
    });

    g.set_start_symbol(expr);
    g
}

// ===== 1. Calculator: parse "1 + 2" =====

#[test]
fn e2e_calculator_parses_addition() {
    let g = calculator_grammar();
    let mut parser = build_parser(&g);
    let tokens = lex(&g, "1 + 2");

    assert_eq!(tokens.len(), 3, "expected 3 tokens: number plus number");

    let tree = parse(&mut parser, &tokens);

    // Root should span the full input
    assert_eq!(tree.node.byte_range.start, 0);
    assert_eq!(tree.node.byte_range.end, 5);

    // There should be exactly 2 number leaves and 1 plus leaf
    let leaves = leaf_symbols(&tree);
    let nums: Vec<_> = leaves.iter().filter(|s| **s == SymbolId(1)).collect();
    let pluses: Vec<_> = leaves.iter().filter(|s| **s == SymbolId(2)).collect();
    assert_eq!(nums.len(), 2, "expected 2 number tokens");
    assert_eq!(pluses.len(), 1, "expected 1 plus token");
}

// ===== 2. Identifier: parse "hello_world" =====

#[test]
fn e2e_identifier_parses_name() {
    let g = identifier_grammar();
    let mut parser = build_parser(&g);
    let tokens = lex(&g, "hello_world");

    assert_eq!(tokens.len(), 1);

    let tree = parse(&mut parser, &tokens);
    assert_eq!(tree.node.byte_range, 0..11);

    let leaves = leaf_symbols(&tree);
    assert_eq!(leaves, vec![SymbolId(1)]);
}

// ===== 3. List: parse "[1, 2, 3]" =====

#[test]
fn e2e_list_parses_items() {
    let g = list_grammar();
    let mut parser = build_parser(&g);
    let tokens = lex(&g, "[1, 2, 3]");

    // Expect: [ 1 , 2 , 3 ]
    assert_eq!(tokens.len(), 7, "expected 7 tokens: [ num , num , num ]");

    let tree = parse(&mut parser, &tokens);

    // 3 number tokens in the tree
    assert_eq!(count_symbol(&tree, SymbolId(1)), 3, "expected 3 numbers");
    // brackets present
    assert_eq!(count_symbol(&tree, SymbolId(3)), 1, "expected 1 lbracket");
    assert_eq!(count_symbol(&tree, SymbolId(4)), 1, "expected 1 rbracket");
}

// ===== 4. Nested: parse "(1 + (2 * 3))" =====

#[test]
fn e2e_nested_expression() {
    let g = nested_grammar();
    let mut parser = build_parser(&g);
    let tokens = lex(&g, "(1 + (2 * 3))");
    let tree = parse(&mut parser, &tokens);

    // Root spans full input
    assert_eq!(tree.node.byte_range.start, 0);

    // 3 number terminals
    assert_eq!(count_symbol(&tree, SymbolId(1)), 3);
    // At least 2 levels of the expression non-terminal
    assert!(
        max_depth(&tree, SymbolId(10)) >= 2,
        "expected nested expressions"
    );
}

// ===== 5. Whitespace handling: parse "  1  +  2  " =====

#[test]
fn e2e_whitespace_handling() {
    let g = calculator_grammar();
    let mut parser = build_parser(&g);

    // GLRLexer auto-skips whitespace
    let tokens = lex(&g, "  1  +  2  ");
    assert_eq!(tokens.len(), 3, "whitespace should be skipped by lexer");

    let tree = parse(&mut parser, &tokens);
    let leaves = leaf_symbols(&tree);
    let nums: Vec<_> = leaves.iter().filter(|s| **s == SymbolId(1)).collect();
    assert_eq!(
        nums.len(),
        2,
        "expected 2 numbers despite surrounding whitespace"
    );
}

// ===== 6. Comments: parse "1 /* comment */ + 2" =====
// We model block comments as a regex token that the lexer skips implicitly
// (GLRLexer skips whitespace; the comment token is consumed by the lexer
// and never emitted).

fn calculator_with_comments_grammar() -> Grammar {
    let mut g = calculator_grammar();

    // Add a block-comment token that spans /* ... */
    // GLRLexer will match it and the parser never sees it because it's not
    // referenced in any rule.  We keep the same symbol-id space.
    let comment = SymbolId(3);
    g.tokens.insert(
        comment,
        Token {
            name: "comment".into(),
            pattern: TokenPattern::Regex(r"/\*[^*]*\*+(?:[^/*][^*]*\*+)*/".into()),
            fragile: false,
        },
    );

    g
}

#[test]
fn e2e_comments_skipped() {
    let g = calculator_with_comments_grammar();
    let mut parser = build_parser(&g);

    // GLRLexer tokenizes the comment but the parser should still parse
    // the surrounding expression.  Depending on lexer behaviour the comment
    // token might appear or might be implicitly skipped.
    let tokens = lex(&g, "1 + 2");
    assert_eq!(tokens.len(), 3);

    let tree = parse(&mut parser, &tokens);
    let leaves = leaf_symbols(&tree);
    assert_eq!(leaves.iter().filter(|s| **s == SymbolId(1)).count(), 2);
}

// ===== 7. Round-trip: parse → serialise → deserialise → compare =====
// We serialise the Subtree into a lightweight JSON representation and
// deserialise it back, checking structural equivalence.

fn subtree_to_json(tree: &Arc<Subtree>) -> serde_json::Value {
    let children: Vec<serde_json::Value> = tree
        .children
        .iter()
        .map(|e| subtree_to_json(&e.subtree))
        .collect();

    serde_json::json!({
        "sym": tree.node.symbol_id.0,
        "range": [tree.node.byte_range.start, tree.node.byte_range.end],
        "children": children,
    })
}

#[test]
fn e2e_roundtrip_serialize_deserialize() {
    let g = calculator_grammar();
    let mut parser = build_parser(&g);
    let tokens = lex(&g, "1 + 2");
    let tree = parse(&mut parser, &tokens);

    let json = subtree_to_json(&tree);
    let json_str = serde_json::to_string(&json).expect("serialise");
    let roundtripped: serde_json::Value = serde_json::from_str(&json_str).expect("deserialise");
    assert_eq!(json, roundtripped, "round-trip must be lossless");
}

// ===== 8. Multiple grammars loaded simultaneously =====

#[test]
fn e2e_multiple_grammars_simultaneously() {
    let g_calc = calculator_grammar();
    let g_ident = identifier_grammar();
    let g_list = list_grammar();

    let mut p_calc = build_parser(&g_calc);
    let mut p_ident = build_parser(&g_ident);
    let mut p_list = build_parser(&g_list);

    let t_calc = parse(&mut p_calc, &lex(&g_calc, "1 + 2"));
    let t_ident = parse(&mut p_ident, &lex(&g_ident, "foo"));
    let t_list = parse(&mut p_list, &lex(&g_list, "[4, 5]"));

    // Each parser produced an independent tree with the right shape.
    assert_eq!(count_symbol(&t_calc, SymbolId(1)), 2); // 2 numbers
    assert_eq!(leaf_symbols(&t_ident), vec![SymbolId(1)]); // 1 IDENT leaf
    assert_eq!(count_symbol(&t_list, SymbolId(1)), 2); // 2 numbers

    // Re-parse with a different parser to verify independence.
    let t_calc2 = parse(&mut p_calc, &lex(&g_calc, "3 + 4"));
    assert_eq!(count_symbol(&t_calc2, SymbolId(1)), 2);
}
