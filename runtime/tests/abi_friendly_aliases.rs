//! Friendly aliases for TS-prefixed ABI types are the same types (#871).

use std::mem::size_of;

#[test]
fn friendly_names_cover_every_aliased_type() {
    // Compile-time identity: assigning friendly to TS name and back only
    // compiles when both names denote one type. Runtime checks cover the
    // value-carrying aliases; the rest are asserted by type ascription.
    let _: adze::ffi::Symbol = 0 as adze::ffi::TSSymbol;
    let _: adze::ffi::TSSymbol = 0 as adze::ffi::Symbol;
    let _: adze::pure_parser::Symbol = 0 as adze::pure_parser::TSSymbol;
    let _: adze::pure_parser::TSSymbol = 0 as adze::pure_parser::Symbol;
    let _: adze::pure_parser::StateId = 0 as adze::pure_parser::TSStateId;
    let _: adze::pure_parser::TSStateId = 0 as adze::pure_parser::StateId;
    let _: adze::pure_parser::FieldId = 0 as adze::pure_parser::TSFieldId;
    let _: adze::pure_parser::TSFieldId = 0 as adze::pure_parser::FieldId;

    fn metadata(v: adze::ffi::SymbolMetadata) -> adze::ffi::TSSymbolMetadata {
        v
    }
    fn action_type(v: adze::ffi::ParseActionType) -> adze::ffi::TSParseActionType {
        v
    }
    fn action_entry(v: adze::ffi::ParseActionEntry) -> adze::ffi::TSParseActionEntry {
        v
    }
    fn ffi_scanner(v: adze::ffi::ExternalScannerData) -> adze::ffi::TSExternalScannerData {
        v
    }
    fn lexer(v: adze::external_scanner_ffi::Lexer) -> adze::external_scanner_ffi::TSLexer {
        v
    }
    fn scanner_data(
        v: adze::external_scanner_ffi::ExternalScannerData,
    ) -> adze::external_scanner_ffi::TSExternalScannerData {
        v
    }
    fn lex_state(v: adze::pure_parser::LexState) -> adze::pure_parser::TSLexState {
        v
    }
    fn language(v: &adze::pure_parser::Language) -> &adze::pure_parser::TSLanguage {
        v
    }
    fn rule(v: adze::pure_parser::Rule) -> adze::pure_parser::TSRule {
        v
    }
    fn parse_action(v: adze::pure_parser::ParseAction) -> adze::pure_parser::TSParseAction {
        v
    }
    fn action_tag(v: adze::ts_format::ActionTag) -> adze::ts_format::TSActionTag {
        v
    }
    fn field_language(v: &adze::field_tree::Language) -> &adze::field_tree::TSLanguage {
        v
    }
    fn lex_mod_state(v: adze::lex::LexState) -> adze::lex::TSLexState {
        v
    }
    fn lex_adapter_state(
        v: adze::lex::ts_lexfn_adapter::LexState,
    ) -> adze::lex::ts_lexfn_adapter::TSLexState {
        v
    }
    // Reference the helpers so the identity signatures are checked.
    let _ = (
        metadata,
        action_type,
        action_entry,
        ffi_scanner,
        lexer,
        scanner_data,
        lex_state,
        language,
        rule,
        parse_action,
        action_tag,
        field_language,
        lex_mod_state,
        lex_adapter_state,
    );

    assert_eq!(
        size_of::<adze::ffi::SymbolMetadata>(),
        size_of::<adze::ffi::TSSymbolMetadata>()
    );
    assert_eq!(
        size_of::<adze::external_scanner_ffi::Lexer>(),
        size_of::<adze::external_scanner_ffi::TSLexer>()
    );
    assert_eq!(
        size_of::<adze::pure_parser::Language>(),
        size_of::<adze::pure_parser::TSLanguage>()
    );
    assert_eq!(
        size_of::<adze::ts_format::ActionTag>(),
        size_of::<adze::ts_format::TSActionTag>()
    );
}

#[test]
fn realistic_use_through_friendly_names_only() {
    let metadata = adze::ffi::SymbolMetadata {
        visible: true,
        named: true,
        supertype: false,
    };
    let entry = adze::ffi::ParseActionEntry {
        type_: adze::ffi::ParseActionType::Shift,
        state: 1,
        symbol: 2,
        child_count: 0,
        production_id: 0,
    };
    assert!(metadata.visible && entry.state == 1);

    let state = adze::pure_parser::LexState {
        lex_state: 3,
        external_lex_state: 4,
    };
    let rule = adze::pure_parser::Rule {
        lhs: 5,
        rhs_len: 2,
        _pad: 0,
    };
    let action = adze::pure_parser::ParseAction {
        action_type: adze::ts_format::ActionTag::Shift as u8,
        extra: 0,
        child_count: 1,
        dynamic_precedence: 0,
        symbol: 6,
    };
    assert_eq!(state.lex_state + rule.rhs_len as u16, 5);
    assert_eq!(action.symbol, 6);

    let language = adze::field_tree::Language {
        field_names: vec!["name"],
        symbol_names: vec!["identifier"],
        production_field_map: vec![],
    };
    assert_eq!(language.field_id_for_name("name"), Some(0));
    assert_eq!(language.symbol_name(0), "identifier");

    let mut parser = adze::pure_parser::Parser::new();
    let _ = &mut parser;
}
