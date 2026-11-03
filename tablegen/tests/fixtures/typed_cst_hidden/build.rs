use adze_ir::{FieldId, Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

fn main() {
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    println!("cargo::rustc-cfg=adze_unsafe_attrs");
    let mut grammar = Grammar::new("schema_hidden_consumer".into());
    grammar.start_symbol = Some(SymbolId(10));
    for (id, name, pattern) in [(1, "word", "[a-z]+"), (2, "_sep", r"\s+")] {
        grammar.tokens.insert(
            SymbolId(id),
            Token {
                name: name.into(),
                pattern: TokenPattern::Regex(pattern.into()),
                fragile: false,
            },
        );
    }
    grammar
        .rule_names
        .insert(SymbolId(10), "source_file".into());
    grammar.rule_names.insert(SymbolId(11), "item".into());
    grammar.fields.insert(FieldId(1), "value".into());
    grammar.add_rule(Rule {
        lhs: SymbolId(10),
        rhs: vec![
            Symbol::NonTerminal(SymbolId(11)),
            Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(2)))),
        ],
        precedence: None,
        associativity: None,
        fields: vec![(FieldId(1), 0), (FieldId(1), 1)],
        production_id: ProductionId(0),
    });
    grammar.add_rule(Rule {
        lhs: SymbolId(11),
        rhs: vec![Symbol::Terminal(SymbolId(1))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });
    // Exercise the helper-mediated target that the old raw-RHS walk missed.
    grammar.normalize();
    let result = adze_tool::pure_rust_builder::build_parser(
        grammar,
        adze_tool::pure_rust_builder::BuildOptions::default(),
    )
    .expect("normalized hidden-target parser generation");
    std::fs::write(
        std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("generated.rs"),
        result.parser_code,
    )
    .unwrap();
    let mut foreign = Grammar::new("schema_hidden_consumer".into());
    foreign.start_symbol = Some(SymbolId(10));
    foreign.tokens.insert(
        SymbolId(2),
        Token {
            name: "foreign_token".into(),
            pattern: TokenPattern::String("z".into()),
            fragile: false,
        },
    );
    foreign
        .rule_names
        .insert(SymbolId(10), "source_file".into());
    foreign.add_rule(Rule {
        lhs: SymbolId(10),
        rhs: vec![Symbol::Terminal(SymbolId(2))],
        fields: vec![],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
    });
    let generated = adze_tool::pure_rust_builder::build_parser(
        foreign,
        adze_tool::pure_rust_builder::BuildOptions::default(),
    )
    .unwrap();
    std::fs::write(
        std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("foreign_generated.rs"),
        generated.parser_code,
    )
    .unwrap();
}
