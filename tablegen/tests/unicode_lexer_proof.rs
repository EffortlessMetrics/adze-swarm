//! Runtime execution proof for the unicode-identifier lexer arm (#878).
//!
//! Emission contract (helpers, unicode arm, unicode-aware keyword guard) copied
//! verbatim from the candidate lane's `tablegen/src/lexer_gen.rs` (READ ONLY).
//! The in-lane generator still drops XID spellings (tripwire below), so the
//! shape is constructed and driven via the same rustc compile-and-execute path
//! as `generated_lexer`'s config-lexer execution test. Replace with real
//! generator output when the candidate lane merges.

use adze_ir::SymbolId;
use adze_ir::builder::GrammarBuilder;
use adze_tablegen::lexer_gen::generate_lexer_with_table;
use std::collections::BTreeMap;

/// Tripwire: the in-lane generator cannot classify XID spellings yet (owned by the unicode candidate lane); flip this when that lane merges.
#[test]
fn unicode_spelling_has_no_inlane_arm_yet() {
    let grammar = GrammarBuilder::new("unicode_gap")
        .token("ident", r"[\p{XID_Start}_][\p{XID_Continue}_]*")
        .rule("S", vec!["ident"])
        .start("S")
        .build();
    let map = BTreeMap::from([(SymbolId(1), 1usize)]);
    let code = generate_lexer_with_table(&grammar, &map, None).to_string();
    assert!(
        !code.contains("lexer_utf8_char_at_rel"),
        "in-lane generator emits a unicode arm; use real generator output"
    );
}

#[test]
fn unicode_emitted_matcher_executes_at_runtime() {
    let source = [HEADER, UNICODE_BODY, DRIVER].concat();
    let clock = std::time::SystemTime::now();
    let nonce = clock
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
            .expect("execute compiler for emitted matcher");
    let ok = compiler.status.success();
    let err = String::from_utf8_lossy(&compiler.stderr);
    assert!(ok, "emitted unicode matcher must compile: {err}");
    let execution = std::process::Command::new(&executable)
        .output()
        .expect("run emitted matcher");
    let ok = execution.status.success();
    let err = String::from_utf8_lossy(&execution.stderr);
    assert!(ok, "unicode semantics: {err}");
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
#[repr(C)]
struct LexerBackingView { input_ptr: *const u8, input_len: usize, pos: usize }
unsafe fn lexer_view(lexer: *mut adze::lex::TsLexer) -> *mut LexerBackingView {
    unsafe { (*lexer).data as *mut LexerBackingView }
}
unsafe fn lexer_byte_at_rel(lexer: *mut adze::lex::TsLexer, rel: usize) -> u32 {
    unsafe {
        let view = &*lexer_view(lexer);
        let idx = view.pos.saturating_add(rel);
        if idx < view.input_len { *view.input_ptr.add(idx) as u32 } else { 0 }
    }
}
unsafe fn lexer_pos(lexer: *mut adze::lex::TsLexer) -> usize { unsafe { (*lexer_view(lexer)).pos } }
unsafe fn lexer_set_pos(lexer: *mut adze::lex::TsLexer, pos: usize) {
    unsafe { (*lexer_view(lexer)).pos = pos; }
}
#[inline]
fn is_word_char(byte: u32) -> bool {
    byte != 0 && ((byte as u8).is_ascii_alphanumeric() || byte == b'_' as u32)
}
#[inline]
fn utf8_char_width(first: u32) -> usize {
    match first {
        0x01..=0x7F => 1, 0xC2..=0xDF => 2, 0xE0..=0xEF => 3, 0xF0..=0xF4 => 4, _ => 0,
    }
}
unsafe fn lexer_utf8_char_at_rel(lexer: *mut adze::lex::TsLexer, rel: usize) -> (usize, u32) {
    unsafe {
        let b0 = lexer_byte_at_rel(lexer, rel);
        if b0 < 0x80 { return ((b0 != 0) as usize, b0); }
        let width = utf8_char_width(b0);
        if width < 2 { return (0, 0); }
        let b1 = lexer_byte_at_rel(lexer, rel + 1);
        if !(0x80..=0xBF).contains(&b1) { return (0, 0); }
        if width == 2 { return (2, ((b0 & 0x1F) << 6) | (b1 & 0x3F)); }
        let b2 = lexer_byte_at_rel(lexer, rel + 2);
        if !(0x80..=0xBF).contains(&b2) { return (0, 0); }
        if width == 3 {
            let cp = ((b0 & 0x0F) << 12) | ((b1 & 0x3F) << 6) | (b2 & 0x3F);
            if cp < 0x800 || (0xD800..=0xDFFF).contains(&cp) { return (0, 0); }
            return (3, cp);
        }
        let b3 = lexer_byte_at_rel(lexer, rel + 3);
        if !(0x80..=0xBF).contains(&b3) { return (0, 0); }
        let cp = ((b0 & 0x07) << 18) | ((b1 & 0x3F) << 12) | ((b2 & 0x3F) << 6) | (b3 & 0x3F);
        if !(0x10000..=0x10FFFF).contains(&cp) { return (0, 0); }
        (4, cp)
    }
}
#[inline]
fn is_unicode_ident_start(cp: u32) -> bool {
    matches!(cp, 0x41..=0x5A | 0x61..=0x7A | 0x5F)
        || (cp >= 0x80 && char::from_u32(cp).is_some_and(|c| c.is_alphabetic()))
}
#[inline]
fn is_unicode_ident_continue(cp: u32) -> bool {
    if is_unicode_ident_start(cp) || matches!(cp, 0x30..=0x39) { return true; }
    if cp < 0x80 { return false; }
    char::from_u32(cp).is_some_and(|c| c.is_alphanumeric())
        || matches!(cp, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF
            | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}
#[inline]
fn better_match(new_sym: u16, new_len: usize, new_pri: i16,
    old_sym: u16, old_len: usize, old_pri: i16) -> bool {
    if new_len != old_len { return new_len > old_len; }
    if new_pri != old_pri { return new_pri > old_pri; }
    new_sym < old_sym
}
"###;

const UNICODE_BODY: &str = r###"
unsafe extern "C" fn lexer_fn(
    state_ptr: *mut ::std::ffi::c_void,
    lex_mode: adze::pure_parser::TSLexState,
) -> bool {
    if state_ptr.is_null() { return false; }
    let lexer = state_ptr as *mut adze::lex::TsLexer;
    match lex_mode.lex_state {
        0 => {
            let mut best_sym: Option<u16> = None;
            let mut best_len: usize = 0;
            let mut best_pri: i16 = i16::MIN;
            {
                match (|| unsafe {
                    if lexer_byte_at_rel(lexer, 0) != 105 { return None; }
                    if lexer_byte_at_rel(lexer, 1) != 102 { return None; }
                    let next = lexer_byte_at_rel(lexer, 2);
                    if next != 0 && (is_word_char(next) || next >= 0x80) { return None; }
                    Some(2)
                })() {
                    Some(len) if len > 0 => {
                        let replace = match best_sym {
                            None => true,
                            Some(old) => better_match(1, len, 1, old, best_len, best_pri),
                        };
                        if replace { best_sym = Some(1); best_len = len; best_pri = 1; }
                    }
                    _ => {}
                }
            }
            {
                match (|| unsafe {
                    let (mut len, first_cp) = lexer_utf8_char_at_rel(lexer, 0);
                    if len == 0 || !is_unicode_ident_start(first_cp) { return None; }
                    loop {
                        let (width, cp) = lexer_utf8_char_at_rel(lexer, len);
                        if width == 0 || !is_unicode_ident_continue(cp) { break; }
                        len += width;
                    }
                    Some(len)
                })() {
                    Some(len) if len > 0 => {
                        let replace = match best_sym {
                            None => true,
                            Some(old) => better_match(2, len, 0, old, best_len, best_pri),
                        };
                        if replace { best_sym = Some(2); best_len = len; best_pri = 0; }
                    }
                    _ => {}
                }
            }
            let _ = best_pri;
            if let Some(sym) = best_sym {
                let start = unsafe { lexer_pos(lexer) };
                unsafe {
                    lexer_set_pos(lexer, start + best_len);
                    (*lexer).result_symbol = sym;
                    ((*lexer).mark_end)(lexer);
                }
                return true;
            }
            false
        }
        _ => false,
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
