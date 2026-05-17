use adze::parser_v4::ParseNode;
use adze::query::compile_query;
use adze::query::matcher_v2::QueryMatcher;
use adze_glr_core::SymbolMetadata;
use adze_ir::{Grammar, ProductionId, Rule, SymbolId, Token, TokenPattern};

#[derive(Clone)]
struct QueryFixture {
    id: &'static str,
    query: &'static str,
    source: &'static str,
    tree: ParseNode,
    expected_capture_starts: Vec<Vec<usize>>,
}

fn node(symbol: u16, start_byte: usize, end_byte: usize) -> ParseNode {
    let symbol_id = SymbolId(symbol);
    ParseNode {
        symbol: symbol_id,
        symbol_id,
        children: Vec::new(),
        start_byte,
        end_byte,
        field_name: None,
        alias_symbol_id: None,
    }
}

fn field_node(symbol: u16, start_byte: usize, end_byte: usize, field_name: &str) -> ParseNode {
    let mut node = node(symbol, start_byte, end_byte);
    node.field_name = Some(field_name.to_string());
    node
}

fn root(children: Vec<ParseNode>, end_byte: usize) -> ParseNode {
    ParseNode {
        symbol: SymbolId(0),
        symbol_id: SymbolId(0),
        children,
        start_byte: 0,
        end_byte,
        field_name: None,
        alias_symbol_id: None,
    }
}

fn grammar() -> Grammar {
    let mut grammar = Grammar::new("query_fixture".to_string());

    grammar.rules.entry(SymbolId(0)).or_default().push(Rule {
        lhs: SymbolId(0),
        rhs: Vec::new(),
        precedence: None,
        associativity: None,
        fields: Vec::new(),
        production_id: ProductionId(0),
    });
    grammar.rule_names.insert(SymbolId(0), "root".to_string());

    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "identifier".to_string(),
            pattern: TokenPattern::Regex("[a-zA-Z_]+".to_string()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        SymbolId(2),
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex("[0-9]+".to_string()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        SymbolId(3),
        Token {
            name: "operator".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    grammar
}

fn metadata() -> Vec<SymbolMetadata> {
    vec![
        SymbolMetadata {
            name: "root".to_string(),
            is_visible: true,
            is_named: true,
            is_supertype: false,
            is_terminal: false,
            is_extra: false,
            is_fragile: false,
            symbol_id: SymbolId(0),
        },
        SymbolMetadata {
            name: "identifier".to_string(),
            is_visible: true,
            is_named: true,
            is_supertype: false,
            is_terminal: true,
            is_extra: false,
            is_fragile: false,
            symbol_id: SymbolId(1),
        },
        SymbolMetadata {
            name: "number".to_string(),
            is_visible: true,
            is_named: true,
            is_supertype: false,
            is_terminal: true,
            is_extra: false,
            is_fragile: false,
            symbol_id: SymbolId(2),
        },
        SymbolMetadata {
            name: "operator".to_string(),
            is_visible: true,
            is_named: true,
            is_supertype: false,
            is_terminal: true,
            is_extra: false,
            is_fragile: false,
            symbol_id: SymbolId(3),
        },
    ]
}

fn fixtures() -> Vec<QueryFixture> {
    vec![
        QueryFixture {
            id: "named-node-subsequence",
            query: "(root (identifier @id))",
            source: "1 foo",
            tree: root(vec![node(2, 0, 1), node(1, 2, 5)], 5),
            expected_capture_starts: vec![vec![2]],
        },
        QueryFixture {
            id: "field-constraint",
            query: "(root left: (identifier @id))",
            source: "1 foo",
            tree: root(
                vec![field_node(2, 0, 1, "right"), field_node(1, 2, 5, "left")],
                5,
            ),
            expected_capture_starts: vec![vec![2]],
        },
        QueryFixture {
            id: "first-child-anchor-negative",
            query: "(root . (identifier @id))",
            source: "1 foo",
            tree: root(vec![node(2, 0, 1), node(1, 2, 5)], 5),
            expected_capture_starts: Vec::new(),
        },
        QueryFixture {
            id: "last-child-anchor",
            query: "(root (identifier @id) .)",
            source: "1 foo",
            tree: root(vec![node(2, 0, 1), node(1, 2, 5)], 5),
            expected_capture_starts: vec![vec![2]],
        },
        QueryFixture {
            id: "adjacent-anchor",
            query: "(root (identifier @lhs) . (operator @op))",
            source: "a+1",
            tree: root(vec![node(1, 0, 1), node(3, 1, 2), node(2, 2, 3)], 3),
            expected_capture_starts: vec![vec![0, 1]],
        },
        QueryFixture {
            id: "source-aware-predicate",
            query: "(root (identifier @id))\n(#match? @id \"^[a-z]+$\")",
            source: "foo 1",
            tree: root(vec![node(1, 0, 3), node(2, 4, 5)], 5),
            expected_capture_starts: vec![vec![0]],
        },
    ]
}

#[test]
fn supported_query_subset_matches_tree_sitter_shaped_fixture_expectations() {
    let grammar = grammar();
    let metadata = metadata();

    for fixture in fixtures() {
        let query = compile_query(fixture.query, &grammar)
            .unwrap_or_else(|err| panic!("{}: {err}", fixture.id));
        let matcher = QueryMatcher::new(&query, fixture.source, &metadata);
        let matches = matcher.matches(&fixture.tree);

        let capture_starts: Vec<Vec<usize>> = matches
            .iter()
            .map(|query_match| {
                query_match
                    .captures
                    .iter()
                    .map(|capture| capture.node.start_byte)
                    .collect()
            })
            .collect();

        assert_eq!(
            capture_starts, fixture.expected_capture_starts,
            "{} should match the documented supported query subset",
            fixture.id
        );
    }
}
