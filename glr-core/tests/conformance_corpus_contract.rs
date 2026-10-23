//! ADZE-SPEC-0019 contract test: the `cfg.nullable_first_follow.v1`
//! corpus must stay structurally valid, denominator-complete, and
//! expectation-complete without running either engine.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

fn corpus() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
        .join("cases")
        .join("cfg.nullable_first_follow.v1")
        .join("corpus.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    serde_json::from_str(&text).expect("corpus must be valid JSON")
}

fn str_array(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("expected JSON array")
        .iter()
        .map(|item| item.as_str().expect("expected string").to_string())
        .collect()
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name != "EOF"
        && name != "eof"
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

const FAMILIES: [&str; 13] = [
    "single_terminal",
    "direct_epsilon",
    "nullable_prefix",
    "nullable_suffix",
    "fully_nullable_production",
    "nullable_chain",
    "left_recursion",
    "mutual_recursion",
    "repeated_occurrence",
    "terminal_stopping",
    "explicit_start_not_first_declared",
    "unreachable_component",
    "nonproductive_terminal_prefix",
];

#[test]
fn corpus_declares_nonzero_denominator_matching_case_count() {
    let corpus = corpus();
    assert_eq!(corpus["profile"], "cfg.nullable_first_follow.v1");
    assert_eq!(corpus["schema_version"], 1);
    let denominator = corpus["required_denominator"]
        .as_u64()
        .expect("denominator must be an integer");
    assert!(denominator > 0, "denominator must be nonzero");
    assert_eq!(
        denominator as usize,
        corpus["cases"].as_array().unwrap().len(),
        "denominator must equal the shipped case count"
    );
    assert_eq!(denominator as usize, FAMILIES.len());
}

#[test]
fn corpus_covers_every_family_exactly_once() {
    let corpus = corpus();
    let ids: Vec<String> = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["case_id"].as_str().unwrap().to_string())
        .collect();
    let unique: BTreeSet<&String> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "case IDs must be unique");
    for family in FAMILIES {
        assert!(ids.iter().any(|id| id == family), "missing family {family}");
        assert!(
            family
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_lowercase())
                && family
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            "case IDs use the bounded identifier grammar"
        );
    }
}

#[test]
fn every_case_is_structurally_valid_and_expectation_complete() {
    let corpus = corpus();
    for case in corpus["cases"].as_array().unwrap() {
        let case_id = case["case_id"].as_str().unwrap();
        let terminals = str_array(&case["terminals"]);
        let nonterminals = str_array(&case["nonterminals"]);
        let terminal_set: BTreeSet<&String> = terminals.iter().collect();
        let nonterminal_set: BTreeSet<&String> = nonterminals.iter().collect();
        assert_eq!(
            terminal_set.len(),
            terminals.len(),
            "{case_id}: duplicate terminals"
        );
        assert_eq!(
            nonterminal_set.len(),
            nonterminals.len(),
            "{case_id}: duplicate nonterminals"
        );
        assert!(
            terminal_set.is_disjoint(&nonterminal_set),
            "{case_id}: terminal/nonterminal collision"
        );
        for name in terminals.iter().chain(&nonterminals) {
            assert!(valid_name(name), "{case_id}: invalid name {name}");
        }
        assert!(
            nonterminal_set.contains(&case["start"].as_str().unwrap().to_string()),
            "{case_id}: start must be a declared nonterminal"
        );

        let productions = case["productions"].as_array().unwrap();
        let mut seen_productions = BTreeSet::new();
        let mut lhs_covered = BTreeSet::new();
        for production in productions {
            let lhs = production["lhs"].as_str().unwrap();
            assert!(
                nonterminal_set.contains(&lhs.to_string()),
                "{case_id}: production LHS {lhs} undeclared"
            );
            lhs_covered.insert(lhs);
            let rhs = str_array(&production["rhs"]);
            for symbol in &rhs {
                assert!(
                    terminal_set.contains(symbol) || nonterminal_set.contains(symbol),
                    "{case_id}: undefined symbol {symbol}"
                );
            }
            let key = format!("{lhs} -> {}", rhs.join(" "));
            assert!(
                seen_productions.insert(key),
                "{case_id}: duplicate production"
            );
        }
        for nonterminal in &nonterminals {
            assert!(
                lhs_covered.contains(nonterminal.as_str()),
                "{case_id}: nonterminal {nonterminal} has no production"
            );
        }

        let expectation = &case["expectation"];
        assert!(
            matches!(
                expectation["authority"].as_str().unwrap(),
                "reasoned_semantic"
                    | "curated_gold"
                    | "external_oracle"
                    | "snapshot"
                    | "generated_relation"
            ),
            "{case_id}: expectation needs exactly one authority class"
        );
        for name in str_array(&expectation["nullable"]) {
            assert!(
                nonterminal_set.contains(&name),
                "{case_id}: nullable names undeclared {name}"
            );
        }
        for section in ["first", "follow"] {
            let rows = expectation[section]
                .as_object()
                .expect("expectation section must be an object");
            assert_eq!(
                rows.len(),
                nonterminals.len(),
                "{case_id}: {section} must have exactly one row per nonterminal"
            );
            for nonterminal in &nonterminals {
                assert!(
                    rows.contains_key(nonterminal),
                    "{case_id}: {section} row missing for {nonterminal}"
                );
            }
        }
        for (nonterminal, row) in expectation["follow"].as_object().unwrap() {
            assert!(
                row["eof"].is_boolean(),
                "{case_id}: follow[{nonterminal}].eof must be a boolean"
            );
            for name in str_array(&row["terminals"]) {
                assert!(
                    terminal_set.contains(&name),
                    "{case_id}: follow names undeclared terminal {name}"
                );
            }
        }
        for (_, row) in expectation["first"].as_object().unwrap() {
            for name in str_array(row) {
                assert!(
                    terminal_set.contains(&name),
                    "{case_id}: first names undeclared terminal {name}"
                );
            }
        }
    }
}

#[test]
fn canonical_dense_case_matches_spec_example() {
    let corpus = corpus();
    let case = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["case_id"] == "fully_nullable_production")
        .expect("canonical case present");
    let expectation = &case["expectation"];
    assert_eq!(str_array(&expectation["nullable"]), ["A", "B", "S"]);
    assert_eq!(str_array(&expectation["first"]["S"]), ["a", "b"]);
    assert_eq!(str_array(&expectation["follow"]["A"]["terminals"]), ["b"]);
    assert_eq!(expectation["follow"]["A"]["eof"], true);
    assert_eq!(expectation["follow"]["B"]["eof"], true);
}

#[test]
fn invalid_cases_cover_distinct_rejection_classes() {
    let corpus = corpus();
    let invalid = corpus["invalid_cases"].as_array().unwrap();
    assert!(!invalid.is_empty(), "invalid cases must be present");
    let classes: BTreeSet<&str> = invalid
        .iter()
        .map(|case| case["reject_class"].as_str().unwrap())
        .collect();
    assert_eq!(
        classes.len(),
        invalid.len(),
        "reject classes must be distinct"
    );
    for case in invalid {
        assert!(
            case["document"].is_object(),
            "each invalid case carries its document"
        );
    }
}
