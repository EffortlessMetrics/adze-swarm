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
    let mut aliased = grammar.clone();
    aliased.name = "schema_hidden_alias_consumer".into();
    // The current native alias ABI refers to a registered symbol identity.
    // This token is never licensed by a parser action; it supplies only the
    // independently declared visible alias kind.
    aliased.tokens.insert(
        SymbolId(3),
        Token {
            name: "gap".into(),
            pattern: TokenPattern::String("unused_alias".into()),
            fragile: false,
        },
    );
    let (&helper, origin) = aliased.optional_origins.iter().next().unwrap();
    let present_production = aliased.rules[&helper][origin.present_alternative].production_id;
    aliased.alias_sequences.insert(
        present_production,
        adze_ir::AliasSequence {
            aliases: vec![Some("gap".into())],
        },
    );
    let alias_result = adze_tool::pure_rust_builder::build_parser(
        aliased,
        adze_tool::pure_rust_builder::BuildOptions::default(),
    )
    .expect("same canonical optional origin with a visible alias");
    std::fs::write(
        std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("generated_alias.rs"),
        alias_result.parser_code,
    )
    .unwrap();
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
}
