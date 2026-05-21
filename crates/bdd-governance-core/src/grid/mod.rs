//! Core BDD grid contracts used for feature/progress reporting.
//!
//! This crate intentionally owns only scenario-grid concerns (what is tracked and how it
//! is summarized) so governance and parser crates can compose behavior without inheriting
//! unrelated policy details.

#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]
#![cfg_attr(feature = "strict_api", deny(unreachable_pub))]
#![cfg_attr(not(feature = "strict_api"), warn(unreachable_pub))]
#![cfg_attr(feature = "strict_docs", deny(missing_docs))]
#![cfg_attr(not(feature = "strict_docs"), allow(missing_docs))]

mod report;
/// Owner module for BDD scenario status and ledger-row contracts.
pub mod scenario;
mod validation;

pub use report::bdd_progress_report;
pub use scenario::{BddPhase, BddScenario, BddScenarioStatus};
pub use validation::bdd_grid_issues;

/// Validation issue discovered while checking a BDD scenario grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BddGridIssue {
    /// Scenario id appears more than once.
    DuplicateScenarioId {
        /// The duplicate scenario id.
        id: u8,
    },
    /// Scenario title is empty.
    EmptyTitle {
        /// Scenario id carrying the invalid title.
        id: u8,
    },
    /// Scenario reference is empty.
    EmptyReference {
        /// Scenario id carrying the invalid reference.
        id: u8,
    },
    /// A deferred status has no reason text.
    EmptyDeferredReason {
        /// Scenario id with empty deferred reason.
        id: u8,
        /// Phase where the empty deferred reason was found.
        phase: BddPhase,
    },
}

impl BddGridIssue {
    /// Render this issue as a short human-readable line.
    pub fn describe(self) -> String {
        match self {
            Self::DuplicateScenarioId { id } => {
                format!("duplicate scenario id `{id}`")
            }
            Self::EmptyTitle { id } => {
                format!("scenario `{id}` has an empty title")
            }
            Self::EmptyReference { id } => {
                format!("scenario `{id}` has an empty reference")
            }
            Self::EmptyDeferredReason { id, phase } => {
                format!("scenario `{id}` has empty deferred reason in {phase} phase")
            }
        }
    }
}

/// GLR conflict-preservation scenario ledger.
pub const GLR_CONFLICT_PRESERVATION_GRID: &[BddScenario] = &[
    BddScenario {
        id: 1,
        title: "Detect shift/reduce conflicts in ambiguous grammars",
        reference: "docs/archive/plans/BDD_GLR_CONFLICT_PRESERVATION.md",
        core_status: BddScenarioStatus::Implemented,
        runtime_status: BddScenarioStatus::Implemented,
    },
    BddScenario {
        id: 2,
        title: "Preserve conflicts with precedence ordering (PreferShift)",
        reference: "docs/archive/plans/BDD_GLR_CONFLICT_PRESERVATION.md",
        core_status: BddScenarioStatus::Implemented,
        runtime_status: BddScenarioStatus::Implemented,
    },
    BddScenario {
        id: 3,
        title: "Preserve conflicts with precedence ordering (PreferReduce)",
        reference: "docs/archive/plans/BDD_GLR_CONFLICT_PRESERVATION.md",
        core_status: BddScenarioStatus::Implemented,
        runtime_status: BddScenarioStatus::Implemented,
    },
    BddScenario {
        id: 4,
        title: "Use Fork for No Precedence Information",
        reference: "docs/archive/plans/BDD_GLR_CONFLICT_PRESERVATION.md",
        core_status: BddScenarioStatus::Implemented,
        runtime_status: BddScenarioStatus::Implemented,
    },
    BddScenario {
        id: 5,
        title: "Use Fork for Non-Associative Conflicts",
        reference: "docs/archive/plans/BDD_GLR_CONFLICT_PRESERVATION.md",
        core_status: BddScenarioStatus::Implemented,
        runtime_status: BddScenarioStatus::Implemented,
    },
    BddScenario {
        id: 6,
        title: "Generate multi-action cells in parse tables",
        reference: "docs/archive/plans/BDD_GLR_CONFLICT_PRESERVATION.md",
        core_status: BddScenarioStatus::Implemented,
        runtime_status: BddScenarioStatus::Implemented,
    },
    BddScenario {
        id: 7,
        title: "GLR runtime explores both paths",
        reference: "docs/archive/plans/BDD_GLR_CONFLICT_PRESERVATION.md",
        core_status: BddScenarioStatus::Deferred {
            reason: "runtime2 integration pending",
        },
        runtime_status: BddScenarioStatus::Implemented,
    },
    BddScenario {
        id: 8,
        title: "Precedence ordering affects tree selection",
        reference: "docs/archive/plans/BDD_GLR_CONFLICT_PRESERVATION.md",
        core_status: BddScenarioStatus::Deferred {
            reason: "runtime2 integration pending",
        },
        runtime_status: BddScenarioStatus::Implemented,
    },
];

/// Aggregate progress for a phase.
///
/// # Examples
///
/// ```
/// use adze_bdd_governance_core::*;
///
/// let scenarios = [BddScenario {
///     id: 1,
///     title: "example",
///     reference: "REF-1",
///     core_status: BddScenarioStatus::Implemented,
///     runtime_status: BddScenarioStatus::Deferred { reason: "todo" },
/// }];
/// let (done, total) = bdd_progress(BddPhase::Core, &scenarios);
/// assert_eq!(done, 1);
/// assert_eq!(total, 1);
/// ```
pub fn bdd_progress(phase: BddPhase, scenarios: &[BddScenario]) -> (usize, usize) {
    let mut implemented = 0usize;
    for scenario in scenarios {
        if scenario.status(phase).implemented() {
            implemented += 1;
        }
    }
    (implemented, scenarios.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_has_expected_item_count() {
        assert_eq!(GLR_CONFLICT_PRESERVATION_GRID.len(), 8);
    }

    #[test]
    fn progress_summary_reports_counts() {
        let (implemented, total) = bdd_progress(BddPhase::Core, GLR_CONFLICT_PRESERVATION_GRID);
        assert_eq!(implemented, 6);
        assert_eq!(total, 8);
    }

    #[test]
    fn progress_report_text_includes_title() {
        let report =
            bdd_progress_report(BddPhase::Runtime, GLR_CONFLICT_PRESERVATION_GRID, "Runtime");
        assert!(report.contains("Runtime"));
        assert!(report.contains("Scenario 1"));
    }

    #[test]
    fn progress_report_for_valid_grid_has_no_validation_warning() {
        let report = bdd_progress_report(BddPhase::Core, GLR_CONFLICT_PRESERVATION_GRID, "Core");
        assert!(!report.contains("Grid validation found"));
    }

    #[test]
    fn progress_report_uses_generic_heading_and_reference() {
        let report = bdd_progress_report(BddPhase::Core, GLR_CONFLICT_PRESERVATION_GRID, "Core");
        assert!(report.starts_with("=== BDD Scenario Progress Summary ==="));
        assert!(report.contains("Reference: docs/archive/plans/BDD_GLR_CONFLICT_PRESERVATION.md"));
    }

    #[test]
    fn progress_report_omits_reference_for_empty_grid() {
        let report = bdd_progress_report(BddPhase::Core, &[], "Core");
        assert!(!report.contains("Reference:"));
    }

    #[test]
    fn grid_issues_detect_duplicate_id_and_empty_fields() {
        let malformed = [
            BddScenario {
                id: 1,
                title: "",
                reference: "",
                core_status: BddScenarioStatus::Deferred { reason: "" },
                runtime_status: BddScenarioStatus::Implemented,
            },
            BddScenario {
                id: 1,
                title: "ok",
                reference: "docs/ref.md",
                core_status: BddScenarioStatus::Implemented,
                runtime_status: BddScenarioStatus::Deferred { reason: "" },
            },
        ];

        let issues = bdd_grid_issues(&malformed);
        assert!(issues.contains(&BddGridIssue::DuplicateScenarioId { id: 1 }));
        assert!(issues.contains(&BddGridIssue::EmptyTitle { id: 1 }));
        assert!(issues.contains(&BddGridIssue::EmptyReference { id: 1 }));
        assert!(issues.contains(&BddGridIssue::EmptyDeferredReason {
            id: 1,
            phase: BddPhase::Core,
        }));
        assert!(issues.contains(&BddGridIssue::EmptyDeferredReason {
            id: 1,
            phase: BddPhase::Runtime,
        }));
    }
}
