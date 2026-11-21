//! The edit/debug oracle shared by downstream source and installed-package journeys.
//! This module does not launch Cargo, materialize a manifest, or manage a journal.

pub(crate) const FIELD_ANCHOR: &str = "        pub timeout: Option<Timeout>,";
const TYPE_ANCHOR: &str = "    #[adze::extra]";
const BACKOFF_TYPE: &str = r#"    /// Optional delay between retries; absence and zero are distinct.
    #[derive(Debug, PartialEq)]
    pub struct Backoff {
        #[adze::leaf(text = "backoff")]
        _keyword: (),
        #[adze::leaf(text = "=")]
        _equals: (),
        #[adze::leaf(pattern = r"[0-9]+", try_transform = |text| text.parse::<u16>())]
        pub seconds: u16,
        #[adze::leaf(text = ";")]
        _end: (),
    }

"#;

const BASELINE_TEST: &str = r#"use edit_debug_probe::grammar;

#[test]
fn journey_contract() {
    let old = grammar::parse("retries=3;targets=[api,worker];timeout=30;").expect("old input");
    assert_eq!(old.retries, 3);
    assert_eq!(old.targets, vec!["api", "worker"]);
    assert_eq!(old.timeout.expect("timeout").seconds, 30);
    assert!(grammar::parse("retries=3;targets=[api];backoff=7;").is_err());
}
"#;

const EXTENSION_TEST: &str = r#"use adze::errors::ParseErrorReason;
use edit_debug_probe::grammar;

#[test]
fn journey_contract() {
    for value in [0, 7, u16::MAX] {
        let source = format!("retries=3;targets=[api,worker];backoff={value};");
        let policy = grammar::parse(&source).expect("extended input");
        assert_eq!(policy.retries, 3);
        assert_eq!(policy.targets, vec!["api", "worker"]);
        assert_eq!(policy.timeout, None);
        assert_eq!(policy.backoff.expect("explicit backoff").seconds, value);
    }
    let formerly_rejected = grammar::parse("retries=3;targets=[api];backoff=7;").expect("same input rejected by baseline");
    assert_eq!(formerly_rejected.targets, vec!["api"]);
    assert_eq!(formerly_rejected.backoff.expect("new backoff").seconds, 7);
    let old = grammar::parse("retries=3;targets=[api,worker];timeout=30;").expect("old input still works");
    assert_eq!(old.retries, 3);
    assert_eq!(old.targets, vec!["api", "worker"]);
    assert_eq!(old.timeout.expect("old timeout").seconds, 30);
    assert_eq!(old.backoff, None);
    let source = "retries=3;targets=[api];backoff=65536;";
    let errors = grammar::parse(source).expect_err("overflow is not success");
    let error = errors.first().expect("conversion diagnostic");
    assert!(matches!(error.reason, ParseErrorReason::ConversionFailed(_)));
    let start = source.find("65536").expect("token");
    assert_eq!(error.byte_span(), start..start + 5);
}
"#;

pub(crate) fn extended_grammar(base: &str) -> String {
    assert_eq!(
        base.matches(FIELD_ANCHOR).count(),
        1,
        "sample field anchor drifted"
    );
    assert!(base.contains(TYPE_ANCHOR), "sample type anchor drifted");
    base.replacen(
        FIELD_ANCHOR,
        &format!("{FIELD_ANCHOR}\n        pub backoff: Option<Backoff>,"),
        1,
    )
    .replacen(TYPE_ANCHOR, &format!("{BACKOFF_TYPE}{TYPE_ANCHOR}"), 1)
}

/// The exact, ordered source changes for the downstream test.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Phase {
    Baseline,
    Extension,
    Break,
    Repair,
}

impl Phase {
    pub(crate) const ORDERED: [Self; 4] =
        [Self::Baseline, Self::Extension, Self::Break, Self::Repair];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Baseline => "config-edit-baseline",
            Self::Extension => "config-edit-extension",
            Self::Break => "config-edit-break",
            Self::Repair => "config-edit-repair",
        }
    }

    /// Supply the prepared, selected manifest and an isolated target in the caller.
    /// Pass `locked = true` when that exact consumer lock is already prepared.
    pub(crate) fn cargo_args(locked: bool) -> Vec<&'static str> {
        let mut args = vec!["test", "--color", "never"];
        if locked {
            args.extend(["--locked", "--offline"]);
        }
        args.extend([
            "--test",
            "journey_contract",
            "--",
            "--exact",
            "journey_contract",
            "--test-threads=1",
            "--format",
            "pretty",
            "--color",
            "never",
        ]);
        args
    }

    /// Baseline resolves the fresh checkout; later phases must reuse its lock.
    pub(crate) fn source_cargo_args(self) -> Vec<&'static str> {
        Self::cargo_args(self != Self::Baseline)
    }

    pub(crate) fn inputs(self, base: &str) -> (String, &'static str) {
        match self {
            Self::Baseline => (base.to_owned(), BASELINE_TEST),
            Self::Extension | Self::Repair => (extended_grammar(base), EXTENSION_TEST),
            Self::Break => (
                extended_grammar(base).replacen(
                    "pub backoff: Option<Backoff>",
                    "pub backoff: Option<MissingBackoffType1138>",
                    1,
                ),
                EXTENSION_TEST,
            ),
        }
    }

    /// Check the natural Cargo status and complete raw channels, not a log excerpt.
    /// The caller owns capture limits, timeout and journal verification.
    pub(crate) fn audit_output(
        self,
        exit_code: Option<i32>,
        stdout: &[u8],
        stderr: &[u8],
    ) -> Result<(), String> {
        let stdout = std::str::from_utf8(stdout).map_err(|_| "non-UTF-8 stdout")?;
        let stderr = std::str::from_utf8(stderr).map_err(|_| "non-UTF-8 stderr")?;
        let failure = |why| {
            Err(format!(
                "{}: {why}; natural status {exit_code:?}",
                self.label()
            ))
        };
        if self == Self::Break {
            if exit_code != Some(101)
                || !stdout.trim().is_empty()
                || stderr.lines().any(is_libtest_row)
                || !expected_compile_failure(stderr)
            {
                return failure(
                    "expected the deliberate missing-type compilation failure, not other errors or test output",
                );
            }
            return Ok(());
        }
        if exit_code != Some(0)
            || !exact_success_output(stdout)
            || stderr
                .lines()
                .any(|line| is_libtest_row(line) || is_primary_error(line))
        {
            return failure(
                "expected one ordered, exact libtest sequence and no extra test output",
            );
        }
        Ok(())
    }
}

const GENERATOR_HEADER: &str =
    "error: failed to run custom build command for `edit-debug-probe v0.0.0 (";
const GENERATOR_CAUSE: &str =
    "    Caused by: Symbol 'MissingBackoffType1138' referenced in 'Policy' not found";
const RUSTC_MISSING: &str = "error[E0412]: cannot find type `MissingBackoffType1138` in this scope";
const RUSTC_SUMMARY: &str =
    "error: could not compile `edit-debug-probe` (lib) due to 1 previous error";
const SUCCESS_TERMINAL_PREFIX: &str =
    "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in ";

fn is_libtest_row(line: &str) -> bool {
    let line = line.trim();
    line.starts_with("test ")
        || line
            .strip_prefix("running ")
            .and_then(|tail| tail.split_once(' '))
            .is_some_and(|(count, unit)| ascii_digits(count) && matches!(unit, "test" | "tests"))
}

fn is_primary_error(line: &str) -> bool {
    line.starts_with("error:") || line.starts_with("error[")
}

fn ascii_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

fn exact_success_output(stdout: &str) -> bool {
    let mut lines = stdout.lines().filter(|line| !line.is_empty());
    let (Some("running 1 test"), Some("test journey_contract ... ok"), Some(terminal), None) =
        (lines.next(), lines.next(), lines.next(), lines.next())
    else {
        return false;
    };
    terminal
        .strip_prefix(SUCCESS_TERMINAL_PREFIX)
        .and_then(|tail| tail.strip_suffix('s'))
        .and_then(|seconds| seconds.split_once('.'))
        .is_some_and(|(whole, fraction)| {
            ascii_digits(whole) && fraction.len() == 2 && ascii_digits(fraction)
        })
}

fn expected_compile_failure(stderr: &str) -> bool {
    let mut lines = stderr.lines().skip_while(|line| !is_primary_error(line));
    let Some(first) = lines.next() else {
        return false;
    };
    if first
        .strip_prefix(GENERATOR_HEADER)
        .and_then(|tail| tail.strip_suffix(")`"))
        .is_some_and(|path| !path.is_empty())
    {
        return !lines.clone().any(is_primary_error) && generator_failure_block(lines);
    }
    if first != RUSTC_MISSING {
        return false;
    }
    // A rustc diagnostic must point at this fixture's declaration, followed
    // by Cargo's matching package summary, not an unrelated setup error.
    if !stderr
        .lines()
        .filter(|line| is_primary_error(line))
        .eq([RUSTC_MISSING, RUSTC_SUMMARY])
    {
        return false;
    }
    let mut details = lines.take_while(|line| !is_primary_error(line));
    details.find_map(fixture_source_row).is_some_and(|row| {
        details.any(|line| {
            line.split_once('|').is_some_and(|(number, source)| {
                number.trim() == row
                    && source.trim() == "pub backoff: Option<MissingBackoffType1138>,"
            })
        })
    })
}

fn fixture_source_row(line: &str) -> Option<&str> {
    let line = line.trim_start();
    let (row, column) = line
        .strip_prefix("--> src/lib.rs:")
        .or_else(|| line.strip_prefix("--> src\\lib.rs:"))
        .and_then(|tail| tail.split_once(':'))?;
    (ascii_digits(row) && ascii_digits(column)).then_some(row)
}

fn generator_failure_block<'a>(mut lines: impl Iterator<Item = &'a str>) -> bool {
    let (Some(""), Some("Caused by:"), Some(process), Some("  --- stdout")) =
        (lines.next(), lines.next(), lines.next(), lines.next())
    else {
        return false;
    };
    let build_script = process
        .strip_prefix("  process didn't exit successfully: `")
        .and_then(|tail| {
            tail.strip_suffix("` (exit status: 101)")
                .or_else(|| tail.strip_suffix("` (exit code: 101)"))
        })
        .and_then(|path| path.rsplit(['/', '\\']).next());
    if !matches!(
        build_script,
        Some("build-script-build" | "build-script-build.exe")
    ) {
        return false;
    }
    lines
        .by_ref()
        .find(|line| *line == "  --- stderr")
        .is_some()
        && lines.next()
            == Some("  Failed to build pure-Rust parser: Failed to convert grammar JSON to IR")
        && lines.next() == Some("    Caused by: grammar.js source validation failed")
        && lines.next() == Some(GENERATOR_CAUSE)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OK: &[u8] = b"running 1 test\ntest journey_contract ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";

    #[test]
    fn test_cargo_args_after_baseline_require_locked_offline() {
        let expected = [
            "test",
            "--color",
            "never",
            "--locked",
            "--offline",
            "--test",
            "journey_contract",
            "--",
            "--exact",
            "journey_contract",
            "--test-threads=1",
            "--format",
            "pretty",
            "--color",
            "never",
        ];
        assert_eq!(Phase::cargo_args(true), expected);
        assert_eq!(
            Phase::Baseline.source_cargo_args(),
            Phase::cargo_args(false)
        );
        for phase in [Phase::Extension, Phase::Break, Phase::Repair] {
            assert_eq!(phase.source_cargo_args(), expected);
        }
        assert!(!Phase::cargo_args(false).contains(&"--locked"));
    }

    #[test]
    fn test_edit_inputs_after_repair_restore_the_extended_oracle() {
        let baseline = include_str!("../../src/lib.rs");
        let (original, old_test) = Phase::Baseline.inputs(baseline);
        let (edit, edited_test) = Phase::Extension.inputs(baseline);
        let (broken, broken_test) = Phase::Break.inputs(baseline);
        let (repair, repaired_test) = Phase::Repair.inputs(baseline);
        assert_eq!(original, baseline);
        assert_ne!(old_test, edited_test);
        assert_eq!(broken_test, edited_test);
        assert_eq!(repaired_test, edited_test);
        assert_eq!(edit, repair);
        assert_eq!(
            broken.replace("MissingBackoffType1138", "Backoff"),
            edit,
            "only the new field may be deliberately broken"
        );
        assert_eq!(broken.matches("MissingBackoffType1138").count(), 1);
        assert_eq!(
            Phase::ORDERED.map(Phase::label),
            [
                "config-edit-baseline",
                "config-edit-extension",
                "config-edit-break",
                "config-edit-repair"
            ]
        );
    }

    #[test]
    fn test_success_oracle_with_missing_or_duplicate_rows_rejects_output() {
        assert!(Phase::Baseline.audit_output(Some(0), OK, b"").is_ok());
        let ok = std::str::from_utf8(OK).unwrap();
        for changed in [
            ok.replace("1 passed", "0 passed"),
            ok.replace("test journey_contract ... ok", "test unrelated ... ok"),
            format!("{ok}{ok}"),
        ] {
            assert!(
                Phase::Repair
                    .audit_output(Some(0), changed.as_bytes(), b"")
                    .is_err()
            );
        }
        assert!(Phase::Repair.audit_output(None, OK, b"").is_err());
        assert!(Phase::Repair.audit_output(Some(0), b"\xff", b"").is_err());
        let crlf = ok.replace('\n', "\r\n");
        assert!(
            Phase::Extension
                .audit_output(Some(0), crlf.as_bytes(), b"")
                .is_ok()
        );
    }

    const GENERATOR: &str = concat!(
        "error: failed to run custom build command for `edit-debug-probe v0.0.0 (/tmp/selected project)`\n",
        "\n",
        "Caused by:\n",
        "  process didn't exit successfully: `/tmp/target/build-script-build` (exit status: 101)\n",
        "  --- stdout\n",
        "  cargo::rerun-if-changed=src/lib.rs\n",
        "\n",
        "  --- stderr\n",
        "  Failed to build pure-Rust parser: Failed to convert grammar JSON to IR\n",
        "    Caused by: grammar.js source validation failed\n",
        "    Caused by: Symbol 'MissingBackoffType1138' referenced in 'Policy' not found\n",
    );
    const RUSTC: &str = concat!(
        "error[E0412]: cannot find type `MissingBackoffType1138` in this scope\n",
        "  --> src/lib.rs:33:38\n",
        "   |\n",
        "33 |         pub backoff: Option<MissingBackoffType1138>,\n",
        "   |                             ^^^^^^^^^^^^^^^^^^^^^^ not found in this scope\n",
        "For more information about this error, try `rustc --explain E0412`.\n",
        "error: could not compile `edit-debug-probe` (lib) due to 1 previous error\n",
    );

    #[test]
    fn test_break_oracle_without_native_compile_failure_rejects_output() {
        assert!(
            Phase::Break
                .audit_output(Some(101), b"", GENERATOR.as_bytes())
                .is_ok()
        );
        assert!(
            Phase::Break
                .audit_output(Some(101), b"", RUSTC.as_bytes())
                .is_ok()
        );
        let windows_shape = GENERATOR
            .replace(
                "/tmp/target/build-script-build",
                r"C:\probe space\build-script-build.exe",
            )
            .replace("exit status: 101", "exit code: 101")
            .replace('\n', "\r\n");
        assert!(
            Phase::Break
                .audit_output(Some(101), b"", windows_shape.as_bytes())
                .is_ok()
        );
        for (exit, stdout, stderr) in [
            (
                Some(101),
                &b""[..],
                &b"error: failed to download dependency"[..],
            ),
            (Some(101), &b"running 1 test\n"[..], GENERATOR.as_bytes()),
            (Some(99), &b""[..], GENERATOR.as_bytes()),
            (Some(101), OK, GENERATOR.as_bytes()),
            (Some(0), &b""[..], GENERATOR.as_bytes()),
        ] {
            assert!(Phase::Break.audit_output(exit, stdout, stderr).is_err());
        }
    }

    #[test]
    fn test_break_oracle_with_unrelated_cargo_error_rejects_excerpt() {
        let fake = "error: failed to download adze\nnote: pasted old output: Symbol 'MissingBackoffType1138' referenced in 'Policy' not found\n";
        let wrong_context = GENERATOR.replace("edit-debug-probe", "unrelated");
        let moved_cause = GENERATOR
            .replace(GENERATOR_CAUSE, "")
            .replace("  --- stderr", &format!("{GENERATOR_CAUSE}\n  --- stderr"));
        let mixed = format!("error: failed to download adze\n{GENERATOR}");
        let bad_span = RUSTC.replace("--> src/lib.rs:", "--> src/other.rs:");
        let bad_package = RUSTC.replace("edit-debug-probe", "unrelated");
        let bad_row = RUSTC.replace("src/lib.rs:33:", "src/lib.rs:30:");
        let pasted_rustc = RUSTC.replace(RUSTC_MISSING, &format!("note: copied {RUSTC_MISSING}"));
        for stderr in [
            fake,
            &wrong_context,
            &moved_cause,
            &mixed,
            &bad_span,
            &bad_package,
            &bad_row,
            &pasted_rustc,
        ] {
            assert!(
                Phase::Break
                    .audit_output(Some(101), b"", stderr.as_bytes())
                    .is_err()
            );
        }
    }

    #[test]
    fn test_break_oracle_with_libtest_stderr_rejects_output() {
        let mixed = [GENERATOR.as_bytes(), OK].concat();
        assert!(Phase::Break.audit_output(Some(101), b"", &mixed).is_err());
        let unrelated = format!("{GENERATOR}test unrelated ... ok\n");
        assert!(
            Phase::Break
                .audit_output(Some(101), b"", unrelated.as_bytes())
                .is_err()
        );
        let standalone_banner = format!("{GENERATOR}running 2 tests\n");
        assert!(
            Phase::Break
                .audit_output(Some(101), b"", standalone_banner.as_bytes())
                .is_err()
        );
    }

    #[test]
    fn test_success_oracle_with_unrelated_test_rejects_output() {
        let ok = std::str::from_utf8(OK).unwrap();
        for fake in [
            ok.replace(
                "test journey_contract ... ok",
                "test unrelated ... ok\ntest journey_contract ... ok",
            ),
            ok.replace(
                "running 1 test\ntest journey_contract ... ok",
                "test journey_contract ... ok\nrunning 1 test",
            ),
            ok.replace("finished in 0.00s", "finished in 0.00s unexpected"),
        ] {
            assert!(
                Phase::Repair
                    .audit_output(Some(0), fake.as_bytes(), b"")
                    .is_err()
            );
        }
        assert!(
            Phase::Repair
                .audit_output(Some(0), OK, b"test unrelated ... ok\n")
                .is_err()
        );
    }
}
