use core::fmt::Write;

use crate::grid::{BddPhase, BddScenario, bdd_grid_issues, bdd_progress};

/// Shared formatting for BDD progress summaries.
///
/// # Examples
///
/// ```
/// use adze_bdd_governance_core::*;
///
/// let report = bdd_progress_report(
///     BddPhase::Runtime,
///     GLR_CONFLICT_PRESERVATION_GRID,
///     "Runtime",
/// );
/// assert!(report.contains("Runtime"));
/// assert!(report.contains("Scenario 1"));
/// ```
pub fn bdd_progress_report(
    phase: BddPhase,
    scenarios: &[BddScenario],
    phase_title: &str,
) -> String {
    let mut out = String::new();
    let (implemented, total) = bdd_progress(phase, scenarios);

    write_header(&mut out, phase_title);
    write_scenarios(&mut out, phase, scenarios);
    write_summary(&mut out, phase_title, implemented, total, scenarios);
    write_issues(&mut out, scenarios);

    out
}

fn write_header(out: &mut String, phase_title: &str) {
    out.push_str("=== BDD Scenario Progress Summary ===\n");
    out.push_str(phase_title);
    out.push_str("\n\n");
}

fn write_scenarios(out: &mut String, phase: BddPhase, scenarios: &[BddScenario]) {
    for scenario in scenarios {
        let status = scenario.status(phase);
        let _ = write!(
            out,
            "{} Scenario {}: {} - {}",
            status.icon(),
            scenario.id,
            scenario.title,
            status.label()
        );
        let detail = status.detail();
        if !detail.is_empty() {
            out.push_str(" (");
            out.push_str(detail);
            out.push(')');
        }
        out.push('\n');
    }
}

fn write_summary(
    out: &mut String,
    phase_title: &str,
    implemented: usize,
    total: usize,
    scenarios: &[BddScenario],
) {
    out.push('\n');
    let _ = write!(
        out,
        "{}: {}/{} scenarios complete",
        phase_title, implemented, total
    );
    if let Some(reference) = scenarios.first().map(|scenario| scenario.reference) {
        let _ = write!(out, "\nReference: {reference}");
    }
    if implemented < total {
        out.push_str("\nNext: Implement remaining deferred scenarios.");
    }
}

fn write_issues(out: &mut String, scenarios: &[BddScenario]) {
    let issues = bdd_grid_issues(scenarios);
    if issues.is_empty() {
        return;
    }

    let _ = write!(
        out,
        "\n\n⚠ Grid validation found {} issue(s):",
        issues.len()
    );
    for issue in issues {
        out.push_str("\n- ");
        out.push_str(&issue.describe());
    }
}
