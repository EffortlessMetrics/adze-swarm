//! Execute the production-generated Unicode identifier matcher (#878).
//!
//! The lexer body and its helpers come directly from the current generator.
//! Minimal ABI shims and independent literal input expectations are test-owned.
//! These bounded cases do not establish complete Unicode XID conformance or
//! independent upstream Tree-sitter parity.

use adze_ir::builder::GrammarBuilder;
use adze_ir::{LexicalMetadata, SymbolId};
use adze_tablegen::lexer_gen::generate_lexer_with_table;
use std::collections::BTreeMap;

fn generated_unicode_lexer() -> String {
    let mut grammar = GrammarBuilder::new("unicode_execution")
        .token("if_kw", "if")
        .token("ident", r"[\p{XID_Start}_][\p{XID_Continue}_]*")
        .rule("S", vec!["ident"])
        .start("S")
        .build();
    grammar.word_token = Some(SymbolId(2));
    grammar.set_lexical_metadata(
        SymbolId(1),
        LexicalMetadata {
            lexical_priority: 1,
            immediate: false,
        },
    );
    let map = BTreeMap::from([(SymbolId(1), 1usize), (SymbolId(2), 2usize)]);
    generate_lexer_with_table(&grammar, &map, None).to_string()
}

#[test]
fn test_unicode_spelling_emits_native_arm() {
    let generated = generated_unicode_lexer();
    assert!(
        generated.contains("lexer_utf8_char_at_rel"),
        "production generator must emit Unicode decoding"
    );
    assert!(
        generated.contains("is_unicode_ident_start"),
        "production generator must emit the identifier arm"
    );
}

#[test]
fn test_unicode_generated_matcher_executes_at_runtime() {
    let generated = generated_unicode_lexer();
    let source = [HEADER, &generated, DRIVER].concat();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("adze-uni-lex-{nonce}-{}", std::process::id()));
    std::fs::create_dir(&directory).expect("owned compiler test directory");
    let source_path = directory.join("lexer.rs");
    let executable = directory.join(format!("lexer{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&source_path, source).expect("generated lexer source");
    let compiler =
        std::process::Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .arg("--edition=2024")
            .arg(&source_path)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("execute compiler for generated matcher");
    assert!(
        compiler.status.success(),
        "generated Unicode matcher must compile: {}",
        String::from_utf8_lossy(&compiler.stderr)
    );
    let execution = std::process::Command::new(&executable)
        .output()
        .expect("run generated matcher");
    assert!(
        execution.status.success(),
        "Unicode semantics: {}",
        String::from_utf8_lossy(&execution.stderr)
    );
    std::fs::remove_dir_all(directory).expect("remove only the owned compiler test directory");
}

const HEADER: &str = r###"
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
"###;

const DRIVER: &str = r###"
#[repr(C)]
struct Backing<'a> { input: &'a [u8], pos: usize }
unsafe extern "C" fn mark_end(_: *mut adze::lex::TsLexer) {}
fn check(input: &[u8], expected: Option<(u16, usize)>) {
    let mut backing = Backing { input, pos: 0 };
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
        assert_eq!(backing.pos, 0, "failed candidates must not consume input");
    }
}
fn main() {
    check("café".as_bytes(), Some((2, 5)));
    check("λ".as_bytes(), Some((2, 2)));
    check("e\u{301}".as_bytes(), Some((2, 3)));
    check(b"hello", Some((2, 5))); check(b"_x1", Some((2, 3)));
    check(b"if", Some((1, 2))); check(b"ifx", Some((2, 3)));
    check("ifé".as_bytes(), Some((2, 4)));
    check(b"1abc", None); check(b"", None);
    check("\u{301}".as_bytes(), None);
    check(b"\xFF", None); check(b"\xC3", None);
    check(b"\xC0\xAF", None); check(b"\xED\xA0\x80", None);
    check(b"a\xFF", Some((2, 1)));
}
"###;
