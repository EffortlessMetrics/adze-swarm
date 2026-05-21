#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Comprehensive tests for symbol name handling and generation in the tablegen crate.
//!
//! Tests cover three code paths for symbol names:
//! - `LanguageGenerator::generate()` → TokenStream with SYMBOL_NAMES array
//! - `AbiLanguageBuilder::generate()` → TokenStream with SYMBOL_NAME_PTRS array
//! - `LanguageBuilder::generate_language()` → TSLanguage with symbol_names pointer
//!
//! All tests exercise the public API only.

use adze_glr_core::{Action, GotoIndexing, LexMode, ParseTable};
use adze_ir::{ExternalToken, Grammar, ProductionId, Rule, StateId, SymbolId, Token, TokenPattern};
use adze_tablegen::language_gen::LanguageGenerator;
use adze_tablegen::{AbiLanguageBuilder, LanguageBuilder};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn regex_token(name: &str, pattern: &str) -> Token {
    Token {
        name: name.to_string(),
        pattern: TokenPattern::Regex(pattern.to_string()),
        fragile: false,
    }
}

fn string_token(name: &str, literal: &str) -> Token {
    Token {
        name: name.to_string(),
        pattern: TokenPattern::String(literal.to_string()),
        fragile: false,
    }
}

fn make_grammar(
    name: &str,
    tokens: Vec<(SymbolId, Token)>,
    rules: Vec<Rule>,
    rule_names: Vec<(SymbolId, String)>,
) -> Grammar {
    let mut g = Grammar::new(name.to_string());
    for (id, tok) in tokens {
        g.tokens.insert(id, tok);
    }
    for rule in rules {
        g.add_rule(rule);
    }
    for (id, rn) in rule_names {
        g.rule_names.insert(id, rn);
    }
    g
}

/// ParseTable for LanguageGenerator (index 0 = EOF).
fn make_lang_gen_table(grammar: &Grammar, states: usize, symbol_count: usize) -> ParseTable {
    let mut symbol_to_index = BTreeMap::new();
    for i in 0..symbol_count {
        symbol_to_index.insert(SymbolId(i as u16), i);
    }
    let index_to_symbol: Vec<SymbolId> = (0..symbol_count).map(|i| SymbolId(i as u16)).collect();
    ParseTable {
        action_table: vec![vec![vec![]; symbol_count]; states],
        goto_table: vec![vec![StateId(u16::MAX); symbol_count]; states],
        rules: vec![],
        state_count: states,
        symbol_count,
        symbol_to_index,
        index_to_symbol,
        nonterminal_to_index: BTreeMap::new(),
        symbol_metadata: vec![],
        token_count: symbol_count.saturating_sub(1),
        external_token_count: grammar.externals.len(),
        eof_symbol: SymbolId(0),
        start_symbol: SymbolId(0),
        initial_state: StateId(0),
        lex_modes: vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0
            };
            states
        ],
        extras: vec![],
        external_scanner_states: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
        grammar: grammar.clone(),
        goto_indexing: GotoIndexing::NonterminalMap,
    }
}

/// ParseTable for AbiLanguageBuilder with custom symbol_to_index.
fn make_abi_table(
    grammar: &Grammar,
    symbol_to_index: BTreeMap<SymbolId, usize>,
    eof: SymbolId,
) -> ParseTable {
    let symbol_count = symbol_to_index.len();
    let mut index_to_symbol = vec![SymbolId(0); symbol_count];
    for (&sid, &idx) in &symbol_to_index {
        if idx < symbol_count {
            index_to_symbol[idx] = sid;
        }
    }
    ParseTable {
        action_table: vec![vec![vec![]; symbol_count]; 1],
        goto_table: vec![vec![StateId(u16::MAX); symbol_count]; 1],
        rules: vec![],
        state_count: 1,
        symbol_count,
        symbol_to_index,
        index_to_symbol,
        nonterminal_to_index: BTreeMap::new(),
        symbol_metadata: vec![],
        token_count: symbol_count.saturating_sub(1),
        external_token_count: grammar.externals.len(),
        eof_symbol: eof,
        start_symbol: SymbolId(0),
        initial_state: StateId(0),
        lex_modes: vec![LexMode {
            lex_state: 0,
            external_lex_state: 0,
        }],
        extras: vec![],
        external_scanner_states: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
        grammar: grammar.clone(),
        goto_indexing: GotoIndexing::NonterminalMap,
    }
}

/// ParseTable for LanguageBuilder tests.
fn make_lb_table(states: usize, symbol_count: usize) -> ParseTable {
    let mut symbol_to_index = BTreeMap::new();
    for i in 0..symbol_count {
        symbol_to_index.insert(SymbolId(i as u16), i);
    }
    let index_to_symbol: Vec<SymbolId> = (0..symbol_count).map(|i| SymbolId(i as u16)).collect();
    ParseTable {
        action_table: vec![vec![vec![Action::Error]; symbol_count]; states],
        goto_table: vec![vec![StateId(u16::MAX); symbol_count]; states],
        rules: vec![],
        state_count: states,
        symbol_count,
        symbol_to_index,
        index_to_symbol,
        nonterminal_to_index: BTreeMap::new(),
        symbol_metadata: vec![],
        token_count: symbol_count.saturating_sub(1),
        external_token_count: 0,
        eof_symbol: SymbolId(1),
        start_symbol: SymbolId(1),
        initial_state: StateId(0),
        lex_modes: vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0
            };
            states
        ],
        extras: vec![],
        external_scanner_states: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
        grammar: Grammar::default(),
        goto_indexing: GotoIndexing::NonterminalMap,
    }
}

fn make_lb_table_for_grammar(grammar: &Grammar, states: usize) -> ParseTable {
    let mut symbols = vec![SymbolId(0)];
    for symbol_id in grammar.tokens.keys() {
        push_symbol(&mut symbols, *symbol_id);
    }
    for symbol_id in grammar.rule_names.keys() {
        push_symbol(&mut symbols, *symbol_id);
    }
    for rule in grammar.all_rules() {
        push_symbol(&mut symbols, rule.lhs);
    }
    for external in &grammar.externals {
        push_symbol(&mut symbols, external.symbol_id);
    }

    let symbol_count = symbols.len();
    let symbol_to_index = symbols
        .iter()
        .copied()
        .enumerate()
        .map(|(index, symbol_id)| (symbol_id, index))
        .collect();

    ParseTable {
        action_table: vec![vec![vec![Action::Error]; symbol_count]; states],
        goto_table: vec![vec![StateId(u16::MAX); symbol_count]; states],
        rules: vec![],
        state_count: states,
        symbol_count,
        symbol_to_index,
        index_to_symbol: symbols,
        nonterminal_to_index: BTreeMap::new(),
        symbol_metadata: vec![],
        token_count: grammar.tokens.len() + 1,
        external_token_count: grammar.externals.len(),
        eof_symbol: SymbolId(0),
        start_symbol: grammar.start_symbol().unwrap_or(SymbolId(0)),
        initial_state: StateId(0),
        lex_modes: vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0
            };
            states
        ],
        extras: vec![],
        external_scanner_states: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
        grammar: Grammar::default(),
        goto_indexing: GotoIndexing::NonterminalMap,
    }
}

fn push_symbol(symbols: &mut Vec<SymbolId>, symbol_id: SymbolId) {
    if !symbols.contains(&symbol_id) {
        symbols.push(symbol_id);
    }
}

/// Shorthand: AbiLanguageBuilder generate output as String.
fn abi_output(grammar: &Grammar, s2i: BTreeMap<SymbolId, usize>, eof: SymbolId) -> String {
    let pt = make_abi_table(grammar, s2i, eof);
    AbiLanguageBuilder::new(grammar, &pt).generate().to_string()
}

fn abi_output_has_symbol_name(output: &str, name: &str) -> bool {
    let byte_strings = format!("{name}\0")
        .into_bytes()
        .into_iter()
        .map(|byte| format!("{byte}u8"))
        .collect::<Vec<_>>();

    if let Some(start) = output.find(&byte_strings[0]) {
        byte_strings
            .iter()
            .all(|byte_string| output[start..].contains(byte_string))
    } else {
        false
    }
}

/// Read N leaked C-string pointers from a raw array.
unsafe fn read_symbol_names(ptr: *const *const i8, count: usize) -> Vec<String> {
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let name_ptr = unsafe { *ptr.add(i) };
        let cstr = unsafe { std::ffi::CStr::from_ptr(name_ptr) };
        out.push(cstr.to_str().unwrap().to_string());
    }
    out
}

/// Read symbol names via LanguageBuilder::generate_language().
fn lb_symbol_names(grammar: Grammar, pt: ParseTable) -> Vec<String> {
    let builder = LanguageBuilder::new(grammar, pt);
    let lang = builder
        .generate_language()
        .expect("generation should succeed");
    assert!(!lang.symbol_names.is_null());
    unsafe { read_symbol_names(lang.symbol_names, lang.symbol_count as usize) }
}

// ===========================================================================
// 1. LanguageGenerator — SYMBOL_NAMES in generated TokenStream
// ===========================================================================

#[test]
fn lang_gen_eof_sentinel_is_end() {
    let grammar = make_grammar(
        "g",
        vec![(SymbolId(1), regex_token("x", "."))],
        vec![],
        vec![],
    );
    let pt = make_lang_gen_table(&grammar, 1, 2);
    let output = LanguageGenerator::new(&grammar, &pt).generate().to_string();
    assert!(
        output.contains("\"end\""),
        "first symbol name must be \"end\""
    );
}

#[test]
fn lang_gen_single_token_name() {
    let grammar = make_grammar(
        "g",
        vec![(SymbolId(1), regex_token("identifier", "[a-z]+"))],
        vec![],
        vec![],
    );
    let pt = make_lang_gen_table(&grammar, 1, 2);
    let output = LanguageGenerator::new(&grammar, &pt).generate().to_string();
    assert!(output.contains("identifier"));
}

#[test]
fn lang_gen_multiple_tokens_all_present() {
    let grammar = make_grammar(
        "g",
        vec![
            (SymbolId(1), string_token("plus", "+")),
            (SymbolId(2), string_token("minus", "-")),
            (SymbolId(3), regex_token("number", r"\d+")),
        ],
        vec![],
        vec![],
    );
    let pt = make_lang_gen_table(&grammar, 1, 4);
    let output = LanguageGenerator::new(&grammar, &pt).generate().to_string();
    assert!(output.contains("plus"));
    assert!(output.contains("minus"));
    assert!(output.contains("number"));
}

#[test]
fn lang_gen_rule_name_from_map() {
    let grammar = make_grammar(
        "g",
        vec![(SymbolId(1), regex_token("tok", "."))],
        vec![],
        vec![(SymbolId(2), "expression".to_string())],
    );
    let pt = make_lang_gen_table(&grammar, 1, 3);
    let output = LanguageGenerator::new(&grammar, &pt).generate().to_string();
    assert!(output.contains("expression"));
}

#[test]
fn lang_gen_fallback_to_rule_id() {
    let grammar = make_grammar("g", vec![], vec![], vec![]);
    let pt = make_lang_gen_table(&grammar, 1, 2);
    let output = LanguageGenerator::new(&grammar, &pt).generate().to_string();
    assert!(output.contains("rule_1"));
}

#[test]
fn lang_gen_external_token_name() {
    let mut grammar = make_grammar("g", vec![], vec![], vec![]);
    grammar.externals.push(ExternalToken {
        name: "indent".to_string(),
        symbol_id: SymbolId(1),
    });
    let pt = make_lang_gen_table(&grammar, 1, 2);
    let output = LanguageGenerator::new(&grammar, &pt).generate().to_string();
    assert!(output.contains("indent"));
}

#[test]
fn lang_gen_symbol_names_count_matches() {
    let grammar = make_grammar(
        "g",
        vec![
            (SymbolId(1), string_token("a", "a")),
            (SymbolId(2), string_token("b", "b")),
        ],
        vec![],
        vec![],
    );
    let pt = make_lang_gen_table(&grammar, 1, 3);
    let output = LanguageGenerator::new(&grammar, &pt).generate().to_string();
    // All three names should appear
    assert!(output.contains("\"end\""));
    assert!(output.contains("\"a\""));
    assert!(output.contains("\"b\""));
}

#[test]
fn lang_gen_token_over_rule_name() {
    let grammar = make_grammar(
        "g",
        vec![(SymbolId(1), regex_token("tok_name", "."))],
        vec![],
        vec![(SymbolId(1), "rule_name".to_string())],
    );
    let pt = make_lang_gen_table(&grammar, 1, 2);
    let output = LanguageGenerator::new(&grammar, &pt).generate().to_string();
    assert!(output.contains("tok_name"));
}

#[test]
fn lang_gen_has_symbol_names_array() {
    let grammar = make_grammar("g", vec![], vec![], vec![]);
    let pt = make_lang_gen_table(&grammar, 1, 1);
    let output = LanguageGenerator::new(&grammar, &pt).generate().to_string();
    assert!(output.contains("SYMBOL_NAMES"));
}

#[test]
fn lang_gen_has_symbol_names_ptrs_and_struct_ref() {
    let grammar = make_grammar("g", vec![], vec![], vec![]);
    let pt = make_lang_gen_table(&grammar, 1, 1);
    let output = LanguageGenerator::new(&grammar, &pt).generate().to_string();
    assert!(output.contains("SYMBOL_NAMES_PTRS"));
    assert!(output.contains("symbol_names"));
}

#[test]
fn lang_gen_empty_has_end_only() {
    let grammar = make_grammar("g", vec![], vec![], vec![]);
    let pt = make_lang_gen_table(&grammar, 1, 1);
    let output = LanguageGenerator::new(&grammar, &pt).generate().to_string();
    assert!(output.contains("\"end\""));
}

#[test]
fn lang_gen_underscore_prefix() {
    let grammar = make_grammar(
        "g",
        vec![(SymbolId(1), regex_token("_whitespace", r"\s"))],
        vec![],
        vec![],
    );
    let pt = make_lang_gen_table(&grammar, 1, 2);
    let output = LanguageGenerator::new(&grammar, &pt).generate().to_string();
    assert!(output.contains("_whitespace"));
}

#[test]
fn lang_gen_function_name() {
    let grammar = make_grammar("my_lang", vec![], vec![], vec![]);
    let pt = make_lang_gen_table(&grammar, 1, 1);
    let output = LanguageGenerator::new(&grammar, &pt).generate().to_string();
    assert!(output.contains("tree_sitter_my_lang"));
}

#[test]
fn lang_gen_symbol_count_field() {
    let grammar = make_grammar(
        "g",
        vec![(SymbolId(1), regex_token("t", "."))],
        vec![],
        vec![],
    );
    let pt = make_lang_gen_table(&grammar, 1, 3);
    let output = LanguageGenerator::new(&grammar, &pt).generate().to_string();
    assert!(output.contains("symbol_count"));
}

// ===========================================================================
// 2. AbiLanguageBuilder — symbol names in generated TokenStream
// ===========================================================================

#[test]
fn abi_eof_is_end() {
    let grammar = make_grammar("g", vec![], vec![], vec![]);
    let mut s2i = BTreeMap::new();
    s2i.insert(SymbolId(0), 0);
    let output = abi_output(&grammar, s2i, SymbolId(0));
    // "end\0" → 'e'=101
    assert!(output.contains("101u8"));
}

#[test]
fn abi_token_name() {
    let grammar = make_grammar(
        "g",
        vec![(SymbolId(1), string_token("plus", "+"))],
        vec![],
        vec![],
    );
    let mut s2i = BTreeMap::new();
    s2i.insert(SymbolId(0), 0);
    s2i.insert(SymbolId(1), 1);
    let output = abi_output(&grammar, s2i, SymbolId(0));
    // "plus" → 'p'=112
    assert!(output.contains("112u8"));
}

#[test]
fn abi_deterministic_order() {
    let grammar = make_grammar(
        "g",
        vec![
            (SymbolId(5), string_token("beta", "b")),
            (SymbolId(1), string_token("alpha", "a")),
        ],
        vec![],
        vec![],
    );
    let mut s2i = BTreeMap::new();
    s2i.insert(SymbolId(0), 0);
    s2i.insert(SymbolId(1), 1);
    s2i.insert(SymbolId(5), 2);
    let output = abi_output(&grammar, s2i, SymbolId(0));
    let alpha_pos = output.find("97u8").expect("'a' byte");
    let beta_pos = output.find("98u8").expect("'b' byte");
    assert!(alpha_pos < beta_pos);
}

#[test]
fn abi_null_terminated() {
    let grammar = make_grammar(
        "g",
        vec![(SymbolId(1), string_token("x", "x"))],
        vec![],
        vec![],
    );
    let mut s2i = BTreeMap::new();
    s2i.insert(SymbolId(0), 0);
    s2i.insert(SymbolId(1), 1);
    let output = abi_output(&grammar, s2i, SymbolId(0));
    assert!(output.matches("0u8").count() >= 2);
}

#[test]
fn abi_rule_fallback() {
    let grammar = make_grammar("g", vec![], vec![], vec![]);
    let mut s2i = BTreeMap::new();
    s2i.insert(SymbolId(0), 0);
    s2i.insert(SymbolId(7), 1);
    let output = abi_output(&grammar, s2i, SymbolId(0));
    // "rule_7" → 'r'=114
    assert!(output.contains("114u8"));
}

#[test]
fn abi_explicit_rule_name() {
    let grammar = make_grammar("g", vec![], vec![], vec![(SymbolId(3), "stmt".to_string())]);
    let mut s2i = BTreeMap::new();
    s2i.insert(SymbolId(0), 0);
    s2i.insert(SymbolId(3), 1);
    let output = abi_output(&grammar, s2i, SymbolId(0));
    // "stmt" → 's'=115
    assert!(output.contains("115u8"));
}

#[test]
fn abi_sequential_idents() {
    let grammar = make_grammar(
        "g",
        vec![(SymbolId(1), string_token("t", "t"))],
        vec![],
        vec![],
    );
    let mut s2i = BTreeMap::new();
    s2i.insert(SymbolId(0), 0);
    s2i.insert(SymbolId(1), 1);
    let output = abi_output(&grammar, s2i, SymbolId(0));
    assert!(output.contains("SYMBOL_NAME_0"));
    assert!(output.contains("SYMBOL_NAME_1"));
}

#[test]
fn abi_has_ptrs_array_and_sync_ptr() {
    let grammar = make_grammar(
        "g",
        vec![(SymbolId(1), string_token("t", "t"))],
        vec![],
        vec![],
    );
    let mut s2i = BTreeMap::new();
    s2i.insert(SymbolId(0), 0);
    s2i.insert(SymbolId(1), 1);
    let output = abi_output(&grammar, s2i, SymbolId(0));
    assert!(output.contains("SYMBOL_NAME_PTRS"));
    assert!(output.contains("SyncPtr"));
}

#[test]
fn abi_underscore_prefix() {
    let grammar = make_grammar(
        "g",
        vec![(SymbolId(1), regex_token("_h", r"\s"))],
        vec![],
        vec![],
    );
    let mut s2i = BTreeMap::new();
    s2i.insert(SymbolId(0), 0);
    s2i.insert(SymbolId(1), 1);
    let output = abi_output(&grammar, s2i, SymbolId(0));
    // '_' = 95u8
    assert!(output.contains("95u8"));
}

#[test]
fn abi_many_symbols() {
    let mut tokens = vec![];
    let mut s2i = BTreeMap::new();
    s2i.insert(SymbolId(0), 0);
    for i in 1..=20u16 {
        tokens.push((
            SymbolId(i),
            string_token(&format!("s{}", i), &format!("{}", i)),
        ));
        s2i.insert(SymbolId(i), i as usize);
    }
    let grammar = make_grammar("g", tokens, vec![], vec![]);
    let output = abi_output(&grammar, s2i, SymbolId(0));
    assert!(output.contains("SYMBOL_NAME_0"));
    assert!(output.contains("SYMBOL_NAME_20"));
}

#[test]
fn abi_external_token_preserves_name() {
    let mut grammar = make_grammar("g", vec![], vec![], vec![]);
    grammar.externals.push(ExternalToken {
        name: "tmpl".to_string(),
        symbol_id: SymbolId(1),
    });
    let mut s2i = BTreeMap::new();
    s2i.insert(SymbolId(0), 0);
    s2i.insert(SymbolId(1), 1);
    let output = abi_output(&grammar, s2i, SymbolId(0));
    assert!(abi_output_has_symbol_name(&output, "tmpl"));
}

#[test]
fn abi_language_struct_refs_names() {
    let grammar = make_grammar(
        "g",
        vec![(SymbolId(1), string_token("t", "t"))],
        vec![],
        vec![],
    );
    let mut s2i = BTreeMap::new();
    s2i.insert(SymbolId(0), 0);
    s2i.insert(SymbolId(1), 1);
    let output = abi_output(&grammar, s2i, SymbolId(0));
    assert!(output.contains("symbol_names"));
}

// ===========================================================================
// 3. LanguageBuilder — symbol names via generate_language()
// ===========================================================================

#[test]
fn lb_includes_token() {
    let mut grammar = Grammar::new("test".to_string());
    grammar
        .tokens
        .insert(SymbolId(1), regex_token("number", r"\d+"));
    let pt = make_lb_table_for_grammar(&grammar, 3);
    let names = lb_symbol_names(grammar, pt);
    assert!(names.contains(&"number".to_string()));
}

#[test]
fn lb_includes_rule() {
    let mut grammar = Grammar::new("test".to_string());
    grammar.add_rule(Rule {
        lhs: SymbolId(2),
        rhs: vec![],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    let pt = make_lb_table_for_grammar(&grammar, 3);
    let names = lb_symbol_names(grammar, pt);
    assert!(names.iter().any(|n| n.starts_with("rule_")));
}

#[test]
fn lb_includes_external() {
    let mut grammar = Grammar::new("test".to_string());
    grammar.externals.push(ExternalToken {
        name: "heredoc".to_string(),
        symbol_id: SymbolId(10),
    });
    let pt = make_lb_table_for_grammar(&grammar, 3);
    let names = lb_symbol_names(grammar, pt);
    assert!(names.contains(&"heredoc".to_string()));
}

#[test]
fn lb_order_terminals_rules_externals() {
    let mut grammar = Grammar::new("test".to_string());
    grammar.tokens.insert(SymbolId(1), string_token("tok", "t"));
    grammar.add_rule(Rule {
        lhs: SymbolId(2),
        rhs: vec![],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    grammar.externals.push(ExternalToken {
        name: "ext".to_string(),
        symbol_id: SymbolId(10),
    });
    let pt = make_lb_table_for_grammar(&grammar, 3);
    let names = lb_symbol_names(grammar, pt);
    assert_eq!(names[0], "end");
    assert_eq!(names[1], "tok");
    assert!(names[2].starts_with("rule_"));
    assert_eq!(names[3], "ext");
}

#[test]
fn lb_name_count() {
    let mut grammar = Grammar::new("test".to_string());
    grammar.tokens.insert(SymbolId(1), string_token("a", "a"));
    grammar.tokens.insert(SymbolId(2), string_token("b", "b"));
    grammar.externals.push(ExternalToken {
        name: "c".to_string(),
        symbol_id: SymbolId(10),
    });
    let pt = make_lb_table_for_grammar(&grammar, 3);
    let names = lb_symbol_names(grammar, pt);
    assert_eq!(names.len(), 4);
}

#[test]
fn lb_multiple_externals() {
    let mut grammar = Grammar::new("test".to_string());
    grammar.externals.push(ExternalToken {
        name: "indent".to_string(),
        symbol_id: SymbolId(10),
    });
    grammar.externals.push(ExternalToken {
        name: "dedent".to_string(),
        symbol_id: SymbolId(11),
    });
    grammar.externals.push(ExternalToken {
        name: "newline".to_string(),
        symbol_id: SymbolId(12),
    });
    let pt = make_lb_table_for_grammar(&grammar, 3);
    let names = lb_symbol_names(grammar, pt);
    assert!(names.contains(&"indent".to_string()));
    assert!(names.contains(&"dedent".to_string()));
    assert!(names.contains(&"newline".to_string()));
}

#[test]
fn lb_empty_grammar() {
    let grammar = Grammar::new("test".to_string());
    let pt = make_lb_table(1, 2);
    let builder = LanguageBuilder::new(grammar, pt);
    let lang = builder.generate_language().expect("should succeed");
    assert_eq!(lang.version, 15);
}

#[test]
fn lb_version_is_15() {
    let mut grammar = Grammar::new("test".to_string());
    grammar.tokens.insert(SymbolId(1), regex_token("x", "."));
    let pt = make_lb_table(3, 2);
    let builder = LanguageBuilder::new(grammar, pt);
    let lang = builder.generate_language().expect("should succeed");
    assert_eq!(lang.version, 15);
}

#[test]
fn lb_symbol_count_matches() {
    let mut grammar = Grammar::new("test".to_string());
    grammar.tokens.insert(SymbolId(1), regex_token("x", "."));
    let pt = make_lb_table(3, 5);
    let builder = LanguageBuilder::new(grammar, pt);
    let lang = builder.generate_language().expect("should succeed");
    assert_eq!(lang.symbol_count, 5);
}
