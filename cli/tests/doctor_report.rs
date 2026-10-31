//! Pure reporting controls only; no installed CLI/probe execution is claimed.
#[path = "../src/doctor_report.rs"]
mod doctor_report;

use doctor_report::{CheckId, DoctorReport, Finding, Status, Summary};

fn finding(status: Status) -> Finding {
    Finding {
        status,
        message: "Observed fixture state".into(),
        next_action: (status != Status::Passed).then(|| "Repair the selected fixture".into()),
    }
}

fn complete_report() -> DoctorReport {
    let mut report = DoctorReport::default();
    for id in [
        CheckId::Project,
        CheckId::Toolchain,
        CheckId::Dependencies,
        CheckId::Features,
    ] {
        report.record(id, finding(Status::Passed)).expect("unique check");
    }
    report
}

#[test]
fn doctor_empty_report_is_incomplete_with_four_visible_checks() {
    let report = DoctorReport::default();
    assert_eq!(report.summary(), Summary::Incomplete);
    assert_eq!(report.summary().exit_code(), 2);
    let json = report.as_json();
    let checks = json["checks"].as_array().expect("checks");
    assert_eq!(checks.len(), 4);
    assert!(checks.iter().all(|check| check["status"] == "not_run"));
}

#[test]
fn doctor_three_passing_checks_do_not_hide_a_missing_fourth() {
    let mut report = DoctorReport::default();
    for id in [CheckId::Project, CheckId::Toolchain, CheckId::Dependencies] {
        report.record(id, finding(Status::Passed)).expect("unique check");
    }
    assert_eq!(report.summary(), Summary::Incomplete);
    assert_eq!(report.as_json()["checks"][0]["id"], "features");
}

#[test]
fn doctor_complete_setup_does_not_claim_parser_or_registry_proof() {
    let report = complete_report();
    assert_eq!(report.summary(), Summary::ChecksPassed);
    assert_eq!(report.summary().exit_code(), 0);
    assert!(report.render_text().contains(
        "parser correctness and registry installation are not tested"
    ));
}

#[test]
fn doctor_unknown_and_not_run_are_distinct_and_never_success() {
    let mut report = DoctorReport::default();
    report
        .record(CheckId::Toolchain, finding(Status::Unknown))
        .expect("toolchain");
    report
        .record(CheckId::Project, finding(Status::NotRun))
        .expect("project");
    assert_eq!(report.summary(), Summary::Incomplete);
    let json = report.as_json();
    assert_eq!(json["checks"][0]["status"], "unknown");
    assert_eq!(json["checks"][1]["status"], "not_run");
}

#[test]
fn doctor_known_blocker_takes_precedence_over_missing_checks() {
    let mut report = DoctorReport::default();
    report
        .record(CheckId::Dependencies, finding(Status::Blocked))
        .expect("dependencies");
    assert_eq!(report.summary(), Summary::Blocked);
    assert_eq!(report.summary().exit_code(), 1);
    assert_eq!(report.as_json()["checks"][0]["id"], "dependencies");
}

#[test]
fn doctor_internal_error_is_not_a_user_setup_failure() {
    let mut report = DoctorReport::default();
    report
        .record(CheckId::Project, finding(Status::Blocked))
        .expect("project");
    report
        .record(CheckId::Toolchain, finding(Status::Error))
        .expect("toolchain");
    assert_eq!(report.summary(), Summary::Error);
    assert_eq!(report.summary().exit_code(), 2);
    assert_eq!(report.as_json()["checks"][0]["status"], "error");
}

#[test]
fn doctor_duplicate_result_cannot_overwrite_a_blocker() {
    let mut report = DoctorReport::default();
    report
        .record(CheckId::Project, finding(Status::Blocked))
        .expect("first result");
    assert!(report.record(CheckId::Project, finding(Status::Passed)).is_err());
    assert_eq!(report.summary(), Summary::Blocked);
}

#[test]
fn doctor_invalid_finding_is_rejected_before_mutation() {
    let mut report = DoctorReport::default();
    let mut invalid = finding(Status::Passed);
    invalid.message = "  ".into();
    assert!(report.record(CheckId::Project, invalid).is_err());
    let mut invalid = finding(Status::Unknown);
    invalid.next_action = None;
    assert!(report.record(CheckId::Project, invalid).is_err());
    assert_eq!(report.as_json()["checks"][0]["status"], "not_run");
    report
        .record(CheckId::Project, finding(Status::Passed))
        .expect("rejected input left slot empty");
}

#[test]
fn doctor_text_and_json_share_order_status_and_actions() {
    let mut report = complete_report();
    assert!(report.record(CheckId::Project, finding(Status::Blocked)).is_err());
    let text = report.render_text();
    let json = report.as_json();
    let mut previous = 0;
    for check in json["checks"].as_array().expect("checks") {
        let marker = format!(
            "[{}] {}:",
            check["status"].as_str().expect("status"),
            check["id"].as_str().expect("id"),
        );
        let position = text.find(&marker).expect("same row in human output");
        assert!(position >= previous);
        previous = position;
    }
    assert_eq!(json["schema"], "adze.doctor.v1");
    assert_eq!(json["exit_code"], 0);
}

#[test]
fn doctor_human_output_cannot_inject_a_second_check_line() {
    let mut report = DoctorReport::default();
    report
        .record(
            CheckId::Project,
            Finding {
                status: Status::Blocked,
                message: "bad manifest\n[passed] toolchain\u{1b}[32m".into(),
                next_action: Some("repair\nthen retry".into()),
            },
        )
        .expect("finding");
    let text = report.render_text();
    assert!(!text.contains("\n[passed] toolchain"));
    assert!(!text.contains('\u{1b}'));
    assert!(text.contains("Next: repair then retry"));
    assert_eq!(report.summary(), Summary::Blocked);
}
