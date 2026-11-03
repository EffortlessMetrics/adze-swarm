use adze_tool::grammar_js::{GrammarJsConverter, from_json};
use serde_json::json;
fn main() {
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    println!("cargo::rustc-cfg=adze_unsafe_attrs");
    for (literal, leaf, text) in [
        (false, json!({"type":"PATTERN","value":"[a-z]+"}), "word"),
        (true, json!({"type":"STRING","value":"q"}), "q"),
    ] {
        for choice in [false, true] {
            let name = format!(
                "imported_{}_{}",
                if literal { "literal" } else { "pattern" },
                if choice { "choice" } else { "sequence" }
            );
            let reference = json!({"type":"SYMBOL","name":"leaf"});
            let entry = if choice {
                json!({"type":"CHOICE","members":[reference,{"type":"STRING","value":"!"}]})
            } else {
                json!({"type":"SEQ","members":[reference]})
            };
            let value = json!({"name":name,"start_symbol":"source_file","rules":{"source_file":entry,"leaf":leaf}});
            let grammar = GrammarJsConverter::new(from_json(&value).unwrap())
                .convert()
                .unwrap();
            let start = grammar.explicit_start_symbol().unwrap();
            let wrapper = grammar.find_symbol_by_name("leaf").unwrap();
            let binding = grammar.wrapper_token_for(wrapper).unwrap();
            let result =
                adze_tool::build_parser(grammar, adze_tool::BuildOptions::default()).unwrap();
            let code = format!(
                "const GRAMMAR_NAME: &str = {name:?}; pub const BINDING: u16 = {}; pub const WRAPPER: u16 = {}; pub const START: u16 = {}; pub const SOURCE: &str = {text:?}; {}",
                binding.0, wrapper.0, start.0, result.parser_code
            );
            std::fs::write(
                std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join(format!("{name}.rs")),
                code,
            )
            .unwrap();
        }
    }
    use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
    let mut sparse = Grammar::new("sparse_entry".into());
    for (id, name, text) in [(99, "real_token", "x"), (150, "helper_token", "y")] {
        sparse.tokens.insert(
            SymbolId(id),
            Token {
                name: name.into(),
                pattern: TokenPattern::String(text.into()),
                fragile: false,
            },
        );
    }
    for (index, owner, token, name) in [(0, 2, 150, "earlier_helper"), (1, 17, 99, "renamed_entry")]
    {
        sparse.rule_names.insert(SymbolId(owner), name.into());
        sparse.add_rule(Rule {
            lhs: SymbolId(owner),
            rhs: vec![Symbol::Terminal(SymbolId(token))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(index),
        });
    }
    sparse.set_start_symbol(SymbolId(17));
    let result = adze_tool::build_parser(sparse, adze_tool::BuildOptions::default()).unwrap();
    std::fs::write(
        std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("sparse.rs"),
        format!(
            "const GRAMMAR_NAME: &str = \"sparse_entry\"; {}",
            result.parser_code
        ),
    )
    .unwrap();
    let mut ambiguous = Grammar::new("sparse_ambiguous".into());
    for (id, name, text) in [(99, "x", "x"), (150, "y", "y")] {
        ambiguous.tokens.insert(
            SymbolId(id),
            Token {
                name: name.into(),
                pattern: TokenPattern::String(text.into()),
                fragile: false,
            },
        );
    }
    for (production, owner, token, name) in [
        (0, 2, 150, "earlier_helper"),
        (1, 17, 99, "renamed_entry"),
        (2, 17, 99, "renamed_entry"),
    ] {
        ambiguous.rule_names.insert(SymbolId(owner), name.into());
        ambiguous.add_rule(Rule {
            lhs: SymbolId(owner),
            rhs: vec![Symbol::Terminal(SymbolId(token))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(production),
        });
    }
    ambiguous.set_start_symbol(SymbolId(17));
    let result = adze_tool::build_parser(ambiguous, adze_tool::BuildOptions::default()).unwrap();
    std::fs::write(
        std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("sparse_ambiguous.rs"),
        format!(
            "const GRAMMAR_NAME: &str = \"sparse_ambiguous\"; {}",
            result.parser_code
        ),
    )
    .unwrap();
    adze_tool::build_parser_for_crate(
        std::path::Path::new("src/lib.rs"),
        adze_tool::BuildOptions::default(),
    )
    .unwrap();
}
