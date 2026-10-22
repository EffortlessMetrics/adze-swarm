//! Candidate-built batch JSON driver for `cfg.nullable_first_follow.v1` (#976).
//!
//! Development example only: it is excluded from the published
//! `adze-glr-core` crate by the package `include` list and is not a public
//! API or CLI promise.
//!
//! The driver is an adapter over the existing public
//! [`FirstFollowSets::compute`]. It never reimplements nullable/FIRST/FOLLOW
//! semantics.
//!
//! Protocol (stdout contains only this JSON surface; diagnostics go to
//! stderr):
//!
//! Request, from stdin or one file argument (`-` means stdin):
//! `{"profile":"cfg.nullable_first_follow.v1","schema_version":1,"cases":[CASE,...]}`
//! where CASE is `{"case_id":..,"start":..,"terminals":[..],
//! "nonterminals":[..],"productions":[{"lhs":..,"rhs":[..]}]}`.
//! A single case document (an object with `case_id`) is also accepted as a
//! one-case batch. Any `expectation`/`expected` key anywhere in a case (or
//! the envelope) is rejected: expected values never reach an engine request.
//!
//! Response: `{"profile":PROFILE,"schema_version":1,"input_digest":HEX,
//! "input_digest_algorithm":"fnv1a64","engine":"adze-glr-core",
//! "engine_version":VER,"results":[ROW,...]}` with exactly one ROW per
//! requested case, in request order. A ROW is either
//! `{"case_id":..,"status":"ok","result":{"nullable":[..],"first":{..},
//! "follow":{..}}}` or `{"case_id":..,"status":STATUS,"reason":..}` with
//! STATUS one of `invalid_input`, `unsupported`, or `engine_error`.
//! For an unparseable envelope there are no identifiable cases, so the
//! response carries top-level `status`/`reason` and an empty `results`.
//!
//! Mapping: `SymbolId(0)` is reserved for EOF, terminal IDs are allocated
//! from 1 upward in declaration order, nonterminal IDs follow disjointly,
//! each production gets one unique checked `ProductionId`, an empty RHS is
//! encoded as [`Symbol::Epsilon`], the start symbol is set explicitly with
//! `Grammar::set_start_symbol`, the grammar is validated, and
//! `FirstFollowSets::compute` is called exactly once per valid case.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::panic::AssertUnwindSafe;

use adze_glr_core::FirstFollowSets;
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use serde_json::{Map, Value, json};

const PROFILE: &str = "cfg.nullable_first_follow.v1";
const SCHEMA_VERSION: u64 = 1;
const MAX_REQUEST_BYTES: usize = 1_048_576;
const MAX_CASES: usize = 256;
const MAX_SYMBOLS: usize = 64;
const MAX_PRODUCTIONS: usize = 256;
const MAX_RHS_SYMBOLS: usize = 32;
const MAX_RHS_OCCURRENCES: usize = 4096;
const MAX_NAME_BYTES: usize = 64;

const STATUS_OK: &str = "ok";
const STATUS_INVALID: &str = "invalid_input";
const STATUS_UNSUPPORTED: &str = "unsupported";
const STATUS_ENGINE: &str = "engine_error";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let bytes = match args.as_slice() {
        [] => read_bounded(&mut std::io::stdin()),
        [arg] if arg == "-" => read_bounded(&mut std::io::stdin()),
        [path] => match std::fs::File::open(path) {
            Ok(mut file) => read_bounded(&mut file),
            Err(error) => {
                eprintln!("conformance_sets: cannot open {path}: {error}");
                print_response(&global_response(
                    STATUS_INVALID,
                    &format!("io_error: cannot read request: {error}"),
                    "0000000000000000",
                ));
                return;
            }
        },
        _ => {
            eprintln!("usage: conformance_sets [REQUEST_FILE|-]");
            print_response(&global_response(
                STATUS_INVALID,
                "io_error: expected at most one request file argument",
                "0000000000000000",
            ));
            std::process::exit(2);
        }
    };
    let bytes = match bytes {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("conformance_sets: cannot read request: {error}");
            print_response(&global_response(
                STATUS_INVALID,
                &format!("io_error: cannot read request: {error}"),
                "0000000000000000",
            ));
            return;
        }
    };
    print_response(&handle_bytes(&bytes));
}

fn print_response(response: &Value) {
    let text = serde_json::to_string(response).expect("response is serializable");
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{text}");
    let _ = stdout.flush();
}

fn read_bounded(reader: &mut impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn fnv1a64(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn handle_bytes(bytes: &[u8]) -> Value {
    let digest = fnv1a64(bytes);
    if bytes.len() > MAX_REQUEST_BYTES {
        return global_response(
            STATUS_INVALID,
            "bound_exceeded: request_bytes exceeds 1048576",
            &digest,
        );
    }
    let value: Value = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(error) => {
            return global_response(STATUS_INVALID, &format!("malformed_json: {error}"), &digest);
        }
    };
    handle_value(&value, &digest)
}

fn global_response(status: &str, reason: &str, digest: &str) -> Value {
    json!({
        "profile": PROFILE,
        "schema_version": SCHEMA_VERSION,
        "input_digest": digest,
        "input_digest_algorithm": "fnv1a64",
        "engine": "adze-glr-core",
        "engine_version": env!("CARGO_PKG_VERSION"),
        "status": status,
        "reason": reason,
        "results": [],
    })
}

fn base_response(digest: &str, results: Vec<Value>) -> Value {
    json!({
        "profile": PROFILE,
        "schema_version": SCHEMA_VERSION,
        "input_digest": digest,
        "input_digest_algorithm": "fnv1a64",
        "engine": "adze-glr-core",
        "engine_version": env!("CARGO_PKG_VERSION"),
        "results": results,
    })
}

fn handle_value(request: &Value, digest: &str) -> Value {
    let Some(envelope) = request.as_object() else {
        return global_response(STATUS_INVALID, "malformed_request: expected_object", digest);
    };
    let (cases, single_case) = if let Some(cases) = envelope.get("cases") {
        let Some(cases) = cases.as_array() else {
            return global_response(
                STATUS_INVALID,
                "malformed_request: cases_must_be_array",
                digest,
            );
        };
        (cases.clone(), false)
    } else if envelope.contains_key("case_id") {
        (vec![request.clone()], true)
    } else {
        return global_response(STATUS_INVALID, "malformed_request: missing_cases", digest);
    };

    // Envelope-level strict profile/schema checks fail every requested row.
    let mut envelope_failure: Option<String> = None;
    if !single_case {
        let allowed: BTreeSet<&str> = ["profile", "schema_version", "cases", "input_digest"]
            .into_iter()
            .collect();
        if let Some(key) = envelope.keys().find(|key| !allowed.contains(key.as_str())) {
            envelope_failure = Some(format!("malformed_request: unknown_envelope_key:{key}"));
        }
    }
    if envelope_failure.is_none() {
        if let Some(key) = find_expectation_key(request) {
            envelope_failure = Some(format!("expectation_not_allowed: {key}"));
        } else if !single_case {
            match envelope.get("profile") {
                Some(Value::String(profile)) if profile == PROFILE => {}
                Some(Value::String(profile)) => {
                    envelope_failure = Some(format!("profile_mismatch: {profile}"));
                }
                Some(_) => {
                    envelope_failure = Some("profile_mismatch: profile_must_be_string".to_string());
                }
                None => envelope_failure = Some("profile_mismatch: missing_profile".to_string()),
            }
            if envelope_failure.is_none()
                && let Some(version) = envelope.get("schema_version")
                && version.as_u64() != Some(SCHEMA_VERSION)
            {
                envelope_failure = Some(format!("profile_mismatch: schema_version:{version}"));
            }
        }
    }
    // For a single-case request, a case-level expectation key is handled per
    // row below; do not let the envelope check above mask the case identity.
    if single_case {
        envelope_failure = None;
    }
    if cases.len() > MAX_CASES {
        envelope_failure = Some(format!(
            "bound_exceeded: cases {} exceeds {MAX_CASES}",
            cases.len()
        ));
    }

    let results: Vec<Value> = cases
        .iter()
        .map(|case| {
            if let Some(reason) = &envelope_failure {
                failure_row(case, STATUS_INVALID, reason)
            } else {
                process_case(case)
            }
        })
        .collect();
    base_response(digest, results)
}

fn find_expectation_key(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for (key, nested) in map {
                if matches!(
                    key.as_str(),
                    "expectation" | "expectations" | "expected" | "expected_values"
                ) {
                    return Some(key.clone());
                }
                if let Some(found) = find_expectation_key(nested) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(find_expectation_key),
        _ => None,
    }
}

fn extracted_case_id(case: &Value) -> String {
    case.get("case_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

fn failure_row(case: &Value, status: &str, reason: &str) -> Value {
    json!({
        "case_id": extracted_case_id(case),
        "status": status,
        "reason": reason,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProductionInput {
    lhs: String,
    rhs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CaseInput {
    case_id: String,
    start: String,
    terminals: Vec<String>,
    nonterminals: Vec<String>,
    productions: Vec<ProductionInput>,
}

fn process_case(case: &Value) -> Value {
    let input = match parse_case(case) {
        Ok(input) => input,
        Err(reason) => return failure_row(case, STATUS_INVALID, &reason),
    };
    match run_case(&input) {
        Ok(result) => json!({
            "case_id": input.case_id,
            "status": STATUS_OK,
            "result": result,
        }),
        Err((status, reason)) => json!({
            "case_id": input.case_id,
            "status": status,
            "reason": reason,
        }),
    }
}

fn valid_name(name: &str) -> Result<(), String> {
    if name == "EOF" || name == "eof" {
        return Err(format!("reserved_eof_name: {name}"));
    }
    let valid = !name.is_empty()
        && name.len() <= MAX_NAME_BYTES
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if valid {
        Ok(())
    } else {
        Err(format!("invalid_name: {name}"))
    }
}

fn valid_case_id(case_id: &str) -> bool {
    !case_id.is_empty()
        && case_id.len() <= MAX_NAME_BYTES
        && case_id
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase())
        && case_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn string_array(value: Option<&Value>, field: &str) -> Result<Vec<String>, String> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Err(format!("malformed_case: {field}_must_be_array"));
    };
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("malformed_case: {field}_must_be_strings"))
        })
        .collect()
}

fn parse_case(case: &Value) -> Result<CaseInput, String> {
    if let Some(key) = find_expectation_key(case) {
        return Err(format!("expectation_not_allowed: {key}"));
    }
    let Some(object) = case.as_object() else {
        return Err("malformed_case: expected_object".to_string());
    };
    let allowed: BTreeSet<&str> = [
        "profile",
        "case_id",
        "start",
        "terminals",
        "nonterminals",
        "productions",
    ]
    .into_iter()
    .collect();
    if let Some(key) = object.keys().find(|key| !allowed.contains(key.as_str())) {
        return Err(format!("malformed_case: unknown_case_key:{key}"));
    }
    if let Some(profile) = object.get("profile") {
        match profile.as_str() {
            Some(PROFILE) => {}
            Some(other) => return Err(format!("profile_mismatch: {other}")),
            None => return Err("profile_mismatch: profile_must_be_string".to_string()),
        }
    }
    let case_id = object
        .get("case_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "malformed_case: case_id_must_be_string".to_string())?;
    if !valid_case_id(case_id) {
        return Err(format!("malformed_case: invalid_case_id:{case_id}"));
    }
    let terminals = string_array(object.get("terminals"), "terminals")?;
    let nonterminals = string_array(object.get("nonterminals"), "nonterminals")?;
    if nonterminals.is_empty() {
        return Err("malformed_case: nonterminals_must_not_be_empty".to_string());
    }
    for name in terminals.iter().chain(&nonterminals) {
        valid_name(name)?;
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for name in &terminals {
        if !seen.insert(name.as_str()) {
            return Err(format!("duplicate_name: {name}"));
        }
    }
    let terminal_set: BTreeSet<&str> = terminals.iter().map(String::as_str).collect();
    let mut nonterminal_set: BTreeSet<&str> = BTreeSet::new();
    for name in &nonterminals {
        if terminal_set.contains(name.as_str()) {
            return Err(format!("terminal_nonterminal_collision: {name}"));
        }
        if !nonterminal_set.insert(name.as_str()) {
            return Err(format!("duplicate_name: {name}"));
        }
    }
    if terminals.len() + nonterminals.len() > MAX_SYMBOLS {
        return Err(format!(
            "bound_exceeded: total_symbols {} exceeds {MAX_SYMBOLS}",
            terminals.len() + nonterminals.len()
        ));
    }
    let start = object
        .get("start")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing_or_unknown_start: start_must_be_string".to_string())?;
    if !nonterminal_set.contains(start) {
        return Err(format!("missing_or_unknown_start: {start}"));
    }
    let Some(production_values) = object.get("productions").and_then(Value::as_array) else {
        return Err("malformed_case: productions_must_be_array".to_string());
    };
    if production_values.len() > MAX_PRODUCTIONS {
        return Err(format!(
            "bound_exceeded: productions {} exceeds {MAX_PRODUCTIONS}",
            production_values.len()
        ));
    }
    let mut productions = Vec::with_capacity(production_values.len());
    let mut seen_productions: BTreeSet<(String, Vec<String>)> = BTreeSet::new();
    let mut lhs_covered: BTreeSet<&str> = BTreeSet::new();
    let mut occurrences: usize = 0;
    for production in production_values {
        let Some(production_object) = production.as_object() else {
            return Err("malformed_case: production_must_be_object".to_string());
        };
        let production_allowed: BTreeSet<&str> = ["lhs", "rhs"].into_iter().collect();
        if let Some(key) = production_object
            .keys()
            .find(|key| !production_allowed.contains(key.as_str()))
        {
            return Err(format!("malformed_case: unknown_production_key:{key}"));
        }
        let lhs = production_object
            .get("lhs")
            .and_then(Value::as_str)
            .ok_or_else(|| "malformed_case: production_lhs_must_be_string".to_string())?;
        if !nonterminal_set.contains(lhs) {
            return Err(format!("production_lhs_not_nonterminal: {lhs}"));
        }
        let rhs = string_array(production_object.get("rhs"), "production_rhs")?;
        if rhs.len() > MAX_RHS_SYMBOLS {
            return Err(format!(
                "bound_exceeded: rhs_len {} exceeds {MAX_RHS_SYMBOLS}",
                rhs.len()
            ));
        }
        occurrences += rhs.len();
        if occurrences > MAX_RHS_OCCURRENCES {
            return Err(format!(
                "bound_exceeded: rhs_occurrences exceeds {MAX_RHS_OCCURRENCES}"
            ));
        }
        for symbol in &rhs {
            if !terminal_set.contains(symbol.as_str()) && !nonterminal_set.contains(symbol.as_str())
            {
                return Err(format!("undefined_symbol: {symbol}"));
            }
        }
        if !seen_productions.insert((lhs.to_string(), rhs.clone())) {
            return Err(format!("duplicate_production: {lhs}"));
        }
        lhs_covered.insert(lhs);
        productions.push(ProductionInput {
            lhs: lhs.to_string(),
            rhs,
        });
    }
    for nonterminal in &nonterminals {
        if !lhs_covered.contains(nonterminal.as_str()) {
            return Err(format!("nonterminal_without_production: {nonterminal}"));
        }
    }
    Ok(CaseInput {
        case_id: case_id.to_string(),
        start: start.to_string(),
        terminals,
        nonterminals,
        productions,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Mapping {
    terminal_ids: BTreeMap<String, SymbolId>,
    nonterminal_ids: BTreeMap<String, SymbolId>,
    terminal_names: BTreeMap<u16, String>,
    nonterminal_names: BTreeMap<u16, String>,
}

fn build_mapping(case: &CaseInput) -> Result<Mapping, String> {
    let mut terminal_ids = BTreeMap::new();
    let mut terminal_names = BTreeMap::new();
    // SymbolId(0) is reserved for EOF; terminals start at 1.
    for (index, name) in case.terminals.iter().enumerate() {
        let raw = u16::try_from(index + 1)
            .map_err(|_| "bound_exceeded: terminal_id_overflow".to_string())?;
        terminal_ids.insert(name.clone(), SymbolId(raw));
        terminal_names.insert(raw, name.clone());
    }
    let mut nonterminal_ids = BTreeMap::new();
    let mut nonterminal_names = BTreeMap::new();
    let first_nonterminal = case.terminals.len() + 1;
    for (index, name) in case.nonterminals.iter().enumerate() {
        let raw = u16::try_from(first_nonterminal + index)
            .map_err(|_| "bound_exceeded: nonterminal_id_overflow".to_string())?;
        if terminal_names.contains_key(&raw) {
            return Err(format!("mapping_collision: symbol_id {raw}"));
        }
        nonterminal_ids.insert(name.clone(), SymbolId(raw));
        nonterminal_names.insert(raw, name.clone());
    }
    Ok(Mapping {
        terminal_ids,
        nonterminal_ids,
        terminal_names,
        nonterminal_names,
    })
}

fn build_grammar(case: &CaseInput, mapping: &Mapping) -> Grammar {
    let mut grammar = Grammar::new(format!("conformance_{}", case.case_id));
    for (name, id) in &mapping.terminal_ids {
        grammar.tokens.insert(
            *id,
            Token {
                name: name.clone(),
                pattern: TokenPattern::String(name.clone()),
                fragile: false,
            },
        );
    }
    for (name, id) in &mapping.nonterminal_ids {
        grammar.rule_names.insert(*id, name.clone());
    }
    for (index, production) in case.productions.iter().enumerate() {
        let lhs = mapping.nonterminal_ids[&production.lhs];
        let rhs: Vec<Symbol> = if production.rhs.is_empty() {
            // The IR epsilon representation for an empty RHS.
            vec![Symbol::Epsilon]
        } else {
            production
                .rhs
                .iter()
                .map(|name| {
                    if let Some(id) = mapping.terminal_ids.get(name) {
                        Symbol::Terminal(*id)
                    } else {
                        Symbol::NonTerminal(mapping.nonterminal_ids[name])
                    }
                })
                .collect()
        };
        grammar.add_rule(Rule {
            lhs,
            rhs,
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(index as u16),
        });
    }
    let start = mapping.nonterminal_ids[&case.start];
    grammar.set_start_symbol(start);
    grammar
}

/// Project one FIRST/FOLLOW bitset back to canonical terminal names.
///
/// Bit 0 is EOF: it is an adapter error in FIRST and becomes `eof=true` in
/// FOLLOW. Any nonterminal or unknown bit is an adapter error; unknown
/// output identities are never discarded.
fn project_bits(
    bits: &[usize],
    allow_eof: bool,
    kind: &str,
    mapping: &Mapping,
) -> Result<(Vec<String>, bool), String> {
    let mut terminals = Vec::new();
    let mut eof = false;
    for bit in bits {
        if *bit == 0 {
            if allow_eof {
                eof = true;
                continue;
            }
            return Err(format!("adapter_projection: {kind}_contains_eof_bit"));
        }
        let raw = u16::try_from(*bit)
            .map_err(|_| format!("adapter_projection: {kind}_contains_unknown_bit:{bit}"))?;
        if let Some(name) = mapping.terminal_names.get(&raw) {
            terminals.push(name.clone());
        } else if mapping.nonterminal_names.contains_key(&raw) {
            return Err(format!(
                "adapter_projection: {kind}_contains_nonterminal_bit:{bit}"
            ));
        } else {
            return Err(format!(
                "adapter_projection: {kind}_contains_unknown_bit:{bit}"
            ));
        }
    }
    terminals.sort();
    terminals.dedup();
    Ok((terminals, eof))
}

fn run_case(case: &CaseInput) -> Result<Value, (&'static str, String)> {
    let mapping = build_mapping(case).map_err(|reason| (STATUS_ENGINE, reason))?;
    let grammar = build_grammar(case, &mapping);
    grammar
        .validate()
        .map_err(|error| (STATUS_ENGINE, format!("grammar_validation: {error}")))?;
    // Exactly one production computation per valid case.
    let computed =
        std::panic::catch_unwind(AssertUnwindSafe(|| FirstFollowSets::compute(&grammar)));
    let sets = match computed {
        Ok(Ok(sets)) => sets,
        Ok(Err(error)) => {
            let reason = format!("{error}");
            if reason.contains("Complex symbols") {
                return Err((STATUS_UNSUPPORTED, format!("unsupported: {reason}")));
            }
            return Err((STATUS_ENGINE, format!("engine_error: {reason}")));
        }
        Err(_) => {
            return Err((
                STATUS_ENGINE,
                "engine_error: computation_panicked".to_string(),
            ));
        }
    };

    let mut nullable: Vec<String> = case
        .nonterminals
        .iter()
        .filter(|name| sets.is_nullable(mapping.nonterminal_ids[*name]))
        .cloned()
        .collect();
    nullable.sort();

    let mut sorted_nonterminals = case.nonterminals.clone();
    sorted_nonterminals.sort();
    let mut first = Map::new();
    let mut follow = Map::new();
    for name in &sorted_nonterminals {
        let id = mapping.nonterminal_ids[name];
        let first_bits: Vec<usize> = sets
            .first(id)
            .ok_or_else(|| {
                (
                    STATUS_ENGINE,
                    format!("adapter_projection: missing_first_row:{name}"),
                )
            })?
            .ones()
            .collect();
        let (first_terminals, _) = project_bits(&first_bits, false, "first", &mapping)
            .map_err(|reason| (STATUS_ENGINE, reason))?;
        first.insert(name.clone(), json!(first_terminals));

        let follow_bits: Vec<usize> = sets
            .follow(id)
            .ok_or_else(|| {
                (
                    STATUS_ENGINE,
                    format!("adapter_projection: missing_follow_row:{name}"),
                )
            })?
            .ones()
            .collect();
        let (follow_terminals, eof) = project_bits(&follow_bits, true, "follow", &mapping)
            .map_err(|reason| (STATUS_ENGINE, reason))?;
        follow.insert(
            name.clone(),
            json!({ "terminals": follow_terminals, "eof": eof }),
        );
    }
    Ok(json!({
        "nullable": nullable,
        "first": first,
        "follow": follow,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn request(cases: Vec<Value>) -> Value {
        let request = json!({
            "profile": PROFILE,
            "schema_version": SCHEMA_VERSION,
            "cases": cases,
        });
        handle_value(&request, "testdigest")
    }

    fn run_one(case: Value) -> Value {
        let response = request(vec![case]);
        assert_eq!(response["profile"], PROFILE);
        let results = response["results"].as_array().expect("results array");
        assert_eq!(results.len(), 1, "exactly one row per case");
        results[0].clone()
    }

    fn case(
        case_id: &str,
        start: &str,
        terminals: Value,
        nonterminals: Value,
        productions: Value,
    ) -> Value {
        json!({
            "case_id": case_id,
            "start": start,
            "terminals": terminals,
            "nonterminals": nonterminals,
            "productions": productions,
        })
    }

    fn simple_case() -> Value {
        case(
            "single_terminal",
            "S",
            json!(["a"]),
            json!(["S"]),
            json!([{ "lhs": "S", "rhs": ["a"] }]),
        )
    }

    fn assert_ok(row: &Value) -> &Value {
        assert_eq!(row["status"], STATUS_OK, "row: {row}");
        &row["result"]
    }

    fn assert_invalid(row: &Value, needle: &str) {
        assert_eq!(row["status"], STATUS_INVALID, "row: {row}");
        let reason = row["reason"].as_str().expect("reason string");
        assert!(
            reason.contains(needle),
            "reason {reason:?} must contain {needle:?}"
        );
    }

    #[test]
    fn test_single_terminal_ok() {
        let row = run_one(simple_case());
        let result = assert_ok(&row);
        assert_eq!(result["nullable"], json!([]));
        assert_eq!(result["first"], json!({ "S": ["a"] }));
        assert_eq!(
            result["follow"],
            json!({ "S": { "terminals": [], "eof": true } })
        );
    }

    #[test]
    fn test_epsilon_production() {
        let row = run_one(case(
            "direct_epsilon",
            "S",
            json!([]),
            json!(["S"]),
            json!([{ "lhs": "S", "rhs": [] }]),
        ));
        let result = assert_ok(&row);
        assert_eq!(result["nullable"], json!(["S"]));
        assert_eq!(result["first"], json!({ "S": [] }));
        assert_eq!(result["follow"]["S"]["eof"], true);
    }

    #[test]
    fn test_explicit_start_differs_from_declaration_order() {
        let row = run_one(case(
            "explicit_start",
            "S",
            json!(["a", "b"]),
            json!(["A", "S"]),
            json!([
                { "lhs": "A", "rhs": ["a"] },
                { "lhs": "S", "rhs": ["A", "b"] },
            ]),
        ));
        let result = assert_ok(&row);
        // EOF seeds only S, never the first-declared A.
        assert_eq!(result["follow"]["S"]["eof"], true);
        assert_eq!(result["follow"]["A"]["eof"], false);
        assert_eq!(result["follow"]["A"]["terminals"], json!(["b"]));
    }

    #[test]
    fn test_mapping_ids_disjoint_and_names_roundtrip() {
        let input = parse_case(&case(
            "mapping_case",
            "S",
            json!(["a", "b"]),
            json!(["S", "A"]),
            json!([
                { "lhs": "S", "rhs": ["A", "b"] },
                { "lhs": "A", "rhs": ["a"] },
                { "lhs": "A", "rhs": [] },
            ]),
        ))
        .expect("valid case");
        let mapping = build_mapping(&input).expect("mapping");
        assert_eq!(mapping.terminal_ids["a"], SymbolId(1));
        assert_eq!(mapping.terminal_ids["b"], SymbolId(2));
        let terminal_id_set: BTreeSet<u16> = mapping.terminal_names.keys().copied().collect();
        let nonterminal_id_set: BTreeSet<u16> = mapping.nonterminal_names.keys().copied().collect();
        assert!(terminal_id_set.is_disjoint(&nonterminal_id_set));
        assert!(!terminal_id_set.contains(&0));
        assert!(!nonterminal_id_set.contains(&0));
        for (name, id) in mapping.terminal_ids.iter().chain(&mapping.nonterminal_ids) {
            let reverse = mapping
                .terminal_names
                .get(&id.0)
                .or_else(|| mapping.nonterminal_names.get(&id.0))
                .expect("reverse name");
            assert_eq!(reverse, name);
        }
        let grammar = build_grammar(&input, &mapping);
        assert_eq!(
            grammar.explicit_start_symbol(),
            Some(mapping.nonterminal_ids["S"])
        );
        let production_ids: BTreeSet<u16> = grammar
            .all_rules()
            .map(|rule| rule.production_id.0)
            .collect();
        assert_eq!(production_ids.len(), input.productions.len());
        assert!(grammar.validate().is_ok());
    }

    #[test]
    fn test_unreachable_component_gets_rows_without_eof() {
        let row = run_one(case(
            "unreachable_component",
            "S",
            json!(["a", "b", "c"]),
            json!(["S", "U", "V"]),
            json!([
                { "lhs": "S", "rhs": ["a"] },
                { "lhs": "U", "rhs": ["V", "b"] },
                { "lhs": "V", "rhs": ["c"] },
                { "lhs": "V", "rhs": [] },
            ]),
        ));
        let result = assert_ok(&row);
        assert_eq!(result["first"]["U"], json!(["b", "c"]));
        assert_eq!(result["follow"]["U"]["eof"], false);
        assert_eq!(result["follow"]["V"]["terminals"], json!(["b"]));
    }

    #[test]
    fn test_recursive_nonproductive_prefix() {
        let row = run_one(case(
            "nonproductive_terminal_prefix",
            "S",
            json!(["a"]),
            json!(["S", "A"]),
            json!([
                { "lhs": "S", "rhs": ["a", "A"] },
                { "lhs": "A", "rhs": ["A"] },
            ]),
        ));
        let result = assert_ok(&row);
        assert_eq!(result["first"]["S"], json!(["a"]));
        assert_eq!(result["first"]["A"], json!([]));
        assert_eq!(result["follow"]["A"]["eof"], true);
    }

    #[test]
    fn test_rejection_classes() {
        let undefined = case(
            "undefined_case",
            "S",
            json!(["a"]),
            json!(["S"]),
            json!([{ "lhs": "S", "rhs": ["a", "Missing"] }]),
        );
        assert_invalid(&run_one(undefined), "undefined_symbol");

        let no_production = case(
            "no_production_case",
            "S",
            json!(["a"]),
            json!(["S", "A"]),
            json!([{ "lhs": "S", "rhs": ["a"] }]),
        );
        assert_invalid(&run_one(no_production), "nonterminal_without_production");

        let duplicate_name = case(
            "duplicate_name_case",
            "S",
            json!(["a", "a"]),
            json!(["S"]),
            json!([{ "lhs": "S", "rhs": ["a"] }]),
        );
        assert_invalid(&run_one(duplicate_name), "duplicate_name");

        let collision = case(
            "collision_case",
            "S",
            json!(["a", "S"]),
            json!(["S"]),
            json!([{ "lhs": "S", "rhs": ["a"] }]),
        );
        assert_invalid(&run_one(collision), "terminal_nonterminal_collision");

        let duplicate_production = case(
            "duplicate_production_case",
            "S",
            json!(["a"]),
            json!(["S"]),
            json!([
                { "lhs": "S", "rhs": ["a"] },
                { "lhs": "S", "rhs": ["a"] },
            ]),
        );
        assert_invalid(&run_one(duplicate_production), "duplicate_production");

        let reserved_terminal = case(
            "reserved_case",
            "S",
            json!(["EOF"]),
            json!(["S"]),
            json!([{ "lhs": "S", "rhs": ["EOF"] }]),
        );
        assert_invalid(&run_one(reserved_terminal), "reserved_eof_name");

        let reserved_nonterminal = case(
            "reserved_nt_case",
            "eof",
            json!(["a"]),
            json!(["eof"]),
            json!([{ "lhs": "eof", "rhs": ["a"] }]),
        );
        assert_invalid(&run_one(reserved_nonterminal), "reserved_eof_name");

        let unknown_start = case(
            "unknown_start_case",
            "Missing",
            json!(["a"]),
            json!(["S"]),
            json!([{ "lhs": "S", "rhs": ["a"] }]),
        );
        assert_invalid(&run_one(unknown_start), "missing_or_unknown_start");

        let lhs_not_nonterminal = case(
            "lhs_case",
            "S",
            json!(["a"]),
            json!(["S"]),
            json!([{ "lhs": "a", "rhs": ["a"] }]),
        );
        assert_invalid(
            &run_one(lhs_not_nonterminal),
            "production_lhs_not_nonterminal",
        );
    }

    #[test]
    fn test_bound_rejections() {
        // 65 total declared symbols.
        let many_terminals: Vec<String> = (0..64).map(|i| format!("t{i}")).collect();
        let too_many_symbols = case(
            "too_many_symbols",
            "S",
            json!(many_terminals),
            json!(["S"]),
            json!([{ "lhs": "S", "rhs": ["t0"] }]),
        );
        assert_invalid(&run_one(too_many_symbols), "bound_exceeded");

        // 257 productions.
        let many_productions: Vec<Value> = (0..257)
            .map(|_| json!({ "lhs": "S", "rhs": ["a"] }))
            .collect();
        let too_many_productions = case(
            "too_many_productions",
            "S",
            json!(["a"]),
            json!(["S"]),
            json!(many_productions),
        );
        assert_invalid(&run_one(too_many_productions), "bound_exceeded");

        // RHS of 33 symbols.
        let long_rhs: Vec<String> = (0..33).map(|_| "a".to_string()).collect();
        let too_long_rhs = case(
            "too_long_rhs",
            "S",
            json!(["a"]),
            json!(["S"]),
            json!([{ "lhs": "S", "rhs": long_rhs }]),
        );
        assert_invalid(&run_one(too_long_rhs), "bound_exceeded");

        // 129 distinct productions of length 32 exceed 4096 occurrences.
        let occ_productions: Vec<Value> = (0..129usize)
            .map(|index| {
                let rhs: Vec<&str> = (0..32)
                    .map(|bit| {
                        if bit < 7 && (index >> bit) & 1 == 1 {
                            "b"
                        } else {
                            "a"
                        }
                    })
                    .collect();
                json!({ "lhs": "S", "rhs": rhs })
            })
            .collect();
        let too_many_occurrences = case(
            "too_many_occurrences",
            "S",
            json!(["a", "b"]),
            json!(["S"]),
            json!(occ_productions),
        );
        assert_invalid(&run_one(too_many_occurrences), "bound_exceeded");
    }

    #[test]
    fn test_malformed_and_profile_mismatch_and_expectation() {
        let malformed = handle_bytes(b"{\"profile\":");
        assert_eq!(malformed["status"], STATUS_INVALID);
        assert!(
            malformed["reason"]
                .as_str()
                .expect("reason")
                .contains("malformed_json")
        );
        assert_eq!(malformed["results"], json!([]));
        assert_eq!(malformed["profile"], PROFILE);

        let wrong_profile = json!({
            "profile": "cfg.other.v9",
            "schema_version": 1,
            "cases": [simple_case()],
        });
        let response = handle_value(&wrong_profile, "d");
        assert_eq!(response["profile"], PROFILE);
        let results = response["results"].as_array().expect("results");
        assert_eq!(results.len(), 1);
        assert_invalid(&results[0], "profile_mismatch");

        let mut with_expectation = simple_case();
        with_expectation["expectation"] = json!({ "nullable": [] });
        assert_invalid(&run_one(with_expectation), "expectation_not_allowed");
    }

    #[test]
    fn test_unknown_output_bit_rejected_at_projection() {
        let input = parse_case(&simple_case()).expect("valid");
        let mapping = build_mapping(&input).expect("mapping");
        let s_id = mapping.nonterminal_ids["S"].0 as usize;
        // EOF bit in FIRST is an adapter error.
        assert!(project_bits(&[0], false, "first", &mapping).is_err());
        // A nonterminal bit in FIRST is an adapter error.
        assert!(project_bits(&[s_id], false, "first", &mapping).is_err());
        // An entirely unknown bit is never discarded.
        assert!(project_bits(&[999], true, "follow", &mapping).is_err());
        // FOLLOW bit 0 maps to eof=true and terminals roundtrip.
        let (terminals, eof) = project_bits(&[0, 1], true, "follow", &mapping).expect("follow");
        assert!(eof);
        assert_eq!(terminals, vec!["a".to_string()]);
    }

    #[test]
    fn test_batch_rows_exactly_once_in_order() {
        let invalid = case(
            "bad_case",
            "Missing",
            json!(["a"]),
            json!(["S"]),
            json!([{ "lhs": "S", "rhs": ["a"] }]),
        );
        let response = request(vec![simple_case(), invalid, simple_case()]);
        let results = response["results"].as_array().expect("results");
        assert_eq!(results.len(), 3);
        let ids: Vec<&str> = results
            .iter()
            .map(|row| row["case_id"].as_str().expect("case_id"))
            .collect();
        assert_eq!(ids, vec!["single_terminal", "bad_case", "single_terminal"]);
        assert_eq!(results[0]["status"], STATUS_OK);
        assert_eq!(results[1]["status"], STATUS_INVALID);
        assert_eq!(results[2]["status"], STATUS_OK);
    }

    fn corpus_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("conformance")
            .join("cases")
            .join("cfg.nullable_first_follow.v1")
            .join("corpus.json")
    }

    #[test]
    fn test_all_dense_corpus_cases_when_present() {
        let path = corpus_path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!(
                "skipping corpus test: {} not present (contract branch not merged)",
                path.display()
            );
            return;
        };
        let corpus: Value = serde_json::from_str(&text).expect("corpus JSON");
        assert_eq!(corpus["profile"], PROFILE);
        let cases = corpus["cases"].as_array().expect("cases");
        assert_eq!(
            cases.len() as u64,
            corpus["required_denominator"]
                .as_u64()
                .expect("denominator")
        );
        let engine_cases: Vec<Value> = cases
            .iter()
            .map(|item| {
                json!({
                    "case_id": item["case_id"],
                    "start": item["start"],
                    "terminals": item["terminals"],
                    "nonterminals": item["nonterminals"],
                    "productions": item["productions"],
                })
            })
            .collect();
        let response = request(engine_cases);
        let results = response["results"].as_array().expect("results");
        assert_eq!(results.len(), cases.len());
        for (item, row) in cases.iter().zip(results) {
            assert_eq!(row["case_id"], item["case_id"]);
            let result = assert_ok(row);
            let expected = &item["expectation"];
            assert_eq!(
                result["nullable"], expected["nullable"],
                "{} nullable",
                item["case_id"]
            );
            for nonterminal in item["nonterminals"].as_array().expect("nonterminals") {
                let name = nonterminal.as_str().expect("name");
                assert_eq!(
                    result["first"][name], expected["first"][name],
                    "{} first[{name}]",
                    item["case_id"]
                );
                assert_eq!(
                    result["follow"][name], expected["follow"][name],
                    "{} follow[{name}]",
                    item["case_id"]
                );
            }
        }
    }

    #[test]
    fn test_corpus_invalid_cases_rejected_when_present() {
        let path = corpus_path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!(
                "skipping corpus invalid test: {} not present (contract branch not merged)",
                path.display()
            );
            return;
        };
        let corpus: Value = serde_json::from_str(&text).expect("corpus JSON");
        for invalid in corpus["invalid_cases"].as_array().expect("invalid_cases") {
            let row = run_one(invalid["document"].clone());
            assert_eq!(
                row["status"], STATUS_INVALID,
                "{} must be rejected: {row}",
                invalid["case_id"]
            );
        }
    }
}
