//! A reduction's lookahead must be lexed before it can drive that reduction.
use adze_glr_core::{Action, LexMode, ParseTable};
use adze_ir::{Grammar, RuleId, StateId, SymbolId, Token, TokenPattern};
use adze_tablegen::lexer_gen::generate_lexer_with_table;
use std::collections::BTreeMap;

#[test]
fn test_emitted_modes_admit_real_reduction_lookahead_and_exclude_errors() {
    let mut grammar = Grammar::new("reduction_lookahead".to_owned());
    for (id, pattern) in [
        (1, TokenPattern::Regex(r"\d+".to_owned())),
        (2, TokenPattern::Regex("[a-z]+".to_owned())),
        (3, TokenPattern::String("+".to_owned())),
        (4, TokenPattern::Regex("[-+*/]".to_owned())),
    ] {
        grammar.tokens.insert(
            SymbolId(id),
            Token {
                name: format!("t{id}"),
                pattern,
                fragile: false,
            },
        );
    }
    let mapping = BTreeMap::from([
        (SymbolId(1), 1),
        (SymbolId(2), 2),
        (SymbolId(3), 3),
        (SymbolId(4), 4),
    ]);
    let rows = [
        Action::Reduce(RuleId(0)),
        Action::Shift(StateId(0)),
        Action::Accept,
        Action::Fork(vec![Action::Error, Action::Reduce(RuleId(0))]),
        Action::Error,
        Action::Fork(vec![Action::Error, Action::Recover]),
    ]
    .into_iter()
    .map(|action| {
        vec![
            vec![Action::Error],
            vec![action],
            vec![Action::Error],
            vec![Action::Error],
            vec![Action::Error],
        ]
    })
    .collect::<Vec<_>>();
    let mut table = ParseTable {
        lex_modes: (0..rows.len())
            .map(|state| LexMode {
                lex_state: state as u16,
                external_lex_state: 0,
            })
            .collect(),
        action_table: rows,
        ..ParseTable::default()
    };
    let modeful = generate_lexer_with_table(&grammar, &mapping, Some(&table)).to_string();
    table.action_table = vec![vec![vec![Action::Error]; 5]];
    table.lex_modes = vec![LexMode {
        lex_state: 0,
        external_lex_state: 0,
    }];
    let only_error = generate_lexer_with_table(&grammar, &mapping, Some(&table)).to_string();
    let prelude = r#"
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
#[repr(C)] struct Backing<'a> { input: &'a [u8], pos: usize }
unsafe extern "C" fn mark_end(_: *mut adze::lex::TsLexer) {}
"#;
    let runner = r#"
pub fn lex(mode: u16, source: &str) -> Option<(u16, usize)> {
    let mut backing = crate::Backing { input: source.as_bytes(), pos: 0 };
    let mut lexer = adze::lex::TsLexer {
        data: &mut backing as *mut _ as *mut std::ffi::c_void,
        result_symbol: u16::MAX, mark_end: crate::mark_end,
    };
    let matched = unsafe { lexer_fn(&mut lexer as *mut _ as *mut std::ffi::c_void,
        adze::pure_parser::TSLexState { lex_state: mode }) };
    if matched { Some((lexer.result_symbol, backing.pos)) } else {
        assert_eq!(backing.pos, 0, "failed mode cannot consume input");
        None
    }
}
"#;
    let controls = r#"
fn main() {
    for mode in 0..=3 {
        assert_eq!(modeful::lex(mode, "17x"), Some((1, 2)), "mode {mode} must expose numeric lookahead");
        assert_eq!(modeful::lex(mode, "abc"), None, "Error regex column cannot become eligible");
        assert_eq!(modeful::lex(mode, "+"), None, "Error literal/class columns cannot become eligible");
    }
    for mode in [4, 5] {
        for source in ["17", "abc", "+"] {
            assert_eq!(modeful::lex(mode, source), None, "mode {mode} has no ordinary parse action");
        }
    }
    for source in ["17", "abc", "+"] {
        assert_eq!(only_error::lex(0, source), None, "an explicit Error-only table cannot fall back to all tokens");
    }
}
"#;
    let source = format!(
        "{prelude}\nmod modeful {{ use crate::adze; {modeful} {runner} }}\nmod only_error {{ use crate::adze; {only_error} {runner} }}\n{controls}"
    );
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "adze-reduction-lexer-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let source_path = directory.join("lexer.rs");
    let executable = directory.join(format!("lexer{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&source_path, source).unwrap();
    let compiler =
        std::process::Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .arg("--edition=2024")
            .arg(&source_path)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
    assert!(
        compiler.status.success(),
        "{}",
        String::from_utf8_lossy(&compiler.stderr)
    );
    let run = std::process::Command::new(&executable).output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    std::fs::remove_dir_all(directory).unwrap();
}
