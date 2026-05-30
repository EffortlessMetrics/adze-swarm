//! Active goal manifest checker.
//!
//! Validates `.adze/goals/active.toml`:
//! - parses correctly
//! - top-level id/title/status/owner exist
//! - work_item IDs are unique
//! - ready items have commands
//! - blocked items have blocked_by or issue trackers
//! - complete items have PR references
//! - referenced paths exist

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeSet;

use super::{SimpleCheckMode, workspace_root};

const GOAL_PATH: &str = ".adze/goals/active.toml";

const VALID_STATUSES: &[&str] = &["active", "complete", "paused", "superseded"];

const VALID_WORK_ITEM_STATUSES: &[&str] = &["ready", "active", "blocked", "complete", "superseded"];

#[derive(Debug, Default, Deserialize)]
struct GoalFile {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    owner: String,
    #[serde(default)]
    created: String,
    #[serde(default)]
    objective: String,
    #[serde(default)]
    end_state: Vec<String>,
    #[serde(default, rename = "work_item")]
    work_items: Vec<WorkItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct WorkItem {
    #[serde(default)]
    id: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    proposal: String,
    #[serde(default)]
    spec: String,
    #[serde(default)]
    adr: String,
    #[serde(default)]
    plan: String,
    #[serde(default)]
    blocked_by: Vec<String>,
    #[serde(default)]
    issues: Vec<u64>,
    #[serde(default)]
    prs: Vec<u64>,
    #[serde(default)]
    commands: Vec<String>,
}

fn has_blocking_reason(item: &WorkItem) -> bool {
    !item.blocked_by.is_empty() || !item.issues.is_empty()
}

pub fn run(mode: &str) -> Result<()> {
    let mode = SimpleCheckMode::parse(mode)?;
    let root = workspace_root()?;
    let goal_path = root.join(GOAL_PATH);

    let raw = std::fs::read_to_string(&goal_path)
        .with_context(|| format!("reading {}", goal_path.display()))?;

    let file: GoalFile = toml::from_str(&raw).with_context(|| format!("parsing {}", GOAL_PATH))?;

    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // Top-level required fields
    if file.id.is_empty() {
        errors.push("missing top-level id".into());
    }
    if file.title.is_empty() {
        errors.push("missing top-level title".into());
    }
    if file.status.is_empty() {
        errors.push("missing top-level status".into());
    }
    if file.owner.is_empty() {
        errors.push("missing top-level owner".into());
    }
    if file.created.is_empty() {
        errors.push("missing top-level created".into());
    }
    if file.objective.trim().is_empty() {
        errors.push("missing top-level objective".into());
    }
    if file.end_state.is_empty() {
        warnings.push("empty end_state".into());
    }

    // Status validation
    if !file.status.is_empty() && !VALID_STATUSES.contains(&file.status.as_str()) {
        errors.push(format!("invalid campaign status '{}'", file.status));
    }

    // Work item ID uniqueness
    let mut seen_ids: BTreeSet<String> = BTreeSet::new();
    let mut all_item_ids: BTreeSet<&str> = BTreeSet::new();
    for item in &file.work_items {
        if !seen_ids.insert(item.id.clone()) {
            errors.push(format!("duplicate work_item id: {}", item.id));
        }
        all_item_ids.insert(&item.id);
    }

    // Per work-item checks
    for item in &file.work_items {
        // Required fields
        if item.id.is_empty() {
            errors.push("work_item with empty id".into());
            continue;
        }
        if item.status.is_empty() {
            errors.push(format!("work_item {} missing status", item.id));
        }

        // Status validation
        if !item.status.is_empty() && !VALID_WORK_ITEM_STATUSES.contains(&item.status.as_str()) {
            errors.push(format!(
                "work_item {} has invalid status '{}'",
                item.id, item.status
            ));
        }

        // Ready/active items should have commands
        if (item.status == "ready" || item.status == "active") && item.commands.is_empty() {
            warnings.push(format!(
                "work_item {} is '{}' but has no commands",
                item.id, item.status
            ));
        }

        // Blocked items must name either an internal dependency or an external issue.
        if item.status == "blocked" && !has_blocking_reason(item) {
            warnings.push(format!(
                "work_item {} is 'blocked' but has no blocked_by or issues",
                item.id
            ));
        }

        // blocked_by references must be valid work_item IDs
        for blocker in &item.blocked_by {
            if !all_item_ids.contains(blocker.as_str()) {
                errors.push(format!(
                    "work_item {} blocked_by '{}' is not a known work_item id",
                    item.id, blocker
                ));
            }
        }

        // Complete items should have PRs or proof
        if item.status == "complete" && item.prs.is_empty() && item.commands.is_empty() {
            warnings.push(format!(
                "work_item {} is 'complete' but has no prs or commands",
                item.id
            ));
        }

        // Referenced paths should exist
        for ref_path in [&item.proposal, &item.spec, &item.adr, &item.plan]
            .into_iter()
            .filter(|p| !p.is_empty())
        {
            // Strip known prefix patterns to get the actual file path
            let path_to_check = if ref_path.starts_with("docs/")
                || ref_path.starts_with("plans/")
                || ref_path.starts_with("policy/")
            {
                root.join(ref_path)
            } else if ref_path.starts_with("ADZE-") {
                // Short artifact ID reference, not a file path — skip path check
                continue;
            } else {
                root.join(ref_path)
            };

            if !path_to_check.exists() {
                warnings.push(format!(
                    "work_item {} references path '{}' which does not exist",
                    item.id, ref_path
                ));
            }
        }
    }

    // Report
    println!(
        "active-goal: {} work items in campaign '{}'",
        file.work_items.len(),
        file.title
    );

    let complete = file
        .work_items
        .iter()
        .filter(|i| i.status == "complete")
        .count();
    let ready = file
        .work_items
        .iter()
        .filter(|i| i.status == "ready")
        .count();
    let active = file
        .work_items
        .iter()
        .filter(|i| i.status == "active")
        .count();
    let blocked = file
        .work_items
        .iter()
        .filter(|i| i.status == "blocked")
        .count();
    println!(
        "  complete: {}, active: {}, ready: {}, blocked: {}",
        complete, active, ready, blocked
    );

    for w in &warnings {
        eprintln!("  warning: {w}");
    }
    for e in &errors {
        eprintln!("  error: {e}");
    }

    if errors.is_empty() {
        println!(
            "active-goal: all checks passed ({} warnings)",
            warnings.len()
        );
        Ok(())
    } else if mode == SimpleCheckMode::Advisory {
        eprintln!(
            "active-goal: advisory mode reported {} errors in {}",
            errors.len(),
            GOAL_PATH
        );
        Ok(())
    } else {
        anyhow::bail!(
            "active-goal: {} errors found in {}",
            errors.len(),
            GOAL_PATH
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{WorkItem, has_blocking_reason};

    fn blocked_item(blocked_by: Vec<&str>, issues: Vec<u64>) -> WorkItem {
        WorkItem {
            id: "blocked".to_string(),
            status: "blocked".to_string(),
            proposal: String::new(),
            spec: String::new(),
            adr: String::new(),
            plan: String::new(),
            blocked_by: blocked_by
                .into_iter()
                .map(|value| value.to_string())
                .collect(),
            issues,
            prs: Vec::new(),
            commands: Vec::new(),
        }
    }

    #[test]
    fn blocked_item_with_internal_dependency_has_blocking_reason() {
        let item = blocked_item(vec!["source-of-truth"], Vec::new());
        assert!(has_blocking_reason(&item));
    }

    #[test]
    fn blocked_item_with_issue_tracker_has_blocking_reason() {
        let item = blocked_item(Vec::new(), vec![325]);
        assert!(has_blocking_reason(&item));
    }

    #[test]
    fn blocked_item_without_dependency_or_issue_lacks_blocking_reason() {
        let item = blocked_item(Vec::new(), Vec::new());
        assert!(!has_blocking_reason(&item));
    }
}
