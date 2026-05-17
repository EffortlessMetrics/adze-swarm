//! Compatibility shim for the standalone runtime governance API.
//! Kept for backward-compatible public paths (`adze::parser_selection::*`).

pub use adze_bdd_governance_core::*;

/// Select the parser backend for the current compile-time feature profile.
pub const fn current_backend_for(has_conflicts: bool) -> ParserBackend {
    ParserBackend::select(has_conflicts)
}

/// Return a BDD progress report for the active runtime profile.
pub fn bdd_progress_report_for_current_profile(phase: BddPhase, phase_title: &str) -> String {
    bdd_progress_report_with_profile_runtime(
        phase,
        GLR_CONFLICT_PRESERVATION_GRID,
        phase_title,
        parser_feature_profile_for_runtime(),
    )
}

/// Build the active runtime governance matrix for a phase.
pub fn bdd_governance_matrix_for_current_profile(phase: BddPhase) -> BddGovernanceMatrix {
    bdd_governance_matrix_for_profile(phase, parser_feature_profile_for_runtime())
}

/// Build a governance matrix for a runtime2-compatible profile.
pub fn bdd_governance_matrix_for_runtime2_profile(
    phase: BddPhase,
    pure_rust_glr: bool,
) -> BddGovernanceMatrix {
    bdd_governance_matrix_for_runtime2(phase, pure_rust_glr)
}

/// Return a BDD status line for the active runtime profile.
pub fn bdd_status_line_for_current_profile(phase: BddPhase) -> String {
    bdd_progress_status_line_for_profile(phase, parser_feature_profile_for_runtime())
}

/// Build a governance snapshot for the active runtime profile.
pub fn runtime_governance_snapshot(phase: BddPhase) -> BddGovernanceSnapshot {
    bdd_governance_snapshot(
        phase,
        GLR_CONFLICT_PRESERVATION_GRID,
        parser_feature_profile_for_runtime(),
    )
}

/// Build a BDD report for an explicit runtime2 profile.
pub fn bdd_progress_report_for_runtime2_profile(
    phase: BddPhase,
    phase_title: &str,
    profile: ParserFeatureProfile,
) -> String {
    bdd_progress_report_with_profile_runtime(
        phase,
        GLR_CONFLICT_PRESERVATION_GRID,
        phase_title,
        profile,
    )
}

/// Build a BDD status line for an explicit runtime2 profile.
pub fn bdd_progress_status_line_for_runtime2_profile(
    phase: BddPhase,
    profile: ParserFeatureProfile,
) -> String {
    bdd_progress_status_line_for_profile(phase, profile)
}

/// Resolve runtime2 backend resolution from an explicit profile.
pub const fn resolve_backend_for_runtime2_profile(
    profile: ParserFeatureProfile,
    has_conflicts: bool,
) -> ParserBackend {
    resolve_backend_for_profile(profile, has_conflicts)
}

/// Resolve runtime2 backend resolution directly from the `pure-rust-glr` toggle.
pub const fn resolve_runtime2_backend(pure_rust_glr: bool, has_conflicts: bool) -> ParserBackend {
    resolve_backend_for_profile(
        parser_feature_profile_for_runtime2(pure_rust_glr),
        has_conflicts,
    )
}

/// Build a runtime2 governance snapshot for an explicit profile.
pub fn runtime2_governance_snapshot(
    phase: BddPhase,
    profile: ParserFeatureProfile,
) -> BddGovernanceSnapshot {
    bdd_governance_snapshot(phase, GLR_CONFLICT_PRESERVATION_GRID, profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn any_phase() -> BddPhase {
        BddPhase::Runtime
    }

    #[test]
    fn current_backend_for_no_conflicts_matches_parser_backend_select() {
        let direct = ParserBackend::select(false);
        let shim = current_backend_for(false);
        assert_eq!(direct, shim);
    }

    #[test]
    fn bdd_progress_report_includes_phase_title() {
        let title = "coverage-shim-test";
        let report = bdd_progress_report_for_current_profile(any_phase(), title);
        assert!(
            report.contains(title),
            "expected report to include phase title; got: {report}"
        );
    }

    #[test]
    fn bdd_governance_matrix_for_current_profile_returns_matrix() {
        let phase = any_phase();
        let matrix = bdd_governance_matrix_for_current_profile(phase);
        // Reproduce via the underlying helper and compare for shim equivalence.
        let expected =
            bdd_governance_matrix_for_profile(phase, parser_feature_profile_for_runtime());
        assert_eq!(matrix, expected);
    }

    #[test]
    fn bdd_governance_matrix_for_runtime2_profile_matches_direct_call() {
        let phase = any_phase();
        let shim_on = bdd_governance_matrix_for_runtime2_profile(phase, true);
        let direct_on = bdd_governance_matrix_for_runtime2(phase, true);
        assert_eq!(shim_on, direct_on);

        let shim_off = bdd_governance_matrix_for_runtime2_profile(phase, false);
        let direct_off = bdd_governance_matrix_for_runtime2(phase, false);
        assert_eq!(shim_off, direct_off);
    }

    #[test]
    fn bdd_status_line_for_current_profile_matches_direct_call() {
        let phase = any_phase();
        let shim = bdd_status_line_for_current_profile(phase);
        let direct =
            bdd_progress_status_line_for_profile(phase, parser_feature_profile_for_runtime());
        assert_eq!(shim, direct);
    }

    #[test]
    fn runtime_governance_snapshot_matches_direct_call() {
        let phase = any_phase();
        let shim = runtime_governance_snapshot(phase);
        let direct = bdd_governance_snapshot(
            phase,
            GLR_CONFLICT_PRESERVATION_GRID,
            parser_feature_profile_for_runtime(),
        );
        assert_eq!(shim, direct);
    }

    #[test]
    fn bdd_progress_report_for_runtime2_profile_includes_title() {
        let title = "runtime2-coverage-shim";
        let report = bdd_progress_report_for_runtime2_profile(
            any_phase(),
            title,
            parser_feature_profile_for_runtime2(true),
        );
        assert!(report.contains(title));
    }

    #[test]
    fn bdd_progress_status_line_for_runtime2_profile_matches_direct_call() {
        let phase = any_phase();
        let profile = parser_feature_profile_for_runtime2(true);
        let shim = bdd_progress_status_line_for_runtime2_profile(phase, profile);
        let direct = bdd_progress_status_line_for_profile(phase, profile);
        assert_eq!(shim, direct);
    }

    #[test]
    fn resolve_backend_for_runtime2_profile_matches_direct_call() {
        let profile = parser_feature_profile_for_runtime2(true);
        let shim_no_conflict = resolve_backend_for_runtime2_profile(profile, false);
        let direct_no_conflict = resolve_backend_for_profile(profile, false);
        assert_eq!(shim_no_conflict, direct_no_conflict);
    }

    #[test]
    fn resolve_runtime2_backend_matches_runtime2_profile_resolution() {
        // pure_rust_glr toggled in both directions to cover the const fn path.
        let direct_on =
            resolve_backend_for_profile(parser_feature_profile_for_runtime2(true), false);
        assert_eq!(resolve_runtime2_backend(true, false), direct_on);

        let direct_off =
            resolve_backend_for_profile(parser_feature_profile_for_runtime2(false), false);
        assert_eq!(resolve_runtime2_backend(false, false), direct_off);
    }

    #[test]
    fn runtime2_governance_snapshot_matches_direct_call() {
        let phase = any_phase();
        let profile = parser_feature_profile_for_runtime2(false);
        let shim = runtime2_governance_snapshot(phase, profile);
        let direct = bdd_governance_snapshot(phase, GLR_CONFLICT_PRESERVATION_GRID, profile);
        assert_eq!(shim, direct);
    }
}
