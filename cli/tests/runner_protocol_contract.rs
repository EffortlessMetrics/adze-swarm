//! Raw private-wire fixtures reach the real CLI parent (#1066).
//!
//! `run_raw` writes the supplied bytes without framing or normalizing them.
//! These are transport controls; unchanged generated tree/projection/recovery
//! canaries separately qualify the actual grammar and document path.
#![cfg(unix)]

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::process::Output;
use std::time::Duration;

fn payload() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema":"adze.document.v1", "source":{"byte_len":5},
        "language":{"name":"fixture"},
        "metadata":{"outcome":"complete", "error_count":0, "incremental_requested":false,
            "incremental_used":false, "fallback_reason":null},
        "diagnostics":[], "ambiguities":[],
        "tree":{"root":{
            "id":0,"kind":"source_file","kind_id":1,"grammar_kind":"source_file","grammar_id":1,
            "alias_symbol_id":null,"has_alias":false,
            "range":{"start_byte":0,"end_byte":5,"start_point":{"row":0,"column":0},"end_point":{"row":0,"column":5}},
            "flags":{"named":true,"visible":true,"extra":false,"terminal":false,"supertype":false,
                "error":false,"missing":false,"has_error":false},
            "text":"hello", "children":[]
        }}
    }))
    .unwrap()
}

fn header(payload_bytes: usize) -> Value {
    json!({
        "protocol":"adze.static-runner", "version":2,
        "operation":"document-json", "terminal":"output",
        "payload_bytes":payload_bytes
    })
}

fn join_raw(header: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut bytes = header.to_vec();
    bytes.push(b'\n');
    bytes.extend_from_slice(payload);
    bytes
}

fn valid_frame(payload: &[u8]) -> Vec<u8> {
    join_raw(&serde_json::to_vec(&header(payload.len())).unwrap(), payload)
}

fn run_raw(mode: &str, bytes: &[u8], stderr: &[u8], status: u8) -> Output {
    let fixture = tempfile::Builder::new()
        .prefix("adze protocol ' λ ")
        .tempdir()
        .unwrap();
    let bin = fixture.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let cargo = bin.join("cargo");
    std::fs::write(
        &cargo,
        "#!/bin/sh\nset -eu\n[ \"$#\" = 8 ]\n[ \"$1\" = run ]\n[ \"$2\" = --quiet ]\n[ \"$3\" = --manifest-path ]\n[ -f \"$4\" ]\n[ \"$5\" = -- ]\n[ \"$6\" = \"$EXPECTED_INPUT\" ]\n[ \"$7\" = \"$EXPECTED_OPERATION\" ]\n[ \"$8\" = --adze-static-runner-protocol=2 ]\n[ \"$PWD\" = \"$EXPECTED_CWD\" ]\n[ \"$ADZE_USE_PURE_RUST\" = 1 ]\ncat \"$WIRE_FILE\"\ncat \"$STDERR_FILE\" >&2\nexit \"$EXIT_CODE\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    let wire = fixture.path().join("wire.bin");
    let err = fixture.path().join("stderr.bin");
    std::fs::write(&wire, bytes).unwrap();
    std::fs::write(&err, stderr).unwrap();
    let grammar = fixture.path().join("grammar ' λ.rs");
    let input = fixture.path().join("input ' λ.txt");
    std::fs::write(&grammar, "#[adze::grammar(\"fixture\")] mod grammar {}\n").unwrap();
    std::fs::write(&input, "hello").unwrap();
    let mut paths = vec![bin];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let operation = match mode {
        "tree" | "json" | "sexp" | "dot" => "document-json",
        other => other,
    };
    cargo_bin_cmd!("adze")
        .arg("parse")
        .arg(&grammar)
        .arg(&input)
        .args(["--output", mode])
        .current_dir(fixture.path())
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("EXPECTED_INPUT", &input)
        .env("EXPECTED_OPERATION", operation)
        .env("EXPECTED_CWD", fixture.path().canonicalize().unwrap())
        .env("WIRE_FILE", wire)
        .env("STDERR_FILE", err)
        .env("EXIT_CODE", status.to_string())
        .timeout(Duration::from_secs(15))
        .output()
        .unwrap()
}

fn reject_raw(bytes: &[u8]) {
    reject_raw_with_reason(bytes, "static runner protocol rejected");
}

fn reject_raw_with_reason(bytes: &[u8], expected_reason: &str) {
    let output = run_raw("document-json", bytes, b"legitimate stderr diagnostic\n", 0);
    assert!(!output.status.success(), "admitted raw frame: {bytes:?}");
    assert!(output.stdout.is_empty(), "leaked wire bytes: {output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("static runner protocol rejected"),
        "{output:?}"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(expected_reason),
        "expected {expected_reason:?}: {output:?}"
    );
}

#[test]
fn test_versioned_request_and_raw_public_bytes_keep_existing_positions_and_stderr() {
    let mut bytes = b" \n".to_vec();
    bytes.extend_from_slice(&payload());
    bytes.extend_from_slice(b"\n\n");
    let output = run_raw(
        "document-json",
        &valid_frame(&bytes),
        b"legitimate stderr diagnostic\n\xff\n",
        0,
    );
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, bytes);
}

#[test]
fn test_protocol_version_and_requested_operation_must_match() {
    let bytes = payload();
    for (key, value) in [
        ("protocol", json!("adze.document.v1")),
        ("version", json!(1)),
        ("version", json!(3)),
        ("operation", json!("tree-json")),
        ("operation", json!("unknown-operation")),
    ] {
        let mut wrong = header(bytes.len());
        wrong[key] = value;
        reject_raw(&join_raw(&serde_json::to_vec(&wrong).unwrap(), &bytes));
    }
}

#[test]
fn test_header_requires_every_typed_field_and_rejects_unknown_fields() {
    let bytes = payload();
    for missing in ["protocol", "version", "operation", "terminal", "payload_bytes"] {
        let mut wrong = header(bytes.len());
        wrong.as_object_mut().unwrap().remove(missing);
        reject_raw(&join_raw(&serde_json::to_vec(&wrong).unwrap(), &bytes));
    }
    for (key, value) in [
        ("protocol", json!(2)),
        ("version", json!("2")),
        ("version", json!(4294967296_u64)),
        ("operation", json!(null)),
        ("terminal", json!(true)),
        ("payload_bytes", json!(-1)),
        ("payload_bytes", json!(1.5)),
        ("payload_bytes", json!("1")),
        ("extra", json!("undeclared")),
    ] {
        let mut wrong = header(bytes.len());
        wrong[key] = value;
        reject_raw(&join_raw(&serde_json::to_vec(&wrong).unwrap(), &bytes));
    }
}

#[test]
fn test_duplicate_header_fields_are_not_silently_overwritten() {
    let bytes = payload();
    for extra in [
        r#","protocol":"adze.static-runner"}"#,
        r#","version":2}"#,
        r#","operation":"document-json"}"#,
        r#","terminal":"output"}"#,
        r#","payload_bytes":0}"#,
    ] {
        // Append raw duplicate keys after serialization; neither run_raw nor
        // any helper reparses/re-serializes this malformed header.
        let mut duplicate = serde_json::to_vec(&header(bytes.len())).unwrap();
        assert_eq!(duplicate.pop(), Some(b'}'));
        duplicate.extend_from_slice(extra.as_bytes());
        reject_raw_with_reason(&join_raw(&duplicate, &bytes), "duplicate field");
    }
}

#[test]
fn test_zero_exit_requires_an_output_terminal_without_classifying_parser_outcome() {
    let bytes = payload();
    for terminal in ["", "started", "error", "failed", "recovered", "complete"] {
        let mut wrong = header(bytes.len());
        wrong["terminal"] = json!(terminal);
        reject_raw(&join_raw(&serde_json::to_vec(&wrong).unwrap(), &bytes));
    }
    // Existing runner_output_contract separately admits each canonical
    // complete/recovered/failed document outcome inside terminal="output".
}

#[test]
fn test_strict_framing_rejects_noise_truncation_extra_bytes_and_duplicate_terminals() {
    let bytes = payload();
    let frame = valid_frame(&bytes);
    let mut truncated = frame.clone();
    truncated.pop();
    reject_raw(&truncated);
    for prefix in [b"noise\n".as_slice(), b"\n", b"\xff\n"] {
        let mut noisy = prefix.to_vec();
        noisy.extend_from_slice(&frame);
        reject_raw(&noisy);
    }
    for suffix in [b"x".as_slice(), b"\n", frame.as_slice()] {
        let mut extra = frame.clone();
        extra.extend_from_slice(suffix);
        reject_raw(&extra);
    }
    for length in [0, bytes.len() - 1, bytes.len() + 1] {
        reject_raw(&join_raw(
            &serde_json::to_vec(&header(length)).unwrap(),
            &bytes,
        ));
    }
    let mut huge = header(bytes.len());
    huge["payload_bytes"] = json!(u64::MAX);
    reject_raw(&join_raw(&serde_json::to_vec(&huge).unwrap(), &bytes));
}

#[test]
fn test_header_limit_counts_its_line_feed_and_never_scans_into_payload() {
    let bytes = payload();
    let mut bounded = serde_json::to_vec(&header(bytes.len())).unwrap();
    bounded.resize(4095, b' ');
    let frame = join_raw(&bounded, &bytes);
    let output = run_raw("document-json", &frame, b"", 0);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, bytes);
    bounded.push(b' ');
    reject_raw(&join_raw(&bounded, &bytes));
    // This has exactly 4096 bytes and no LF, not a complete bounded header.
    reject_raw(&bounded);
}

#[test]
fn test_zero_exit_without_a_complete_result_cannot_succeed() {
    for bytes in [b"".as_slice(), b"{}", b"{}\n", b"[]\n", b"null\n"] {
        reject_raw(bytes);
    }
    // A syntactically complete private frame still needs the existing public
    // admission step; no valid payload is invented from an empty body.
    let output = run_raw("document-json", &valid_frame(b""), b"", 0);
    assert!(!output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("static runner output"));
}

#[test]
fn test_nonzero_status_precedes_valid_success_frames_and_raw_malformed_frames() {
    let frame = valid_frame(&payload());
    for bytes in [frame.as_slice(), b"\xff\n", b"{}\ntruncated", b""] {
        let output = run_raw(
            "document-json",
            bytes,
            b"Error: controlled runner failure\n",
            23,
        );
        assert!(!output.status.success(), "{output:?}");
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("controlled runner failure"), "{stderr}");
        assert!(stderr.contains("23"), "{stderr}");
        assert!(!stderr.contains("protocol rejected"), "{stderr}");
        assert!(!stderr.contains("not UTF-8"), "{stderr}");
    }
}

#[test]
fn test_payload_length_is_bytes_and_utf8_payload_admission_stays_canonical() {
    let mut document: Value = serde_json::from_slice(&payload()).unwrap();
    document["language"]["name"] = json!("fixture λ");
    let text = serde_json::to_string(&document).unwrap();
    let output = run_raw("document-json", &valid_frame(text.as_bytes()), b"", 0);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, text.as_bytes());
    let wrong = header(text.chars().count());
    reject_raw(&join_raw(
        &serde_json::to_vec(&wrong).unwrap(),
        text.as_bytes(),
    ));
    let output = run_raw("document-json", &valid_frame(b"\xff"), b"", 0);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not UTF-8"));
}
