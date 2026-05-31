use adze::parser_v4::ParseNode;
use adze::query::compile_query;
use adze::query::matcher_v2::QueryMatcher;
use adze_glr_core::SymbolMetadata;
use adze_ir::{Grammar, ProductionId, Rule, SymbolId};
use tree_sitter::StreamingIterator;
use tree_sitter_json as ts_json;
use tree_sitter_runtime_standard as tree_sitter;

const UPSTREAM_GRAMMAR: &str = "tree-sitter-json";
const UPSTREAM_CRATE_VERSION: &str = "tree-sitter-json 0.24.8";
const SOURCE: &str = r#"{"answer": 42}"#;
const QUERY: &str = r#"
(document
  (object
    (pair
      key: (string) @key
      value: (number) @value)))
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CaptureReceipt {
    name: String,
    kind: String,
    start_byte: usize,
    end_byte: usize,
    text: String,
}

fn receipt(
    name: impl Into<String>,
    kind: impl Into<String>,
    start_byte: usize,
    end_byte: usize,
    text: impl Into<String>,
) -> CaptureReceipt {
    CaptureReceipt {
        name: name.into(),
        kind: kind.into(),
        start_byte,
        end_byte,
        text: text.into(),
    }
}

fn upstream_json_tree() -> (tree_sitter::Language, tree_sitter::Tree) {
    let language: tree_sitter::Language = ts_json::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .expect("tree-sitter-json language loads");
    let tree = parser.parse(SOURCE, None).expect("JSON source parses");
    assert!(
        !tree.root_node().has_error(),
        "upstream {UPSTREAM_GRAMMAR} parse should be error-free"
    );
    (language, tree)
}

fn collect_upstream_captures(
    language: &tree_sitter::Language,
    tree: &tree_sitter::Tree,
) -> Vec<CaptureReceipt> {
    let query =
        tree_sitter::Query::new(language, QUERY).expect("supported upstream query should compile");
    let capture_names = query.capture_names();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), SOURCE.as_bytes());
    let mut captures = Vec::new();

    while let Some(query_match) = matches.next() {
        for capture in query_match.captures {
            let start_byte = capture.node.start_byte();
            let end_byte = capture.node.end_byte();
            captures.push(receipt(
                capture_names[capture.index as usize],
                capture.node.kind(),
                start_byte,
                end_byte,
                &SOURCE[start_byte..end_byte],
            ));
        }
    }

    captures
}

fn grammar_from_language(language: &tree_sitter::Language) -> Grammar {
    let mut grammar = Grammar::new("tree_sitter_json_upstream_differential".to_string());

    for id in 0..language.node_kind_count() {
        let symbol = SymbolId(id as u16);
        let Some(name) = language.node_kind_for_id(symbol.0) else {
            continue;
        };

        grammar.rule_names.insert(symbol, name.to_string());
        grammar.rules.entry(symbol).or_default().push(Rule {
            lhs: symbol,
            rhs: Vec::new(),
            precedence: None,
            associativity: None,
            fields: Vec::new(),
            production_id: ProductionId(symbol.0),
        });
    }

    grammar
}

fn metadata_from_language(language: &tree_sitter::Language) -> Vec<SymbolMetadata> {
    (0..language.node_kind_count())
        .map(|id| {
            let symbol = SymbolId(id as u16);
            SymbolMetadata {
                name: language
                    .node_kind_for_id(symbol.0)
                    .unwrap_or("<unknown>")
                    .to_string(),
                is_visible: language.node_kind_is_visible(symbol.0),
                is_named: language.node_kind_is_named(symbol.0),
                is_supertype: language.node_kind_is_supertype(symbol.0),
                is_terminal: false,
                is_extra: false,
                is_fragile: false,
                symbol_id: symbol,
            }
        })
        .collect()
}

fn parse_node_from_upstream(node: tree_sitter::Node<'_>) -> ParseNode {
    let symbol = SymbolId(node.kind_id());
    let mut children = Vec::new();

    for child_index in 0..node.child_count() {
        let child = node
            .child(child_index)
            .expect("tree-sitter child index should be valid");
        let mut child_node = parse_node_from_upstream(child);
        child_node.field_name = node
            .field_name_for_child(child_index as u32)
            .map(str::to_string);
        children.push(child_node);
    }

    ParseNode {
        symbol,
        symbol_id: symbol,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        field_name: None,
        alias_symbol_id: None,
        children,
    }
}

fn collect_adze_captures(
    language: &tree_sitter::Language,
    tree: &tree_sitter::Tree,
) -> Vec<CaptureReceipt> {
    let grammar = grammar_from_language(language);
    let metadata = metadata_from_language(language);
    let query =
        compile_query(QUERY, &grammar).expect("Adze query compiler accepts supported slice");
    let capture_names = query.capture_names();
    let root = parse_node_from_upstream(tree.root_node());
    let matcher = QueryMatcher::new(&query, SOURCE, &metadata);
    let mut captures = Vec::new();

    for query_match in matcher.matches(&root) {
        for capture in query_match.captures {
            let name = capture_names[capture.index as usize];
            captures.push(receipt(
                name,
                metadata[capture.node.symbol_id.0 as usize].name.as_str(),
                capture.node.start_byte,
                capture.node.end_byte,
                &SOURCE[capture.node.start_byte..capture.node.end_byte],
            ));
        }
    }

    captures
}

#[test]
fn json_supported_subset_captures_match_upstream_tree_sitter() {
    let (language, tree) = upstream_json_tree();
    let expected = vec![
        receipt("key", "string", 1, 9, r#""answer""#),
        receipt("value", "number", 11, 13, "42"),
    ];

    let upstream = collect_upstream_captures(&language, &tree);
    assert_eq!(
        upstream, expected,
        "{UPSTREAM_GRAMMAR} {UPSTREAM_CRATE_VERSION} receipt should stay explicit"
    );

    let adze = collect_adze_captures(&language, &tree);
    assert_eq!(
        adze, expected,
        "Adze captures should match the single supported-subset upstream receipt"
    );
}
