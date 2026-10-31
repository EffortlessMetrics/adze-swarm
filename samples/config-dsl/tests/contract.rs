use adze::errors::ParseErrorReason;
use adze_config_dsl::grammar;
use std::path::Path;
use std::process::{Command, Output};

#[test]
fn policy_has_exact_application_values() {
    let policy = grammar::parse(include_str!("../fixtures/valid.policy")).expect("valid policy");
    assert_eq!(policy.retries, 3);
    assert_eq!(policy.targets, vec!["api", "worker"]);
    assert_eq!(policy.timeout.expect("timeout").seconds, 30);
}

#[test]
fn absence_and_numeric_boundaries_keep_their_meaning() {
    for retries in [0, u16::MAX] {
        let source = format!("retries={retries};targets=[api];");
        let policy = grammar::parse(&source).expect("boundary is valid");
        assert_eq!(policy.retries, retries);
        assert_eq!(policy.targets, vec!["api"]);
        assert_eq!(policy.timeout, None);
    }
    let policy = grammar::parse("retries=0;targets=[api];timeout=0;").expect("explicit zero");
    assert_eq!(policy.timeout.expect("present zero").seconds, 0);
}

#[test]
fn comments_and_line_endings_do_not_change_values() {
    let lf = "# café λ\nretries=3; # retries\ntargets=[api,worker];\ntimeout=30;\n";
    let expected = grammar::parse(include_str!("../fixtures/valid.policy")).expect("reference");
    for source in [lf.to_owned(), lf.replace('\n', "\r\n")] {
        assert_eq!(grammar::parse(&source).expect("comment/line-ending variant"), expected);
    }
}

#[test]
fn overflow_is_structured_and_points_at_the_token() {
    for token in ["65536".to_owned(), "9".repeat(80)] {
        let source = format!("retries={token};targets=[api];");
        let result = std::panic::catch_unwind(|| grammar::parse(&source));
        let errors = result.expect("overflow must not panic").expect_err("overflow must fail");
        let first = errors.first().expect("conversion diagnostic");
        assert!(matches!(first.reason, ParseErrorReason::ConversionFailed(_)));
        assert_eq!(first.byte_span(), 8..8 + token.len());
    }
}

#[test]
fn optional_numeric_field_is_fallible_too() {
    let source = "retries=3;targets=[api];timeout=65536;";
    let start = source.find("65536").expect("overflow token");
    let errors = grammar::parse(source).expect_err("optional overflow must fail");
    let first = errors.first().expect("conversion diagnostic");
    assert!(matches!(first.reason, ParseErrorReason::ConversionFailed(_)));
    assert_eq!(first.byte_span(), start..start + 5);
}

#[test]
fn malformed_inputs_never_become_successful_default_policies() {
    for source in [
        "", "retries=3;targets=[];", "retries=3;targets=[api,];",
        "retries=3;targets=[api]", "retry=3;targets=[api];",
        "retries=-1;targets=[api];", "retries=3;targets=[api];timeout=;",
    ] {
        let errors = grammar::parse(source).expect_err("malformed policy must fail");
        assert!(!errors.is_empty());
        for error in errors {
            let span = error.byte_span();
            assert!(span.start <= span.end && span.end <= source.len(), "{error:?}");
        }
    }
}

fn run_fixture(name: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_adze-config-dsl"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name))
        .output()
        .expect("run sample binary")
}

#[test]
fn cli_success_is_exact() {
    let output = run_fixture("valid.policy");
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(String::from_utf8(output.stdout).expect("UTF-8 output"),
        "retries=3\ntargets=api,worker\ntimeout_seconds=30\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn cli_parse_failure_has_no_success_output() {
    let output = run_fixture("overflow.policy");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn cli_usage_and_io_failures_are_distinct_from_parse_failure() {
    let missing_argument = Command::new(env!("CARGO_BIN_EXE_adze-config-dsl"))
        .output().expect("run sample binary");
    assert_eq!(missing_argument.status.code(), Some(2));
    assert!(missing_argument.stdout.is_empty());
    assert!(String::from_utf8(missing_argument.stderr).expect("UTF-8 usage").contains("usage:"));
    let missing_file = run_fixture("does-not-exist.policy");
    assert_eq!(missing_file.status.code(), Some(2));
    assert!(missing_file.stdout.is_empty());
    assert!(!missing_file.stderr.is_empty());
}
