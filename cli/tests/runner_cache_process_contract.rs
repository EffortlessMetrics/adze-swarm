//! Native acquisition/publication controls for disabled cache groundwork.
//!
//! Cargo supplies the exact real CLI bootstrap. The tiny dependency-free Cargo
//! package is an owned process fixture, not an Adze-generated grammar and not a
//! warm-cache acceptance oracle. Existing generated parser canaries stay separate.
#![cfg(any(target_os = "linux", windows))]

#[path = "../src/runner_cache.rs"]
mod runner_cache;
#[path = "../src/runner_invocation.rs"]
mod runner_invocation;
#[cfg(target_os = "linux")]
#[path = "../src/runner_process.rs"]
mod runner_process;
#[path = "../src/runner_protocol.rs"]
mod runner_protocol;
#[cfg(windows)]
#[path = "../src/runner_windows.rs"]
mod runner_windows;
#[cfg(windows)]
#[path = "../src/runner_windows_handles.rs"]
mod runner_windows_handles;
#[cfg(windows)]
#[path = "../src/runner_windows_request.rs"]
mod runner_windows_request;

use runner_invocation::{ProcessFailure, ProcessRunner, RunnerInvocation};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const PAYLOAD: &str = "{\"schema\":\"adze.ambiguity.v1\",\"document_schema\":\"adze.document.v1\",\"language\":{\"name\":\"fixture\"},\"ambiguities\":[]}\n";

// The fixed bytes below are a native process/transport fixture, never a claim
// that a fake producer proves generated parser behavior.
const FIXTURE_MAIN: &str = r##"use std::io::Write;
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.is_empty() {
        println!("owned fixture");
    } else if args[0] == "--failure" {
        std::io::stdout().write_all(b"purported success bytes\n").unwrap();
        eprintln!("Error: owned fixture failure");
        std::process::exit(37);
    } else {
        assert_eq!(args.len(), 3);
        assert_eq!(args[0], "input space λ.txt");
        assert_eq!(args[1], "ambiguity-json");
        assert_eq!(args[2], "--adze-static-runner-protocol=2");
        assert_eq!(std::env::var_os("ADZE_USE_PURE_RUST"), Some("1".into()));
        let payload = "{\"schema\":\"adze.ambiguity.v1\",\"document_schema\":\"adze.document.v1\",\"language\":{\"name\":\"fixture\"},\"ambiguities\":[]}\n";
        println!("{{\"protocol\":\"adze.static-runner\",\"version\":2,\"operation\":\"ambiguity-json\",\"terminal\":\"output\",\"payload_bytes\":{}}}", payload.len());
        std::io::stdout().write_all(payload.as_bytes()).unwrap();
        eprintln!("legitimate fixture stderr");
    }
}
"##;

fn save_error(directory: &Path, error: &anyhow::Error) {
    let _ = std::fs::write(directory.join("last-error.txt"), format!("{error:#}\n"));
    let channels = error
        .downcast_ref::<runner_cache::CargoFailed>()
        .map(|failed| (&failed.stdout, &failed.stderr));
    #[cfg(target_os = "linux")]
    let channels = channels.or_else(|| {
        error
            .downcast_ref::<runner_process::CaptureError>()
            .map(|failed| (&failed.stdout, &failed.stderr))
    });
    #[cfg(windows)]
    let channels = channels.or_else(|| {
        error
            .downcast_ref::<runner_windows::CaptureError>()
            .map(|failed| (&failed.stdout, &failed.stderr))
    });
    if let Some((stdout, stderr)) = channels {
        let _ = std::fs::write(directory.join("last-error.stdout"), stdout);
        let _ = std::fs::write(directory.join("last-error.stderr"), stderr);
    }
}

fn require<T>(result: anyhow::Result<T>, directory: &Path) -> T {
    match result {
        Ok(value) => value,
        Err(error) => {
            save_error(directory, &error);
            #[cfg(target_os = "linux")]
            if let Some(signal) = error
                .downcast_ref::<runner_process::CaptureError>()
                .and_then(runner_process::CaptureError::cancellation)
            {
                eprintln!(
                    "native cache control cancelled; fixture {}: {error:#}",
                    directory.display()
                );
                runner_process::exit_after_cancellation(signal);
            }
            #[cfg(windows)]
            if let Some(event) = error
                .downcast_ref::<runner_windows::CaptureError>()
                .and_then(runner_windows::CaptureError::cancellation)
            {
                eprintln!(
                    "native cache control cancelled; fixture {}: {error:#}",
                    directory.display()
                );
                std::process::exit(event.exit_code());
            }
            panic!(
                "native cache control failed; fixture {}: {error:#}",
                directory.display()
            );
        }
    }
}

fn capture_fixture(
    process: &ProcessRunner,
    executable: &Path,
    args: Vec<OsString>,
    directory: &Path,
) -> std::process::Output {
    require(
        process.capture(&RunnerInvocation {
            program: executable.as_os_str().to_owned(),
            args,
            environment: vec![("ADZE_USE_PURE_RUST".into(), "1".into())],
        }),
        directory,
    )
}

fn native_contract(directory: &Path) {
    let cli = PathBuf::from(env!("CARGO_BIN_EXE_adze"));
    assert!(
        cli.is_file(),
        "Cargo's exact CLI artifact must exist: {}",
        cli.display()
    );
    eprintln!("native cache bootstrap: {}", cli.display());
    let process = ProcessRunner::new(cli);
    #[cfg(windows)]
    assert!(runner_windows::bootstrap_if_requested().is_none());

    // Real compiler acquisition remains required. The pure path/content units
    // deliberately do not claim to have launched rustc.
    let fingerprint = require(runner_cache::rustc_fingerprint(&process), directory)
        .expect("real rustc must supply a successful fingerprint");
    assert!(
        fingerprint
            .lines()
            .any(|line| line.starts_with("release: ") && line.len() > 9)
    );
    assert!(
        fingerprint
            .lines()
            .any(|line| line.starts_with("host: ") && line.len() > 6)
    );
    assert_eq!(
        require(runner_cache::rustc_fingerprint(&process), directory).as_deref(),
        Some(fingerprint.as_str())
    );
    eprintln!("native cache fingerprint:\n{fingerprint}");
    std::fs::write(directory.join("fingerprint.txt"), &fingerprint).unwrap();

    let grammar = directory.join("grammar space λ.rs");
    std::fs::write(&grammar, "").unwrap();
    let spec = runner_cache::RunnerSpec {
        grammar: &grammar,
        module: "fixture",
        cargo_toml: "[package]\nname = \"adze-cli-parse-runner\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n".into(),
        build_rs: "fn main() {}".into(),
        main_rs: FIXTURE_MAIN.into(),
    };
    let key = require(
        runner_cache::grammar_candidate_key(&spec, &process),
        directory,
    )
    .expect("real candidate identity");
    let root = directory.join("cache");
    std::fs::create_dir(&root).unwrap();
    let config = runner_cache::CacheConfig {
        root: root.clone(),
        max_entries: 2,
    };
    let staging = runner_cache::claim_staging_dir(&root).expect("owned staging");
    let built = require(
        runner_cache::build_and_publish(&spec, &config, &key, &staging, &process),
        directory,
    );
    let runner_cache::CachedBuild::Ready(executable) = built else {
        panic!("expected owned fixture publication")
    };
    assert_eq!(
        executable
            .extension()
            .and_then(|extension| extension.to_str()),
        if cfg!(windows) { Some("exe") } else { None }
    );
    let output = capture_fixture(&process, &executable, Vec::new(), directory);
    assert!(
        output.status.success(),
        "owned executable failed: {output:?}"
    );
    assert_eq!(output.stdout, b"owned fixture\n");
    assert!(runner_cache::validate_entry(&root, &key).is_some());
    // Preserve the prior oracle: isolated publication never grants production
    // lookup/build admission, even after a complete fixture entry exists.
    assert!(runner_cache::lookup(&spec).is_none());
    assert!(matches!(
        require(runner_cache::build_cached(&spec, &process), directory),
        runner_cache::CachedBuild::Unavailable
    ));

    assert_eq!(runner_protocol::PROTOCOL_NAME, "adze.static-runner");
    assert_eq!(
        runner_protocol::request_token(),
        "--adze-static-runner-protocol=2"
    );
    let mut framed = capture_fixture(
        &process,
        &executable,
        vec![
            "input space λ.txt".into(),
            "ambiguity-json".into(),
            runner_protocol::request_token().into(),
        ],
        directory,
    );
    assert!(
        framed.status.success(),
        "framed native fixture failed: {framed:?}"
    );
    assert_eq!(framed.stderr, b"legitimate fixture stderr\n");
    require(
        runner_protocol::unframe(&mut framed.stdout, "ambiguity-json"),
        directory,
    );
    assert_eq!(framed.stdout, PAYLOAD.as_bytes());
    let failure = capture_fixture(&process, &executable, vec!["--failure".into()], directory);
    assert_eq!(failure.status.code(), Some(37));
    assert_eq!(failure.stdout, b"purported success bytes\n");
    assert_eq!(failure.stderr, b"Error: owned fixture failure\n");

    // NUL is a concrete OS/request setup refusal on both platforms. It must
    // remain typed through caller context and cache fallback classification.
    let error = process
        .capture(&RunnerInvocation {
            program: "refused\0program".into(),
            args: Vec::new(),
            environment: Vec::new(),
        })
        .expect_err("invalid program must refuse");
    assert!(error.downcast_ref::<ProcessFailure>().is_some());
    #[cfg(target_os = "linux")]
    assert!(
        error
            .downcast_ref::<runner_process::CaptureError>()
            .is_some()
    );
    #[cfg(windows)]
    assert!(
        error
            .downcast_ref::<runner_windows::CaptureError>()
            .is_some()
    );
    let Err(error) = runner_cache::finish_cached_build(Err(error.context("cache process caller")))
    else {
        panic!("actual process refusal was swallowed into fallback")
    };
    assert!(error.downcast_ref::<ProcessFailure>().is_some());
    #[cfg(target_os = "linux")]
    assert!(
        error
            .downcast_ref::<runner_process::CaptureError>()
            .is_some()
    );
    #[cfg(windows)]
    assert!(
        error
            .downcast_ref::<runner_windows::CaptureError>()
            .is_some()
    );
    save_error(directory, &error);

    // Actual Cargo failure remains distinct from framing and cache misses.
    let bad_spec = runner_cache::RunnerSpec {
        cargo_toml: "[package\n".into(),
        ..spec
    };
    let bad_staging = runner_cache::claim_staging_dir(&root).expect("second owned staging");
    let outcome = runner_cache::build_and_publish(&bad_spec, &config, &key, &bad_staging, &process);
    let Err(error) = runner_cache::finish_cached_build(outcome) else {
        panic!("actual failed Cargo build became a miss or successful publication")
    };
    let failed = error
        .downcast_ref::<runner_cache::CargoFailed>()
        .expect("original Cargo failure");
    assert!(!failed.status.success());
    assert!(!failed.stderr.is_empty());
    assert!(!bad_staging.join("entry").exists());
    std::fs::write(directory.join("failed-cargo.stdout"), &failed.stdout).unwrap();
    std::fs::write(directory.join("failed-cargo.stderr"), &failed.stderr).unwrap();
    eprintln!(
        "native cache process/publication controls complete; production reuse remains disabled"
    );
}

#[test]
fn test_cache_process_custody_preserves_refusal_and_owned_publication() {
    // One controller owns the only native captures in this target. The shared
    // cache module's other unit tests use independent literal fingerprints.
    let directory = tempfile::Builder::new()
        .prefix("adze cache native λ ")
        .tempdir()
        .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        native_contract(directory.path())
    }));
    if let Err(failure) = result {
        let retained = directory.keep();
        eprintln!(
            "retained failed native cache fixture: {}",
            retained.display()
        );
        std::panic::resume_unwind(failure);
    }
}
