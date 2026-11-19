//! Fail-closed cache admission for issue #1067: incomplete dependency/config
//! custody must use Cargo on every parse and never execute existing entries.
//!
//! Uses a real `adze init` starter and counts actual Cargo spawns solely
//! through an owned test fixture. The shipping CLI does not observe the log.

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

struct CountingCargo {
    _bin: tempfile::TempDir,
    executable: PathBuf,
    count_file: PathBuf,
    shim_path: OsString,
    original_path: Option<OsString>,
}

impl CountingCargo {
    fn new(count_file: &Path) -> Self {
        let real = Path::new(env!("CARGO"));
        assert!(
            real.is_absolute() && real.is_file(),
            "cannot bind real Cargo: {real:?}"
        );
        let bin = tempfile::Builder::new()
            .prefix("observed cargo ")
            .tempdir_in(count_file.parent().unwrap())
            .unwrap();
        let executable = bin
            .path()
            .join(format!("cargo{}", std::env::consts::EXE_SUFFIX));
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cargo_count_forwarder.rs");
        let built = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args([
                "--edition=2024",
                "-Dwarnings",
                "--crate-name",
                "adze_test_cargo_forwarder",
            ])
            .arg(source)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("compile the test-only Cargo forwarding fixture");
        assert!(
            built.status.success(),
            "Cargo forwarding fixture compilation failed: {}",
            String::from_utf8_lossy(&built.stderr)
        );

        let original_path = std::env::var_os("PATH");
        let mut paths = vec![bin.path().to_owned()];
        if let Some(path) = original_path.as_deref() {
            paths.extend(std::env::split_paths(path));
        }
        Self {
            _bin: bin,
            executable,
            count_file: count_file.to_owned(),
            shim_path: std::env::join_paths(paths).unwrap(),
            original_path,
        }
    }

    fn configure(&self, command: &mut assert_cmd::Command) {
        command
            .env("PATH", &self.shim_path)
            // Invoke the original absolute path, not canonicalized (a rustup
            // proxy may rely on its cargo executable name).
            .env("ADZE_TEST_FORWARD_CARGO", env!("CARGO"))
            .env("ADZE_TEST_FORWARD_LOG", &self.count_file);
        if let Some(path) = &self.original_path {
            command.env("ADZE_TEST_FORWARD_PATH", path);
        } else {
            command.env_remove("ADZE_TEST_FORWARD_PATH");
        }
    }
}

fn cargo_spawns(count_file: &Path) -> usize {
    std::fs::read_to_string(count_file)
        .map(|text| text.lines().filter(|line| !line.trim().is_empty()).count())
        .unwrap_or(0)
}

fn parse_with_cache(
    grammar: &Path,
    input: &Path,
    cache_dir: &Path,
    cargo: &CountingCargo,
) -> assert_cmd::assert::Assert {
    // A shared target dir keeps repeated cold builds in one test from
    // recompiling unchanged path dependencies; it changes artifact
    // placement only, never build identity or Cargo spawn counts.
    let target_dir = cache_dir.with_file_name("shared-target");
    let mut cmd = cargo_bin_cmd!("adze");
    cargo.configure(&mut cmd);
    cmd.env("ADZE_RUNNER_CACHE_DIR", cache_dir)
        .env("CARGO_TARGET_DIR", &target_dir)
        .arg("parse")
        .arg(grammar)
        .arg(input)
        .assert()
}

#[test]
fn test_owned_cargo_observer_forwards_native_status_and_output() {
    let temp = tempfile::tempdir().unwrap();
    let count_file = temp.path().join("cargo count λ.txt");
    let cargo = CountingCargo::new(&count_file);
    for (index, args) in [
        ["--version", "--verbose"],
        ["--version", "--adze-test-no-such-option"],
    ]
    .iter()
    .enumerate()
    {
        let expected = Command::new(env!("CARGO"))
            .args(args)
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert_eq!(expected.status.success(), index == 0);
        let mut observed = assert_cmd::Command::new(&cargo.executable);
        cargo.configure(&mut observed);
        let actual = observed
            .args(args)
            .current_dir(temp.path())
            .timeout(std::time::Duration::from_secs(15))
            .output()
            .unwrap();
        assert_eq!(actual.status, expected.status);
        assert_eq!(actual.stdout, expected.stdout);
        assert_eq!(actual.stderr, expected.stderr);
        assert_eq!(cargo_spawns(&count_file), index + 1);
    }
}

#[test]
fn test_unresolved_build_identity_rebuilds_and_parses_fresh_input() {
    let temp = tempfile::tempdir().expect("tempdir");
    let spaced = temp.path().join("cache dir ♥");
    std::fs::create_dir_all(&spaced).expect("spaced dir");
    let cache_dir = spaced.join("runner-cache");
    let count_file = spaced.join("cargo-count.txt");
    let cargo = CountingCargo::new(&count_file);
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg("calc")
        .arg("--output")
        .arg(&spaced)
        .assert()
        .success();
    let grammar = spaced.join("calc/src/grammar.rs");
    let input_a = spaced.join("a.txt");
    let input_b = spaced.join("b input ♥.txt");
    std::fs::write(&input_a, "123").expect("input a");
    std::fs::write(&input_b, "1 + 2").expect("input b");

    let first = parse_with_cache(&grammar, &input_a, &cache_dir, &cargo)
        .success()
        .stdout(predicate::str::contains("Expr"))
        .stdout(predicate::str::contains("[0..3]"))
        .get_output()
        .stdout
        .clone();
    assert_eq!(cargo_spawns(&count_file), 1);
    parse_with_cache(&grammar, &input_b, &cache_dir, &cargo)
        .success()
        .stdout(predicate::str::contains("Expr_Add"))
        .stdout(predicate::str::contains("[0..5]"));
    assert_eq!(
        cargo_spawns(&count_file),
        2,
        "unresolved dependencies/configuration require Cargo validation on every invocation"
    );
    let repeated = parse_with_cache(&grammar, &input_a, &cache_dir, &cargo)
        .success()
        .stdout(predicate::str::contains("[0..3]"))
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        first, repeated,
        "fresh builds retain independently expected output"
    );
    assert_eq!(cargo_spawns(&count_file), 3);
    assert!(
        !cache_dir.exists(),
        "unsupported admission must not read, create or publish entries"
    );
}

#[test]
fn test_runner_cache_disablement_falls_back_to_temp_build() {
    let temp = tempfile::tempdir().expect("tempdir");
    let cache_dir = temp.path().join("runner-cache");
    let count_file = temp.path().join("cargo-count.txt");
    let cargo = CountingCargo::new(&count_file);

    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg("nocache")
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();
    let grammar = temp.path().join("nocache").join("src").join("grammar.rs");
    let input = temp.path().join("input.txt");
    std::fs::write(&input, "7").expect("write input");

    // Disabled cache: every parse pays a fresh Cargo build, correctly.
    let target_dir = cache_dir.with_file_name("shared-target");
    for _ in 0..2 {
        let mut cmd = cargo_bin_cmd!("adze");
        cargo.configure(&mut cmd);
        cmd.env("ADZE_RUNNER_CACHE_DIR", &cache_dir)
            .env("CARGO_TARGET_DIR", &target_dir)
            .env("ADZE_RUNNER_CACHE", "off")
            .arg("parse")
            .arg(&grammar)
            .arg(&input)
            .assert()
            .success()
            .stdout(predicate::str::contains("[0..1]"));
    }
    assert_eq!(
        cargo_spawns(&count_file),
        2,
        "disabled cache must rebuild every parse"
    );
    assert!(
        !cache_dir.exists()
            || std::fs::read_dir(&cache_dir)
                .map(|entries| entries.count() == 0)
                .unwrap_or(true),
        "disabled cache must not publish entries"
    );
}

#[test]
fn test_unsupported_closure_falls_back_to_fresh_build() {
    let temp = tempfile::tempdir().expect("tempdir");
    let cache_dir = temp.path().join("runner-cache");
    let count_file = temp.path().join("cargo-count.txt");
    let cargo = CountingCargo::new(&count_file);

    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg("dyninclude")
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();
    let grammar = temp
        .path()
        .join("dyninclude")
        .join("src")
        .join("grammar.rs");
    // Dynamic `include!` arguments cannot be fingerprinted: rustc accepts
    // them, so the legacy build stays correct, but the cache must refuse.
    std::fs::write(grammar.with_file_name("data.txt"), "probe").expect("write data");
    let mut source = std::fs::read_to_string(&grammar).expect("read grammar");
    source.push_str("\nconst _PROBE: &str = include_str!(concat!(\"data\", \".txt\"));\n");
    std::fs::write(&grammar, source).expect("rewrite grammar");
    let input = temp.path().join("input.txt");
    std::fs::write(&input, "9").expect("write input");

    for _ in 0..2 {
        parse_with_cache(&grammar, &input, &cache_dir, &cargo)
            .success()
            .stdout(predicate::str::contains("[0..1]"));
    }
    assert_eq!(
        cargo_spawns(&count_file),
        2,
        "unfingerprintable includes must rebuild every parse, never reuse"
    );
    assert!(
        !cache_dir.exists()
            || std::fs::read_dir(&cache_dir)
                .map(|entries| entries.count() == 0)
                .unwrap_or(true),
        "unsupported envelope must not publish entries"
    );
}

// Retain the requested zero-Cargo oracle as explicitly pending acceptance.
// Passing fallback tests do not satisfy this obligation.
fn corrupt_every_entry_manifest(cache_dir: &Path) -> usize {
    let mut corrupted = 0;
    for entry in std::fs::read_dir(cache_dir)
        .expect("read cache dir")
        .flatten()
    {
        let manifest = entry.path().join("manifest.json");
        if manifest.is_file() {
            std::fs::write(&manifest, "definitely not a manifest").expect("corrupt manifest");
            corrupted += 1;
        }
    }
    corrupted
}

#[test]
#[ignore = "#1067: direct reuse requires complete dependency/configuration custody"]
fn test_cached_runner_builds_once_reuses_binary_and_recovers() {
    let temp = tempfile::tempdir().expect("tempdir");
    // Spaces and non-ASCII names exercise argv/path handling end to end.
    let spaced = temp.path().join("cache dir ♥");
    std::fs::create_dir_all(&spaced).expect("spaced dir");
    let cache_dir = spaced.join("runner-cache");
    let count_file = spaced.join("cargo-count.txt");
    let cargo = CountingCargo::new(&count_file);

    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg("calc")
        .arg("--output")
        .arg(&spaced)
        .assert()
        .success();
    let grammar = spaced.join("calc").join("src").join("grammar.rs");
    let input_a = spaced.join("a.txt");
    let input_b = spaced.join("b input ♥.txt");
    std::fs::write(&input_a, "123").expect("write input a");
    std::fs::write(&input_b, "1 + 2").expect("write input b");

    // Cold parse builds exactly once through Cargo.
    let first = parse_with_cache(&grammar, &input_a, &cache_dir, &cargo)
        .success()
        .stdout(predicate::str::contains("Expr"))
        .stdout(predicate::str::contains("[0..3]"))
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        cargo_spawns(&count_file),
        1,
        "cold parse must build the runner exactly once"
    );

    // Warm parse of DIFFERENT input reuses the binary: zero new Cargo
    // invocations, fresh parse of the new text.
    parse_with_cache(&grammar, &input_b, &cache_dir, &cargo)
        .success()
        .stdout(predicate::str::contains("Expr_Add"))
        .stdout(predicate::str::contains("[0..5]"));
    assert_eq!(
        cargo_spawns(&count_file),
        1,
        "warm parse must execute the cached binary without Cargo"
    );

    // Same input parses identically cold and warm: caching never changes the
    // outcome or diagnostics.
    let warm_same = parse_with_cache(&grammar, &input_a, &cache_dir, &cargo)
        .success()
        .stdout(predicate::str::contains("[0..3]"))
        .get_output()
        .stdout
        .clone();
    assert_eq!(first, warm_same, "cached output must match the cold output");
    assert_eq!(cargo_spawns(&count_file), 1);

    // A corrupt entry is refused and rebuilt, never executed.
    let corrupted = corrupt_every_entry_manifest(&cache_dir);
    assert!(corrupted >= 1, "cold build must have published an entry");
    parse_with_cache(&grammar, &input_a, &cache_dir, &cargo)
        .success()
        .stdout(predicate::str::contains("[0..3]"));
    assert_eq!(
        cargo_spawns(&count_file),
        2,
        "corrupt entry must trigger exactly one rebuild"
    );

    // Changed grammar bytes are a miss even though nothing else moved.
    let mut source = std::fs::read_to_string(&grammar).expect("read grammar");
    source.push_str("\n// cache-invalidation probe\n");
    std::fs::write(&grammar, source).expect("rewrite grammar");
    parse_with_cache(&grammar, &input_a, &cache_dir, &cargo)
        .success()
        .stdout(predicate::str::contains("[0..3]"));
    assert_eq!(
        cargo_spawns(&count_file),
        3,
        "changed grammar content must miss and rebuild once"
    );

    // The rebuilt entry is reused again with zero Cargo invocations.
    parse_with_cache(&grammar, &input_b, &cache_dir, &cargo)
        .success()
        .stdout(predicate::str::contains("[0..5]"));
    assert_eq!(cargo_spawns(&count_file), 3);
}
