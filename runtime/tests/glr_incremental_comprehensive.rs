#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]
//! Comprehensive tests for the GLR incremental parsing module.
//!
//! Covers: Edit, GLREdit, GLRToken, ForestNode, ChunkIdentifier,
//! IncrementalGLRParser (fresh parse, incremental reparse, forest splicing),
//! reuse counters, fork tracking, and edge cases.

#[cfg(feature = "incremental_glr")]
mod tests {
    use adze::glr_incremental::{
        ChunkIdentifier, Edit, ForestNode, ForkAlternative, GLREdit, GLRToken,
        IncrementalGLRParser, get_reuse_count, reset_reuse_counter,
    };
    use adze::glr_lexer::{GLRLexer, TokenWithPosition};
    use adze::subtree::{Subtree, SubtreeNode};
    use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
    use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
    use std::sync::Arc;

    // ── helpers ──────────────────────────────────────────────────────

    fn arith_grammar() -> Grammar {
        let mut g = Grammar::new("arithmetic".to_string());

        let num = SymbolId(1);
        let plus = SymbolId(2);
        let minus = SymbolId(3);
        let star = SymbolId(4);
        let lparen = SymbolId(5);
        let rparen = SymbolId(6);

        let expr = SymbolId(10);
        let term = SymbolId(11);
        let factor = SymbolId(12);
        let source = SymbolId(13);

        for (id, name, pat) in [
            (num, "NUM", TokenPattern::Regex(r"\d+".into())),
            (plus, "PLUS", TokenPattern::String("+".into())),
            (minus, "MINUS", TokenPattern::String("-".into())),
            (star, "STAR", TokenPattern::String("*".into())),
            (lparen, "LPAREN", TokenPattern::String("(".into())),
            (rparen, "RPAREN", TokenPattern::String(")".into())),
        ] {
            g.tokens.insert(
                id,
                Token {
                    name: name.into(),
                    pattern: pat,
                    fragile: false,
                },
            );
        }

        g.rules.insert(
            expr,
            vec![
                Rule {
                    lhs: expr,
                    rhs: vec![
                        Symbol::NonTerminal(expr),
                        Symbol::Terminal(plus),
                        Symbol::NonTerminal(term),
                    ],
                    precedence: Some(adze_ir::PrecedenceKind::Static(1)),
                    associativity: Some(adze_ir::Associativity::Left),
                    fields: vec![],
                    production_id: ProductionId(0),
                },
                Rule {
                    lhs: expr,
                    rhs: vec![
                        Symbol::NonTerminal(expr),
                        Symbol::Terminal(minus),
                        Symbol::NonTerminal(term),
                    ],
                    precedence: Some(adze_ir::PrecedenceKind::Static(1)),
                    associativity: Some(adze_ir::Associativity::Left),
                    fields: vec![],
                    production_id: ProductionId(1),
                },
                Rule {
                    lhs: expr,
                    rhs: vec![Symbol::NonTerminal(term)],
                    precedence: None,
                    associativity: None,
                    fields: vec![],
                    production_id: ProductionId(2),
                },
            ],
        );
        g.rule_names.insert(expr, "expr".into());

        g.rules.insert(
            term,
            vec![
                Rule {
                    lhs: term,
                    rhs: vec![
                        Symbol::NonTerminal(term),
                        Symbol::Terminal(star),
                        Symbol::NonTerminal(factor),
                    ],
                    precedence: Some(adze_ir::PrecedenceKind::Static(2)),
                    associativity: Some(adze_ir::Associativity::Left),
                    fields: vec![],
                    production_id: ProductionId(3),
                },
                Rule {
                    lhs: term,
                    rhs: vec![Symbol::NonTerminal(factor)],
                    precedence: None,
                    associativity: None,
                    fields: vec![],
                    production_id: ProductionId(4),
                },
            ],
        );
        g.rule_names.insert(term, "term".into());

        g.rules.insert(
            factor,
            vec![
                Rule {
                    lhs: factor,
                    rhs: vec![Symbol::Terminal(num)],
                    precedence: None,
                    associativity: None,
                    fields: vec![],
                    production_id: ProductionId(5),
                },
                Rule {
                    lhs: factor,
                    rhs: vec![
                        Symbol::Terminal(lparen),
                        Symbol::NonTerminal(expr),
                        Symbol::Terminal(rparen),
                    ],
                    precedence: None,
                    associativity: None,
                    fields: vec![],
                    production_id: ProductionId(6),
                },
            ],
        );
        g.rule_names.insert(factor, "factor".into());

        g.rules.insert(
            source,
            vec![Rule {
                lhs: source,
                rhs: vec![Symbol::NonTerminal(expr)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(7),
            }],
        );
        g.rule_names.insert(source, "source_file".into());

        g.set_start_symbol(source);
        g
    }

    fn build_table(g: &Grammar) -> adze_glr_core::ParseTable {
        let ff = FirstFollowSets::compute(g).unwrap();
        build_lr1_automaton(g, &ff).expect("build parse table")
    }

    fn tokenize(g: &Grammar, input: &str) -> Vec<TokenWithPosition> {
        GLRLexer::new(g, input.to_string())
            .expect("lexer")
            .tokenize_all()
    }

    fn to_glr(tokens: &[TokenWithPosition]) -> Vec<GLRToken> {
        tokens
            .iter()
            .map(|t| GLRToken {
                symbol: t.symbol_id,
                text: t.text.as_bytes().to_vec(),
                start_byte: t.byte_offset,
                end_byte: t.byte_offset + t.byte_length,
            })
            .collect()
    }

    fn make_parser(g: &Grammar) -> IncrementalGLRParser {
        IncrementalGLRParser::new(g.clone(), build_table(g))
    }

    fn fresh_parse(
        parser: &mut IncrementalGLRParser,
        g: &Grammar,
        input: &str,
    ) -> (Vec<GLRToken>, Arc<ForestNode>) {
        let toks = to_glr(&tokenize(g, input));
        let forest = parser.parse_incremental(&toks, &[]).unwrap();
        (toks, forest)
    }

    fn make_token(sym: u16, text: &[u8], start: usize) -> GLRToken {
        GLRToken {
            symbol: SymbolId(sym),
            text: text.to_vec(),
            start_byte: start,
            end_byte: start + text.len(),
        }
    }

    fn leaf_node(start: usize, end: usize) -> ForestNode {
        ForestNode {
            symbol: SymbolId(1),
            alternatives: vec![],
            byte_range: start..end,
            token_range: 0..1,
            cached_subtree: None,
        }
    }

    // ─── Test 1: Edit struct constructors ────────────────────────────

    #[test]
    fn test_edit_constructors() {
        let e1 = Edit::new(10, 20, 25);
        assert_eq!(
            (e1.start_byte, e1.old_end_byte, e1.new_end_byte),
            (10, 20, 25)
        );

        let e2 = Edit::bytes(5, 15, 12);
        assert_eq!(
            (e2.start_byte, e2.old_end_byte, e2.new_end_byte),
            (5, 15, 12)
        );

        let e3 = Edit::new(0, 0, 0);
        assert_eq!((e3.start_byte, e3.old_end_byte, e3.new_end_byte), (0, 0, 0));
    }

    // ─── Test 2: Reuse counter reset and atomic increment ───────────

    #[test]
    fn test_reuse_counter() {
        reset_reuse_counter();
        assert_eq!(get_reuse_count(), 0);

        adze::glr_incremental::SUBTREE_REUSE_COUNT
            .fetch_add(42, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(get_reuse_count(), 42);

        reset_reuse_counter();
        assert_eq!(get_reuse_count(), 0);
    }

    // ─── Test 3: ForestNode overlap detection ───────────────────────

    #[test]
    fn test_forest_node_overlap() {
        let n = leaf_node(10, 20);

        // Overlapping cases
        assert!(n.overlaps_edit(&(5..15))); // partial left
        assert!(n.overlaps_edit(&(15..25))); // partial right
        assert!(n.overlaps_edit(&(12..18))); // contained
        assert!(n.overlaps_edit(&(5..25))); // surrounding

        // Non-overlapping cases
        assert!(!n.overlaps_edit(&(0..10))); // before (adjacent)
        assert!(!n.overlaps_edit(&(20..30))); // after (adjacent)

        // overlaps() synonym
        assert!(n.overlaps(&(15..25)));
        assert!(!n.overlaps(&(20..30)));
    }

    // ─── Test 4: find_reusable_subtrees with no children ────────────

    #[test]
    fn test_find_reusable_no_children() {
        let n = leaf_node(0, 10);
        assert!(n.find_reusable_subtrees(&(20..30)).is_empty());
    }

    // ─── Test 5: find_reusable_subtrees with children ───────────────

    #[test]
    fn test_find_reusable_with_children() {
        let child1 = Arc::new(leaf_node(0, 5));
        let child2 = Arc::new(leaf_node(5, 10));
        let child3 = Arc::new(leaf_node(10, 15));

        let subtree = Arc::new(Subtree::new(
            SubtreeNode {
                symbol_id: SymbolId(1),
                is_error: false,
                byte_range: 0..15,
            },
            vec![],
        ));

        let parent = ForestNode {
            symbol: SymbolId(1),
            alternatives: vec![ForkAlternative {
                fork_id: 0,
                rule_id: None,
                children: vec![child1, child2, child3],
                subtree,
            }],
            byte_range: 0..15,
            token_range: 0..3,
            cached_subtree: None,
        };

        // Edit overlaps parent → no children returned
        assert!(parent.find_reusable_subtrees(&(6..8)).is_empty());

        // Edit far away → parent doesn't overlap, all children returned
        assert_eq!(parent.find_reusable_subtrees(&(100..200)).len(), 3);
    }

    // ─── Test 6: ChunkIdentifier prefix boundary ────────────────────

    #[test]
    fn test_chunk_prefix_boundary() {
        // All tokens match, edit far away
        let edit = GLREdit {
            old_range: 100..101,
            new_text: b"x".to_vec(),
            old_token_range: 0..0,
            new_tokens: vec![],
            old_tokens: vec![],
            old_forest: None,
        };
        let ci = ChunkIdentifier::new(None, &edit);
        let old = vec![make_token(1, b"a", 0), make_token(2, b"b", 1)];
        let new = vec![make_token(1, b"a", 0), make_token(2, b"b", 1)];
        assert_eq!(ci.find_prefix_boundary(&old, &new), 2);

        // Edit at byte 1 stops prefix at token 1
        let edit2 = GLREdit {
            old_range: 1..2,
            new_text: b"x".to_vec(),
            old_token_range: 0..0,
            new_tokens: vec![],
            old_tokens: vec![],
            old_forest: None,
        };
        let ci2 = ChunkIdentifier::new(None, &edit2);
        let old2 = vec![make_token(1, b"a", 0), make_token(2, b"b", 1)];
        let new2 = vec![make_token(1, b"a", 0), make_token(2, b"x", 1)];
        assert_eq!(ci2.find_prefix_boundary(&old2, &new2), 1);

        // Symbol mismatch stops prefix
        let edit3 = GLREdit {
            old_range: 100..101,
            new_text: vec![],
            old_token_range: 0..0,
            new_tokens: vec![],
            old_tokens: vec![],
            old_forest: None,
        };
        let ci3 = ChunkIdentifier::new(None, &edit3);
        let old3 = vec![make_token(1, b"a", 0), make_token(2, b"b", 1)];
        let new3 = vec![make_token(1, b"a", 0), make_token(99, b"b", 1)];
        assert_eq!(ci3.find_prefix_boundary(&old3, &new3), 1);
    }

    // ─── Test 7: ChunkIdentifier suffix boundary ────────────────────

    #[test]
    fn test_chunk_suffix_boundary() {
        let edit = GLREdit {
            old_range: 2..3,
            new_text: b"x".to_vec(),
            old_token_range: 0..0,
            new_tokens: vec![],
            old_tokens: vec![],
            old_forest: None,
        };
        let ci = ChunkIdentifier::new(None, &edit);

        let old = vec![
            make_token(1, b"a", 0),
            make_token(2, b"+", 2),
            make_token(1, b"c", 4),
        ];
        let new = vec![
            make_token(1, b"a", 0),
            make_token(2, b"x", 2),
            make_token(1, b"c", 4),
        ];
        assert_eq!(ci.find_suffix_boundary(&old, &new, 0), 1);
    }

    // ─── Test 8: ChunkIdentifier suffix empty when all changed ──────

    #[test]
    fn test_chunk_suffix_empty_when_all_changed() {
        let edit = GLREdit {
            old_range: 0..10,
            new_text: b"xxxxxxxxxx".to_vec(),
            old_token_range: 0..0,
            new_tokens: vec![],
            old_tokens: vec![],
            old_forest: None,
        };
        let ci = ChunkIdentifier::new(None, &edit);
        let old = vec![make_token(1, b"a", 0), make_token(2, b"b", 5)];
        let new = vec![make_token(1, b"x", 0), make_token(2, b"y", 5)];
        assert_eq!(ci.find_suffix_boundary(&old, &new, 0), 0);
    }

    // ─── Test 9: Fresh parse single number ──────────────────────────

    #[test]
    fn test_fresh_parse_single_number() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let (_, forest) = fresh_parse(&mut p, &g, "42");
        assert!(!forest.alternatives.is_empty());
    }

    // ─── Test 10: Fresh parse addition ──────────────────────────────

    #[test]
    fn test_fresh_parse_addition() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let (_, forest) = fresh_parse(&mut p, &g, "1+2");
        assert!(!forest.alternatives.is_empty());
    }

    // ─── Test 11: Fresh parse nested parentheses ────────────────────

    #[test]
    fn test_fresh_parse_nested_parens() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let (_, forest) = fresh_parse(&mut p, &g, "((1+2)*3)");
        assert!(!forest.alternatives.is_empty());
    }

    // ─── Test 12: Fresh parse complex expression with range check ───

    #[test]
    fn test_fresh_parse_complex_expression() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let input = "1+2*3-4+5";
        let (_, forest) = fresh_parse(&mut p, &g, input);
        assert!(!forest.alternatives.is_empty());
        assert_eq!(forest.byte_range.start, 0);
        assert_eq!(forest.byte_range.end, input.len());
    }

    // ─── Test 13: Incremental no-op edit ────────────────────────────

    #[test]
    fn test_incremental_noop_edit() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let (old_toks, old_forest) = fresh_parse(&mut p, &g, "1+2");

        let edit = GLREdit {
            old_range: 1..1,
            new_text: vec![],
            old_token_range: 0..0,
            new_tokens: vec![],
            old_tokens: old_toks.clone(),
            old_forest: Some(old_forest),
        };

        reset_reuse_counter();
        let new_forest = p.parse_incremental(&old_toks, &[edit]).unwrap();
        assert!(!new_forest.alternatives.is_empty());
        let status = p.last_parse_status();
        assert_eq!(status.invalidated_ranges, vec![1..1]);
        assert!(!status.full_reparse_fallback);
        assert!(status.reused_node_count > 0);
    }

    // ─── Test 14: Replace first token ───────────────────────────────

    #[test]
    fn test_replace_first_token() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let (old_toks, old_forest) = fresh_parse(&mut p, &g, "1+2*3");
        let new_toks = to_glr(&tokenize(&g, "9+2*3"));

        let edit = GLREdit {
            old_range: 0..1,
            new_text: b"9".to_vec(),
            old_token_range: 0..1,
            new_tokens: vec![new_toks[0].clone()],
            old_tokens: old_toks,
            old_forest: Some(old_forest),
        };

        let f = p.parse_incremental(&new_toks, &[edit]).unwrap();
        assert!(!f.alternatives.is_empty());
    }

    // ─── Test 15: Replace last token ────────────────────────────────

    #[test]
    fn test_replace_last_token() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let (old_toks, old_forest) = fresh_parse(&mut p, &g, "1+2*3");
        let new_toks = to_glr(&tokenize(&g, "1+2*7"));

        let edit = GLREdit {
            old_range: 4..5,
            new_text: b"7".to_vec(),
            old_token_range: 4..5,
            new_tokens: vec![new_toks[4].clone()],
            old_tokens: old_toks,
            old_forest: Some(old_forest),
        };

        let f = p.parse_incremental(&new_toks, &[edit]).unwrap();
        assert!(!f.alternatives.is_empty());
    }

    // ─── Test 16: Replace middle operator ───────────────────────────

    #[test]
    fn test_replace_middle_operator() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let (old_toks, old_forest) = fresh_parse(&mut p, &g, "1+2+3");
        let new_toks = to_glr(&tokenize(&g, "1+2*3"));

        let edit = GLREdit {
            old_range: 3..4,
            new_text: b"*".to_vec(),
            old_token_range: 3..4,
            new_tokens: vec![new_toks[3].clone()],
            old_tokens: old_toks,
            old_forest: Some(old_forest),
        };

        let f = p.parse_incremental(&new_toks, &[edit]).unwrap();
        assert!(!f.alternatives.is_empty());
    }

    // ─── Test 17: Insert tokens ─────────────────────────────────────

    #[test]
    fn test_insert_tokens() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let (old_toks, old_forest) = fresh_parse(&mut p, &g, "1+3");
        let new_toks = to_glr(&tokenize(&g, "1+2+3"));

        let edit = GLREdit {
            old_range: 2..2,
            new_text: b"2+".to_vec(),
            old_token_range: 2..2,
            new_tokens: vec![new_toks[2].clone(), new_toks[3].clone()],
            old_tokens: old_toks,
            old_forest: Some(old_forest),
        };

        let f = p.parse_incremental(&new_toks, &[edit]).unwrap();
        assert!(!f.alternatives.is_empty());
    }

    // ─── Test 18: Delete tokens ─────────────────────────────────────

    #[test]
    fn test_delete_tokens() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let (old_toks, old_forest) = fresh_parse(&mut p, &g, "1+2+3");
        let new_toks = to_glr(&tokenize(&g, "1+3"));

        let edit = GLREdit {
            old_range: 2..4,
            new_text: vec![],
            old_token_range: 2..4,
            new_tokens: vec![],
            old_tokens: old_toks,
            old_forest: Some(old_forest),
        };

        let f = p.parse_incremental(&new_toks, &[edit]).unwrap();
        assert!(!f.alternatives.is_empty());
    }

    // ─── Test 19: Expand token to sub-expression ────────────────────

    #[test]
    fn test_expand_token_to_subexpression() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let (old_toks, old_forest) = fresh_parse(&mut p, &g, "1+2");
        let new_toks = to_glr(&tokenize(&g, "1+(3*4)"));

        let edit = GLREdit {
            old_range: 2..3,
            new_text: b"(3*4)".to_vec(),
            old_token_range: 2..3,
            new_tokens: new_toks[2..7].to_vec(),
            old_tokens: old_toks,
            old_forest: Some(old_forest),
        };

        let f = p.parse_incremental(&new_toks, &[edit]).unwrap();
        assert!(!f.alternatives.is_empty());
    }

    // ─── Test 20: new_with_forest constructors ──────────────────────

    #[test]
    fn test_new_with_forest() {
        let g = arith_grammar();
        let table = build_table(&g);

        // With None
        let mut p1 = IncrementalGLRParser::new_with_forest(g.clone(), table.clone(), None);
        let (_, f1) = fresh_parse(&mut p1, &g, "1+2");
        assert!(!f1.alternatives.is_empty());

        // With previous forest
        let mut p2 = IncrementalGLRParser::new_with_forest(g.clone(), table, Some(f1));
        let new_toks = to_glr(&tokenize(&g, "1+3"));
        let f2 = p2.parse_incremental(&new_toks, &[]).unwrap();
        assert!(!f2.alternatives.is_empty());
    }

    // ─── Test 21: Sequential incremental edits ──────────────────────

    #[test]
    fn test_sequential_incremental_edits() {
        let g = arith_grammar();
        let mut p = make_parser(&g);

        let (toks1, forest1) = fresh_parse(&mut p, &g, "1+2");

        let toks2 = to_glr(&tokenize(&g, "1+3"));
        let edit2 = GLREdit {
            old_range: 2..3,
            new_text: b"3".to_vec(),
            old_token_range: 2..3,
            new_tokens: vec![toks2[2].clone()],
            old_tokens: toks1,
            old_forest: Some(forest1),
        };
        let forest2 = p.parse_incremental(&toks2, &[edit2]).unwrap();

        let toks3 = to_glr(&tokenize(&g, "1+3+4"));
        let edit3 = GLREdit {
            old_range: 3..3,
            new_text: b"+4".to_vec(),
            old_token_range: 3..3,
            new_tokens: vec![toks3[3].clone(), toks3[4].clone()],
            old_tokens: toks2,
            old_forest: Some(forest2),
        };
        let forest3 = p.parse_incremental(&toks3, &[edit3]).unwrap();
        assert!(!forest3.alternatives.is_empty());
    }

    // ─── Test 22: Reuse counter on incremental parse ────────────────

    #[test]
    fn test_reuse_counter_on_incremental() {
        let g = arith_grammar();
        let mut p = make_parser(&g);

        let mut expr = String::new();
        for i in 0..20 {
            if i > 0 {
                expr.push('+');
            }
            expr.push_str(&i.to_string());
        }
        let (old_toks, old_forest) = fresh_parse(&mut p, &g, &expr);

        let last = old_toks.len() - 1;
        let start = old_toks[last].start_byte;
        let end = old_toks[last].end_byte;
        let mut new_expr = expr.clone();
        new_expr.replace_range(start..end, "99");

        let new_toks = to_glr(&tokenize(&g, &new_expr));
        let edit = GLREdit {
            old_range: start..end,
            new_text: b"99".to_vec(),
            old_token_range: last..last + 1,
            new_tokens: vec![new_toks[new_toks.len() - 1].clone()],
            old_tokens: old_toks,
            old_forest: Some(old_forest),
        };

        reset_reuse_counter();
        let f = p.parse_incremental(&new_toks, &[edit]).unwrap();
        assert!(!f.alternatives.is_empty());
        assert!(
            get_reuse_count() > 0,
            "compatible incremental edit should reuse at least one subtree"
        );
        let status = p.last_parse_status();
        assert!(!status.full_reparse_fallback);
        assert!(status.reused_node_count > 0);
        assert!(status.reused_node_count <= new_toks.len());
        assert_eq!(status.invalidated_ranges, vec![start..end]);
    }

    #[test]
    fn test_status_reports_explicit_fallback_when_old_forest_missing() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let new_toks = to_glr(&tokenize(&g, "1+2"));

        let edit = GLREdit {
            old_range: 0..0,
            new_text: b"1+2".to_vec(),
            old_token_range: 0..0,
            new_tokens: new_toks.clone(),
            old_tokens: vec![],
            old_forest: None,
        };

        let forest = p.parse_incremental(&new_toks, &[edit]).unwrap();
        assert!(!forest.alternatives.is_empty());

        let status = p.last_parse_status();
        assert!(status.full_reparse_fallback);
        assert_eq!(status.reused_node_count, 0);
        assert_eq!(status.invalidated_ranges, vec![0..0]);
        assert_eq!(status.fallback_reason, Some("missing_old_forest"));
    }

    // ─── Test 23: Parse error on invalid input ──────────────────────

    #[test]
    fn test_fresh_parse_error_on_invalid_input() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let toks = to_glr(&tokenize(&g, "+"));
        // Should not panic, may error or produce error recovery
        let _ = p.parse_incremental(&toks, &[]);
    }

    // ─── Test 24: Forest byte_range covers input ────────────────────

    #[test]
    fn test_forest_ranges_cover_input() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let input = "1+2*3";
        let (_, forest) = fresh_parse(&mut p, &g, input);
        assert_eq!(forest.byte_range.start, 0);
        assert_eq!(forest.byte_range.end, input.len());
    }

    // ─── Test 25: GLRToken and GLREdit field access ─────────────────

    #[test]
    fn test_glr_token_and_edit_fields() {
        let t = GLRToken {
            symbol: SymbolId(42),
            text: b"hello".to_vec(),
            start_byte: 10,
            end_byte: 15,
        };
        assert_eq!(t.symbol, SymbolId(42));
        assert_eq!(t.end_byte - t.start_byte, 5);

        let t2 = t.clone();
        assert_eq!(t.text, t2.text);

        let e = GLREdit {
            old_range: 5..10,
            new_text: b"abc".to_vec(),
            old_token_range: 1..3,
            new_tokens: vec![],
            old_tokens: vec![],
            old_forest: None,
        };
        assert_eq!(e.old_range, 5..10);
        assert!(e.old_forest.is_none());
    }

    // ─── Test 26: Two edits simultaneously ──────────────────────────

    #[test]
    fn test_two_edits_simultaneously() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let (old_toks, old_forest) = fresh_parse(&mut p, &g, "1+2+3+4");
        let new_toks = to_glr(&tokenize(&g, "5+2+3+8"));

        let edit1 = GLREdit {
            old_range: 0..1,
            new_text: b"5".to_vec(),
            old_token_range: 0..1,
            new_tokens: vec![new_toks[0].clone()],
            old_tokens: old_toks.clone(),
            old_forest: Some(old_forest.clone()),
        };
        let edit2 = GLREdit {
            old_range: 6..7,
            new_text: b"8".to_vec(),
            old_token_range: 6..7,
            new_tokens: vec![new_toks[6].clone()],
            old_tokens: old_toks,
            old_forest: Some(old_forest),
        };

        let f = p.parse_incremental(&new_toks, &[edit1, edit2]).unwrap();
        assert!(!f.alternatives.is_empty());
    }

    // ─── Test 27: Incremental without old forest falls back ─────────

    #[test]
    fn test_incremental_without_old_forest() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let new_toks = to_glr(&tokenize(&g, "1+2"));

        let edit = GLREdit {
            old_range: 0..1,
            new_text: b"1".to_vec(),
            old_token_range: 0..1,
            new_tokens: vec![new_toks[0].clone()],
            old_tokens: vec![],
            old_forest: None,
        };

        let f = p.parse_incremental(&new_toks, &[edit]).unwrap();
        assert!(!f.alternatives.is_empty());
    }

    // ─── Test 28: Empty token stream ────────────────────────────────

    #[test]
    fn test_fresh_parse_empty_tokens() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let _ = p.parse_incremental(&[], &[]);
    }

    // ─── Test 29: Large expression parses ───────────────────────────

    #[test]
    fn test_large_expression_parses() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let mut expr = String::new();
        for i in 0..100 {
            if i > 0 {
                expr.push('+');
            }
            expr.push_str(&i.to_string());
        }
        let (_, forest) = fresh_parse(&mut p, &g, &expr);
        assert!(!forest.alternatives.is_empty());
        assert_eq!(forest.byte_range.end, expr.len());
    }

    // ─── Test 30: Idempotent fresh parse ────────────────────────────

    #[test]
    fn test_fresh_parse_idempotent() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let (_, f1) = fresh_parse(&mut p, &g, "1*2+3");
        let (_, f2) = fresh_parse(&mut p, &g, "1*2+3");
        assert_eq!(f1.alternatives.len(), f2.alternatives.len());
        assert_eq!(f1.byte_range, f2.byte_range);
    }

    // ─── Test 31: Incremental inside parenthesized expression ───────

    #[test]
    fn test_incremental_inside_parens() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let (old_toks, old_forest) = fresh_parse(&mut p, &g, "(1+2)*3");
        let new_toks = to_glr(&tokenize(&g, "(1+9)*3"));

        let edit = GLREdit {
            old_range: 3..4,
            new_text: b"9".to_vec(),
            old_token_range: 3..4,
            new_tokens: vec![new_toks[3].clone()],
            old_tokens: old_toks,
            old_forest: Some(old_forest),
        };

        let f = p.parse_incremental(&new_toks, &[edit]).unwrap();
        assert!(!f.alternatives.is_empty());
    }

    // ─── Test 32: Many sequential reparsings ────────────────────────

    #[test]
    fn test_many_sequential_reparsings() {
        let g = arith_grammar();
        let mut p = make_parser(&g);
        let mut current = "1+2".to_string();
        let (mut cur_toks, mut cur_forest) = fresh_parse(&mut p, &g, &current);

        for i in 3..15 {
            let new = format!("{}+{}", current, i);
            let new_toks = to_glr(&tokenize(&g, &new));
            let append = format!("+{}", i);
            let edit = GLREdit {
                old_range: current.len()..current.len(),
                new_text: append.as_bytes().to_vec(),
                old_token_range: cur_toks.len()..cur_toks.len(),
                new_tokens: new_toks[cur_toks.len()..].to_vec(),
                old_tokens: cur_toks,
                old_forest: Some(cur_forest),
            };
            cur_forest = p.parse_incremental(&new_toks, &[edit]).unwrap();
            cur_toks = new_toks;
            current = new;
        }
        assert!(!cur_forest.alternatives.is_empty());
    }

    // ─── Test 33: ForkAlternative and ForestNode metadata ───────────

    #[test]
    fn test_fork_alternative_and_forest_metadata() {
        let subtree = Arc::new(Subtree::new(
            SubtreeNode {
                symbol_id: SymbolId(1),
                is_error: false,
                byte_range: 0..1,
            },
            vec![],
        ));
        let alt = ForkAlternative {
            fork_id: 7,
            rule_id: Some(adze_ir::RuleId(3)),
            children: vec![],
            subtree: subtree.clone(),
        };
        assert_eq!(alt.fork_id, 7);
        assert_eq!(alt.rule_id, Some(adze_ir::RuleId(3)));
        assert!(alt.children.is_empty());

        // ForestNode with cached_subtree
        let n = ForestNode {
            symbol: SymbolId(1),
            alternatives: vec![],
            byte_range: 0..5,
            token_range: 0..1,
            cached_subtree: Some(subtree.clone()),
        };
        assert!(n.cached_subtree.is_some());
        assert_eq!(n.cached_subtree.unwrap().node.symbol_id, SymbolId(1));

        // Clone and Debug
        let n2 = leaf_node(5, 15).clone();
        assert_eq!(n2.byte_range, 5..15);
        let dbg = format!("{:?}", n2);
        assert!(dbg.contains("ForestNode"));
    }
}
