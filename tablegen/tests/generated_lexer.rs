//! Characterization tests for mode-aware non-destructive generated lexer (#926).

use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Grammar, LexicalMetadata, SymbolId};
use adze_tablegen::lexer_gen::{generate_lexer, generate_lexer_with_table};
use std::collections::BTreeMap;

fn build_table(grammar: &Grammar) -> adze_glr_core::ParseTable {
    let mut working = grammar.clone();
    let first_follow = FirstFollowSets::compute_normalized(&mut working).expect("FIRST/FOLLOW");
    build_lr1_automaton(&working, &first_follow).expect("LR(1)")
}

fn codegen(grammar: &Grammar, table: Option<&adze_glr_core::ParseTable>) -> String {
    let symbol_to_index: BTreeMap<SymbolId, usize> = grammar
        .tokens
        .keys()
        .enumerate()
        .map(|(idx, id)| (*id, idx + 1))
        .collect();
    generate_lexer_with_table(grammar, &symbol_to_index, table).to_string()
}

#[test]
fn generated_lexer_uses_non_destructive_byte_peek() {
    let grammar = GrammarBuilder::new("peek")
        .token("if_kw", "if")
        .token("in_kw", "in")
        .rule("S", vec!["if_kw"])
        .start("S")
        .build();
    let code = codegen(&grammar, None);
    assert!(code.contains("lexer_byte_at_rel"), "non-destructive peek");
    assert!(code.contains("lexer_set_pos"), "commit cursor once");
    assert!(
        !code.contains("advance)(lexer"),
        "no destructive advance in candidates"
    );
}

#[test]
fn generated_lexer_maximal_munch_emits_both_eq_candidates() {
    let grammar = GrammarBuilder::new("eq")
        .token("assign", "=")
        .token("eq_eq", "==")
        .rule("S", vec!["assign", "eq_eq"])
        .start("S")
        .build();
    let code = codegen(&grammar, None);
    assert!(code.contains("better_match"), "maximal-munch selector");
    assert!(code.contains("61u32"), "'=' byte check");
    assert!(
        code.matches("61u32").count() >= 2,
        "'==' uses two byte checks"
    );
}

#[test]
fn generated_lexer_enforces_lex_mode_dispatch() {
    let grammar = GrammarBuilder::new("modes")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a", "b"])
        .start("S")
        .build();
    let table = build_table(&grammar);
    let code = codegen(&grammar, Some(&table));
    assert!(
        code.contains("lex_mode") && code.contains("lex_state"),
        "generated lexer must read lex_mode; code snippet: {}",
        &code[code.find("lexer_fn").unwrap_or(0)
            ..code.len().min(code.find("lexer_fn").unwrap_or(0) + 400)]
    );
}

#[test]
fn generated_lexer_keyword_and_word_token_both_register_candidates() {
    let mut grammar = GrammarBuilder::new("word")
        .token("identifier", "[a-zA-Z_][a-zA-Z0-9_]*")
        .token("if_kw", "if")
        .rule("S", vec!["identifier"])
        .start("S")
        .build();
    let ident = grammar
        .tokens
        .iter()
        .find(|(_, token)| token.name == "identifier")
        .map(|(id, _)| *id)
        .expect("identifier");
    grammar.word_token = Some(ident);
    grammar.set_lexical_metadata(
        ident,
        LexicalMetadata {
            lexical_priority: 0,
            immediate: false,
        },
    );
    let if_id = grammar
        .tokens
        .iter()
        .find(|(_, token)| token.name == "if_kw")
        .map(|(id, _)| *id)
        .expect("if");
    grammar.set_lexical_metadata(
        if_id,
        LexicalMetadata {
            lexical_priority: 1,
            immediate: false,
        },
    );

    let code = codegen(&grammar, None);
    assert!(code.contains("is_identifier_start"), "word-token pattern");
    assert!(code.contains("is_word_char"), "keyword boundary check");
    assert!(code.contains("better_match"), "priority tie-break");
}

#[test]
fn generated_lexer_duplicate_patterns_keep_distinct_symbols() {
    use adze_ir::{Grammar, Token, TokenPattern};
    let mut grammar = Grammar::new("dup".to_string());
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "plus_a".to_string(),
            pattern: TokenPattern::String("+".into()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        SymbolId(2),
        Token {
            name: "plus_b".to_string(),
            pattern: TokenPattern::String("+".into()),
            fragile: false,
        },
    );
    let map = BTreeMap::from([(SymbolId(1), 1usize), (SymbolId(2), 2usize)]);
    let code = generate_lexer_with_table(&grammar, &map, None).to_string();
    assert!(
        code.matches("better_match").count() >= 2,
        "duplicate literals should register distinct candidates"
    );
}

#[test]
fn legacy_generate_lexer_entry_still_available() {
    let grammar = GrammarBuilder::new("legacy")
        .token("x", "x")
        .rule("S", vec!["x"])
        .start("S")
        .build();
    let map = BTreeMap::from([(SymbolId(1), 1usize)]);
    let code = generate_lexer(&grammar, &map).to_string();
    assert!(code.contains("lexer_fn"));
}

#[test]
fn test_generated_lexer_executes_config_numeric_whitespace_and_comment_patterns() {
    use adze_ir::{Token, TokenPattern};

    let mut grammar = Grammar::new("config_lexical_subset".to_string());
    for (id, name, pattern) in [
        (1, "number", r"[0-9]+"),
        (2, "space", r"[ \t\r\n]+"),
        (3, "comment", r"#[^\r\n]*"),
    ] {
        grammar.tokens.insert(
            SymbolId(id),
            Token {
                name: name.to_owned(),
                pattern: TokenPattern::Regex(pattern.to_owned()),
                fragile: false,
            },
        );
    }
    run_generated_lexer_control(
        &grammar,
        r###"
fn main() {
    check("0", Some((1, 1)));
    check("65535;", Some((1, 5)));
    check("-1", None);
    check("١", None);
    check(" \t\r\nx", Some((2, 4)));
    check("\u{000b}", None);
    check("\u{000c}", None);
    check("#", Some((3, 1)));
    check("# café λ\r\nnext", Some((3, "# café λ".len())));
    check("##\n", Some((3, 2)));
    check("# no final newline", Some((3, 18)));
    check("# before\0after\n", Some((3, "# before\0after".len())));
    check("x#", None);
}
"###,
    );
}

#[test]
fn test_generated_lexer_executes_single_lowercase_and_preserves_lowercase_plus() {
    use adze_ir::{Token, TokenPattern};

    let mut grammar = Grammar::new("single_lowercase".to_string());
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "letter".to_owned(),
            pattern: TokenPattern::Regex(r"[a-z]".to_owned()),
            fragile: false,
        },
    );
    run_generated_lexer_control(
        &grammar,
        r###"
fn main() {
    check("", None);
    for byte in 0u8..=127 {
        let input = char::from(byte).to_string();
        let expected = (b'a'..=b'z').contains(&byte).then_some((1, 1));
        check(&input, expected);
    }
    check("abcdefghijklmnopqrstuvwxyz", Some((1, 1)));
    check("abc]", Some((1, 1)));
    check("za", Some((1, 1)));
    check("aé", Some((1, 1)));
    check("a\0", Some((1, 1)));
    check("éa", None);
    check("λ", None);
    check("١", None);
}
"###,
    );

    grammar
        .tokens
        .get_mut(&SymbolId(1))
        .expect("letter")
        .pattern = TokenPattern::Regex(r"[a-z]+".to_owned());
    run_generated_lexer_control(
        &grammar,
        r###"
fn main() {
    check("", None);
    for byte in 0u8..=127 {
        let input = char::from(byte).to_string();
        let expected = (b'a'..=b'z').contains(&byte).then_some((1, 1));
        check(&input, expected);
    }
    check("abcdefghijklmnopqrstuvwxyz", Some((1, 26)));
    check("abc]", Some((1, 3)));
    check("za", Some((1, 2)));
    check("aé", Some((1, 1)));
    check("a\0", Some((1, 1)));
    check("éa", None);
    check("λ", None);
    check("١", None);
}
"###,
    );
}

fn run_generated_lexer_control(grammar: &Grammar, control: &str) {
    use std::process::Command;

    let generated = codegen(grammar, None);
    // Compile and execute the emitted matcher. The minimal callback ABI below
    // provides the same repr(C) input-slice/position backing as the runtime.
    let source = [
        r###"
mod adze {
    pub mod pure_parser { pub struct TSLexState { pub lex_state: u16 } }
    pub mod lex {
        pub struct TsLexer {
            pub data: *mut std::ffi::c_void,
            pub result_symbol: u16,
            pub mark_end: unsafe extern "C" fn(*mut TsLexer),
        }
    }
}
"###,
        &generated,
        r###"
#[repr(C)]
struct Backing<'a> { input: &'a [u8], pos: usize }
unsafe extern "C" fn mark_end(_: *mut adze::lex::TsLexer) {}
fn check(input: &str, expected: Option<(u16, usize)>) {
    let mut backing = Backing { input: input.as_bytes(), pos: 0 };
    let mut lexer = adze::lex::TsLexer {
        data: &mut backing as *mut _ as *mut std::ffi::c_void,
        result_symbol: u16::MAX,
        mark_end,
    };
    let matched = unsafe { lexer_fn(&mut lexer as *mut _ as *mut std::ffi::c_void,
        adze::pure_parser::TSLexState { lex_state: 0 }) };
    assert_eq!(matched, expected.is_some(), "input {input:?}");
    if let Some((symbol, length)) = expected {
        assert_eq!((lexer.result_symbol, backing.pos), (symbol, length), "input {input:?}");
    } else {
        assert_eq!(backing.pos, 0, "failed candidates must not consume input {input:?}");
    }
}
"###,
        control,
    ]
    .concat();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("timestamp")
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("adze-config-lexer-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&directory).expect("owned compiler test directory");
    let source_path = directory.join("lexer.rs");
    let executable = directory.join(format!("lexer{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&source_path, source).expect("generated lexer source");
    let compiler = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .arg("--edition=2024")
        .arg(&source_path)
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("execute compiler for emitted matcher");
    assert!(
        compiler.status.success(),
        "generated lexer must compile: {}",
        String::from_utf8_lossy(&compiler.stderr)
    );
    let execution = Command::new(&executable)
        .output()
        .expect("execute emitted matcher");
    assert!(
        execution.status.success(),
        "generated lexer must honor exact byte spans: {}",
        String::from_utf8_lossy(&execution.stderr)
    );
    std::fs::remove_dir_all(directory).expect("remove only the owned compiler test directory");
}
