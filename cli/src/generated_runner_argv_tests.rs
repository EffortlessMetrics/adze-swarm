//! Execute the real generated runner's private argument guards (#1066).
//!
//! One generated parser is built once, then invoked directly for every case.
//! These test-owned deadlines do not qualify production descendant custody.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::ffi::OsStr;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

const GRAMMAR: &str = r#"#[adze::grammar("generated_runner_argv")]
pub mod grammar {
    #[adze::language]
    #[derive(Debug)]
    pub struct Number {
        #[adze::leaf(pattern = r"[0-9]+", try_transform = |text| text.parse::<u32>())]
        pub value: u32,
    }
}
"#;

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn fail_owned_child(
    child: &mut Child,
    fixture: &Path,
    label: &str,
    reason: &str,
    stdout_path: &Path,
    stderr_path: &Path,
) -> ! {
    // Only the Child handle spawned by this control is killed and polled.
    // A bounded inability to confirm reaping is reported, never called clean.
    let kill = child.kill();
    let cleanup_deadline = Instant::now() + Duration::from_secs(5);
    let cleanup = loop {
        match child.try_wait() {
            Ok(Some(status)) => break format!("owned child reaped with {status}"),
            Err(error) => break format!("unconfirmed cleanup: try_wait failed: {error}"),
            Ok(None) if Instant::now() >= cleanup_deadline => {
                break "unconfirmed cleanup: owned child did not reap within five seconds".into();
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
        }
    };
    let stdout = fs::read(stdout_path)
        .unwrap_or_else(|error| format!("captured stdout could not be read: {error}").into_bytes());
    let stderr = fs::read(stderr_path)
        .unwrap_or_else(|error| format!("captured stderr could not be read: {error}").into_bytes());
    panic!(
        "{label}: {reason}; fixture={}\nkill={kill:?}; {cleanup}\nstdout {}:\n{}\nstderr {}:\n{}",
        fixture.display(),
        stdout_path.display(),
        String::from_utf8_lossy(&stdout),
        stderr_path.display(),
        String::from_utf8_lossy(&stderr)
    );
}

fn capture_owned(command: &mut Command, fixture: &Path, label: &str, timeout: Duration) -> Output {
    let stdout_path = fixture.join(format!("{label}.stdout"));
    let stderr_path = fixture.join(format!("{label}.stderr"));
    command
        .stdin(Stdio::null())
        .stdout(File::create(&stdout_path).expect("create owned stdout file"))
        .stderr(File::create(&stderr_path).expect("create owned stderr file"));
    eprintln!(
        "generated-runner-argv case={label} fixture={} deadline_seconds={} command={command:?}",
        fixture.display(),
        timeout.as_secs()
    );
    let mut child = command.spawn().unwrap_or_else(|error| {
        panic!(
            "{label}: spawn failed: {error}; fixture={}",
            fixture.display()
        )
    });
    // Close the controller's writer handles; the owned child keeps its copies.
    command.stdout(Stdio::null()).stderr(Stdio::null());
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Err(error) => fail_owned_child(
                &mut child,
                fixture,
                label,
                &format!("try_wait failed: {error}"),
                &stdout_path,
                &stderr_path,
            ),
            Ok(None) if Instant::now() >= deadline => fail_owned_child(
                &mut child,
                fixture,
                label,
                "execution deadline exceeded",
                &stdout_path,
                &stderr_path,
            ),
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
        }
    };
    Output {
        status,
        stdout: fs::read(stdout_path).expect("read completed stdout"),
        stderr: fs::read(stderr_path).expect("read completed stderr"),
    }
}

fn require_success(output: &Output, fixture: &Path, label: &str) {
    assert!(
        output.status.success(),
        "{label}: {}; fixture={}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        fixture.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn build_once(fixture: &Path) -> (PathBuf, Vec<u8>) {
    let grammar = fixture.join("grammar source π.rs");
    fs::write(&grammar, GRAMMAR).expect("write independent numeric grammar");
    let module = super::single_top_level_grammar_module(&grammar).expect("actual module selection");
    let manifest = super::parse_runner_cargo_toml().expect("actual runner manifest helper");
    let build = super::parse_runner_build_rs(&grammar);
    let main = super::parse_runner_main_rs(&grammar, &module);
    fs::create_dir(fixture.join("src")).expect("create generated source directory");
    fs::write(fixture.join("Cargo.toml"), &manifest).expect("write actual runner manifest");
    fs::write(fixture.join("build.rs"), &build).expect("write actual runner build source");
    fs::write(fixture.join("src/main.rs"), &main).expect("write actual runner main source");

    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let repository_lock = fs::read(repository.join("Cargo.lock")).expect("reviewed lockfile");
    fs::write(fixture.join("Cargo.lock"), &repository_lock).expect("seed generated runner lock");
    let target = fixture.join("native target");
    let cargo = |operation: &str| {
        let mut command = Command::new(env!("CARGO"));
        command
            .arg(operation)
            .arg("--offline")
            .arg("--manifest-path")
            .arg(fixture.join("Cargo.toml"))
            .current_dir(repository)
            .env("CARGO_TARGET_DIR", &target)
            .env("CARGO_TERM_COLOR", "never")
            .env("ADZE_USE_PURE_RUST", "1")
            .env_remove("CARGO_BUILD_TARGET");
        command
    };

    // Preparing this standalone root may adjust its copied lock. The subsequent
    // single native build must use that exact prepared lock unchanged.
    let mut metadata_command = cargo("metadata");
    metadata_command.args(["--format-version", "1"]);
    let metadata_output = capture_owned(
        &mut metadata_command,
        fixture,
        "metadata",
        Duration::from_secs(120),
    );
    require_success(&metadata_output, fixture, "prepare standalone lock");
    let metadata: Value =
        serde_json::from_slice(&metadata_output.stdout).expect("actual Cargo metadata JSON");
    let expected_manifest = fixture.join("Cargo.toml").canonicalize().unwrap();
    let packages: Vec<_> = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|package| {
            package["name"] == "adze-cli-parse-runner"
                && package["version"] == "0.0.0"
                && package["manifest_path"]
                    .as_str()
                    .and_then(|path| Path::new(path).canonicalize().ok())
                    .as_ref()
                    == Some(&expected_manifest)
        })
        .collect();
    assert_eq!(packages.len(), 1, "identify this generated root package");
    let package_id = packages[0]["id"].as_str().unwrap();
    assert_eq!(metadata["resolve"]["root"], package_id);
    let prepared_lock = fs::read(fixture.join("Cargo.lock")).unwrap();
    eprintln!(
        "generated-runner-argv package_id={package_id} repository_lock_sha256={} prepared_lock_sha256={} manifest_sha256={} build_sha256={} main_sha256={} grammar_sha256={}",
        digest(&repository_lock),
        digest(&prepared_lock),
        digest(manifest.as_bytes()),
        digest(build.as_bytes()),
        digest(main.as_bytes()),
        digest(GRAMMAR.as_bytes())
    );

    let mut build_command = cargo("build");
    build_command.args([
        "--locked",
        "--bin",
        "adze-cli-parse-runner",
        "--message-format=json",
    ]);
    let output = capture_owned(
        &mut build_command,
        fixture,
        "build",
        Duration::from_secs(600),
    );
    require_success(&output, fixture, "single generated native build");
    assert_eq!(fs::read(fixture.join("Cargo.lock")).unwrap(), prepared_lock);
    let artifacts: Vec<PathBuf> = String::from_utf8(output.stdout)
        .expect("Cargo artifact stream is UTF-8")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).expect("Cargo artifact JSON line"))
        .filter(|event| {
            event["reason"] == "compiler-artifact"
                && event["package_id"] == package_id
                && event["target"]["name"] == "adze-cli-parse-runner"
                && event["target"]["kind"] == serde_json::json!(["bin"])
        })
        .filter_map(|event| event["executable"].as_str().map(PathBuf::from))
        .collect();
    assert_eq!(artifacts.len(), 1, "one exact generated-package executable");
    let executable = artifacts.into_iter().next().unwrap();
    assert!(executable.is_file(), "actual Cargo artifact must exist");
    let expected_name = format!("adze-cli-parse-runner{}", std::env::consts::EXE_SUFFIX);
    assert_eq!(executable.file_name(), Some(OsStr::new(&expected_name)));
    eprintln!("generated-runner-argv executable={}", executable.display());
    (executable, prepared_lock)
}

fn invoke(executable: &Path, fixture: &Path, label: &str, arguments: &[&OsStr]) -> Output {
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .current_dir(fixture)
        .env("ADZE_USE_PURE_RUST", "1");
    capture_owned(&mut command, fixture, label, Duration::from_secs(20))
}

fn assert_document_and_payload(mut framed: Vec<u8>) {
    let header_end = framed.iter().position(|byte| *byte == b'\n').unwrap();
    let header: Value = serde_json::from_slice(&framed[..header_end]).unwrap();
    let original_payload = framed[header_end + 1..].to_vec();
    assert_eq!(header["protocol"], "adze.static-runner");
    assert_eq!(header["version"], 2);
    assert_eq!(header["operation"], "document-json");
    assert_eq!(header["terminal"], "output");
    assert_eq!(
        header["payload_bytes"].as_u64(),
        Some(u64::try_from(original_payload.len()).unwrap())
    );
    assert!(original_payload.ends_with(b"\n"));
    assert!(!original_payload.ends_with(b"\n\n"));
    super::runner_protocol::unframe(&mut framed, "document-json").expect("actual protocol decoder");
    assert_eq!(framed, original_payload, "unframing preserves exact bytes");
    let admitted = super::runner_output::RunnerOutput::parse(&framed, "document-json")
        .expect("existing output interpreter accepts real generated payload");
    assert_eq!(admitted.text().as_bytes(), original_payload.as_slice());

    // Literal facts are independent of the protocol decoder and payload copier.
    // Non-leaf JSON text is intentionally null; inspect the actual terminal.
    let document: Value = serde_json::from_slice(&framed).unwrap();
    assert_eq!(document["schema"], "adze.document.v1");
    assert_eq!(document["language"]["name"], "generated_runner_argv");
    assert_eq!(document["source"]["byte_len"], 2);
    assert_eq!(document["metadata"]["outcome"], "complete");
    assert_eq!(document["metadata"]["error_count"], 0);
    assert_eq!(document["diagnostics"], serde_json::json!([]));
    assert_eq!(document["ambiguities"], serde_json::json!([]));
    let root = &document["tree"]["root"];
    let expected_range = serde_json::json!({
        "start_byte": 0, "end_byte": 2,
        "start_point": { "row": 0, "column": 0 },
        "end_point": { "row": 0, "column": 2 },
    });
    assert_eq!(root["range"], expected_range);
    assert_eq!(root["flags"]["has_error"], false);
    let mut pending = vec![root];
    let mut terminals = Vec::new();
    while let Some(node) = pending.pop() {
        let children = node["children"].as_array().unwrap();
        if node["flags"]["terminal"] == true {
            assert!(children.is_empty());
            terminals.push(node);
        }
        for edge in children.iter().rev() {
            pending.push(&edge["node"]);
        }
    }
    assert_eq!(terminals.len(), 1, "one real numeric terminal");
    assert_eq!(terminals[0]["text"], "42");
    assert_eq!(terminals[0]["range"], expected_range);
}

fn run_cases(fixture: &Path) {
    let (executable, prepared_lock) = build_once(fixture);
    let missing = fixture.join("missing input 雪.txt");
    assert!(!missing.exists());
    let operation = OsStr::new("document-json");
    let token = super::runner_protocol::request_token();
    let token = OsStr::new(&token);
    for (label, arguments, error) in [
        (
            "missing-token",
            vec![missing.as_os_str(), operation],
            "missing private runner protocol request",
        ),
        (
            "wrong-token",
            vec![
                missing.as_os_str(),
                operation,
                OsStr::new("--adze-static-runner-protocol=wrong λ"),
            ],
            "unsupported private runner protocol request",
        ),
        (
            "extra-argument",
            vec![
                missing.as_os_str(),
                operation,
                token,
                OsStr::new("extra argv 雪"),
            ],
            "unsupported private runner protocol request",
        ),
    ] {
        let output = invoke(&executable, fixture, label, &arguments);
        assert!(!output.status.success(), "{label} unexpectedly succeeded");
        assert!(output.stdout.is_empty(), "{label} produced a success frame");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(stderr.trim_end(), format!("Error: {error:?}"), "{label}");
    }
    assert!(
        !missing.exists(),
        "guard controls must not create the input"
    );

    let io_error = fs::read_to_string(&missing).unwrap_err();
    assert_eq!(io_error.kind(), std::io::ErrorKind::NotFound);
    let output = invoke(
        &executable,
        fixture,
        "correct-token-missing-input",
        &[missing.as_os_str(), operation, token],
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap().trim_end(),
        format!("Error: {io_error:?}"),
        "the correct token must reach actual input I/O"
    );

    let input = fixture.join("source 42 λ.txt");
    fs::write(&input, "42").unwrap();
    let output = invoke(
        &executable,
        fixture,
        "correct-token-real-source",
        &[input.as_os_str(), operation, token],
    );
    require_success(&output, fixture, "actual generated positive control");
    assert!(output.stderr.is_empty());
    assert_document_and_payload(output.stdout);
    assert_eq!(fs::read(fixture.join("Cargo.lock")).unwrap(), prepared_lock);
}

#[test]
fn test_generated_runner_protocol_args_precede_input_io() {
    let fixture = tempfile::Builder::new()
        .prefix("adze generated argv π ")
        .tempdir()
        .expect("create owned fixture");
    let path = fixture.path().to_path_buf();
    let result = std::panic::catch_unwind(|| run_cases(&path));
    if let Err(payload) = result {
        let retained = fixture.keep();
        eprintln!(
            "generated-runner-argv failed; fixture and complete stdout/stderr retained at {}",
            retained.display()
        );
        std::panic::resume_unwind(payload);
    }
}
