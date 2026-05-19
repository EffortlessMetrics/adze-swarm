//! Imported-shape smoke coverage for Tree-sitter compatibility.
//!
//! This is intentionally smaller than the selected-tree parity matrix. It keeps
//! common imported-grammar shapes visible in one place so adoption work can see
//! which shapes are covered and which remain support-tier gaps.

#![cfg(all(test, feature = "ts-compat", feature = "pure-rust", feature = "query"))]

use adze::{
    adze_glr_core::SymbolMetadata,
    adze_ir::{
        ExternalToken, FieldId, Grammar, ProductionId, Rule, RuleId, SymbolId, Token, TokenPattern,
    },
    parser_v4::ParseNode,
    query::{QueryCursor, compile_query},
    ts_compat::{Language, Node, Parser},
};
use std::sync::Arc;

fn symbol_named(lang: &Language, name: &str) -> SymbolId {
    let index = lang
        .table
        .symbol_metadata
        .iter()
        .position(|metadata| metadata.name == name)
        .unwrap_or_else(|| panic!("arithmetic fixture should expose {name:?}"));
    let symbol = lang.table.symbol_metadata[index].symbol_id;
    assert_eq!(
        symbol.0 as usize, index,
        "fixture metadata should be indexed by symbol id"
    );
    symbol
}

fn push_symbol(
    lang: &mut Language,
    name: &str,
    is_visible: bool,
    is_named: bool,
    is_terminal: bool,
    is_extra: bool,
) -> SymbolId {
    let symbol = SymbolId(lang.table.symbol_metadata.len() as u16);
    lang.table.symbol_metadata.push(SymbolMetadata {
        name: name.to_string(),
        is_visible,
        is_named,
        is_supertype: false,
        is_terminal,
        is_extra,
        is_fragile: false,
        symbol_id: symbol,
    });
    lang.table.symbol_count = lang
        .table
        .symbol_count
        .max(lang.table.symbol_metadata.len());
    lang.table
        .index_to_symbol
        .resize(lang.table.symbol_metadata.len(), SymbolId(0));
    lang.table.index_to_symbol[symbol.0 as usize] = symbol;
    symbol
}

fn imported_shape_language() -> (Language, SymbolId, SymbolId, SymbolId, SymbolId) {
    let mut lang = (*adze_example::ts_langs::arithmetic()).clone();
    let source_file = symbol_named(&lang, "source_file");
    let expression = symbol_named(&lang, "expression");
    let operator = symbol_named(&lang, "-");

    for (index, name) in ["left", "operator", "right"].into_iter().enumerate() {
        let field_id = FieldId(index as u16);
        lang.grammar.fields.insert(field_id, name.to_string());
        lang.table.field_names.push(name.to_string());
        lang.table
            .field_map
            .insert((RuleId(2), index as u16), index as u16);
    }

    let subtraction_rule = lang
        .grammar
        .rules
        .get_mut(&expression)
        .and_then(|rules| {
            rules
                .iter_mut()
                .find(|rule| rule.production_id == ProductionId(2))
        })
        .expect("arithmetic fixture should expose subtraction expression rule");
    subtraction_rule.fields = vec![(FieldId(0), 0), (FieldId(1), 1), (FieldId(2), 2)];

    let hidden = push_symbol(&mut lang, "_hidden_expression", false, true, false, false);
    let outer_alias = push_symbol(&mut lang, "binary_expression", true, true, false, false);
    let inner_alias = push_symbol(&mut lang, "literal_expression", true, true, false, false);
    lang.table.symbol_metadata[operator.0 as usize].is_extra = true;

    let source_file_rule = lang
        .table
        .rules
        .iter()
        .position(|rule| rule.lhs == source_file && rule.rhs_len == 1)
        .expect("arithmetic fixture should reduce source_file from expression");
    let binary_rule = RuleId(2).0 as usize;
    lang.table
        .alias_sequences
        .resize_with(binary_rule.max(source_file_rule) + 1, Vec::new);
    lang.table.alias_sequences[source_file_rule] = vec![Some(outer_alias)];
    lang.table.alias_sequences[binary_rule] = vec![Some(inner_alias), None, Some(inner_alias)];

    (lang, expression, hidden, outer_alias, inner_alias)
}

fn language_with_external_token_shape() -> (Language, SymbolId) {
    let mut lang = (*adze_example::ts_langs::arithmetic()).clone();
    let indent = push_symbol(&mut lang, "indent", true, true, true, false);
    lang.table.external_token_count = 1;
    lang.grammar.externals.push(ExternalToken {
        name: "indent".to_string(),
        symbol_id: indent,
    });
    (lang, indent)
}

fn collect_missing_nodes<'tree>(node: Node<'tree>, missing: &mut Vec<Node<'tree>>) {
    if node.is_missing() {
        missing.push(node.clone());
    }

    for index in 0..node.child_count() {
        let child = node.child(index).expect("child index should be valid");
        collect_missing_nodes(child, missing);
    }
}

#[test]
fn imported_shape_smoke_covers_aliases_fields_hidden_anonymous_extra_and_missing_nodes() {
    let (lang, expression_symbol, hidden_symbol, outer_alias, inner_alias) =
        imported_shape_language();
    let lang = Arc::new(lang);
    let mut parser = Parser::new();
    parser
        .set_language(Arc::clone(&lang))
        .expect("language should install");

    assert_eq!(
        lang.node_kind_for_id(hidden_symbol.0),
        Some("_hidden_expression")
    );
    assert!(!lang.node_kind_is_visible(hidden_symbol.0));
    assert!(lang.node_kind_is_named(hidden_symbol.0));

    let source = "1-2";
    let tree = parser.parse(source, None).expect("fixture should parse");
    let root = tree.root_node();
    let expression = root
        .child(0)
        .expect("root should expose aliased expression child");
    let left = expression
        .child_by_field_name("left")
        .expect("left field should resolve");
    let operator = expression
        .child_by_field_name("operator")
        .expect("operator field should resolve");
    let right = expression
        .child_by_field_name("right")
        .expect("right field should resolve");

    assert_eq!(expression.kind(), "binary_expression");
    assert_eq!(expression.kind_id(), outer_alias.0);
    assert_eq!(expression.grammar_name(), "expression");
    assert_eq!(expression.grammar_id(), expression_symbol.0);
    assert_eq!(left.kind(), "literal_expression");
    assert_eq!(left.kind_id(), inner_alias.0);
    assert_eq!(right.kind(), "literal_expression");
    assert_eq!(right.kind_id(), inner_alias.0);

    assert_eq!(operator.kind(), "-");
    assert!(!operator.is_named(), "operator is an anonymous token shape");
    assert!(
        operator.is_extra(),
        "operator was marked as an extra token shape"
    );
    assert_eq!(operator.text(source.as_bytes()), "-");
    assert_eq!(expression.field_id_for_child(1).map(|id| id.get()), Some(2));

    let recovered = parser
        .parse("1-", None)
        .expect("truncated input should still return an inspectable tree");
    let recovered_root = recovered.root_node();
    assert!(recovered.has_errors());
    assert!(recovered_root.has_error());

    let mut missing = Vec::new();
    collect_missing_nodes(recovered_root, &mut missing);
    assert!(
        !missing.is_empty(),
        "truncated imported-shape fixture should expose missing nodes"
    );
}

#[test]
fn imported_shape_smoke_records_external_token_metadata() {
    let (lang, indent) = language_with_external_token_shape();

    assert_eq!(lang.table.external_token_count, 1);
    assert_eq!(lang.grammar.externals.len(), 1);
    assert_eq!(lang.grammar.externals[0].name, "indent");
    assert_eq!(lang.grammar.externals[0].symbol_id, indent);
    assert_eq!(lang.node_kind_for_id(indent.0), Some("indent"));
    assert!(lang.node_kind_is_visible(indent.0));
    assert!(lang.node_kind_is_named(indent.0));
}

#[test]
fn imported_shape_smoke_covers_query_captures() {
    let grammar = query_shape_grammar();
    let tree = query_root(
        vec![
            query_field_node(1, 0, 3, "left"),
            query_node(3, 3, 4),
            query_field_node(2, 4, 5, "right"),
        ],
        5,
    );
    let query = compile_query(
        "(root left: (identifier @name) (operator @operator) right: (number @value))",
        &grammar,
    )
    .expect("query shape should compile");

    let mut cursor = QueryCursor::new();
    let matches = cursor.collect_matches(&query, &tree);
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0]
            .captures
            .iter()
            .map(|capture| (
                capture.index,
                capture.node.start_byte,
                capture.node.end_byte
            ))
            .collect::<Vec<_>>(),
        vec![(0, 0, 3), (1, 3, 4), (2, 4, 5)]
    );
}

fn query_node(symbol: u16, start_byte: usize, end_byte: usize) -> ParseNode {
    let symbol = SymbolId(symbol);
    ParseNode {
        symbol,
        symbol_id: symbol,
        children: Vec::new(),
        start_byte,
        end_byte,
        field_name: None,
        alias_symbol_id: None,
    }
}

fn query_field_node(
    symbol: u16,
    start_byte: usize,
    end_byte: usize,
    field_name: &str,
) -> ParseNode {
    let mut node = query_node(symbol, start_byte, end_byte);
    node.field_name = Some(field_name.to_string());
    node
}

fn query_root(children: Vec<ParseNode>, end_byte: usize) -> ParseNode {
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

fn query_shape_grammar() -> Grammar {
    let mut grammar = Grammar::new("imported_shape_query_smoke".to_string());
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
