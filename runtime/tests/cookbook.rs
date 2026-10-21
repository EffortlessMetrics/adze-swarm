//! Tested cookbook recipes for the GLR toolkit productization lane.

#![cfg(all(test, feature = "pure-rust", feature = "glr", feature = "ts-compat"))]

use adze::{
    adze_glr_core::SymbolMetadata,
    adze_ir::{Grammar, ProductionId, Rule, SymbolId, Token, TokenPattern},
    document::SyntaxNode,
    parser_v4::ParseNode,
    query::{compile_query, matcher_v2::QueryMatcher},
    ts_compat::Parser as TsParser,
};

#[test]
fn cookbook_typed_parser_recipes_cover_common_grammar_shapes() {
    use adze_example::fielded_precedence_typed_cst_contract::grammar::{self, Expr};

    let expr = grammar::parse("1 + 2 * 3").expect("fielded arithmetic recipe should parse");
    assert_eq!(
        expr,
        Expr::Add {
            left: Box::new(Expr::Number(1)),
            operator: (),
            right: Box::new(Expr::Mul {
                left: Box::new(Expr::Number(2)),
                operator: (),
                right: Box::new(Expr::Number(3)),
            }),
        }
    );

    adze_example::csv_list::grammar::parse("alpha")
        .expect("list-shape recipe should parse one identifier");
    adze_example::object_like_contract::grammar::parse("{ answer: 42 }")
        .expect("object-like recipe should parse a keyed value");
}

#[test]
fn cookbook_document_and_diagnostic_recipes_cover_tooling_path() {
    use adze_example::fielded_precedence_typed_cst_contract::grammar::{self, Expr};

    let source = "1+2*3";
    let document = grammar::parse_document(source)
        .expect("parse_document recipe should return an inspectable document");
    let document_ast: Expr = document
        .ast()
        .expect("document AST projection should succeed");
    assert_eq!(
        document_ast,
        grammar::parse(source).expect("typed parser recipe should parse")
    );

    let root = grammar::syntax::source_file(&document)
        .expect("typed CST recipe should cast the document root");
    assert_eq!(root.text(), Some(source));
    assert!(root.child(0).is_some());

    let diagnostic_document = grammar::parse_document("1 +")
        .expect("recoverable bad input should still produce document facts");
    let diagnostic = diagnostic_document
        .diagnostics()
        .first()
        .expect("diagnostic recipe should expose a structured diagnostic");
    assert_eq!(diagnostic.byte_span(), 3..3);
    assert!(
        diagnostic.expected.iter().any(|token| token == "number"),
        "diagnostic recipe should expose expected-token names: {:?}",
        diagnostic.expected
    );
}

#[test]
fn cookbook_document_status_range_and_slice_recipe_distinguishes_clean_and_recovered_input() {
    use adze_example::fielded_precedence_typed_cst_contract::grammar;

    // There is no complete/recovered/failed status enum on the document;
    // the supported proxy is diagnostics plus tree error facts.
    let clean_source = "1+2*3";
    let clean =
        grammar::parse_document(clean_source).expect("clean input should return a document");
    assert!(clean.diagnostics().is_empty());
    assert!(!clean.tree().has_errors());
    assert_eq!(clean.metadata().error_count, 0);
    assert_eq!(
        clean.source_slice(clean.tree().root().byte_range()),
        Some(clean_source),
        "the root range should slice back to the full source"
    );

    let recovered =
        grammar::parse_document("1 +").expect("recovered input should still return a document");
    assert!(!recovered.diagnostics().is_empty());
    assert!(recovered.tree().has_errors());
    assert!(recovered.metadata().error_count > 0);
}

#[test]
fn cookbook_typed_cst_fields_stay_on_document_recipe() {
    use adze_example::fielded_precedence_typed_cst_contract::grammar::{self, Expr};

    let source = "1+2";
    let document = grammar::parse_document(source).expect("document should parse");
    let add = find_kind(document.tree().root(), "Expr_Add")
        .expect("generic document tree should contain the Add node");
    let left = add
        .edge_by_field_name("left")
        .and_then(|edge| edge.child())
        .expect("Add node should expose a left field edge");
    let right = add
        .edge_by_field_name("right")
        .and_then(|edge| edge.child())
        .expect("Add node should expose a right field edge");
    assert_eq!(left.utf8_text().ok(), Some("1"));
    assert_eq!(right.utf8_text().ok(), Some("2"));

    let typed = grammar::parse(source).expect("typed parser should parse");
    assert_eq!(
        typed,
        Expr::Add {
            left: Box::new(Expr::Number(1)),
            operator: (),
            right: Box::new(Expr::Number(2)),
        },
        "generic field edges and the typed AST should describe the same fields"
    );
}

#[cfg(feature = "serialization")]
#[test]
fn cookbook_document_json_projection_uses_pinned_schema_recipe() {
    use adze::document::ADZE_DOCUMENT_JSON_SCHEMA;
    use adze_example::fielded_precedence_typed_cst_contract::grammar;

    let document = grammar::parse_document("1+2").expect("document should parse");
    let json = document.to_json_value();
    assert_eq!(
        json["schema"].as_str(),
        Some(ADZE_DOCUMENT_JSON_SCHEMA),
        "JSON projection should carry the pinned experimental schema id"
    );
}

fn find_kind<'doc>(
    node: adze::document::AdzeNode<'doc>,
    kind_name: &str,
) -> Option<adze::document::AdzeNode<'doc>> {
    if node.kind_name() == Some(kind_name) {
        return Some(node);
    }
    let mut index = 0;
    while let Some(child) = node.child(index) {
        if let Some(found) = find_kind(child, kind_name) {
            return Some(found);
        }
        index += 1;
    }
    None
}

#[test]
fn cookbook_glr_and_ts_compat_recipes_cover_selected_tree_and_ambiguity() {
    use adze_example::ambiguous_expr::grammar::{self, Expr};

    let source = "1 + 2 + 3";
    let selected_ast =
        grammar::parse(source).expect("ambiguous GLR recipe should produce a selected typed AST");
    let document = grammar::parse_document(source)
        .expect("ambiguous GLR recipe should return a selected document");
    let document_ast: Expr = document
        .ast()
        .expect("selected document tree should extract the same typed AST");
    assert_eq!(
        selected_ast, document_ast,
        "parse() and parse_document().ast() should agree on the selected tree"
    );

    let ambiguity = document
        .ambiguities()
        .first()
        .expect("ambiguous GLR recipe should expose an ambiguity summary");
    assert!(
        ambiguity.alternatives.len() > 1,
        "ambiguity summary should retain multiple alternatives: {ambiguity:?}"
    );

    let mut parser = TsParser::new();
    parser
        .set_language(adze_example::ts_langs::arithmetic())
        .expect("Tree-sitter compatibility recipe should set a language");
    let tree = parser
        .parse("1-2", None)
        .expect("Tree-sitter compatibility recipe should parse selected tree");
    assert_eq!(tree.root_node().kind(), "source_file");
    assert_eq!(
        tree.root_node().to_sexp(),
        "(source_file (expression (expression) (expression)))"
    );
}

#[test]
fn cookbook_query_capture_recipe_matches_identifier_nodes() {
    let grammar = query_fixture_grammar();
    let metadata = query_fixture_metadata();
    let query = compile_query("(root (identifier @name))", &grammar)
        .expect("query capture recipe should compile");
    let tree = ParseNode {
        symbol: SymbolId(0),
        symbol_id: SymbolId(0),
        children: vec![node(2, 0, 1), node(1, 2, 5)],
        start_byte: 0,
        end_byte: 5,
        field_name: None,
        alias_symbol_id: None,
    };
    let matches = QueryMatcher::new(&query, "1 foo", &metadata).matches(&tree);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].captures.len(), 1);
    assert_eq!(query.capture_names(), vec!["name"]);
    assert_eq!(matches[0].captures[0].index, 0);
    assert_eq!(matches[0].captures[0].node.start_byte, 2);
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

fn query_fixture_grammar() -> Grammar {
    let mut grammar = Grammar::new("cookbook_query_fixture".to_string());
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
    grammar
}

fn query_fixture_metadata() -> Vec<SymbolMetadata> {
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
    ]
}
