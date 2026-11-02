use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

pub fn grammar() -> Grammar {
    let mut grammar = Grammar::new("mode_lookahead".into());
    grammar.set_start_symbol(SymbolId(10));
    for (id, name, pattern) in [
        (1, "word", TokenPattern::Regex(r"\w+".into())),
        (2, "number", TokenPattern::Regex(r"\d+".into())),
        (3, "a", TokenPattern::String("a".into())),
        (4, "b", TokenPattern::String("b".into())),
    ] {
        grammar.tokens.insert(
            SymbolId(id),
            Token {
                name: name.into(),
                pattern,
                fragile: false,
            },
        );
    }
    grammar
        .rule_names
        .insert(SymbolId(10), "source_file".into());
    grammar.rule_names.insert(SymbolId(11), "prefix".into());
    use Symbol::{NonTerminal as n, Terminal as t};
    for (index, (lhs, rhs)) in [
        (10, vec![t(SymbolId(3)), t(SymbolId(2))]),
        (10, vec![t(SymbolId(4)), t(SymbolId(2))]),
        (10, vec![n(SymbolId(11)), t(SymbolId(1))]),
        (11, vec![t(SymbolId(4))]),
    ]
    .into_iter()
    .enumerate()
    {
        grammar.add_rule(Rule {
            lhs: SymbolId(lhs),
            rhs,
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(index as u16),
        });
    }
    grammar
}
