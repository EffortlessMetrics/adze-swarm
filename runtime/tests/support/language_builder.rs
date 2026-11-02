use adze::pure_parser::{ExternalScanner, TSLanguage, TSLexState, TSParseAction, TSRule};
use adze::ts_format::{TSActionTag, choose_action_with_precedence};
use adze_glr_core::{Action, ParseTable};
use adze_ir::{Grammar, StateId, Symbol, SymbolId};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::CString;
use std::os::raw::c_void;

/// Normalize a ParseTable to Tree-sitter's expectations:
/// - Dense columns 0..N-1
/// - Columns [0..tcols) are terminals, [tcols..N) are nonterminals
/// - NT gotos added as Shift actions in the action table
/// - Accept injected at GOTO(I0, start) on EOF
#[allow(dead_code)]
pub fn normalize_table_for_ts(table: &mut ParseTable) {
    // === Step 1: Dense remap with terminals first, then nonterminals ===
    let tcols = table.token_count + table.external_token_count;

    // Classify by original column kinds (token vs NT) to preserve tcols split
    let old_sym_to_col = table.symbol_to_index.clone();
    let mut terms: Vec<(SymbolId, usize)> = Vec::new();
    let mut nterms: Vec<(SymbolId, usize)> = Vec::new();

    for (&sym, &col) in &old_sym_to_col {
        if sym.0 == 65535 {
            continue;
        } // drop augmented sentinel
        if col < tcols {
            terms.push((sym, col));
        } else {
            nterms.push((sym, col));
        }
    }

    // Stable deterministic order (by SymbolId)
    terms.sort_by_key(|(s, _)| s.0);
    nterms.sort_by_key(|(s, _)| s.0);

    // Dense order = [terms..., nterms...]
    let all_syms: Vec<SymbolId> = terms.iter().chain(&nterms).map(|(s, _)| *s).collect();

    // Build dense col map
    let mut dense_col: BTreeMap<SymbolId, usize> = BTreeMap::new();
    for (i, &s) in all_syms.iter().enumerate() {
        dense_col.insert(s, i);
    }

    // Rebuild symbol_to_index/index_to_symbol
    let old_action_table = table.action_table.clone();
    let old_goto_table = table.goto_table.clone();

    table.symbol_to_index.clear();
    table.index_to_symbol.clear();
    for (i, &s) in all_syms.iter().enumerate() {
        table.symbol_to_index.insert(s, i);
        table.index_to_symbol.push(s);
    }

    // Allocate dense action/goto
    let new_cols = table.index_to_symbol.len();
    table.action_table = vec![vec![vec![]; new_cols]; table.state_count];
    table.goto_table = vec![vec![StateId(0); new_cols]; table.state_count];
    table.symbol_count = new_cols; // keep metadata consistent

    // Copy old cells → new dense positions
    for (sym, old_col) in old_sym_to_col {
        if sym.0 == 65535 {
            continue;
        }
        let Some(&new_col) = dense_col.get(&sym) else {
            continue;
        };
        for st in 0..table.state_count {
            if st < old_action_table.len() && old_col < old_action_table[st].len() {
                table.action_table[st][new_col] = old_action_table[st][old_col].clone();
            }
            if st < old_goto_table.len() && old_col < old_goto_table[st].len() {
                table.goto_table[st][new_col] = old_goto_table[st][old_col];
            }
        }
    }

    // === Step 2: Add NT gotos to action table (as Shifts) ===
    for st in 0..table.state_count {
        for nt_col in tcols..table.symbol_count {
            let next = table.goto_table[st][nt_col];
            if next != StateId(0) {
                let cell = &mut table.action_table[st][nt_col];
                if !cell
                    .iter()
                    .any(|a| matches!(a, Action::Shift(s) if *s == next))
                {
                    cell.push(Action::Shift(next));
                }
            }
        }
    }

    // === Step 3: Compute start NT and Accept injection ===
    // Candidates: NT columns in I0 that have a Shift
    let mut start_candidates: Vec<SymbolId> = Vec::new();
    for col in tcols..table.symbol_count {
        if table.action_table[0][col]
            .iter()
            .any(|a| matches!(a, Action::Shift(_)))
        {
            start_candidates.push(table.index_to_symbol[col]);
        }
    }

    // Use pre-set start symbol if provided; otherwise, fall back to heuristic
    let start_nt = if table.start_symbol != SymbolId(0) {
        table.start_symbol
    } else if let Some(s) = start_candidates.iter().max_by_key(|s| s.0) {
        *s
    } else {
        // Fallback: pick a LHS present in rules that's an NT
        let lhs_nts: BTreeSet<_> = table.rules.iter().map(|r| r.lhs).collect();
        lhs_nts
            .iter()
            .filter(|s| table.symbol_to_index.get(s).is_some_and(|&c| c >= tcols))
            .max_by_key(|s| s.0)
            .copied()
            .unwrap_or_else(|| {
                table
                    .index_to_symbol
                    .get(tcols)
                    .copied()
                    .unwrap_or(SymbolId(0))
            })
    };
    table.start_symbol = start_nt;

    // Accept state = goto(I0, start_nt)
    let start_col = *table
        .symbol_to_index
        .get(&start_nt)
        .expect("start_nt must be in symbol_to_index");
    let accept_state = table.action_table[0][start_col]
        .iter()
        .find_map(|a| {
            if let Action::Shift(s) = a {
                Some(*s)
            } else {
                None
            }
        })
        .or_else(|| {
            // Fallback to goto_table if present
            let s = table.goto_table[0][start_col];
            (s != StateId(0)).then_some(s)
        })
        .unwrap_or(StateId(1)); // last resort (nonzero to avoid [0] row)

    // Inject Accept on EOF at accept_state row
    let eof_col = *table
        .symbol_to_index
        .get(&table.eof_symbol)
        .expect("EOF must be in symbol_to_index");
    let row = accept_state.0 as usize;
    if !table.action_table[row][eof_col]
        .iter()
        .any(|a| matches!(a, Action::Accept))
    {
        table.action_table[row][eof_col].push(Action::Accept);
    }

    // === Step 4: Invariants (debug-only but cheap) ===
    #[cfg(debug_assertions)]
    {
        // Dense property: col(i) == i
        for i in 0..table.index_to_symbol.len() {
            let sym = table.index_to_symbol[i];
            let &mapped = table.symbol_to_index.get(&sym).unwrap();
            assert_eq!(mapped, i, "Non-dense mapping at column {}", i);
        }

        // Token boundary preserved: all NTs must be in [tcols..)
        for col in 0..tcols.min(table.index_to_symbol.len()) {
            assert!(
                table.index_to_symbol[col].0 != 65535,
                "Sentinel leaked into token space"
            );
        }

        // Accept exists on EOF somewhere
        let has_accept = (0..table.state_count).any(|st| {
            eof_col < table.action_table[st].len()
                && table.action_table[st][eof_col]
                    .iter()
                    .any(|a| matches!(a, Action::Accept))
        });
        assert!(has_accept, "No Accept action present on EOF");

        // No 65535 in column arrays
        assert!(
            !table.index_to_symbol.iter().any(|s| s.0 == 65535),
            "Augmented sentinel leaked into index_to_symbol"
        );
    }
}

/// Build a stable action set and return a flat parse_table (indices into `ts_actions`)
/// GLR cells may contain multiple actions; we export the first one to the TS surface.
pub fn encode_actions(parse_table: &ParseTable) -> (Vec<TSParseAction>, Vec<u16>) {
    // 0 = Error
    let mut ts_actions: Vec<TSParseAction> = vec![TSParseAction {
        action_type: TSActionTag::Error as u8,
        extra: 0,
        child_count: 0,
        dynamic_precedence: 0,
        symbol: 0,
    }];

    // Intern identical actions so we reuse indices
    let mut intern: HashMap<(u8, u8, u8, u16), u16> = HashMap::new();
    intern.insert((TSActionTag::Error as u8, 0, 0, 0), 0);

    // Helper: intern and return index
    let mut push_action = |a: TSParseAction| -> u16 {
        let key = (a.action_type, a.extra, a.child_count, a.symbol);
        if let Some(&idx) = intern.get(&key) {
            return idx;
        }
        let idx = ts_actions.len() as u16;
        ts_actions.push(a);
        intern.insert(key, idx);
        idx
    };

    // For each state×symbol pick **one** action (simple LR(1) surface)
    let mut flat: Vec<u16> =
        Vec::with_capacity(parse_table.state_count * parse_table.index_to_symbol.len());

    for s in 0..parse_table.state_count {
        for c in 0..parse_table.index_to_symbol.len() {
            let cell = parse_table
                .action_table
                .get(s)
                .and_then(|row| row.get(c))
                .cloned()
                .unwrap_or_default();

            // Use precedence-aware choose_action to pick the best action from GLR cells
            let chosen = choose_action_with_precedence(&cell, parse_table);
            let idx = if let Some(a) = chosen {
                match a {
                    Action::Shift(StateId(tgt)) => push_action(TSParseAction {
                        action_type: TSActionTag::Shift as u8,
                        extra: 0,
                        child_count: 0,
                        dynamic_precedence: 0,
                        symbol: tgt,
                    }),
                    Action::Reduce(rule_idx) => {
                        if rule_idx.0 as usize >= parse_table.rules.len() {
                            // Skip invalid rule indices
                            eprintln!(
                                "WARNING: Invalid rule index {} at state {}, col {}",
                                rule_idx.0, s, c
                            );
                            0
                        } else {
                            let pr = &parse_table.rules[rule_idx.0 as usize];
                            // Get the dynamic precedence for this rule
                            let dyn_prec =
                                if (rule_idx.0 as usize) < parse_table.dynamic_prec_by_rule.len() {
                                    parse_table.dynamic_prec_by_rule[rule_idx.0 as usize] as i8
                                } else {
                                    0
                                };
                            push_action(TSParseAction {
                                action_type: TSActionTag::Reduce as u8,
                                extra: 0,
                                child_count: pr.rhs_len as u8, // RHS length
                                dynamic_precedence: dyn_prec,
                                symbol: rule_idx.0, // Store the rule ID, not the LHS
                            })
                        }
                    }
                    Action::Accept => push_action(TSParseAction {
                        action_type: TSActionTag::Accept as u8,
                        extra: 0,
                        child_count: 0,
                        dynamic_precedence: 0,
                        symbol: 0,
                    }),
                    Action::Fork(_) => 0, // treat as error for the TS surface for now
                    _ => 0,               // treat other actions as error
                }
            } else {
                0 // error
            };
            flat.push(idx);
        }
    }

    (ts_actions, flat)
}

/// JSON lexer function for Tree-sitter
/// Handles: { } : , "string" number
#[allow(dead_code)]
unsafe extern "C" fn json_lexer(lexer: *mut c_void, _state: TSLexState) -> bool {
    use adze::lex::TsLexer;
    let lex = unsafe { &mut *(lexer as *mut TsLexer) };

    // Skip whitespace
    loop {
        let ch = unsafe { (lex.lookahead)(lex) };
        if ch == 0 {
            return false;
        }
        match ch as u8 as char {
            ' ' | '\t' | '\n' | '\r' => unsafe { (lex.advance)(lex, true) },
            _ => break,
        }
    }

    let ch = unsafe { (lex.lookahead)(lex) } as u8 as char;
    match ch {
        '{' => {
            unsafe { (lex.advance)(lex, false) };
            unsafe { (lex.mark_end)(lex) };
            lex.result_symbol = 0; // LBRACE
            true
        }
        '}' => {
            unsafe { (lex.advance)(lex, false) };
            unsafe { (lex.mark_end)(lex) };
            lex.result_symbol = 1; // RBRACE
            true
        }
        ':' => {
            unsafe { (lex.advance)(lex, false) };
            unsafe { (lex.mark_end)(lex) };
            lex.result_symbol = 2; // COLON
            true
        }
        ',' => {
            unsafe { (lex.advance)(lex, false) };
            unsafe { (lex.mark_end)(lex) };
            lex.result_symbol = 3; // COMMA
            true
        }
        '"' => {
            // STRING
            unsafe { (lex.advance)(lex, false) };
            loop {
                let la = unsafe { (lex.lookahead)(lex) };
                if la == 0 {
                    break;
                }
                if la as u8 as char == '"' {
                    unsafe { (lex.advance)(lex, false) };
                    break;
                } else if la as u8 as char == '\\' {
                    unsafe { (lex.advance)(lex, false) };
                    if unsafe { (lex.lookahead)(lex) } != 0 {
                        unsafe { (lex.advance)(lex, false) };
                    }
                } else {
                    unsafe { (lex.advance)(lex, false) };
                }
            }
            unsafe { (lex.mark_end)(lex) };
            lex.result_symbol = 4; // STRING
            true
        }
        '-' | '0'..='9' => {
            // NUMBER
            unsafe { (lex.advance)(lex, false) };
            loop {
                let la = unsafe { (lex.lookahead)(lex) };
                if la == 0 {
                    break;
                }
                let c = la as u8 as char;
                if c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-') {
                    unsafe { (lex.advance)(lex, false) };
                } else {
                    break;
                }
            }
            unsafe { (lex.mark_end)(lex) };
            lex.result_symbol = 5; // NUMBER
            true
        }
        _ => false,
    }
}

/// Build a TSLanguage from grammar and parse table
pub fn build_ts_language(grammar: &Grammar, parse_table: &ParseTable) -> TSLanguage {
    // Build symbol names as C strings (*const u8)
    let mut symbol_names_c: Vec<CString> = Vec::new();
    let mut symbol_names_ptrs: Vec<*const u8> = Vec::new();

    // Add all symbols (tokens and non-terminals)
    for i in 0..parse_table.symbol_count {
        let sym_id = parse_table.index_to_symbol[i];
        let name = grammar
            .rule_names
            .get(&sym_id)
            .cloned()
            .or_else(|| grammar.tokens.get(&sym_id).map(|t| t.name.clone()))
            .unwrap_or_else(|| format!("symbol_{}", sym_id.0));

        let c_string = CString::new(name).unwrap();
        symbol_names_ptrs.push(c_string.as_ptr() as *const u8);
        symbol_names_c.push(c_string);
    }

    // Leak the data to get static references
    let _symbol_names_c = Box::leak(Box::new(symbol_names_c));
    let symbol_names_ptrs = Box::leak(Box::new(symbol_names_ptrs));

    // Build field names as C strings (*const u8)
    let mut field_names_c: Vec<CString> = Vec::new();
    let mut field_names_ptrs: Vec<*const u8> = Vec::new();

    // First entry is always empty
    let empty = CString::new("").unwrap();
    field_names_ptrs.push(empty.as_ptr() as *const u8);
    field_names_c.push(empty);

    // Add field names in sorted order
    let mut field_ids: Vec<_> = grammar.fields.keys().cloned().collect();
    field_ids.sort_by_key(|f| f.0);
    for field_id in field_ids {
        let c_string = CString::new(grammar.fields[&field_id].clone()).unwrap();
        field_names_ptrs.push(c_string.as_ptr() as *const u8);
        field_names_c.push(c_string);
    }

    let _field_names_c = Box::leak(Box::new(field_names_c));
    let field_names_ptrs = Box::leak(Box::new(field_names_ptrs));
    let public_symbol_map = Box::leak(
        parse_table
            .index_to_symbol
            .iter()
            .map(|symbol| symbol.0)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );

    // Build symbol metadata
    let mut symbol_metadata = Vec::new();
    // Use index_to_symbol.len() instead of symbol_count to match actual array size
    for i in 0..parse_table.index_to_symbol.len() {
        if i < parse_table.symbol_metadata.len() {
            let meta = &parse_table.symbol_metadata[i];
            // Pack into a single byte: bit 0 = visible, bit 1 = named
            let byte = (meta.is_visible as u8) | ((meta.is_named as u8) << 1);
            symbol_metadata.push(byte);
        } else {
            // Default metadata for any extra symbols
            symbol_metadata.push(0);
        }
    }
    let symbol_metadata = Box::leak(Box::new(symbol_metadata));

    // Build lex modes
    let mut lex_modes = Vec::new();
    for _ in 0..parse_table.state_count {
        lex_modes.push(TSLexState {
            lex_state: 0,
            external_lex_state: 0,
        });
    }
    let lex_modes = Box::leak(Box::new(lex_modes));

    // Build parse actions & full (uncompressed) parse table with real actions
    let (ts_actions, full_parse_table) = encode_actions(parse_table);
    let parse_actions = Box::leak(Box::new(ts_actions));
    let full_parse_table = Box::leak(Box::new(full_parse_table));

    // Build productions (lhs per rule) - must use column index not symbol ID
    let mut production_lhs = Vec::new();
    for (i, r) in parse_table.rules.iter().enumerate() {
        // Map the LHS symbol to its column index
        let lhs_col = parse_table
            .symbol_to_index
            .get(&r.lhs)
            .copied()
            .unwrap_or_else(|| {
                eprintln!(
                    "WARNING: Rule {}: LHS symbol {:?} not found in symbol_to_index",
                    i, r.lhs
                );
                0
            }) as u16;
        production_lhs.push(lhs_col);
    }
    let production_lhs = Box::leak(Box::new(production_lhs));

    // Helper to create a TSRule from a rule and symbol_to_index
    fn make_ts_rule(
        r: &adze_glr_core::ParseRule,
        symbol_to_index: &BTreeMap<SymbolId, usize>,
    ) -> TSRule {
        let lhs_col = symbol_to_index.get(&r.lhs).copied().unwrap_or(0) as u16;
        TSRule {
            lhs: lhs_col,
            rhs_len: r.rhs_len as u8,
            _pad: 0,
        }
    }

    // Build TSRule array for the decoder to get rhs_len
    let mut ts_rules = Vec::new();
    for r in &parse_table.rules {
        ts_rules.push(make_ts_rule(r, &parse_table.symbol_to_index));
    }
    let ts_rules = Box::leak(Box::new(ts_rules));

    // Build alias sequences storing RHS symbol IDs for each rule
    let mut sorted_rules: Vec<_> = grammar.rules.values().flat_map(|rs| rs.iter()).collect();
    sorted_rules.sort_by_key(|r| r.production_id.0);

    let max_rhs_len = sorted_rules.iter().map(|r| r.rhs.len()).max().unwrap_or(0);
    let mut alias_map = Vec::new();
    let mut alias_sequences = Vec::new();
    for r in &sorted_rules {
        alias_map.push(alias_sequences.len() as u16);
        for sym in &r.rhs {
            let sid = match sym {
                Symbol::Terminal(t) | Symbol::NonTerminal(t) => *t,
                _ => SymbolId(0),
            };
            alias_sequences.push(sid.0);
        }
        while alias_sequences.len() % max_rhs_len != 0 {
            alias_sequences.push(0);
        }
    }
    let alias_map = Box::leak(Box::new(alias_map));
    let alias_sequences = Box::leak(Box::new(alias_sequences));

    // Build primary_state_ids array (all states are primary in our simple implementation)
    let primary_state_ids: Vec<u16> = (0..parse_table.state_count as u16).collect();
    let primary_state_ids = Box::leak(Box::new(primary_state_ids));

    // Tree-sitter ABI expects `eof_symbol` to be a *column index* in parse-table space,
    // not a raw SymbolId.
    let eof_symbol_col = parse_table
        .symbol_to_index
        .get(&parse_table.eof_symbol)
        .copied()
        .unwrap_or(0) as u16;

    // If we have states, they should all be large states for simplicity
    // since we're not implementing compression
    TSLanguage {
        version: 15,
        symbol_count: parse_table.index_to_symbol.len() as u32,
        alias_count: sorted_rules.len() as u32,
        token_count: parse_table.token_count as u32,
        external_token_count: parse_table.external_token_count as u32,
        state_count: parse_table.state_count as u32,
        large_state_count: parse_table.state_count as u32, // All states are large states
        // This fixture allocates one alias/metadata row per sorted rule.
        // Advertise that allocation so checked metadata readers can use it.
        production_id_count: sorted_rules.len() as u32,
        field_count: grammar.fields.len() as u32,
        max_alias_sequence_length: max_rhs_len as u16,
        production_id_map: std::ptr::null(),
        parse_table: full_parse_table.as_ptr(),
        small_parse_table: std::ptr::null(),
        small_parse_table_map: std::ptr::null(),
        parse_actions: parse_actions.as_ptr(),
        symbol_names: symbol_names_ptrs.as_ptr(),
        field_names: field_names_ptrs.as_ptr(),
        field_map_slices: std::ptr::null(),
        field_map_entries: std::ptr::null(),
        symbol_metadata: symbol_metadata.as_ptr(),
        public_symbol_map: public_symbol_map.as_ptr(),
        alias_map: alias_map.as_ptr(),
        alias_sequences: alias_sequences.as_ptr(),
        lex_modes: lex_modes.as_ptr(),
        lex_fn: None, // Will be set per-grammar
        keyword_lex_fn: None,
        keyword_capture_token: 0,
        external_scanner: ExternalScanner::default(),
        primary_state_ids: primary_state_ids.as_ptr(),
        production_lhs_index: production_lhs.as_ptr(),
        production_count: parse_table.rules.len() as u16,
        eof_symbol: eof_symbol_col,
        rules: ts_rules.as_ptr(),
        rule_count: parse_table.rules.len() as u16,
    }
}

/// Helper to set lexer function safely
#[inline]
#[allow(dead_code)]
fn set_lex_fn(
    lang: &mut TSLanguage,
    f: unsafe extern "C" fn(*mut core::ffi::c_void, TSLexState) -> bool,
) {
    lang.lex_fn = Some(f);
}

/// Build a TSLanguage for JSON grammar with its specific lexer
#[allow(dead_code)]
pub fn build_json_ts_language(grammar: &Grammar, parse_table: &ParseTable) -> TSLanguage {
    let mut lang = build_ts_language(grammar, parse_table);
    // Set the JSON lexer using the safe helper
    set_lex_fn(&mut lang, json_lexer);
    lang
}

// Lex mode constants for external token testing
#[allow(dead_code)]
const MODE_START: u16 = 0;
#[allow(dead_code)]
const MODE_NORMAL: u16 = 1;

/// INDENT lexer function for Tree-sitter
/// Emits INDENT once at start, then WORD tokens
#[allow(dead_code)]
unsafe extern "C" fn indent_lexer(lexer: *mut c_void, state: TSLexState) -> bool {
    use adze::lex::TsLexer;
    let lex = unsafe { &mut *(lexer as *mut TsLexer) };

    // Skip whitespace
    loop {
        let ch = unsafe { (lex.lookahead)(lex) };
        if ch == 0 {
            return false;
        }
        match ch as u8 as char {
            ' ' | '\t' | '\r' | '\n' => unsafe { (lex.advance)(lex, true) },
            _ => break,
        }
    }

    // Emit INDENT only once per parse: in START mode (state 0)
    // We assume at BOL since we don't have get_column exposed
    let at_bol = true; // Conservative: assume we're at beginning of line
    if state.lex_state == MODE_START && at_bol {
        unsafe { (lex.mark_end)(lex) };
        lex.result_symbol = 1; // INDENT
        return true;
    }

    // WORD
    let ch = unsafe { (lex.lookahead)(lex) } as u8 as char;
    if ch.is_ascii_alphabetic() {
        unsafe { (lex.advance)(lex, false) };
        while unsafe { (lex.lookahead)(lex) } != 0 {
            let c = unsafe { (lex.lookahead)(lex) } as u8 as char;
            if c.is_ascii_alphabetic() {
                unsafe { (lex.advance)(lex, false) };
            } else {
                break;
            }
        }
        unsafe { (lex.mark_end)(lex) };
        lex.result_symbol = 2; // WORD
        return true;
    }
    false
}

/// Build a TSLanguage for INDENT grammar with external token support
#[allow(dead_code)]
pub fn build_indent_ts_language(grammar: &Grammar, parse_table: &ParseTable) -> TSLanguage {
    let mut lang = build_ts_language(grammar, parse_table);

    // IMPORTANT: mark there is 1 external token (INDENT)
    lang.external_token_count = 1;

    // Provide lex modes: state 0 => START, others => NORMAL
    let mut modes = vec![
        TSLexState {
            lex_state: MODE_NORMAL,
            external_lex_state: 0
        };
        parse_table.state_count
    ];
    if !modes.is_empty() {
        modes[0] = TSLexState {
            lex_state: MODE_START,
            external_lex_state: 0,
        };
    }
    let modes = Box::leak(modes.into_boxed_slice());
    lang.lex_modes = modes.as_ptr();

    set_lex_fn(&mut lang, indent_lexer);
    lang
}
