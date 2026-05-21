use crate::grid::{BddGridIssue, BddPhase, BddScenario, BddScenarioStatus};

/// Validate structural integrity of a scenario grid.
///
/// This helps governance reporting fail loudly when malformed rows are introduced.
pub fn bdd_grid_issues(scenarios: &[BddScenario]) -> Vec<BddGridIssue> {
    let mut issues = Vec::new();
    let mut seen = [false; 256];

    for scenario in scenarios {
        track_duplicate_ids(scenario, &mut seen, &mut issues);
        track_empty_fields(scenario, &mut issues);
        track_empty_deferred_reasons(scenario, &mut issues);
    }

    issues
}

fn track_duplicate_ids(
    scenario: &BddScenario,
    seen: &mut [bool; 256],
    issues: &mut Vec<BddGridIssue>,
) {
    let idx = usize::from(scenario.id);
    if seen[idx] {
        issues.push(BddGridIssue::DuplicateScenarioId { id: scenario.id });
    } else {
        seen[idx] = true;
    }
}

fn track_empty_fields(scenario: &BddScenario, issues: &mut Vec<BddGridIssue>) {
    if scenario.title.trim().is_empty() {
        issues.push(BddGridIssue::EmptyTitle { id: scenario.id });
    }

    if scenario.reference.trim().is_empty() {
        issues.push(BddGridIssue::EmptyReference { id: scenario.id });
    }
}

fn track_empty_deferred_reasons(scenario: &BddScenario, issues: &mut Vec<BddGridIssue>) {
    for phase in [BddPhase::Core, BddPhase::Runtime] {
        let status = scenario.status(phase);
        if let BddScenarioStatus::Deferred { reason } = status
            && reason.trim().is_empty()
        {
            issues.push(BddGridIssue::EmptyDeferredReason {
                id: scenario.id,
                phase,
            });
        }
    }
}
