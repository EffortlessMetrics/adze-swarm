//! Profile-aware report/status rendering for BDD governance tracking.
//!
//! This module intentionally owns formatting concerns so governance matrix core
//! logic can stay focused on typed snapshots and matrix composition.

use core::fmt::Write;

pub use crate::status::{
    GLR_CONFLICT_FALLBACK, bdd_progress_status_line, describe_backend_for_conflicts,
};
use crate::{BddPhase, BddScenario, ParserFeatureProfile, bdd_progress, bdd_progress_report};

mod profile_sections;

use profile_sections::{
    conflict_backend_line, feature_profile_line, governance_progress_line,
    non_conflict_backend_line,
};

/// Compose BDD progress with parser profile diagnostics in one report.
pub fn bdd_progress_report_with_profile(
    phase: BddPhase,
    scenarios: &[BddScenario],
    phase_title: &str,
    profile: ParserFeatureProfile,
) -> String {
    let mut out = bdd_progress_report(phase, scenarios, phase_title);
    let (implemented, total) = bdd_progress(phase, scenarios);

    let _ = writeln!(&mut out);
    let _ = writeln!(&mut out, "{}", feature_profile_line(profile));
    let _ = writeln!(&mut out, "{}", non_conflict_backend_line(profile));
    let _ = writeln!(&mut out, "{}", conflict_backend_line(profile));
    let _ = writeln!(&mut out, "{}", governance_progress_line(implemented, total));

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GLR_CONFLICT_PRESERVATION_GRID, ParserBackend};

    #[test]
    fn conflict_backend_description_prefers_glr() {
        let profile = ParserFeatureProfile {
            pure_rust: true,
            tree_sitter_standard: false,
            tree_sitter_c2rust: false,
            glr: true,
        };
        assert_eq!(
            describe_backend_for_conflicts(profile),
            ParserBackend::GLR.name()
        );
    }

    #[test]
    fn report_with_profile_is_annotated() {
        let profile = ParserFeatureProfile::current();
        let report = bdd_progress_report_with_profile(
            BddPhase::Runtime,
            GLR_CONFLICT_PRESERVATION_GRID,
            "Runtime",
            profile,
        );

        assert!(report.contains("Feature profile:"));
        assert!(report.contains("Non-conflict backend:"));
        assert!(report.contains("Conflict grammars:"));
        assert!(report.contains("Governance progress:"));
    }

    #[test]
    fn status_line_stable_shape() {
        let profile = ParserFeatureProfile {
            pure_rust: false,
            tree_sitter_standard: true,
            tree_sitter_c2rust: false,
            glr: false,
        };

        let status =
            bdd_progress_status_line(BddPhase::Runtime, GLR_CONFLICT_PRESERVATION_GRID, profile);
        assert!(status.starts_with("runtime:"));
        assert!(status.contains("tree-sitter C runtime"));
        assert!(status.contains("tree-sitter-standard"));
    }
}
