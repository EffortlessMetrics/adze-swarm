//! Route conflicted generated grammars through the stack-aware streaming driver (#857 / #891).

#![cfg(all(feature = "glr", feature = "pure-rust"))]

use crate::decoder::{decode_grammar, decode_rule_fields};
use crate::glr_parser::{
    AlternativeSummary, AmbiguitySummary, SelectionReason, subtree_node_count,
    subtree_selection_key,
};
use crate::glr_streaming_internal_lexer::make_generated_internal_streaming_lexer;
use crate::glr_streaming_lex_contract::{
    conflict_shift_targets_require_distinct_lex_modes, distinct_internal_lex_states,
    fixed_mode_bridge_uses_only_state_zero_lex_mode,
    grammar_requires_stack_aware_streaming_lex_contract,
};
use crate::pure_parser::TSLanguage;
use crate::subtree::{ChildEdge, FIELD_NONE, Subtree, SubtreeNode};
use adze_glr_core::parse_forest::ERROR_SYMBOL;
use adze_glr_core::{
    Driver, ForestView, LexMode, ParseTable, build_lex_modes_from_shiftable_terminals,
    driver::GlrError,
};
use adze_ir::{Grammar, Symbol, SymbolId};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

#[cfg(feature = "external_scanners")]
use crate::glr_streaming_external_scanner::make_generated_external_streaming_scanner;

/// Which engine executed the most recent true-GLR parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrueGlrParseRoute {
    /// Legacy whole-input state-0 pretokenization bridge.
    FixedPretokenizationBridge,
    /// Stack-aware `Driver::parse_streaming` path.
    StreamingDriver,
}

const ROUTE_UNSET: u8 = 0;
const ROUTE_FIXED_BRIDGE: u8 = 1;
const ROUTE_STREAMING: u8 = 2;

static LAST_TRUE_GLR_ROUTE: AtomicU8 = AtomicU8::new(ROUTE_UNSET);

/// Returns the route used by the latest true-GLR parse in this process.
pub fn last_true_glr_parse_route() -> Option<TrueGlrParseRoute> {
    match LAST_TRUE_GLR_ROUTE.load(Ordering::SeqCst) {
        ROUTE_FIXED_BRIDGE => Some(TrueGlrParseRoute::FixedPretokenizationBridge),
        ROUTE_STREAMING => Some(TrueGlrParseRoute::StreamingDriver),
        _ => None,
    }
}

fn record_route(route: TrueGlrParseRoute) {
    let encoded = match route {
        TrueGlrParseRoute::FixedPretokenizationBridge => ROUTE_FIXED_BRIDGE,
        TrueGlrParseRoute::StreamingDriver => ROUTE_STREAMING,
    };
    LAST_TRUE_GLR_ROUTE.store(encoded, Ordering::SeqCst);
}

/// Selected parse tree and optional ambiguity summary from the streaming driver.
pub struct StreamingGlrParseResult {
    /// Deterministically selected root subtree.
    pub root: Arc<Subtree>,
    /// Retained ambiguity metadata when multiple complete roots exist.
    pub ambiguities: Option<AmbiguitySummary>,
}

/// Returns whether a conflicted table should execute through the streaming driver now.
pub fn should_route_conflict_table_through_streaming_driver(
    language: &'static TSLanguage,
    parse_table: &ParseTable,
) -> bool {
    if language.lex_fn.is_none() {
        return false;
    }

    let prepared = prepare_streaming_parse_table(language, parse_table.clone());
    if prepared
        .lex_modes
        .iter()
        .any(|mode| mode.external_lex_state != 0)
    {
        return true;
    }

    if !conflict_shift_targets_require_distinct_lex_modes(&prepared) {
        return false;
    }

    let grammar = decode_grammar(language);
    grammar_requires_stack_aware_streaming_lex_contract(&grammar)
        && distinct_internal_lex_states(&prepared).len() >= 2
        && fixed_mode_bridge_uses_only_state_zero_lex_mode(language)
}

/// Prepare a conflicted parse table for stack-aware streaming execution.
pub fn prepare_streaming_parse_table(
    language: &'static TSLanguage,
    mut parse_table: ParseTable,
) -> ParseTable {
    crate::__private::align_true_glr_parse_table_to_language_symbols(language, &mut parse_table);
    populate_streaming_parse_table_lex_modes(language, &mut parse_table);
    parse_table
}

/// Select lex modes that match the generated lexer ABI instead of inventing
/// shiftable-terminal signatures that the generated `lex_fn` cannot execute.
fn populate_streaming_parse_table_lex_modes(
    language: &'static TSLanguage,
    parse_table: &mut ParseTable,
) {
    if fixed_mode_bridge_uses_only_state_zero_lex_mode(language) {
        let base_mode = generated_language_base_lex_mode(language);
        let state_count = usize::try_from(parse_table.state_count).unwrap_or(0);
        parse_table.lex_modes = vec![base_mode; state_count];
        return;
    }

    if language_lex_modes_match_state_count(language, parse_table.state_count) {
        parse_table.lex_modes = lex_modes_from_generated_language(language);
        return;
    }

    parse_table.lex_modes = build_lex_modes_from_shiftable_terminals(
        &parse_table.action_table,
        &parse_table.external_scanner_states,
    );
}

fn generated_language_base_lex_mode(language: &TSLanguage) -> LexMode {
    if language.lex_modes.is_null() {
        return LexMode {
            lex_state: 0,
            external_lex_state: 0,
        };
    }

    // SAFETY: generated languages expose one lex mode per parser state and the
    // fixed-bridge contract guarantees every state shares the same mode.
    let mode = unsafe { *language.lex_modes };
    LexMode {
        lex_state: mode.lex_state,
        external_lex_state: mode.external_lex_state,
    }
}

fn language_lex_modes_match_state_count(language: &TSLanguage, state_count: u32) -> bool {
    !language.lex_modes.is_null()
        && language.state_count > 0
        && language.state_count == state_count
}

fn lex_modes_from_generated_language(language: &TSLanguage) -> Vec<LexMode> {
    (0..language.state_count)
        .map(|state| {
            // SAFETY: `state < state_count` and generated languages expose one
            // lex mode per parser state.
            let mode = unsafe { *language.lex_modes.add(state as usize) };
            LexMode {
                lex_state: mode.lex_state,
                external_lex_state: mode.external_lex_state,
            }
        })
        .collect()
}

/// Parse conflicted input through `Driver::parse_streaming` when a generated lexer exists.
pub fn parse_with_streaming_driver(
    input: &str,
    language: &'static TSLanguage,
    parse_table: ParseTable,
    grammar: &Grammar,
) -> Result<StreamingGlrParseResult, GlrError> {
    if language.lex_fn.is_none() {
        return Err(GlrError::Lex(
            "generated language is missing lex_fn for streaming driver".to_string(),
        ));
    }

    record_route(TrueGlrParseRoute::StreamingDriver);
    let parse_table = prepare_streaming_parse_table(language, parse_table);
    let mut driver = Driver::new(&parse_table);
    let mut internal_lexer = make_generated_internal_streaming_lexer(language);

    let forest =
        parse_with_optional_external_scanner(&mut driver, input, language, &mut internal_lexer)?;

    materialize_streaming_forest(&forest, language, &parse_table, grammar)
}

/// Select a complete alternative from an already-produced streaming [`Forest`].
///
/// This is the #930 adapter seam: production routing produces a forest, then this
/// function materializes deterministic selected-tree + ambiguity facts without
/// depending on lexer/driver token commitment.
pub fn materialize_streaming_forest(
    forest: &adze_glr_core::Forest,
    language: &'static TSLanguage,
    parse_table: &ParseTable,
    grammar: &Grammar,
) -> Result<StreamingGlrParseResult, GlrError> {
    let view = forest.view();
    let roots = view.roots();
    if roots.is_empty() {
        return Err(GlrError::Parse(
            "streaming forest produced no complete parse roots".to_string(),
        ));
    }

    let mut alternatives = Vec::with_capacity(roots.len());
    let mut subtrees = Vec::with_capacity(roots.len());
    for (index, &root_id) in roots.iter().enumerate() {
        let subtree = forest_view_to_subtree(view, root_id, language, parse_table, grammar);
        let span = view.span(root_id);
        alternatives.push(AlternativeSummary {
            index,
            root_symbol: SymbolId(view.kind(root_id) as u16),
            span: span.start as usize..span.end as usize,
            dynamic_precedence: subtree.dynamic_prec,
            in_error: subtree.node.is_error,
            cost: 0,
            node_count: subtree_node_count(&subtree),
        });
        subtrees.push(subtree);
    }

    let selected_index = select_streaming_root_index(&subtrees);
    let ambiguities = if roots.len() > 1 {
        let mut span_start = usize::MAX;
        let mut span_end = 0usize;
        for alt in &alternatives {
            span_start = span_start.min(alt.span.start);
            span_end = span_end.max(alt.span.end);
        }
        Some(AmbiguitySummary {
            span: span_start..span_end,
            alternatives,
            selected: Some(selected_index),
            selection_reason: SelectionReason::StableStructuralTieBreak,
        })
    } else {
        None
    };

    let root = subtrees.get(selected_index).cloned().ok_or_else(|| {
        GlrError::Parse(format!(
            "streaming forest selected index {selected_index} is out of range for {} roots",
            roots.len()
        ))
    })?;

    if root.node.is_error {
        return Err(GlrError::Parse(
            "streaming forest selected an error-only root without a complete parse".to_string(),
        ));
    }

    Ok(StreamingGlrParseResult { root, ambiguities })
}

/// Materialize an [`crate::document::AdzeDocument`] from a streaming forest (#930).
///
/// Callable independently of production parser routing: callers supply an already
/// produced forest plus language metadata.
pub fn materialize_streaming_forest_document(
    source: &str,
    forest: &adze_glr_core::Forest,
    language: &'static TSLanguage,
    grammar_name: &str,
    grammar: &Grammar,
    parse_table: &ParseTable,
) -> Result<crate::document::AdzeDocument, GlrError> {
    let parsed = materialize_streaming_forest(forest, language, parse_table, grammar)?;
    Ok(crate::__private::adze_document_from_streaming_parse(
        source,
        parsed,
        language,
        grammar_name,
        grammar,
        parse_table,
    ))
}

fn parse_with_optional_external_scanner<L>(
    driver: &mut Driver<'_>,
    input: &str,
    #[cfg_attr(
        not(feature = "external_scanners"),
        expect(
            unused_variables,
            reason = "policy:pr4-external-gate: language only needed when external_scanners is enabled"
        )
    )]
    language: &'static TSLanguage,
    internal_lexer: L,
) -> Result<adze_glr_core::Forest, GlrError>
where
    L: FnMut(&str, usize, LexMode) -> Option<adze_glr_core::ts_lexer::NextToken>,
{
    #[cfg(feature = "external_scanners")]
    {
        if language.external_scanner.scan.is_some() {
            let mut external_scanner = make_generated_external_streaming_scanner(language);
            return driver.parse_streaming(
                input,
                internal_lexer,
                Some(move |scan_input, pos, valid, mode| {
                    external_scanner
                        .scan_at(scan_input, pos, valid, mode)
                        .ok()
                        .flatten()
                }),
            );
        }
    }

    driver.parse_streaming(
        input,
        internal_lexer,
        None::<fn(&str, usize, &[bool], LexMode) -> Option<adze_glr_core::ts_lexer::NextToken>>,
    )
}

fn select_streaming_root_index(subtrees: &[Arc<Subtree>]) -> usize {
    let mut best_index = 0usize;
    let mut best_key = subtree_selection_key(&subtrees[0]);
    for (index, subtree) in subtrees.iter().enumerate().skip(1) {
        let key = subtree_selection_key(subtree);
        if key < best_key {
            best_index = index;
            best_key = key;
        }
    }
    best_index
}

fn forest_view_to_subtree(
    view: &dyn ForestView,
    node_id: u32,
    language: &'static TSLanguage,
    parse_table: &ParseTable,
    grammar: &Grammar,
) -> Arc<Subtree> {
    #[derive(Clone, Copy)]
    struct PendingNode {
        id: u32,
        expanded: bool,
    }

    let mut pending = vec![PendingNode {
        id: node_id,
        expanded: false,
    }];
    let mut built: Vec<Arc<Subtree>> = Vec::new();

    while let Some(node) = pending.pop() {
        if !node.expanded {
            pending.push(PendingNode {
                id: node.id,
                expanded: true,
            });
            for &child_id in view.best_children(node.id).iter().rev() {
                pending.push(PendingNode {
                    id: child_id,
                    expanded: false,
                });
            }
            continue;
        }

        let span = view.span(node.id);
        let symbol = SymbolId(view.kind(node.id) as u16);
        let error_meta = view.error_meta(node.id);
        let child_ids = view.best_children(node.id);
        let split_at = built.len().saturating_sub(child_ids.len());
        let child_subtrees = built.split_off(split_at);

        let child_symbols = child_subtrees
            .iter()
            .map(|child| child.node.symbol_id)
            .collect::<Vec<_>>();
        let rule_id = match_rule_id(parse_table, grammar, symbol, &child_symbols);
        let dynamic_prec = rule_id
            .and_then(|id| parse_table.dynamic_prec_by_rule.get(id))
            .copied()
            .unwrap_or(0) as i32;

        let children = if child_subtrees.is_empty() {
            Vec::new()
        } else if let Some(rule_id) = rule_id {
            let fields = fields_for_production(language, parse_table, rule_id);
            child_subtrees
                .into_iter()
                .enumerate()
                .map(|(child_index, subtree)| {
                    let field_id = fields
                        .iter()
                        .find(|(_, position)| *position == child_index)
                        .map(|(field_id, _)| *field_id)
                        .unwrap_or(FIELD_NONE);
                    ChildEdge::new(subtree, field_id)
                })
                .collect()
        } else {
            child_subtrees
                .into_iter()
                .map(ChildEdge::new_without_field)
                .collect()
        };

        let is_error = symbol == ERROR_SYMBOL || error_meta.is_error || error_meta.missing;
        let byte_range = if error_meta.missing {
            let pos = span.start as usize;
            pos..pos
        } else {
            span.start as usize..span.end as usize
        };
        let subtree = Arc::new(Subtree::with_dynamic_prec_and_fields(
            SubtreeNode {
                symbol_id: symbol,
                is_error,
                byte_range,
            },
            children,
            dynamic_prec,
        ));
        built.push(subtree);
    }

    built
        .pop()
        .expect("forest root conversion produces one subtree")
}

fn fields_for_production(
    language: &'static TSLanguage,
    parse_table: &ParseTable,
    rule_id: usize,
) -> Vec<(u16, usize)> {
    let mut fields = decode_rule_fields(language, rule_id)
        .into_iter()
        .map(|(field_id, position)| (field_id.0, position))
        .collect::<Vec<_>>();
    if fields.is_empty() {
        let rule = adze_ir::RuleId(rule_id as u16);
        for ((mapped_rule, child_index), field_id) in &parse_table.field_map {
            if *mapped_rule == rule {
                fields.push((*field_id, *child_index as usize));
            }
        }
    }
    fields
}

fn match_rule_id(
    parse_table: &ParseTable,
    grammar: &Grammar,
    lhs: SymbolId,
    child_symbols: &[SymbolId],
) -> Option<usize> {
    if child_symbols.is_empty() {
        return parse_table
            .rules
            .iter()
            .enumerate()
            .find(|(_, rule)| rule.lhs == lhs && rule.rhs_len == 0)
            .map(|(index, _)| index);
    }

    for (rule_id, parse_rule) in parse_table.rules.iter().enumerate() {
        if parse_rule.lhs != lhs || parse_rule.rhs_len as usize != child_symbols.len() {
            continue;
        }
        if let Some(candidates) = grammar.rules.get(&lhs) {
            for rule in candidates {
                if rule.production_id.0 as usize != rule_id {
                    continue;
                }
                if rhs_symbol_ids(&rule.rhs) == child_symbols {
                    return Some(rule_id);
                }
            }
        }
    }

    parse_table
        .rules
        .iter()
        .enumerate()
        .find(|(_, rule)| rule.lhs == lhs && rule.rhs_len as usize == child_symbols.len())
        .map(|(index, _)| index)
}

fn rhs_symbol_ids(rhs: &[Symbol]) -> Vec<SymbolId> {
    rhs.iter()
        .filter_map(|symbol| match symbol {
            Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => Some(*id),
            _ => None,
        })
        .collect()
}

pub(crate) fn glr_error_to_parse_errors(
    input: &str,
    error: GlrError,
) -> Vec<crate::errors::ParseError> {
    let source_len = input.len();
    let (start, message) = match error {
        GlrError::Lex(message) | GlrError::Parse(message) | GlrError::Other(message) => {
            (extract_byte_offset(&message).unwrap_or(0), message)
        }
    };
    let end = if start < source_len {
        diagnostic_end_for_byte(input.as_bytes(), start)
    } else {
        source_len
    };
    vec![crate::errors::ParseError {
        reason: crate::errors::ParseErrorReason::UnexpectedToken(message),
        start,
        end,
        expected: vec![],
    }]
}

fn extract_byte_offset(message: &str) -> Option<usize> {
    message
        .split("byte ")
        .nth(1)
        .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|digits| digits.parse().ok())
}

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

pub(crate) fn record_fixed_bridge_route() {
    record_route(TrueGlrParseRoute::FixedPretokenizationBridge);
}
