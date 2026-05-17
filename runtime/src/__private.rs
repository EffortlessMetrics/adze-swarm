//! # DO NOT USE THIS MODULE!
//!
//! This module contains functions for use in the expanded macros produced by adze.
//! They need to be public so they can be accessed at all (\*cough\* macro hygiene), but
//! they are not intended to actually be called in any other circumstance.

use crate::Extract;
#[cfg(feature = "pure-rust")]
use core::ffi::{CStr, c_char};

#[cfg(feature = "pure-rust")]
use crate::pure_parser::ParsedNode;
#[cfg(not(feature = "pure-rust"))]
use crate::tree_sitter;

#[cfg(feature = "pure-rust")]
/// A cursor for navigating parsed nodes in pure-rust mode
pub struct TreeCursor<'a> {
    node: &'a ParsedNode,
    children: &'a [ParsedNode],
    current_index: usize,
}

#[cfg(feature = "pure-rust")]
impl<'a> TreeCursor<'a> {
    /// Creates a new tree cursor for the given node.
    pub fn new(node: &'a ParsedNode) -> Self {
        Self {
            node,
            children: &node.children,
            current_index: 0,
        }
    }

    /// Moves the cursor to the first child node.
    pub fn goto_first_child(&mut self) -> bool {
        if !self.children.is_empty() {
            self.current_index = 0;
            true
        } else {
            false
        }
    }

    /// Moves the cursor to the next sibling node.
    pub fn goto_next_sibling(&mut self) -> bool {
        if self.current_index + 1 < self.children.len() {
            self.current_index += 1;
            true
        } else {
            false
        }
    }

    /// Returns the current node.
    pub fn node(&self) -> &'a ParsedNode {
        if self.current_index < self.children.len() {
            &self.children[self.current_index]
        } else {
            self.node
        }
    }

    /// Returns the field name for the current child node if available
    pub fn field_name(&self) -> Option<&'static str> {
        if self.current_index >= self.children.len() {
            return None;
        }
        let child = &self.children[self.current_index];
        let field_id = child.field_id?;
        let lang_ptr = self.node.language?;
        // SAFETY: `lang_ptr` is obtained from a valid `ParsedNode` whose lifetime
        // outlives this iterator. The field_id is bounds-checked below.
        unsafe {
            let lang = &*lang_ptr;
            if field_id >= lang.field_count as u16 {
                return None;
            }
            if lang.field_names.is_null() {
                return None;
            }
            let field_names =
                core::slice::from_raw_parts(lang.field_names, lang.field_count as usize);
            let name_ptr = field_names[field_id as usize];
            if name_ptr.is_null() {
                return None;
            }
            CStr::from_ptr(name_ptr as *const c_char).to_str().ok()
        }
    }
}

#[cfg(not(feature = "pure-rust"))]
pub fn extract_struct_or_variant<T>(
    node: tree_sitter::Node,
    construct_expr: impl Fn(&mut Option<tree_sitter::TreeCursor>, &mut usize) -> T,
) -> T {
    let mut parent_cursor = node.walk();
    let has_child = parent_cursor.goto_first_child();

    let mut cursor_opt = if has_child { Some(parent_cursor) } else { None };

    // If the node has only one child and it's a wrapper, we might need to go deeper
    // But Tree-sitter cursors usually point to the immediate children.
    // The issue is likely that 'Program' kind is being matched instead of its fields.

    construct_expr(&mut cursor_opt, &mut node.start_byte())
}

/// Extracts a struct or variant from a parsed node.
#[cfg(feature = "pure-rust")]
pub fn extract_struct_or_variant<T>(
    node: &ParsedNode,
    construct_expr: impl Fn(&mut Option<TreeCursor>, &mut usize) -> T,
) -> T {
    // Debug output commented out
    // eprintln!("DEBUG extract_struct_or_variant: node.symbol={}, children={}", node.symbol, node.children.len());
    // for (i, child) in node.children.iter().enumerate() {
    //     eprintln!("  child[{}]: symbol={}, field_name={:?}", i, child.symbol, child.field_name);
    // }

    let mut cursor = TreeCursor::new(node);
    let mut cursor_opt = if cursor.goto_first_child() {
        Some(cursor)
    } else {
        None
    };
    let mut start_byte = node.start_byte;
    construct_expr(&mut cursor_opt, &mut start_byte)
}

#[cfg(not(feature = "pure-rust"))]
pub fn extract_field<LT: Extract<T>, T>(
    cursor_opt: &mut Option<tree_sitter::TreeCursor>,
    source: &[u8],
    last_idx: &mut usize,
    field_name: &str,
    closure_ref: Option<&LT::LeafFn>,
) -> T {
    if let Some(cursor) = cursor_opt.as_mut() {
        loop {
            let n = cursor.node();
            let name = cursor.field_name();
            if name == Some(field_name) || (name.is_none() && n.kind() == field_name) {
                let out = LT::extract(Some(n), source, *last_idx, closure_ref);

                if !cursor.goto_next_sibling() {
                    *cursor_opt = None;
                };

                *last_idx = n.end_byte();

                return out;
            } else if name.is_some() {
                return LT::extract(None, source, *last_idx, closure_ref);
            } else {
                // If it's an anonymous node, skip it and continue
                *last_idx = n.end_byte();
            }

            if !cursor.goto_next_sibling() {
                return LT::extract(None, source, *last_idx, closure_ref);
            }
        }
    } else {
        LT::extract(None, source, *last_idx, closure_ref)
    }
}

/// Extracts a field from the current position in the tree.
#[cfg(feature = "pure-rust")]
pub fn extract_field<LT: Extract<T>, T>(
    cursor_opt: &mut Option<TreeCursor>,
    source: &[u8],
    last_idx: &mut usize,
    field_name: &str,
    closure_ref: Option<&LT::LeafFn>,
) -> T {
    if let Some(cursor) = cursor_opt.as_mut() {
        // Handle special case where a node has no children and represents a single-field struct
        let n = cursor.node();
        if n.children.is_empty() && cursor.current_index == 0 {
            *cursor_opt = None;
            *last_idx = n.end_byte;
            return LT::extract(Some(n), source, *last_idx, closure_ref);
        }

        loop {
            let n = cursor.node();
            if let Some(name) = cursor.field_name() {
                if name == field_name {
                    let out = LT::extract(Some(n), source, *last_idx, closure_ref);

                    if !cursor.goto_next_sibling() {
                        *cursor_opt = None;
                    }

                    *last_idx = n.end_byte;

                    return out;
                } else {
                    return LT::extract(None, source, *last_idx, closure_ref);
                }
            } else if field_name
                .parse::<usize>()
                .is_ok_and(|idx| idx == cursor.current_index)
            {
                let out = LT::extract(Some(n), source, *last_idx, closure_ref);

                if !cursor.goto_next_sibling() {
                    *cursor_opt = None;
                }

                *last_idx = n.end_byte;

                return out;
            } else {
                *last_idx = n.end_byte;
            }

            if !cursor.goto_next_sibling() {
                return LT::extract(None, source, *last_idx, closure_ref);
            }
        }
    } else {
        LT::extract(None, source, *last_idx, closure_ref)
    }
}

#[cfg(not(feature = "pure-rust"))]
pub fn parse<T: Extract<T>>(
    input: &str,
    language: impl Fn() -> tree_sitter::Language,
) -> core::result::Result<T, Vec<crate::errors::ParseError>> {
    let mut parser = crate::tree_sitter::Parser::new();
    parser.set_language(&language()).map_err(|_| {
        vec![crate::errors::ParseError {
            reason: crate::errors::ParseErrorReason::UnexpectedToken(
                "Failed to initialize TreeSitter language".to_string(),
            ),
            start: 0,
            end: 0,
            expected: vec![],
        }]
    })?;

    let tree = parser.parse(input, None).ok_or_else(|| {
        vec![crate::errors::ParseError {
            reason: crate::errors::ParseErrorReason::UnexpectedToken(
                "TreeSitter parser returned no tree".to_string(),
            ),
            start: 0,
            end: 0,
            expected: vec![],
        }]
    })?;

    let root_node = tree.root_node();

    if root_node.has_error() {
        let mut errors = vec![];
        crate::errors::collect_parsing_errors(&root_node, input.as_bytes(), &mut errors);

        Err(errors)
    } else {
        Ok(<T as crate::Extract<_>>::extract(
            Some(root_node),
            input.as_bytes(),
            0,
            None,
        ))
    }
}

/// Parses an input string and extracts a value using the pure-rust parser.
#[cfg(feature = "pure-rust")]
pub fn parse<T: Extract<T>>(
    input: &str,
    language: impl Fn() -> &'static crate::pure_parser::TSLanguage,
) -> core::result::Result<T, Vec<crate::errors::ParseError>> {
    // Select parser backend based on feature flags
    use crate::parser_selection::ParserBackend;
    let backend = crate::parser_selection::current_backend_for(T::HAS_CONFLICTS);

    match backend {
        ParserBackend::GLR => {
            // GLR parser path (parser_v4)
            parse_with_glr::<T>(input, language)
        }
        ParserBackend::PureRust => {
            // Simple LR parser path (pure_parser)
            parse_with_pure_parser::<T>(input, language)
        }
        ParserBackend::TreeSitter => {
            let errors = vec![crate::errors::ParseError {
                reason: crate::errors::ParseErrorReason::UnexpectedToken(
                    "TreeSitter backend is not supported in pure-rust mode".to_string(),
                ),
                start: 0,
                end: 0,
                expected: vec![],
            }];
            Err(errors)
        }
    }
}

/// Parses an input string into the native parse document alpha.
#[cfg(feature = "pure-rust")]
pub fn parse_document(
    input: &str,
    language: impl Fn() -> &'static crate::pure_parser::TSLanguage,
    grammar_name: &str,
) -> core::result::Result<crate::document::AdzeDocument, Vec<crate::errors::ParseError>> {
    let lang = language();
    let grammar = crate::decoder::decode_grammar(lang);
    let parse_table = crate::decoder::decode_parse_table(lang);

    #[cfg(feature = "glr")]
    if parse_table_has_conflicts(&parse_table) {
        return parse_document_with_true_glr_runtime(
            input,
            lang,
            grammar_name,
            grammar,
            parse_table,
        );
    }

    let mut parser = crate::pure_parser::Parser::new();
    parser.set_language(lang).map_err(|e| {
        vec![crate::errors::ParseError {
            reason: crate::errors::ParseErrorReason::UnexpectedToken(e),
            start: 0,
            end: 0,
            expected: vec![],
        }]
    })?;

    let crate::pure_parser::ParseResult {
        root,
        errors: parser_errors,
    } = parser.parse_string(input);

    let error_count = parser_errors.len();
    let diagnostics = document_diagnostics_for_parse_errors(input, lang, &parser_errors);
    let root = root
        .as_ref()
        .map(|root_node| convert_parsed_node_to_document_node(root_node, lang))
        .unwrap_or_else(|| {
            synthetic_document_root_for_errors(input, parse_table.start_symbol, &diagnostics)
        });

    Ok(
        crate::document::AdzeDocument::from_parse_result_with_diagnostics(
            input,
            root,
            error_count,
            crate::document::DocumentRuntime {
                language_name: grammar_name,
                grammar: &grammar,
                parse_table: &parse_table,
                pure_language: Some(lang),
            },
            diagnostics,
        ),
    )
}

#[cfg(all(feature = "glr", feature = "pure-rust"))]
fn parse_table_has_conflicts(parse_table: &adze_glr_core::ParseTable) -> bool {
    use adze_glr_core::conflict_inspection::state_has_conflicts;
    use adze_ir::StateId;

    (0..parse_table.state_count)
        .any(|state| state_has_conflicts(parse_table, StateId(state as u16)))
}

#[cfg(all(feature = "glr", feature = "pure-rust"))]
fn parse_document_with_true_glr_runtime(
    input: &str,
    language: &'static crate::pure_parser::TSLanguage,
    grammar_name: &str,
    grammar: adze_ir::Grammar,
    parse_table: adze_glr_core::ParseTable,
) -> core::result::Result<crate::document::AdzeDocument, Vec<crate::errors::ParseError>> {
    let source = input.as_bytes();
    let mut runtime_parse_table = parse_table.clone();
    align_true_glr_parse_table_to_language_symbols(language, &mut runtime_parse_table);
    let mut parser = crate::glr_parser::GLRParser::new(runtime_parse_table, grammar.clone());

    if let Some(lex_fn) = language.lex_fn {
        let tokens = match lex_with_language_fn(language, lex_fn, source) {
            Ok(tokens) => tokens,
            Err(errors) => {
                return Ok(parse_document_from_glr_errors(
                    input,
                    language,
                    grammar_name,
                    &grammar,
                    &parse_table,
                    errors,
                ));
            }
        };
        for token in tokens {
            parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        }
    } else {
        let mut lexer = match crate::glr_lexer::GLRLexer::new(&grammar, input.to_string()) {
            Ok(lexer) => lexer,
            Err(message) => {
                return Ok(parse_document_from_glr_errors(
                    input,
                    language,
                    grammar_name,
                    &grammar,
                    &parse_table,
                    vec![crate::errors::ParseError {
                        reason: crate::errors::ParseErrorReason::UnexpectedToken(message),
                        start: 0,
                        end: source.len(),
                        expected: vec![],
                    }],
                ));
            }
        };

        while let Some(token) = lexer.next_token() {
            parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        }
        if let Some((start, end)) = lexer.invalid_span() {
            return Ok(parse_document_from_glr_errors(
                input,
                language,
                grammar_name,
                &grammar,
                &parse_table,
                vec![crate::errors::ParseError {
                    reason: crate::errors::ParseErrorReason::UnexpectedToken(
                        "unexpected token while lexing".to_string(),
                    ),
                    start,
                    end,
                    expected: vec![],
                }],
            ));
        }
    }

    parser.process_eof(source.len());
    let root_node = match parser.finish() {
        Ok(root_node) => root_node,
        Err(message) => {
            return Ok(parse_document_from_glr_errors(
                input,
                language,
                grammar_name,
                &grammar,
                &parse_table,
                vec![crate::errors::ParseError {
                    reason: crate::errors::ParseErrorReason::UnexpectedToken(message),
                    start: 0,
                    end: source.len(),
                    expected: vec![],
                }],
            ));
        }
    };
    let ambiguities = parser
        .finish_ambiguity_summary()
        .ok()
        .flatten()
        .into_iter()
        .collect::<Vec<_>>();
    let root = convert_subtree_to_document_node(&root_node, language);

    Ok(
        crate::document::AdzeDocument::from_parse_result_with_diagnostics_and_ambiguities(
            input,
            root,
            0,
            crate::document::DocumentRuntime {
                language_name: grammar_name,
                grammar: &grammar,
                parse_table: &parse_table,
                pure_language: Some(language),
            },
            Vec::new(),
            ambiguities,
        ),
    )
}

#[cfg(all(feature = "glr", feature = "pure-rust"))]
fn parse_document_from_glr_errors(
    input: &str,
    language: &'static crate::pure_parser::TSLanguage,
    grammar_name: &str,
    grammar: &adze_ir::Grammar,
    parse_table: &adze_glr_core::ParseTable,
    errors: Vec<crate::errors::ParseError>,
) -> crate::document::AdzeDocument {
    let diagnostics = errors
        .iter()
        .map(|error| {
            let start_byte = error.start.min(input.len());
            let end_byte = if error.end > start_byte {
                error.end.min(input.len())
            } else {
                diagnostic_end_for_byte(input.as_bytes(), start_byte)
            };
            crate::document::ParseDiagnostic {
                start_byte,
                end_byte,
                point_range: crate::document::PointRange::from_byte_range(
                    input,
                    start_byte..end_byte,
                ),
                found: Some(error.reason.to_string()),
                expected: error.expected.clone(),
                related_nodes: Vec::new(),
                message: error.to_string(),
            }
        })
        .collect::<Vec<_>>();
    let error_count = diagnostics.len();
    let root = synthetic_document_root_for_errors(input, parse_table.start_symbol, &diagnostics);

    crate::document::AdzeDocument::from_parse_result_with_diagnostics_and_ambiguities(
        input,
        root,
        error_count,
        crate::document::DocumentRuntime {
            language_name: grammar_name,
            grammar,
            parse_table,
            pure_language: Some(language),
        },
        diagnostics,
        Vec::new(),
    )
}

/// Parse using the simple LR parser (pure_parser)
#[cfg(feature = "pure-rust")]
fn parse_with_pure_parser<T: Extract<T>>(
    input: &str,
    language: impl Fn() -> &'static crate::pure_parser::TSLanguage,
) -> core::result::Result<T, Vec<crate::errors::ParseError>> {
    let mut parser = crate::pure_parser::Parser::new();
    let lang = language();
    parser.set_language(lang).map_err(|e| {
        vec![crate::errors::ParseError {
            reason: crate::errors::ParseErrorReason::UnexpectedToken(e),
            start: 0,
            end: 0,
            expected: vec![],
        }]
    })?;

    let parse_result = parser.parse_string(input);

    let crate::pure_parser::ParseResult {
        root,
        errors: parser_errors,
    } = parse_result;

    if !parser_errors.is_empty() {
        let errors = parser_errors
            .into_iter()
            .map(|e| {
                let symbol_name = symbol_name_for_diagnostic(lang, e.found);
                let expected = expected_symbol_names_for_diagnostic(lang, &e.expected);
                crate::errors::ParseError {
                    reason: crate::errors::ParseErrorReason::UnexpectedToken(
                        unexpected_token_message(symbol_name, expected.clone()),
                    ),
                    start: e.position,
                    end: diagnostic_end_for_byte(input.as_bytes(), e.position),
                    expected,
                }
            })
            .collect();
        return Err(errors);
    }

    if let Some(ref root_node) = root
        && root_node.has_error()
    {
        let mut errors = vec![];
        crate::errors::collect_parsing_errors(root_node, input.as_bytes(), &mut errors);
        if !errors.is_empty() {
            return Err(errors);
        }
    }

    let root_node = match root {
        Some(root_node) => root_node,
        None => {
            return Err(vec![crate::errors::ParseError {
                reason: crate::errors::ParseErrorReason::UnexpectedToken(
                    "Parsed result missing root node".to_string(),
                ),
                start: 0,
                end: 0,
                expected: vec![],
            }]);
        }
    };

    // Check if the root node is source_file wrapper
    // In the augmented grammar, we have S' -> source_file -> actual_language_root
    // source_file is typically a wrapper node with a single non-extra child
    let non_extra_root_children: Vec<_> =
        root_node.children.iter().filter(|c| !c.is_extra).collect();
    let extract_node = if root_node.kind() == "source_file" && non_extra_root_children.len() == 1 {
        // This is source_file, extract from its first non-extra child
        non_extra_root_children[0]
    } else {
        // Extract from root directly
        &root_node
    };

    Ok(<T as crate::Extract<_>>::extract(
        Some(extract_node),
        input.as_bytes(),
        0,
        None,
    ))
}

/// Parse using the GLR parser (parser_v4)
#[cfg(all(feature = "glr", feature = "pure-rust"))]
fn parse_with_glr<T: Extract<T>>(
    input: &str,
    language: impl Fn() -> &'static crate::pure_parser::TSLanguage,
) -> core::result::Result<T, Vec<crate::errors::ParseError>> {
    use crate::parser_v4::Parser;
    use adze_glr_core::conflict_inspection::state_has_conflicts;
    use adze_ir::StateId;

    // Get the language and inspect the parse table for real conflicts.
    let lang = language();
    let parse_table = crate::decoder::decode_parse_table(lang);
    let has_conflicts = (0..parse_table.state_count)
        .any(|state| state_has_conflicts(&parse_table, StateId(state as u16)));

    if has_conflicts {
        return parse_with_true_glr_runtime::<T>(input, lang, parse_table);
    }

    // Create parser from TSLanguage with the correct grammar name for external scanner lookup
    let mut parser = Parser::from_language(lang, T::GRAMMAR_NAME.to_string());

    // Parse to get root ParseNode and parser error count.
    let source_bytes = input.as_bytes();
    let (root_node, error_count) = parser.parse_tree_with_error_count(input).map_err(|e| {
        vec![crate::errors::ParseError {
            reason: crate::errors::ParseErrorReason::UnexpectedToken(e.to_string()),
            start: 0,
            end: 0,
            expected: vec![],
        }]
    })?;

    if error_count > 0 {
        // Fallback for grammars/inputs where parser_v4 still reports recoveries.
        // Keep GLR as default routing, but preserve user-visible correctness.
        return parse_with_pure_parser::<T>(input, language);
    }

    // Convert parser_v4::ParseNode to pure_parser::ParsedNode
    let parsed_node = convert_parse_node_v4_to_pure(&root_node, lang, source_bytes);

    // Match pure parser behavior: unwrap source_file wrapper when present.
    let non_extra_root_children: Vec<_> = parsed_node
        .children
        .iter()
        .filter(|c| !c.is_extra)
        .collect();
    let extract_node = if parsed_node.kind() == "source_file" && non_extra_root_children.len() == 1
    {
        non_extra_root_children[0]
    } else {
        &parsed_node
    };

    // Extract typed AST using the Extract trait
    Ok(<T as crate::Extract<_>>::extract(
        Some(extract_node),
        input.as_bytes(),
        0,
        None,
    ))
}

#[cfg(all(feature = "glr", feature = "pure-rust"))]
fn parse_with_true_glr_runtime<T: Extract<T>>(
    input: &str,
    language: &'static crate::pure_parser::TSLanguage,
    mut parse_table: adze_glr_core::ParseTable,
) -> core::result::Result<T, Vec<crate::errors::ParseError>> {
    let source = input.as_bytes();
    align_true_glr_parse_table_to_language_symbols(language, &mut parse_table);
    let grammar = crate::decoder::decode_grammar(language);
    let mut parser = crate::glr_parser::GLRParser::new(parse_table, grammar.clone());

    if let Some(lex_fn) = language.lex_fn {
        for token in lex_with_language_fn(language, lex_fn, source)? {
            parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        }
    } else {
        let mut lexer = match crate::glr_lexer::GLRLexer::new(&grammar, input.to_string()) {
            Ok(lexer) => lexer,
            Err(message) => {
                return Err(vec![crate::errors::ParseError {
                    reason: crate::errors::ParseErrorReason::UnexpectedToken(message),
                    start: 0,
                    end: source.len(),
                    expected: vec![],
                }]);
            }
        };

        while let Some(token) = lexer.next_token() {
            parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        }
        if let Some((start, end)) = lexer.invalid_span() {
            return Err(vec![crate::errors::ParseError {
                reason: crate::errors::ParseErrorReason::UnexpectedToken(
                    "unexpected token while lexing".to_string(),
                ),
                start,
                end,
                expected: vec![],
            }]);
        }
    }

    parser.process_eof(source.len());
    let root_node = match parser.finish() {
        Ok(root_node) => root_node,
        Err(message) => {
            return Err(vec![crate::errors::ParseError {
                reason: crate::errors::ParseErrorReason::UnexpectedToken(message),
                start: 0,
                end: source.len(),
                expected: vec![],
            }]);
        }
    };

    let parsed_node = convert_subtree_to_pure(&root_node, language, source);
    let non_extra_root_children: Vec<_> = parsed_node
        .children
        .iter()
        .filter(|c| !c.is_extra)
        .collect();
    let extract_node = if parsed_node.kind() == "source_file" && non_extra_root_children.len() == 1
    {
        non_extra_root_children[0]
    } else {
        &parsed_node
    };

    if extract_node.has_error() {
        let mut errors = vec![];
        crate::errors::collect_parsing_errors(extract_node, source, &mut errors);
        if !errors.is_empty() {
            return Err(errors);
        }
    }

    Ok(<T as crate::Extract<_>>::extract(
        Some(extract_node),
        source,
        0,
        None,
    ))
}

#[cfg(all(feature = "glr", feature = "pure-rust"))]
/// Align a decoded parse table to the generated language's dense symbol IDs.
pub fn align_true_glr_parse_table_to_language_symbols(
    language: &'static crate::pure_parser::TSLanguage,
    parse_table: &mut adze_glr_core::ParseTable,
) {
    use adze_ir::SymbolId;
    use std::collections::BTreeMap;

    if language.public_symbol_map.is_null() {
        return;
    }

    let symbol_count = language.symbol_count as usize;
    if symbol_count == 0
        || symbol_count > u16::MAX as usize
        || parse_table.index_to_symbol.len() != symbol_count
    {
        return;
    }

    // The generated lexer emits table-column symbols. The public symbol map is
    // still useful for ABI decode callers, but the true-GLR runtime must execute
    // against the same dense symbols used by the generated language tables.
    let public_to_column = parse_table.symbol_to_index.clone();
    let raw_symbol = |symbol: SymbolId| {
        public_to_column
            .get(&symbol)
            .and_then(|&column| u16::try_from(column).ok())
            .map(SymbolId)
            .unwrap_or(symbol)
    };

    for rule in &mut parse_table.rules {
        rule.lhs = raw_symbol(rule.lhs);
    }

    parse_table.start_symbol = raw_symbol(parse_table.start_symbol);
    parse_table.eof_symbol = SymbolId(language.eof_symbol);
    parse_table.extras = parse_table.extras.iter().copied().map(raw_symbol).collect();
    for aliases in &mut parse_table.alias_sequences {
        for alias in aliases {
            *alias = alias.map(raw_symbol);
        }
    }

    parse_table.index_to_symbol = (0..symbol_count)
        .filter_map(|column| u16::try_from(column).ok())
        .map(SymbolId)
        .collect();
    parse_table.symbol_to_index = parse_table
        .index_to_symbol
        .iter()
        .copied()
        .enumerate()
        .map(|(column, symbol)| (symbol, column))
        .collect::<BTreeMap<_, _>>();

    let nonterminal_start = parse_table
        .token_count
        .saturating_add(parse_table.external_token_count);
    parse_table.nonterminal_to_index = (nonterminal_start..symbol_count)
        .filter_map(|column| {
            u16::try_from(column)
                .ok()
                .map(|raw| (SymbolId(raw), column))
        })
        .collect();
    *parse_table = std::mem::take(parse_table).remap_goto_to_nonterminal_map();

    for (column, metadata) in parse_table.symbol_metadata.iter_mut().enumerate() {
        if let Ok(raw) = u16::try_from(column) {
            metadata.symbol_id = SymbolId(raw);
        }
    }
}

#[cfg(all(feature = "glr", feature = "pure-rust"))]
/// Tokenize source text with a generated language lexer function.
pub fn lex_with_language_fn(
    language: &'static crate::pure_parser::TSLanguage,
    lex_fn: unsafe extern "C" fn(*mut core::ffi::c_void, crate::pure_parser::TSLexState) -> bool,
    source: &[u8],
) -> core::result::Result<Vec<crate::glr_lexer::TokenWithPosition>, Vec<crate::errors::ParseError>>
{
    use crate::lex::TsLexer;
    use adze_ir::SymbolId;
    use core::ffi::c_void;

    #[repr(C)]
    struct Backing<'a> {
        input: &'a [u8],
        pos: usize,
        mark: usize,
    }

    unsafe extern "C" fn lookahead(lex: *mut TsLexer) -> u32 {
        unsafe {
            if lex.is_null() || (*lex).data.is_null() {
                return 0;
            }
            let backing = &*((*lex).data as *const Backing);
            if backing.pos < backing.input.len() {
                backing.input[backing.pos] as u32
            } else {
                0
            }
        }
    }

    unsafe extern "C" fn advance(lex: *mut TsLexer, _skip: bool) {
        unsafe {
            if lex.is_null() || (*lex).data.is_null() {
                return;
            }
            let backing = &mut *((*lex).data as *mut Backing);
            if backing.pos < backing.input.len() {
                backing.pos += 1;
            }
        }
    }

    unsafe extern "C" fn mark_end(lex: *mut TsLexer) {
        unsafe {
            if lex.is_null() || (*lex).data.is_null() {
                return;
            }
            let backing = &mut *((*lex).data as *mut Backing);
            backing.mark = backing.pos;
        }
    }

    fn is_extra_symbol(language: &crate::pure_parser::TSLanguage, symbol: u16) -> bool {
        if symbol >= language.symbol_count as u16 || language.symbol_metadata.is_null() {
            return false;
        }
        // SAFETY: `symbol < symbol_count` and generated languages expose a
        // `symbol_metadata` array with `symbol_count` entries.
        unsafe { (*language.symbol_metadata.add(symbol as usize) & 0x04) != 0 }
    }

    let mut tokens = Vec::new();
    let mut position = 0usize;
    let lex_mode = if !language.lex_modes.is_null() {
        // SAFETY: generated languages provide one lex mode per state. State 0 is
        // the conservative default for the current generated lexers.
        unsafe { *language.lex_modes }
    } else {
        crate::pure_parser::TSLexState {
            lex_state: 0,
            external_lex_state: 0,
        }
    };

    while position < source.len() {
        while position < source.len() && matches!(source[position], b' ' | b'\t' | b'\n' | b'\r') {
            position += 1;
        }
        if position >= source.len() {
            break;
        }

        let start = position;
        let mut backing = Backing {
            input: source,
            pos: position,
            mark: position,
        };
        let mut ts_lexer = TsLexer {
            lookahead,
            advance,
            mark_end,
            result_symbol: u16::MAX,
            data: &mut backing as *mut _ as *mut c_void,
        };

        // SAFETY: `lex_fn` is the generated language lexer. `ts_lexer` uses the
        // same `TsLexer` ABI layout that generated lexers expect.
        let ok = unsafe { lex_fn(&mut ts_lexer as *mut _ as *mut c_void, lex_mode) };
        let end = if backing.mark > start {
            backing.mark
        } else {
            backing.pos
        };

        if !ok || ts_lexer.result_symbol == u16::MAX || end <= start {
            let invalid_end = diagnostic_end_for_byte(source, start);
            return Err(vec![crate::errors::ParseError {
                reason: crate::errors::ParseErrorReason::UnexpectedToken(
                    "unexpected token while lexing".to_string(),
                ),
                start,
                end: invalid_end,
                expected: vec![],
            }]);
        }

        if !is_extra_symbol(language, ts_lexer.result_symbol) {
            let text = String::from_utf8_lossy(&source[start..end]).into_owned();
            tokens.push(crate::glr_lexer::TokenWithPosition {
                symbol_id: SymbolId(ts_lexer.result_symbol),
                text,
                byte_offset: start,
                byte_length: end - start,
            });
        }

        position = end;
    }

    Ok(tokens)
}

#[cfg(all(feature = "glr", feature = "pure-rust"))]
fn convert_subtree_to_pure(
    subtree: &crate::subtree::Subtree,
    language: &'static crate::pure_parser::TSLanguage,
    source: &[u8],
) -> crate::pure_parser::ParsedNode {
    let is_named = if (subtree.node.symbol_id.0 as usize) < language.symbol_count as usize
        && !language.symbol_metadata.is_null()
    {
        // SAFETY: `symbol_metadata` is a pointer to `symbol_count` entries.
        // `subtree.node.symbol_id` was checked to be in bounds above.
        let metadata = unsafe {
            *language
                .symbol_metadata
                .add(subtree.node.symbol_id.0 as usize)
        };
        (metadata & 0x02) != 0
    } else {
        true
    };

    let children = subtree
        .children
        .iter()
        .map(|child| {
            let mut parsed_child = convert_subtree_to_pure(&child.subtree, language, source);
            parsed_child.field_id = if child.field_id == crate::subtree::FIELD_NONE {
                None
            } else {
                Some(child.field_id)
            };
            parsed_child
        })
        .collect();

    crate::pure_parser::ParsedNode {
        symbol: subtree.node.symbol_id.0,
        children,
        start_byte: subtree.node.byte_range.start,
        end_byte: subtree.node.byte_range.end,
        start_point: byte_to_point(source, subtree.node.byte_range.start),
        end_point: byte_to_point(source, subtree.node.byte_range.end),
        is_extra: false,
        is_error: subtree.node.is_error,
        is_missing: false,
        is_named,
        field_id: None,
        language: Some(language as *const _),
    }
}

#[cfg(all(feature = "glr", feature = "pure-rust"))]
fn convert_subtree_to_document_node(
    subtree: &crate::subtree::Subtree,
    language: &'static crate::pure_parser::TSLanguage,
) -> crate::parser_v4::ParseNode {
    let children = subtree
        .children
        .iter()
        .map(|child| {
            let mut parsed_child = convert_subtree_to_document_node(&child.subtree, language);
            parsed_child.field_name = if child.field_id == crate::subtree::FIELD_NONE {
                None
            } else {
                field_name_by_id(language, child.field_id)
            };
            parsed_child
        })
        .collect();
    let symbol_id = public_symbol_id_for_index(language, subtree.node.symbol_id.0);

    crate::parser_v4::ParseNode {
        symbol: symbol_id,
        symbol_id,
        start_byte: subtree.node.byte_range.start,
        end_byte: subtree.node.byte_range.end,
        field_name: None,
        alias_symbol_id: None,
        children,
    }
}

/// Convert parser_v4::ParseNode to pure_parser::ParsedNode
#[cfg(all(feature = "glr", feature = "pure-rust"))]
fn convert_parse_node_v4_to_pure(
    node: &crate::parser_v4::ParseNode,
    lang: &crate::pure_parser::TSLanguage,
    source: &[u8],
) -> crate::pure_parser::ParsedNode {
    let resolve_field_id = |field_name: &str| -> Option<u16> {
        if lang.field_count == 0 || lang.field_names.is_null() {
            return None;
        }
        // SAFETY: `field_names` points to a static array of `field_count` pointers.
        let field_names =
            unsafe { std::slice::from_raw_parts(lang.field_names, lang.field_count as usize) };
        field_names.iter().enumerate().find_map(|(idx, name_ptr)| {
            if name_ptr.is_null() {
                return None;
            }
            // SAFETY: name_ptr is validated non-null and points to NUL-terminated static bytes.
            let raw = unsafe { std::ffi::CStr::from_ptr(*name_ptr as *const i8) };
            let Ok(name) = raw.to_str() else {
                return None;
            };
            (name == field_name).then_some(idx as u16)
        })
    };
    let is_error_symbol = |symbol: u16| {
        if symbol as u32 >= lang.symbol_count || lang.symbol_names.is_null() {
            return false;
        }

        let symbol_names =
            // SAFETY: `symbol` is bounds-checked above and `symbol_names` is not null.
            // The pointer refers to a static array of `symbol_count` entries.
            unsafe { std::slice::from_raw_parts(lang.symbol_names, lang.symbol_count as usize) };
        let name_ptr = symbol_names[symbol as usize];
        if name_ptr.is_null() {
            return false;
        }

        // SAFETY: `name_ptr` was just null-checked. It points to a static NUL-
        // terminated C string from the language tables.
        let name = unsafe { std::ffi::CStr::from_ptr(name_ptr as *const c_char).to_str() }.ok();
        matches!(name, Some("ERROR"))
    };

    // Recursively convert children
    let children = node
        .children
        .iter()
        .map(|child| convert_parse_node_v4_to_pure(child, lang, source))
        .collect();

    // Read symbol metadata from TSLanguage
    // SAFETY: `symbol_metadata` is a static array of `symbol_count` entries.
    // `node.symbol.0` is bounds-checked before dereferencing.
    let (is_named, is_extra) = unsafe {
        if !lang.symbol_metadata.is_null() && (node.symbol.0 as u32) < lang.symbol_count {
            let metadata = *lang.symbol_metadata.add(node.symbol.0 as usize);
            // Tree-sitter metadata encoding:
            // Bit 0 (0x01): visible
            // Bit 1 (0x02): named
            // Bit 2 (0x04): extra
            // Bit 3 (0x08): supertype
            let is_named = (metadata & 0x02) != 0;
            let is_extra = (metadata & 0x04) != 0;
            (is_named, is_extra)
        } else {
            // Fallback if metadata unavailable
            (true, false)
        }
    };
    let is_empty_error_node =
        node.symbol.0 == 0 && node.children.is_empty() && node.start_byte == node.end_byte;

    crate::pure_parser::ParsedNode {
        symbol: node.symbol.0, // SymbolId.0 -> TSSymbol
        children,
        start_byte: node.start_byte,
        end_byte: node.end_byte,
        start_point: byte_to_point(source, node.start_byte),
        end_point: byte_to_point(source, node.end_byte),
        is_extra,
        is_error: is_error_symbol(node.symbol.0) || is_empty_error_node,
        is_missing: false,
        is_named,
        field_id: node.field_name.as_deref().and_then(resolve_field_id),
        language: Some(lang as *const _),
    }
}

#[cfg(feature = "pure-rust")]
fn convert_parsed_node_to_document_node(
    node: &crate::pure_parser::ParsedNode,
    lang: &crate::pure_parser::TSLanguage,
) -> crate::parser_v4::ParseNode {
    let children = node
        .children
        .iter()
        .map(|child| convert_parsed_node_to_document_node(child, lang))
        .collect();

    let symbol_id = public_symbol_id_for_index(lang, node.symbol);
    crate::parser_v4::ParseNode {
        symbol: symbol_id,
        symbol_id,
        start_byte: node.start_byte,
        end_byte: node.end_byte,
        field_name: node
            .field_id
            .and_then(|field_id| field_name_by_id(lang, field_id)),
        alias_symbol_id: None,
        children,
    }
}

#[cfg(feature = "pure-rust")]
fn document_diagnostics_for_parse_errors(
    input: &str,
    lang: &crate::pure_parser::TSLanguage,
    parser_errors: &[crate::pure_parser::ParseError],
) -> Vec<crate::document::ParseDiagnostic> {
    parser_errors
        .iter()
        .map(|error| {
            let found = symbol_name_for_diagnostic(lang, error.found);
            let expected = expected_symbol_names_for_diagnostic(lang, &error.expected);
            let start_byte = error.position;
            let end_byte = diagnostic_end_for_byte(input.as_bytes(), error.position);
            let point_range =
                crate::document::PointRange::from_byte_range(input, start_byte..end_byte);
            crate::document::ParseDiagnostic {
                start_byte,
                end_byte,
                point_range,
                found: Some(found.clone()),
                expected: expected.clone(),
                related_nodes: Vec::new(),
                message: unexpected_token_message(found, expected),
            }
        })
        .collect()
}

#[cfg(feature = "pure-rust")]
fn synthetic_document_root_for_errors(
    input: &str,
    start_symbol: adze_ir::SymbolId,
    diagnostics: &[crate::document::ParseDiagnostic],
) -> crate::parser_v4::ParseNode {
    let error_span = diagnostics
        .first()
        .map(|diagnostic| diagnostic.start_byte..diagnostic.end_byte)
        .unwrap_or(input.len()..input.len());
    let error_node = crate::parser_v4::ParseNode {
        symbol: adze_ir::SymbolId(0),
        symbol_id: adze_ir::SymbolId(0),
        start_byte: error_span.start,
        end_byte: error_span.end,
        field_name: None,
        alias_symbol_id: None,
        children: Vec::new(),
    };

    crate::parser_v4::ParseNode {
        symbol: start_symbol,
        symbol_id: start_symbol,
        start_byte: 0,
        end_byte: input.len(),
        field_name: None,
        alias_symbol_id: None,
        children: vec![error_node],
    }
}

#[cfg(feature = "pure-rust")]
fn public_symbol_id_for_index(
    lang: &crate::pure_parser::TSLanguage,
    symbol: crate::pure_parser::TSSymbol,
) -> adze_ir::SymbolId {
    let public_symbol =
        if !lang.public_symbol_map.is_null() && usize::from(symbol) < lang.symbol_count as usize {
            // SAFETY: `symbol` is checked against `symbol_count`, and
            // `public_symbol_map` has one entry per generated table column.
            unsafe { *lang.public_symbol_map.add(usize::from(symbol)) }
        } else {
            symbol
        };

    adze_ir::SymbolId(public_symbol)
}

#[cfg(feature = "pure-rust")]
fn field_name_by_id(lang: &crate::pure_parser::TSLanguage, field_id: u16) -> Option<String> {
    if field_id >= lang.field_count as u16 || lang.field_names.is_null() {
        return None;
    }

    // SAFETY: `field_names` points to a static array of `field_count` pointers,
    // and `field_id` was bounds-checked above.
    let field_names =
        unsafe { std::slice::from_raw_parts(lang.field_names, lang.field_count as usize) };
    let name_ptr = field_names[field_id as usize];
    if name_ptr.is_null() {
        return None;
    }

    // SAFETY: `name_ptr` is non-null and points to a NUL-terminated static
    // string emitted in the generated language tables.
    unsafe { CStr::from_ptr(name_ptr as *const c_char) }
        .to_str()
        .ok()
        .map(str::to_string)
}

#[allow(dead_code)]
fn byte_to_point(source: &[u8], byte_pos: usize) -> crate::pure_parser::Point {
    let mut row = 0u32;
    let mut column = 0u32;
    let end = byte_pos.min(source.len());

    for &b in &source[..end] {
        if b == b'\n' {
            row = row.saturating_add(1);
            column = 0;
        } else {
            column = column.saturating_add(1);
        }
    }

    crate::pure_parser::Point { row, column }
}

#[cfg(feature = "pure-rust")]
fn diagnostic_end_for_byte(source: &[u8], start: usize) -> usize {
    if start >= source.len() {
        return source.len();
    }

    std::str::from_utf8(&source[start..])
        .ok()
        .and_then(|tail| tail.chars().next())
        .map(|ch| start + ch.len_utf8())
        .unwrap_or_else(|| (start + 1).min(source.len()))
}

#[cfg(feature = "pure-rust")]
fn symbol_name_for_diagnostic(
    lang: &crate::pure_parser::TSLanguage,
    symbol: crate::pure_parser::TSSymbol,
) -> String {
    if (symbol as u32) >= lang.symbol_count {
        return format!("symbol {symbol} (out of bounds)");
    }

    // SAFETY: `symbol` is bounds-checked above. Generated language metadata
    // points to static arrays that live for the whole parse.
    unsafe {
        let public_symbol = if !lang.public_symbol_map.is_null() {
            *lang.public_symbol_map.add(symbol as usize)
        } else {
            symbol
        };

        if lang.symbol_names.is_null() {
            return format!("symbol {symbol} (public {public_symbol})");
        }

        // `symbol` is the table-column symbol reported by the parser. Generated
        // symbol names are emitted in that same column order; the public symbol
        // map can contain sparse Adze `SymbolId`s and must not be used as an
        // index into the column-ordered name array.
        let symbol_ptr = *lang.symbol_names.add(symbol as usize);
        if symbol_ptr.is_null() {
            return format!("symbol {symbol} (public {public_symbol})");
        }

        let raw_name = CStr::from_ptr(symbol_ptr as *const c_char)
            .to_string_lossy()
            .to_string();
        diagnostic_symbol_name(raw_name)
    }
}

#[cfg(feature = "pure-rust")]
fn diagnostic_symbol_name(raw_name: String) -> String {
    if raw_name.starts_with("_/") && raw_name.ends_with('/') {
        raw_name[1..].to_string()
    } else {
        raw_name
    }
}

#[cfg(feature = "pure-rust")]
fn expected_symbol_names_for_diagnostic(
    lang: &crate::pure_parser::TSLanguage,
    expected: &[crate::pure_parser::TSSymbol],
) -> Vec<String> {
    let mut names = expected
        .iter()
        .copied()
        .filter(|symbol| !is_extra_symbol_for_diagnostic(lang, *symbol))
        .map(|symbol| symbol_name_for_diagnostic(lang, symbol))
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

#[cfg(feature = "pure-rust")]
fn is_extra_symbol_for_diagnostic(
    lang: &crate::pure_parser::TSLanguage,
    symbol: crate::pure_parser::TSSymbol,
) -> bool {
    if (symbol as u32) >= lang.symbol_count || lang.symbol_metadata.is_null() {
        return false;
    }

    // SAFETY: `symbol` is bounds-checked above and `symbol_metadata` points to
    // one metadata byte per generated language symbol.
    unsafe { (*lang.symbol_metadata.add(symbol as usize) & 0x04) != 0 }
}

#[cfg(feature = "pure-rust")]
fn unexpected_token_message(found: String, expected: Vec<String>) -> String {
    if expected.is_empty() {
        found
    } else {
        format!("{found}; expected one of: {}", expected.join(", "))
    }
}

/// Parse using the GLR parser (stub for when feature is not enabled)
#[cfg(all(feature = "pure-rust", not(feature = "glr")))]
fn parse_with_glr<T: Extract<T>>(
    _input: &str,
    _language: impl Fn() -> &'static crate::pure_parser::TSLanguage,
) -> core::result::Result<T, Vec<crate::errors::ParseError>> {
    Err(vec![crate::errors::ParseError {
        reason: crate::errors::ParseErrorReason::UnexpectedToken(
            "GLR parser backend is unavailable because the `glr` feature is disabled".to_string(),
        ),
        start: 0,
        end: 0,
        expected: vec![],
    }])
}

#[cfg(all(test, feature = "pure-rust"))]
mod tests {
    use super::*;
    use crate::pure_parser::{
        ExternalScanner, ParsedNode, Point, TSLanguage, TSLexState, TSParseAction, TSRule,
    };
    use core::ptr;

    #[test]
    #[cfg(feature = "glr")]
    fn given_error_symbol_named_error_when_converting_parse_node_then_marked_as_error() {
        let symbol_error = b"ERROR\0";
        let symbol_root = b"root\0";
        let symbol_names = [symbol_error.as_ptr(), symbol_root.as_ptr()];
        let language = TSLanguage {
            symbol_count: 2,
            symbol_names: symbol_names.as_ptr(),
            symbol_metadata: ptr::null(),
            ..FIELD_LANGUAGE
        };
        let parse_node = crate::parser_v4::ParseNode {
            symbol: adze_ir::SymbolId(0),
            symbol_id: adze_ir::SymbolId(0),
            start_byte: 0,
            end_byte: 0,
            field_name: None,
            alias_symbol_id: None,
            children: vec![],
        };

        let converted = convert_parse_node_v4_to_pure(&parse_node, &language, b"");
        assert!(converted.is_error);
    }

    #[test]
    #[cfg(feature = "glr")]
    fn given_empty_symbol_zero_node_when_name_lookup_absent_then_marked_error_by_shape() {
        let names = [c"root".as_ptr() as *const u8];
        let language = TSLanguage {
            symbol_count: 1,
            symbol_names: names.as_ptr(),
            symbol_metadata: ptr::null(),
            ..FIELD_LANGUAGE
        };
        let parse_node = crate::parser_v4::ParseNode {
            symbol: adze_ir::SymbolId(0),
            symbol_id: adze_ir::SymbolId(0),
            start_byte: 0,
            end_byte: 0,
            field_name: None,
            alias_symbol_id: None,
            children: vec![],
        };

        let converted = convert_parse_node_v4_to_pure(&parse_node, &language, b"");
        assert!(converted.is_error);
    }

    static FIELD_NAME_VALUE: &[u8] = b"value\0";
    static FIELD_NAME_NAME: &[u8] = b"name\0";

    #[repr(transparent)]
    struct FieldNames([*const u8; 2]);
    // SAFETY: The pointers refer to static byte string literals (`b"value\0"` and
    // `b"name\0"`) that are immutable and valid for the lifetime of the program.
    unsafe impl Sync for FieldNames {}

    static FIELD_NAMES: FieldNames =
        FieldNames([FIELD_NAME_VALUE.as_ptr(), FIELD_NAME_NAME.as_ptr()]);
    static LEX_MODES: [TSLexState; 1] = [TSLexState {
        lex_state: 0,
        external_lex_state: 0,
    }];

    static FIELD_LANGUAGE: TSLanguage = TSLanguage {
        version: 15,
        symbol_count: 0,
        alias_count: 0,
        token_count: 0,
        external_token_count: 0,
        state_count: 0,
        large_state_count: 0,
        production_id_count: 0,
        field_count: 2,
        max_alias_sequence_length: 0,
        production_id_map: ptr::null(),
        parse_table: ptr::null(),
        small_parse_table: ptr::null(),
        small_parse_table_map: ptr::null(),
        parse_actions: ptr::null::<TSParseAction>(),
        symbol_names: ptr::null(),
        field_names: FIELD_NAMES.0.as_ptr(),
        field_map_slices: ptr::null(),
        field_map_entries: ptr::null(),
        symbol_metadata: ptr::null(),
        public_symbol_map: ptr::null(),
        alias_map: ptr::null(),
        alias_sequences: ptr::null(),
        lex_modes: LEX_MODES.as_ptr(),
        lex_fn: None,
        keyword_lex_fn: None,
        keyword_capture_token: 0,
        external_scanner: ExternalScanner::default(),
        primary_state_ids: ptr::null(),
        production_lhs_index: ptr::null(),
        production_count: 0,
        eof_symbol: 0,
        rules: ptr::null::<TSRule>(),
        rule_count: 0,
    };

    fn node(
        symbol: u16,
        start: usize,
        end: usize,
        field_id: Option<u16>,
        children: Vec<ParsedNode>,
    ) -> ParsedNode {
        ParsedNode {
            symbol,
            children,
            start_byte: start,
            end_byte: end,
            start_point: Point {
                row: 0,
                column: start as u32,
            },
            end_point: Point {
                row: 0,
                column: end as u32,
            },
            is_extra: false,
            is_error: false,
            is_missing: false,
            is_named: true,
            field_id,
            language: None,
        }
    }

    #[test]
    fn given_parent_with_children_when_extracting_struct_then_cursor_starts_at_first_child() {
        // Given
        let first = node(11, 0, 1, None, vec![]);
        let second = node(22, 1, 2, None, vec![]);
        let root = node(99, 5, 7, None, vec![first, second]);

        // When
        let (first_symbol, initial_start_byte, can_move_to_second, second_symbol) =
            extract_struct_or_variant(&root, |cursor_opt, start_byte| {
                let cursor = cursor_opt
                    .as_mut()
                    .expect("cursor should start at first child");
                let first_symbol = cursor.node().symbol;
                let can_move_to_second = cursor.goto_next_sibling();
                let second_symbol = cursor.node().symbol;
                (first_symbol, *start_byte, can_move_to_second, second_symbol)
            });

        // Then
        assert_eq!(first_symbol, 11);
        assert_eq!(initial_start_byte, 5);
        assert!(can_move_to_second);
        assert_eq!(second_symbol, 22);
    }

    #[test]
    fn given_single_field_struct_when_extract_field_then_parent_node_is_extracted() {
        // Given
        let root = node(7, 2, 5, None, vec![]);
        let mut cursor_opt = Some(TreeCursor::new(&root));
        let mut last_idx = 0;

        // When
        let extracted: String = extract_field::<String, String>(
            &mut cursor_opt,
            b"xxabc",
            &mut last_idx,
            "value",
            None,
        );

        // Then
        assert_eq!(extracted, "abc");
        assert!(cursor_opt.is_none());
        assert_eq!(last_idx, 5);
    }

    #[test]
    fn given_unlabeled_children_when_extracting_named_field_then_result_is_default() {
        // Given
        let child1 = node(1, 0, 1, None, vec![]);
        let child2 = node(2, 1, 2, None, vec![]);
        let root = node(9, 0, 2, None, vec![child1, child2]);
        let mut cursor = TreeCursor::new(&root);
        assert!(cursor.goto_first_child());
        assert!(cursor.goto_next_sibling());
        let mut cursor_opt = Some(cursor);
        let mut last_idx = 1;

        // When
        let extracted: String = extract_field::<String, String>(
            &mut cursor_opt,
            b"ab",
            &mut last_idx,
            "missing_field",
            None,
        );

        // Then
        assert_eq!(extracted, "");
        assert_eq!(last_idx, 2);
        assert!(cursor_opt.is_some());
    }

    #[test]
    fn given_child_with_field_id_but_no_language_when_reading_field_name_then_returns_none() {
        // Given
        let child = node(2, 0, 1, Some(0), vec![]);
        let root = node(1, 0, 1, None, vec![child]);
        let mut cursor = TreeCursor::new(&root);
        assert!(cursor.goto_first_child());

        // When / Then
        assert_eq!(cursor.field_name(), None);
    }

    #[test]
    fn given_valid_field_table_when_reading_field_name_then_cursor_resolves_field_label() {
        // Given
        let child = node(2, 0, 1, Some(1), vec![]);
        let mut root = node(1, 0, 1, None, vec![child]);
        root.language = Some(&FIELD_LANGUAGE as *const _);
        let mut cursor = TreeCursor::new(&root);
        assert!(cursor.goto_first_child());

        // When / Then
        assert_eq!(cursor.field_name(), Some("name"));
    }

    #[test]
    fn given_public_symbol_map_when_converting_pure_node_then_uses_public_symbol_ids() {
        // Given
        let public_symbol_map = [0, 42, 7];
        let language = TSLanguage {
            symbol_count: 3,
            public_symbol_map: public_symbol_map.as_ptr(),
            ..FIELD_LANGUAGE
        };
        let child = node(2, 1, 2, None, vec![]);
        let root = node(1, 0, 3, Some(1), vec![child]);

        // When
        let converted = convert_parsed_node_to_document_node(&root, &language);

        // Then
        assert_eq!(converted.symbol, adze_ir::SymbolId(42));
        assert_eq!(converted.symbol_id, adze_ir::SymbolId(42));
        assert_eq!(converted.field_name.as_deref(), Some("name"));
        assert_eq!(converted.children[0].symbol_id, adze_ir::SymbolId(7));
    }

    #[test]
    fn given_out_of_range_field_id_when_reading_field_name_then_returns_none() {
        // Given
        let child = node(2, 0, 1, Some(2), vec![]);
        let mut root = node(1, 0, 1, None, vec![child]);
        root.language = Some(&FIELD_LANGUAGE as *const _);
        let mut cursor = TreeCursor::new(&root);
        assert!(cursor.goto_first_child());

        // When / Then
        assert_eq!(cursor.field_name(), None);
    }

    #[test]
    fn given_struct_without_children_when_extracting_variant_then_cursor_is_absent() {
        // Given
        let root = node(77, 3, 9, None, vec![]);

        // When
        let (cursor_missing, start_byte) =
            extract_struct_or_variant(&root, |cursor_opt, idx| (cursor_opt.is_none(), *idx));

        // Then
        assert!(cursor_missing);
        assert_eq!(start_byte, 3);
    }

    #[test]
    fn given_missing_cursor_when_extracting_field_then_default_is_returned_without_advancing() {
        // Given
        let mut cursor_opt: Option<TreeCursor> = None;
        let mut last_idx = 4;

        // When
        let extracted: String = extract_field::<String, String>(
            &mut cursor_opt,
            b"abcdef",
            &mut last_idx,
            "name",
            None,
        );

        // Then
        assert_eq!(extracted, "");
        assert_eq!(last_idx, 4);
    }

    #[test]
    fn byte_to_point_tracks_newlines_and_columns() {
        let source = b"ab\ncde\nf";
        assert_eq!(byte_to_point(source, 0), Point { row: 0, column: 0 });
        assert_eq!(byte_to_point(source, 1), Point { row: 0, column: 1 });
        assert_eq!(byte_to_point(source, 2), Point { row: 0, column: 2 });
        assert_eq!(byte_to_point(source, 3), Point { row: 1, column: 0 });
        assert_eq!(byte_to_point(source, 4), Point { row: 1, column: 1 });
        assert_eq!(byte_to_point(source, 7), Point { row: 2, column: 0 });
        assert_eq!(byte_to_point(source, 99), Point { row: 2, column: 1 });
    }

    #[test]
    fn diagnostic_end_for_byte_advances_by_utf8_scalar() {
        let source = "aλ!".as_bytes();

        assert_eq!(diagnostic_end_for_byte(source, 0), 1);
        assert_eq!(diagnostic_end_for_byte(source, 1), 3);
        assert_eq!(diagnostic_end_for_byte(source, 3), 4);
        assert_eq!(diagnostic_end_for_byte(source, 4), 4);
    }

    #[test]
    fn expected_symbol_names_for_diagnostic_are_sorted_and_deduped() {
        let names = [
            c"ERROR".as_ptr() as *const u8,
            c"_whitespace".as_ptr() as *const u8,
            c"plus".as_ptr() as *const u8,
            c"number".as_ptr() as *const u8,
        ];
        let metadata = [0, 0x04, 0, 0];
        let language = TSLanguage {
            symbol_count: 4,
            token_count: 4,
            symbol_names: names.as_ptr(),
            symbol_metadata: metadata.as_ptr(),
            ..FIELD_LANGUAGE
        };

        let expected = expected_symbol_names_for_diagnostic(&language, &[3, 2, 1, 3]);
        assert_eq!(expected, vec!["number".to_string(), "plus".to_string()]);
        assert_eq!(
            unexpected_token_message("ERROR".to_string(), expected),
            "ERROR; expected one of: number, plus"
        );
    }

    #[test]
    fn expected_symbol_names_for_diagnostic_handles_sparse_public_map() {
        let names = [
            c"end".as_ptr() as *const u8,
            c"number".as_ptr() as *const u8,
            c"expr".as_ptr() as *const u8,
        ];
        let metadata = [0, 0, 0];
        let public_symbol_map = [0, 7, 11];
        let language = TSLanguage {
            symbol_count: 3,
            token_count: 2,
            symbol_names: names.as_ptr(),
            symbol_metadata: metadata.as_ptr(),
            public_symbol_map: public_symbol_map.as_ptr(),
            ..FIELD_LANGUAGE
        };

        let expected = expected_symbol_names_for_diagnostic(&language, &[1]);

        assert_eq!(expected, vec!["number".to_string()]);
    }

    #[test]
    fn expected_symbol_names_for_diagnostic_uses_column_names_with_dense_public_remap() {
        let names = [
            c"end".as_ptr() as *const u8,
            c"number".as_ptr() as *const u8,
            c"expr".as_ptr() as *const u8,
        ];
        let metadata = [0, 0, 0];
        let public_symbol_map = [0, 2, 1];
        let language = TSLanguage {
            symbol_count: 3,
            token_count: 2,
            symbol_names: names.as_ptr(),
            symbol_metadata: metadata.as_ptr(),
            public_symbol_map: public_symbol_map.as_ptr(),
            ..FIELD_LANGUAGE
        };

        let expected = expected_symbol_names_for_diagnostic(&language, &[1]);

        assert_eq!(expected, vec!["number".to_string()]);
    }

    #[test]
    fn expected_symbol_names_for_diagnostic_normalize_hidden_pattern_names() {
        let names = [
            c"end".as_ptr() as *const u8,
            c"_/[a-z_]+/".as_ptr() as *const u8,
        ];
        let metadata = [0, 0];
        let language = TSLanguage {
            symbol_count: 2,
            token_count: 2,
            symbol_names: names.as_ptr(),
            symbol_metadata: metadata.as_ptr(),
            ..FIELD_LANGUAGE
        };

        let expected = expected_symbol_names_for_diagnostic(&language, &[1]);

        assert_eq!(expected, vec![r"/[a-z_]+/".to_string()]);
    }

    #[test]
    #[cfg(feature = "glr")]
    fn given_parse_node_with_known_field_name_when_converting_then_field_id_is_preserved() {
        let parse_node = crate::parser_v4::ParseNode {
            symbol: adze_ir::SymbolId(1),
            symbol_id: adze_ir::SymbolId(1),
            start_byte: 0,
            end_byte: 1,
            field_name: Some("name".to_string()),
            alias_symbol_id: None,
            children: vec![],
        };

        let converted = convert_parse_node_v4_to_pure(&parse_node, &FIELD_LANGUAGE, b"x");
        assert_eq!(converted.field_id, Some(1));
    }

    #[test]
    #[cfg(feature = "glr")]
    fn given_nested_parse_nodes_with_field_names_when_converting_then_nested_field_ids_are_preserved()
     {
        let parse_node = crate::parser_v4::ParseNode {
            symbol: adze_ir::SymbolId(1),
            symbol_id: adze_ir::SymbolId(1),
            start_byte: 0,
            end_byte: 2,
            field_name: None,
            alias_symbol_id: None,
            children: vec![crate::parser_v4::ParseNode {
                symbol: adze_ir::SymbolId(2),
                symbol_id: adze_ir::SymbolId(2),
                start_byte: 0,
                end_byte: 2,
                field_name: Some("value".to_string()),
                alias_symbol_id: None,
                children: vec![crate::parser_v4::ParseNode {
                    symbol: adze_ir::SymbolId(3),
                    symbol_id: adze_ir::SymbolId(3),
                    start_byte: 0,
                    end_byte: 1,
                    field_name: Some("name".to_string()),
                    alias_symbol_id: None,
                    children: vec![],
                }],
            }],
        };

        let converted = convert_parse_node_v4_to_pure(&parse_node, &FIELD_LANGUAGE, b"xy");
        assert_eq!(converted.children[0].field_id, Some(0));
        assert_eq!(converted.children[0].children[0].field_id, Some(1));
    }

    #[test]
    #[cfg(feature = "glr")]
    fn given_parse_node_with_missing_or_unknown_field_when_converting_then_field_id_is_none() {
        let absent = crate::parser_v4::ParseNode {
            symbol: adze_ir::SymbolId(1),
            symbol_id: adze_ir::SymbolId(1),
            start_byte: 0,
            end_byte: 1,
            field_name: None,
            alias_symbol_id: None,
            children: vec![],
        };
        let unknown = crate::parser_v4::ParseNode {
            symbol: adze_ir::SymbolId(1),
            symbol_id: adze_ir::SymbolId(1),
            start_byte: 0,
            end_byte: 1,
            field_name: Some("does_not_exist".to_string()),
            alias_symbol_id: None,
            children: vec![],
        };

        let converted_absent = convert_parse_node_v4_to_pure(&absent, &FIELD_LANGUAGE, b"x");
        let converted_unknown = convert_parse_node_v4_to_pure(&unknown, &FIELD_LANGUAGE, b"x");
        assert_eq!(converted_absent.field_id, None);
        assert_eq!(converted_unknown.field_id, None);
    }
}
