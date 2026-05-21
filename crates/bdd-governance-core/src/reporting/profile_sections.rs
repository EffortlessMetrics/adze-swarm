//! Small, single-purpose formatting helpers for profile-aware governance reports.

use crate::{ParserFeatureProfile, describe_backend_for_conflicts};

pub(super) fn feature_profile_line(profile: ParserFeatureProfile) -> String {
    format!("Feature profile: {profile}")
}

pub(super) fn non_conflict_backend_line(profile: ParserFeatureProfile) -> String {
    format!(
        "Non-conflict backend: {}",
        profile.resolve_backend(false).name()
    )
}

pub(super) fn conflict_backend_line(profile: ParserFeatureProfile) -> String {
    format!(
        "Conflict grammars: {}",
        describe_backend_for_conflicts(profile)
    )
}

pub(super) fn governance_progress_line(implemented: usize, total: usize) -> String {
    format!("Governance progress: {implemented}/{total} scenarios implemented")
}
