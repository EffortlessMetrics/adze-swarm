//! End-to-end tests for the `conformance_sets` example driver (#976).
//!
//! These drive the compiled example binary over stdin/file input. The
//! mapping/projection unit tests live in the example itself.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use serde_json::{Value, json};

const PROFILE: &str = "cfg.nullable_first_follow.v1";

fn example_binary() -> &'static PathBuf {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY.get_or_init(|| {
        let current = std::env::current_exe().expect("current test exe");
        // <target>/debug/deps/<test>.exe -> <target>/debug/examples/
        let debug_dir = current
            .parent()
            .and_then(|deps| deps.parent())
            .expect("target debug dir")
            .to_path_buf();
        let candidate = debug_dir
            .join("examples")
            .join(format!("conformance_sets{}", std::env::consts::EXE_SUFFIX));
        if candidate.is_file() {
            return candidate;
        }
        // `cargo test` builds examples in test mode only, so build the real
        // example binary once, into the same target directory.
        let target_dir = debug_dir.parent().expect("target dir").to_path_buf();
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let output = Command::new(cargo)
            .args([
                "build",
                "-p",
                "adze-glr-core",
                "--example",
                "conformance_sets",
            ])
            .arg("--target-dir")
            .arg(&target_dir)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("cargo build --example conformance_sets");
        assert!(
            output.status.success(),
            "building example failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            candidate.is_file(),
            "example binary not found at {}",
            candidate.display()
        );
        candidate
    })
}

fn run_driver(input: &[u8], file_arg: Option<&std::path::Path>) -> Value {
    let mut command = Command::new(example_binary());
    if let Some(path) = file_arg {
        command.arg(path);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn conformance_sets");
    if file_arg.is_none() {
        child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(input)
            .expect("write stdin");
    }
    let output = child.wait_with_output().expect("wait for driver");
    assert!(
        output.status.success(),
        "driver failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "stdout must be exactly one JSON line: {stdout}"
    );
    serde_json::from_str(lines[0]).expect("stdout is one JSON document")
}

fn batch(cases: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "profile": PROFILE,
        "schema_version": 1,
        "cases": cases,
    }))
    .expect("request JSON")
}

fn simple_case(case_id: &str) -> Value {
    json!({
        "case_id": case_id,
        "start": "S",
        "terminals": ["a"],
        "nonterminals": ["S"],
        "productions": [{ "lhs": "S", "rhs": ["a"] }],
    })
}

#[test]
fn test_driver_stdin_stdout_is_single_json_document() {
    let response = run_driver(&batch(json!([simple_case("single_terminal")])), None);
    assert_eq!(response["profile"], PROFILE);
    assert_eq!(response["schema_version"], 1);
    assert!(response["input_digest"].as_str().is_some());
    let results = response["results"].as_array().expect("results");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["status"], "ok");
    assert_eq!(results[0]["result"]["first"], json!({ "S": ["a"] }));
    assert_eq!(
        results[0]["result"]["follow"],
        json!({ "S": { "terminals": [], "eof": true } })
    );
}

#[test]
fn test_driver_file_argument() {
    let path =
        std::env::temp_dir().join(format!("adze_conformance_976_{}.json", std::process::id()));
    std::fs::write(&path, batch(json!([simple_case("from_file")]))).expect("write request file");
    let response = run_driver(&[], Some(&path));
    let _ = std::fs::remove_file(&path);
    let results = response["results"].as_array().expect("results");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["case_id"], "from_file");
    assert_eq!(results[0]["status"], "ok");
}

#[test]
fn test_driver_malformed_profile_and_row_accounting() {
    let malformed = run_driver(b"not json", None);
    assert_eq!(malformed["status"], "invalid_input");
    assert_eq!(malformed["results"], json!([]));

    let wrong_profile = serde_json::to_vec(&json!({
        "profile": "cfg.other.v9",
        "schema_version": 1,
        "cases": [simple_case("one_case"), simple_case("two_case")],
    }))
    .expect("request");
    let response = run_driver(&wrong_profile, None);
    assert_eq!(response["profile"], PROFILE);
    let results = response["results"].as_array().expect("results");
    assert_eq!(results.len(), 2, "one row per requested case");
    assert!(results.iter().all(|row| row["status"] == "invalid_input"));
    let ids: Vec<&str> = results
        .iter()
        .map(|row| row["case_id"].as_str().expect("case_id"))
        .collect();
    assert_eq!(ids, vec!["one_case", "two_case"]);
}

#[test]
fn test_driver_rejects_expectation_in_request() {
    let mut case_with_expectation = simple_case("with_expectation");
    case_with_expectation["expectation"] = json!({ "nullable": [] });
    let response = run_driver(&batch(json!([case_with_expectation])), None);
    let results = response["results"].as_array().expect("results");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["status"], "invalid_input");
    assert!(
        results[0]["reason"]
            .as_str()
            .expect("reason")
            .contains("expectation_not_allowed")
    );
}

#[test]
fn test_driver_corpus_dense_cases_when_present() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
        .join("cases")
        .join("cfg.nullable_first_follow.v1")
        .join("corpus.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!(
            "skipping corpus driver test: {} not present (contract branch not merged)",
            path.display()
        );
        return;
    };
    let corpus: Value = serde_json::from_str(&text).expect("corpus JSON");
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
    let response = run_driver(&batch(json!(engine_cases)), None);
    let results = response["results"].as_array().expect("results");
    assert_eq!(results.len(), cases.len());
    for (item, row) in cases.iter().zip(results) {
        assert_eq!(row["case_id"], item["case_id"]);
        assert_eq!(row["status"], "ok", "case {} row: {row}", item["case_id"]);
        let expected = &item["expectation"];
        assert_eq!(row["result"]["nullable"], expected["nullable"]);
        for nonterminal in item["nonterminals"].as_array().expect("nonterminals") {
            let name = nonterminal.as_str().expect("name");
            assert_eq!(row["result"]["first"][name], expected["first"][name]);
            assert_eq!(row["result"]["follow"][name], expected["follow"][name]);
        }
    }
}

#[test]
fn test_published_crate_excludes_examples() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).expect("read Cargo.toml");
    let include_line = text
        .lines()
        .find(|line| line.trim_start().starts_with("include"))
        .expect("package include list present");
    assert!(
        include_line.contains("\"src/**\""),
        "include list must ship src: {include_line}"
    );
    assert!(
        !include_line.contains("example"),
        "published crate must exclude examples: {include_line}"
    );
    // The driver lives under examples/, outside every included path.
    let driver = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("conformance_sets.rs");
    assert!(driver.is_file(), "driver example must exist on disk");
}
