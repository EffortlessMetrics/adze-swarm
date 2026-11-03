//! Completed public output reaches the real CLI parent without a Cargo build.
//! Unix fake-Cargo fixtures exercise admission, not grammar/parser semantics.
//! Only the transport helper adds a valid private frame; payloads and their
//! existing independent assertions remain unchanged. Malformed wire frames
//! are supplied directly by runner_protocol_contract instead.
#![cfg(unix)]

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;

fn document() -> Value {
    json!({
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
    })
}

fn run(mode: &str, stdout: &[u8], stderr: &[u8], status: u8) -> std::process::Output {
    let fixture = tempfile::Builder::new()
        .prefix("adze output ' λ ")
        .tempdir()
        .unwrap();
    let bin = fixture.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let cargo = bin.join("cargo");
    std::fs::write(&cargo, "#!/bin/sh\ncat \"$ADZE_FIXTURE_STDOUT\"\ncat \"$ADZE_FIXTURE_STDERR\" >&2\nexit \"$ADZE_FIXTURE_STATUS\"\n").unwrap();
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = fixture.path().join("out.bin");
    let err = fixture.path().join("err.bin");
    let operation = match mode {
        "tree" | "json" | "sexp" | "dot" => "document-json",
        other => other,
    };
    let header = format!(
        "{{\"protocol\":\"adze.static-runner\",\"version\":2,\"operation\":\"{operation}\",\"terminal\":\"output\",\"payload_bytes\":{}}}\n",
        stdout.len()
    );
    let mut framed = header.into_bytes();
    framed.extend_from_slice(stdout);
    std::fs::write(&out, framed).unwrap();
    std::fs::write(&err, stderr).unwrap();
    let grammar = fixture.path().join("grammar ' λ.rs");
    std::fs::write(&grammar, "#[adze::grammar(\"fixture\")] mod grammar {}\n").unwrap();
    let input = fixture.path().join("input ' λ.txt");
    std::fs::write(&input, "hello").unwrap();
    let mut paths = vec![bin];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    cargo_bin_cmd!("adze")
        .arg("parse")
        .arg(grammar)
        .arg(input)
        .arg("--output")
        .arg(mode)
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("ADZE_FIXTURE_STDOUT", out)
        .env("ADZE_FIXTURE_STDERR", err)
        .env("ADZE_FIXTURE_STATUS", status.to_string())
        .output()
        .unwrap()
}

fn reject(mode: &str, bytes: &[u8]) {
    let output = run(mode, bytes, b"legitimate stderr diagnostic\n", 0);
    assert!(
        !output.status.success(),
        "{mode} admitted {:?}",
        String::from_utf8_lossy(bytes)
    );
    assert!(
        output.stdout.is_empty(),
        "invalid child output leaked publicly: {:?}",
        output.stdout
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("static runner output"),
        "{:?}",
        output
    );
}

#[test]
fn test_valid_document_bytes_and_stderr_are_independent() {
    let bytes = format!(
        "  {}\n\n",
        serde_json::to_string_pretty(&document()).unwrap()
    );
    let output = run(
        "json",
        bytes.as_bytes(),
        b"legitimate stderr diagnostic\n",
        0,
    );
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(output.stdout, bytes.as_bytes());
    let output = run(
        "tree",
        bytes.as_bytes(),
        b"legitimate stderr diagnostic\n",
        0,
    );
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "source_file [0..5] \"hello\"\n"
    );
}

#[test]
fn test_canonical_document_outcomes_do_not_create_a_new_exit_policy() {
    for outcome in ["complete", "recovered", "failed"] {
        let mut value = document();
        value["metadata"]["outcome"] = json!(outcome);
        let bytes = serde_json::to_vec(&value).unwrap();
        let output = run("document-json", &bytes, b"", 0);
        assert!(output.status.success(), "{outcome}: {:?}", output);
        assert_eq!(output.stdout, bytes);
    }
}

#[test]
fn test_success_requires_utf8_and_one_complete_json_value() {
    for bytes in [
        b"".as_slice(),
        b"\xff",
        b"not JSON",
        b"{\"schema\":",
        b"null",
        b"true",
        b"[]",
    ] {
        reject("json", bytes);
    }
    let bytes = serde_json::to_vec(&document()).unwrap();
    for (prefix, suffix) in [
        ("noise\n", ""),
        ("", "\nnoise"),
        ("", "\n{}"),
        ("", "\nnull"),
    ] {
        reject(
            "json",
            format!(
                "{prefix}{}{suffix}",
                String::from_utf8(bytes.clone()).unwrap()
            )
            .as_bytes(),
        );
    }
}

#[test]
fn test_schema_and_projection_mismatch_are_rejected() {
    let mut value = document();
    value["schema"] = json!("adze.document.v999");
    reject("json", &serde_json::to_vec(&value).unwrap());
    for mode in ["tree-json", "diagnostics-json", "ambiguity-json"] {
        reject(mode, &serde_json::to_vec(&document()).unwrap());
    }
    for key in [
        "schema",
        "source",
        "language",
        "metadata",
        "diagnostics",
        "ambiguities",
        "tree",
    ] {
        let mut value = document();
        value.as_object_mut().unwrap().remove(key);
        reject("json", &serde_json::to_vec(&value).unwrap());
    }
}

#[test]
fn test_malformed_selected_nodes_never_render_default_facts() {
    for mode in ["tree", "sexp", "dot"] {
        for key in ["kind", "range", "flags", "children", "text"] {
            let mut value = document();
            value["tree"]["root"].as_object_mut().unwrap().remove(key);
            reject(mode, &serde_json::to_vec(&value).unwrap());
        }
        for (pointer, wrong) in [
            ("/tree/root", json!(true)),
            ("/tree/root/kind", json!(5)),
            ("/tree/root/range/start_byte", json!(-1)),
            ("/tree/root/range/end_byte", json!("5")),
            ("/tree/root/flags/has_error", json!("false")),
            ("/tree/root/children", json!(null)),
            (
                "/tree/root/children",
                json!([{"field_name":null,"node":null}]),
            ),
        ] {
            let mut value = document();
            *value.pointer_mut(pointer).unwrap() = wrong;
            reject(mode, &serde_json::to_vec(&value).unwrap());
        }
    }
}

#[test]
fn test_projection_payloads_require_their_serialized_fields() {
    for (mode, schema, key) in [
        ("tree-json", "adze.tree.v1", "tree"),
        ("diagnostics-json", "adze.diagnostics.v1", "diagnostics"),
        ("ambiguity-json", "adze.ambiguity.v1", "ambiguities"),
    ] {
        let doc = document();
        let value = json!({"schema":schema,"document_schema":"adze.document.v1","language":doc["language"],(key):doc[key]});
        let bytes = serde_json::to_vec(&value).unwrap();
        let output = run(mode, &bytes, b"", 0);
        assert!(output.status.success(), "{mode}: {:?}", output);
        assert_eq!(output.stdout, bytes);
        for missing in ["document_schema", "language", key] {
            let mut broken = value.clone();
            broken.as_object_mut().unwrap().remove(missing);
            reject(mode, &serde_json::to_vec(&broken).unwrap());
        }
        let mut broken = value.clone();
        broken[key] = json!(true);
        reject(mode, &serde_json::to_vec(&broken).unwrap());
    }
}

#[test]
fn test_malformed_diagnostic_and_ambiguity_fields_are_rejected() {
    for (key, value) in [
        ("diagnostics", json!([{}])),
        ("ambiguities", json!([{}])),
        ("diagnostics", json!(null)),
        ("ambiguities", json!("none")),
    ] {
        let mut doc = document();
        doc[key] = value;
        reject("json", &serde_json::to_vec(&doc).unwrap());
    }
}

#[test]
fn test_nonzero_process_status_wins_over_valid_or_malformed_payload() {
    for bytes in [serde_json::to_vec(&document()).unwrap(), vec![0xff]] {
        let output = run("json", &bytes, b"Error: controlled runner failure\n", 23);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("controlled runner failure"), "{stderr}");
        assert!(stderr.contains("23"), "{stderr}");
        assert!(!stderr.contains("not UTF-8"), "{stderr}");
    }
}

#[test]
fn test_serialized_diagnostic_and_ambiguity_facts_are_preserved() {
    let mut doc = document();
    doc["metadata"]["outcome"] = json!("recovered");
    doc["metadata"]["error_count"] = json!(1);
    doc["tree"]["root"]["flags"]["has_error"] = json!(true);
    doc["diagnostics"] = json!([{
        "start_byte":2,"end_byte":3,"point_range":{"start":{"row":0,"column":2},"end":{"row":0,"column":3}},
        "found":"@","expected":["word"],"related_nodes":[0],"message":"controlled diagnostic"
    }]);
    doc["ambiguities"] = json!([{
        "span":{"start_byte":0,"end_byte":5},"selected":null,"selection_reason":"Unresolved",
        "alternatives":[{"index":0,"root_symbol":1,"span":{"start_byte":0,"end_byte":5},
            "dynamic_precedence":-1,"in_error":false,"cost":0,"node_count":1}]
    }]);
    let bytes = serde_json::to_vec(&doc).unwrap();
    let output = run("json", &bytes, b"", 0);
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(output.stdout, bytes);
    for pointer in [
        "/diagnostics/0/expected",
        "/diagnostics/0/related_nodes",
        "/diagnostics/0/point_range/start/row",
        "/ambiguities/0/alternatives/0/dynamic_precedence",
        "/ambiguities/0/selected",
    ] {
        let mut broken = doc.clone();
        *broken.pointer_mut(pointer).unwrap() = json!("wrong field type");
        reject("json", &serde_json::to_vec(&broken).unwrap());
    }
}

fn deep_document(nodes: usize) -> Value {
    let mut doc = document();
    let template = doc["tree"]["root"].clone();
    let mut node = template.clone();
    for id in 1..nodes {
        let mut parent = template.clone();
        parent["id"] = json!(id);
        parent["text"] = Value::Null;
        parent["children"] =
            json!([{"child_index":0,"field_name":null,"field_id":null,"node":node}]);
        node = parent;
    }
    doc["tree"]["root"] = node;
    doc
}

#[test]
fn test_deep_forwarded_json_preserves_original_document_and_projection_bytes() {
    for nodes in [64, 96] {
        let doc = deep_document(nodes);
        for mode in ["json", "document-json", "tree-json"] {
            let payload = if mode == "tree-json" {
                json!({"schema":"adze.tree.v1","document_schema":"adze.document.v1",
                    "language":doc["language"],"tree":doc["tree"]})
            } else {
                doc.clone()
            };
            let bytes = format!(" {}\n", serde_json::to_string(&payload).unwrap());
            let output = run(mode, bytes.as_bytes(), b"", 0);
            assert!(output.status.success(), "{mode}/{nodes}: {:?}", output);
            assert_eq!(output.stdout, bytes.as_bytes());
        }
    }
}

#[test]
fn test_deep_nodes_still_require_fields_and_one_complete_payload() {
    let doc = deep_document(96);
    let mut malformed = doc.clone();
    let mut leaf = &mut malformed["tree"]["root"];
    while !leaf["children"].as_array().unwrap().is_empty() {
        leaf = &mut leaf["children"][0]["node"];
    }
    leaf.as_object_mut().unwrap().remove("kind");
    reject("json", &serde_json::to_vec(&malformed).unwrap());
    let mut bytes = serde_json::to_vec(&doc).unwrap();
    bytes.pop();
    reject("json", &bytes);
    let mut bytes = serde_json::to_vec(&doc).unwrap();
    bytes.extend_from_slice(b"\n{}");
    reject("json", &bytes);
}

#[test]
fn test_visual_rendering_keeps_its_existing_recursive_decoder_limit() {
    let bytes = serde_json::to_vec(&deep_document(64)).unwrap();
    for mode in ["tree", "sexp", "dot"] {
        let output = run(mode, &bytes, b"", 0);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("recursion limit exceeded"));
    }
}
