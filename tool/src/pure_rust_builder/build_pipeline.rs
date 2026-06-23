use super::{
    BuildOptions, BuildResult, compute_build_stats, desugar_pattern_wrappers,
    open_builder_debug_file,
};
use adze_glr_core::{FirstFollowSets, ParseTable, build_lr1_automaton};
use adze_ir::{Grammar, Symbol, SymbolId};
use adze_tablegen::{AbiLanguageBuilder, NodeTypesGenerator, TypedCstGenerator};
use anyhow::{Context, Result};
use proc_macro2::TokenStream;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use adze_glr_core::Action;

/// Coordinates the pure-Rust parser build while delegating each stage to an
/// SRP-oriented helper method.
pub(super) struct BuildPipeline {
    grammar: Grammar,
    options: BuildOptions,
    grammar_name: String,
}

impl BuildPipeline {
    pub(super) fn new(grammar: Grammar, options: BuildOptions) -> Self {
        let grammar_name = grammar.name.clone();
        Self {
            grammar,
            options,
            grammar_name,
        }
    }

    pub(super) fn run(mut self) -> Result<BuildResult> {
        let first_follow = self.prepare_grammar()?;
        let mut debug_file = open_builder_debug_file(&self.grammar_name);
        self.write_grammar_debug(&mut debug_file);

        let parse_table = self.build_parse_table(&first_follow)?;
        self.write_parse_table_debug(&parse_table, &mut debug_file);

        let language_code = self.generate_language_code(&parse_table, &first_follow)?;
        let node_types_json = self.generate_node_types_json()?;
        let parser_path = self.write_outputs(&parse_table, &language_code, &node_types_json)?;

        Ok(BuildResult {
            grammar_name: self.grammar_name,
            parser_path: parser_path.to_string_lossy().to_string(),
            parser_code: language_code.to_string(),
            node_types_json,
            build_stats: compute_build_stats(&parse_table),
        })
    }

    fn prepare_grammar(&mut self) -> Result<FirstFollowSets> {
        // Ensure the grammar has a symbol registry.
        let _ = self.grammar.get_or_build_registry();

        // Desugar pattern wrappers into unit productions before computing sets
        // so the LR items expose terminal lookaheads.
        desugar_pattern_wrappers(&mut self.grammar)?;

        FirstFollowSets::compute(&self.grammar)
            .with_context(|| "Failed to compute FIRST/FOLLOW sets")
    }

    fn build_parse_table(&self, first_follow: &FirstFollowSets) -> Result<ParseTable> {
        match build_lr1_automaton(&self.grammar, first_follow) {
            Ok(table) => {
                // Apply standard table normalization:
                // 1. Normalize EOF to SymbolId(0)
                // 2. Auto-detect and set appropriate GOTO indexing mode
                let mut normalized = table.normalize_eof_to_zero().with_detected_goto_indexing();

                // Resolve empty-repeat shift-reduce conflicts at table-generation
                // time so pure_parser.rs (which has no Fork handling) never sees them.
                // When a _vec_contents empty-reduce conflicts with a shift and the
                // lookahead token is in the FIRST set of the repeated element, keep
                // only the Shift action (prefer shift over empty-reduce).
                resolve_vec_wrapper_conflicts(&mut normalized, &self.grammar, first_follow);

                // Ensure invariants
                debug_assert_eq!(normalized.eof_symbol, SymbolId(0));
                debug_assert!(
                    normalized
                        .symbol_to_index
                        .contains_key(&normalized.eof_symbol)
                );

                Ok(normalized)
            }
            Err(e) => {
                eprintln!(
                    "ERROR building LR(1) automaton for {}: {}",
                    self.grammar_name, e
                );
                eprintln!(
                    "Grammar stats: {} tokens, {} rules, {} externals",
                    self.grammar.tokens.len(),
                    self.grammar.rules.len(),
                    self.grammar.externals.len()
                );
                Err(anyhow::anyhow!("Failed to build LR(1) automaton: {}", e))
            }
        }
    }

    fn generate_language_code(
        &self,
        parse_table: &ParseTable,
        first_follow: &FirstFollowSets,
    ) -> Result<TokenStream> {
        let language_code = self.generate_abi_language_code(parse_table, first_follow)?;
        let parse_document_code = quote::quote! {
            /// Parse source into the native Adze document alpha.
            pub fn parse_document(
                input: &str,
            ) -> core::result::Result<::adze::document::AdzeDocument, Vec<::adze::errors::ParseError>> {
                ::adze::__private::parse_document(input, || &LANGUAGE, GRAMMAR_NAME)
            }
        };
        let typed_cst_code = TypedCstGenerator::new(&self.grammar).generate();

        Ok(quote::quote! {
            #language_code
            #parse_document_code
            #typed_cst_code
        })
    }

    fn generate_abi_language_code(
        &self,
        parse_table: &ParseTable,
        first_follow: &FirstFollowSets,
    ) -> Result<TokenStream> {
        if self.options.compress_tables {
            use adze_tablegen::compress::TableCompressor;

            let compressor = TableCompressor::new();
            let token_indices =
                adze_tablegen::helpers::collect_token_indices(&self.grammar, parse_table);
            let start_can_be_empty = self
                .grammar
                .start_symbol()
                .map(|sym| first_follow.is_nullable(sym))
                .unwrap_or(false);
            let compressed_tables = compressor
                .compress(parse_table, &token_indices, start_can_be_empty)
                .map_err(|e| anyhow::anyhow!("Failed to compress tables: {}", e))?;

            Ok(AbiLanguageBuilder::new(&self.grammar, parse_table)
                .with_compressed_tables(&compressed_tables)
                .generate())
        } else {
            Ok(AbiLanguageBuilder::new(&self.grammar, parse_table).generate())
        }
    }

    fn generate_node_types_json(&self) -> Result<String> {
        NodeTypesGenerator::new(&self.grammar)
            .generate()
            .map_err(|e| anyhow::anyhow!("Failed to generate NODE_TYPES: {}", e))
    }

    fn write_outputs(
        &self,
        parse_table: &ParseTable,
        language_code: &TokenStream,
        node_types_json: &str,
    ) -> Result<PathBuf> {
        let grammar_dir =
            Path::new(&self.options.out_dir).join(format!("grammar_{}", self.grammar_name));

        if self.options.emit_artifacts {
            self.write_debug_artifacts(&grammar_dir, parse_table, node_types_json)?;
        }

        self.ensure_grammar_dir(&grammar_dir)?;
        self.write_parser_module(&grammar_dir, language_code)
    }

    fn write_debug_artifacts(
        &self,
        grammar_dir: &Path,
        parse_table: &ParseTable,
        node_types_json: &str,
    ) -> Result<()> {
        if grammar_dir.exists() {
            fs::remove_dir_all(grammar_dir).context("Failed to remove old grammar directory")?;
        }
        fs::create_dir_all(grammar_dir).context("Failed to create grammar directory")?;

        let grammar_ir_path = grammar_dir.join("grammar.ir.json");
        let mut grammar_ir_file = fs::File::create(&grammar_ir_path)?;
        grammar_ir_file.write_all(serde_json::to_string_pretty(&self.grammar)?.as_bytes())?;

        let node_types_path = grammar_dir.join("NODE_TYPES.json");
        let mut node_types_file = fs::File::create(&node_types_path)?;
        node_types_file.write_all(node_types_json.as_bytes())?;

        self.write_serialized_parse_table(grammar_dir, parse_table)
    }

    #[cfg(feature = "serialization")]
    fn write_serialized_parse_table(
        &self,
        grammar_dir: &Path,
        parse_table: &ParseTable,
    ) -> Result<()> {
        use adze_tablegen::ParsetableWriter;

        // Extract version from Cargo.toml if available, otherwise use "0.1.0".
        let grammar_version =
            std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".to_string());
        let writer = ParsetableWriter::new(
            &self.grammar,
            parse_table,
            &self.grammar_name,
            &grammar_version,
        );

        let parsetable_path = grammar_dir.join(format!("{}.parsetable", self.grammar_name));
        writer.write_file(&parsetable_path).with_context(|| {
            format!("Failed to write .parsetable file to {:?}", parsetable_path)
        })?;

        let mut debug_file = open_builder_debug_file(&self.grammar_name);
        debug_file_writeln!(
            debug_file,
            "Generated .parsetable file: {}",
            parsetable_path.display()
        );

        Ok(())
    }

    #[cfg(not(feature = "serialization"))]
    fn write_serialized_parse_table(
        &self,
        _grammar_dir: &Path,
        _parse_table: &ParseTable,
    ) -> Result<()> {
        Ok(())
    }

    fn ensure_grammar_dir(&self, grammar_dir: &Path) -> Result<()> {
        if !grammar_dir.exists() {
            fs::create_dir_all(grammar_dir).with_context(|| {
                format!("Failed to create grammar directory at {:?}", grammar_dir)
            })?;
        }

        Ok(())
    }

    fn write_parser_module(
        &self,
        grammar_dir: &Path,
        language_code: &TokenStream,
    ) -> Result<PathBuf> {
        let parser_module_name = format!(
            "parser_{}.rs",
            self.grammar_name.to_lowercase().replace('-', "_")
        );
        let parser_path = grammar_dir.join(&parser_module_name);
        let mut parser_file = fs::File::create(&parser_path)
            .with_context(|| format!("Failed to create parser file at {:?}", parser_path))?;

        self.write_parser_header(&mut parser_file)?;
        self.write_formatted_language_code(&mut parser_file, language_code)?;

        Ok(parser_path)
    }

    fn write_parser_header(&self, parser_file: &mut fs::File) -> Result<()> {
        writeln!(
            parser_file,
            "// Auto-generated parser for {}",
            self.grammar_name
        )?;
        writeln!(parser_file, "// Generated by adze pure-Rust builder")?;
        writeln!(parser_file)?;
        writeln!(
            parser_file,
            "/// Grammar name for external scanner registration"
        )?;
        writeln!(
            parser_file,
            "pub const GRAMMAR_NAME: &str = {:?};",
            self.grammar_name
        )?;
        writeln!(parser_file)?;

        Ok(())
    }

    fn write_formatted_language_code(
        &self,
        parser_file: &mut fs::File,
        language_code: &TokenStream,
    ) -> Result<()> {
        use prettyplease::unparse as pretty_unparse;
        use syn::{File, parse2};

        let file_ast: File =
            parse2(language_code.clone()).expect("generator must produce a parsable Rust file");
        let formatted = pretty_unparse(&file_ast);

        // Always end with a trailing newline (avoids include! edge cases).
        if formatted.ends_with('\n') {
            write!(parser_file, "{}", formatted)?;
        } else {
            writeln!(parser_file, "{}", formatted)?;
        }

        Ok(())
    }

    fn write_grammar_debug(&self, debug_file: &mut Option<fs::File>) {
        debug_file_writeln!(
            debug_file,
            "Debug: Grammar has {} tokens, {} rules",
            self.grammar.tokens.len(),
            self.grammar.rules.len()
        );
        debug_file_writeln!(
            debug_file,
            "Debug: Token names: {:?}",
            self.grammar
                .tokens
                .values()
                .map(|t| &t.name)
                .collect::<Vec<_>>()
        );
        debug_file_writeln!(
            debug_file,
            "Debug: Rule names: {:?}",
            self.grammar.rule_names.values().collect::<Vec<_>>()
        );

        debug_file_writeln!(
            debug_file,
            "Debug: Symbol name to ID mapping in grammar.rule_names:"
        );
        for (symbol_id, name) in &self.grammar.rule_names {
            debug_file_writeln!(debug_file, "  '{}' -> SymbolId({})", name, symbol_id.0);
        }

        self.write_desugaring_debug(debug_file);
    }

    fn write_desugaring_debug(&self, debug_file: &mut Option<fs::File>) {
        debug_file_writeln!(debug_file, "Debug: All rules in grammar:");
        let mut wrappers_with_rules = 0;
        let mut wrappers_without_rules = Vec::new();

        for (symbol_id, rules) in &self.grammar.rules {
            debug_file_writeln!(
                debug_file,
                "  Symbol {:?} has {} rules:",
                symbol_id,
                rules.len()
            );
            for rule in rules {
                debug_file_writeln!(debug_file, "    {:?} -> {:?}", rule.lhs, rule.rhs);
            }

            if let Some(name) = self.grammar.rule_names.get(symbol_id) {
                if rules.len() == 1 && rules[0].rhs.len() == 1 {
                    if let Symbol::Terminal(_) = &rules[0].rhs[0] {
                        wrappers_with_rules += 1;
                        debug_file_writeln!(
                            debug_file,
                            "    -> This appears to be a desugared wrapper"
                        );
                    }
                } else if rules.is_empty() {
                    wrappers_without_rules.push((symbol_id, name.clone()));
                }
            }
        }

        if !wrappers_without_rules.is_empty() {
            debug_file_writeln!(
                debug_file,
                "WARNING: {} non-terminals have no rules:",
                wrappers_without_rules.len()
            );
            for (id, name) in &wrappers_without_rules {
                debug_file_writeln!(debug_file, "  - Symbol {:?}: {}", id, name);
            }
        }

        debug_file_writeln!(
            debug_file,
            "Debug: Found {} desugared wrappers",
            wrappers_with_rules
        );
    }

    fn write_parse_table_debug(&self, parse_table: &ParseTable, debug_file: &mut Option<fs::File>) {
        debug_file_writeln!(
            debug_file,
            "Debug: Parse table has {} states, {} symbols",
            parse_table.state_count,
            parse_table.symbol_count
        );
        debug_file_writeln!(
            debug_file,
            "Debug: token_count={}, external_token_count={}",
            parse_table.token_count,
            parse_table.external_token_count
        );
        debug_file_writeln!(
            debug_file,
            "Debug: Action table has {} entries",
            parse_table.action_table.len()
        );
        debug_file_writeln!(
            debug_file,
            "Debug: Goto table has {} entries",
            parse_table.goto_table.len()
        );

        self.write_terminal_mapping_debug(parse_table, debug_file);
        self.write_action_table_debug(parse_table, debug_file);
        self.write_state_zero_debug(parse_table);
    }

    fn write_terminal_mapping_debug(
        &self,
        parse_table: &ParseTable,
        debug_file: &mut Option<fs::File>,
    ) {
        let unmapped_terminals: Vec<_> = self
            .grammar
            .tokens
            .keys()
            .filter(|token_id| !parse_table.symbol_to_index.contains_key(token_id))
            .collect();

        if !unmapped_terminals.is_empty() {
            debug_file_writeln!(
                debug_file,
                "ERROR: {} terminals not in symbol_to_index:",
                unmapped_terminals.len()
            );
            for tid in &unmapped_terminals {
                let name = self
                    .grammar
                    .tokens
                    .get(*tid)
                    .map(|t| t.name.as_str())
                    .unwrap_or("<unknown>");
                debug_file_writeln!(debug_file, "  - Token {:?}: {}", tid, name);
            }
            eprintln!(
                "ERROR: {} terminals not mapped in parse table",
                unmapped_terminals.len()
            );
        }
    }

    fn write_action_table_debug(
        &self,
        parse_table: &ParseTable,
        debug_file: &mut Option<fs::File>,
    ) {
        debug_file_writeln!(debug_file, "Debug: Action table contents:");
        for (state_idx, state_actions) in parse_table.action_table.iter().enumerate() {
            debug_file_writeln!(debug_file, "  State {}: {:?}", state_idx, state_actions);
        }

        for (state_idx, actions) in parse_table.action_table.iter().enumerate() {
            let non_error_actions: Vec<_> = actions
                .iter()
                .enumerate()
                .filter(|(_, a)| !a.is_empty())
                .collect();
            if !non_error_actions.is_empty() {
                debug_file_writeln!(
                    debug_file,
                    "Debug: State {} has {} non-error actions",
                    state_idx,
                    non_error_actions.len()
                );
            }
        }
    }

    fn write_state_zero_debug(&self, parse_table: &ParseTable) {
        let Some(state0_actions) = parse_table.action_table.first() else {
            return;
        };

        debug_trace!(
            "State 0 debug: {} action cells, {} tokens",
            state0_actions.len(),
            self.grammar.tokens.len()
        );

        let mut token_actions = 0;
        for (symbol_idx, action_cell) in state0_actions.iter().enumerate() {
            if !action_cell.is_empty() {
                for (sym_id, idx) in &parse_table.symbol_to_index {
                    if *idx == symbol_idx && self.grammar.tokens.contains_key(sym_id) {
                        token_actions += 1;
                        break;
                    }
                }
            }
        }

        if token_actions > 0 {
            debug_trace!(
                "State 0 has {} token actions - parser can accept input ✓",
                token_actions
            );
        } else {
            debug_trace!("WARNING: State 0 has no token actions - parser cannot accept input!");
        }
    }
}

/// Resolve empty-repeat shift-reduce conflicts at table-generation time.
///
/// When a grammar has `declarations: Vec<Declaration>` (a repeat that can be
/// empty), the LR(1) table produces a shift-reduce conflict: after parsing the
/// preceding element (e.g. `package main`), the parser can either shift the
/// next token (continue the repeat) or reduce by the empty `_vec_contents`
/// production (terminate the repeat with zero elements).
///
/// The pure-rust parser (`pure_parser.rs`) has no GLR fork handling, so it
/// cannot explore both branches. This function resolves such conflicts by
/// keeping only the Shift action when the lookahead token is in the FIRST set
/// of the repeated element. This makes the parser prefer continuing the repeat
/// over terminating it with an empty list when the next token could start a
/// new element.
fn resolve_vec_wrapper_conflicts(
    table: &mut ParseTable,
    grammar: &Grammar,
    first_follow: &FirstFollowSets,
) {
    // Build the set of "repeat-starter" terminal IDs: tokens that begin a
    // repeated element in any _vec_contents rule. When a shift-reduce conflict
    // involves such a token, prefer shift over the empty reduce.
    let mut repeat_starters: std::collections::HashSet<SymbolId> = std::collections::HashSet::new();

    for (rule_sym, rules) in &grammar.rules {
        let is_vec_contents = grammar
            .rule_names
            .iter()
            .any(|(sid, name)| sid == rule_sym && name.ends_with("_vec_contents"));
        if !is_vec_contents {
            continue;
        }
        for rule in rules {
            if rule.rhs.is_empty() {
                continue;
            }
            for sym in &rule.rhs {
                if let Symbol::NonTerminal(sid) = sym
                    && let Some(first_set) = first_follow.first(*sid)
                {
                    for idx in first_set.ones() {
                        repeat_starters.insert(SymbolId(idx as u16));
                    }
                }
            }
        }
    }

    if repeat_starters.is_empty() {
        return;
    }

    // Walk the action table and resolve conflicts where a cell has both
    // a Shift and a Reduce, and the terminal for that column is a repeat-starter.
    for state in 0..table.action_table.len() {
        for sym_idx in 0..table.action_table[state].len() {
            let cell = &mut table.action_table[state][sym_idx];
            if cell.len() < 2 {
                continue;
            }
            let has_shift = cell.iter().any(|a| matches!(a, Action::Shift(_)));
            let has_reduce = cell.iter().any(|a| matches!(a, Action::Reduce(_)));
            if !has_shift || !has_reduce {
                continue;
            }
            // Check if this column's terminal is a repeat-starter
            let terminal_id = SymbolId(sym_idx as u16);
            if !repeat_starters.contains(&terminal_id) {
                continue;
            }
            // Keep only the Shift action, drop the Reduce (and any Fork/Error)
            let shift_action = cell
                .iter()
                .find(|a| matches!(a, Action::Shift(_)))
                .cloned();
            if let Some(shift) = shift_action {
                cell.clear();
                cell.push(shift);
            }
        }
    }
}
