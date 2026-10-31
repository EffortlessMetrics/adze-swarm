//! Setup-report model for #1131. This does not execute probes or validate a parser.

use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CheckId {
    Project,
    Toolchain,
    Dependencies,
    Features,
}

impl CheckId {
    const ALL: [Self; 4] = [
        Self::Project,
        Self::Toolchain,
        Self::Dependencies,
        Self::Features,
    ];

    fn index(self) -> usize {
        match self {
            Self::Project => 0,
            Self::Toolchain => 1,
            Self::Dependencies => 2,
            Self::Features => 3,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Toolchain => "toolchain",
            Self::Dependencies => "dependencies",
            Self::Features => "features",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Passed,
    Blocked,
    Unknown,
    NotRun,
    Error,
}

impl Status {
    fn name(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Blocked => "blocked",
            Self::Unknown => "unknown",
            Self::NotRun => "not_run",
            Self::Error => "error",
        }
    }

    fn priority(self) -> u8 {
        match self {
            Self::Error => 0,
            Self::Blocked => 1,
            Self::Unknown => 2,
            Self::NotRun => 3,
            Self::Passed => 4,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Finding {
    pub(crate) status: Status,
    pub(crate) message: String,
    pub(crate) next_action: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Summary {
    ChecksPassed,
    Blocked,
    Incomplete,
    Error,
}

impl Summary {
    fn name(&self) -> &'static str {
        match self {
            Self::ChecksPassed => "checks_passed",
            Self::Blocked => "blocked",
            Self::Incomplete => "incomplete",
            Self::Error => "error",
        }
    }

    pub(crate) fn exit_code(&self) -> u8 {
        match self {
            Self::ChecksPassed => 0,
            Self::Blocked => 1,
            Self::Incomplete | Self::Error => 2,
        }
    }
}

/// Every mandatory setup check stays visible, including unexecuted checks.
#[derive(Debug, Default)]
pub(crate) struct DoctorReport {
    findings: [Option<Finding>; 4],
}

impl DoctorReport {
    /// Accept one result per check; a retry belongs in a new report.
    pub(crate) fn record(&mut self, id: CheckId, finding: Finding) -> Result<(), &'static str> {
        if finding.message.trim().is_empty() {
            return Err("a check needs an observed fact or an explicit non-execution reason");
        }
        if finding.status != Status::Passed
            && finding
                .next_action
                .as_deref()
                .is_none_or(|action| action.trim().is_empty())
        {
            return Err("an unsuccessful or unexecuted check needs an actionable next step");
        }
        let slot = &mut self.findings[id.index()];
        if slot.is_some() {
            return Err("duplicate check result; do not overwrite earlier failure evidence");
        }
        *slot = Some(finding);
        Ok(())
    }

    pub(crate) fn summary(&self) -> Summary {
        let statuses = CheckId::ALL.map(|id| self.status(id));
        if statuses.contains(&Status::Error) {
            Summary::Error
        } else if statuses.contains(&Status::Blocked) {
            Summary::Blocked
        } else if statuses.iter().all(|status| *status == Status::Passed) {
            Summary::ChecksPassed
        } else {
            Summary::Incomplete
        }
    }

    fn status(&self, id: CheckId) -> Status {
        self.findings[id.index()]
            .as_ref()
            .map_or(Status::NotRun, |finding| finding.status)
    }

    /// Render both views from the same ordered rows. No environment or paths are collected here.
    pub(crate) fn as_json(&self) -> Value {
        let mut ids = CheckId::ALL;
        ids.sort_by_key(|id| self.status(*id).priority());
        let checks: Vec<Value> = ids
            .into_iter()
            .map(|id| {
                let finding = self.findings[id.index()].as_ref();
                json!({
                    "id": id.name(),
                    "status": self.status(id).name(),
                    "message": finding.map_or("Check was not run.", |value| value.message.as_str()),
                    "next_action": finding.map_or(
                        Some("Run the corresponding bounded setup probe."),
                        |value| value.next_action.as_deref(),
                    ),
                })
            })
            .collect();
        json!({
            "schema": "adze.doctor.v1",
            "scope": "setup checks only; parser correctness and registry installation are not tested",
            "summary": self.summary().name(),
            "exit_code": self.summary().exit_code(),
            "checks": checks,
        })
    }

    pub(crate) fn render_text(&self) -> String {
        let report = self.as_json();
        let mut text = format!(
            "Adze doctor: {}\n{}\n",
            self.summary().name(),
            report["scope"].as_str().unwrap_or_default(),
        );
        if let Some(checks) = report["checks"].as_array() {
            for check in checks {
                text.push_str(&format!(
                    "[{}] {}: {}\n",
                    check["status"].as_str().unwrap_or_default(),
                    check["id"].as_str().unwrap_or_default(),
                    single_line(check["message"].as_str().unwrap_or_default()),
                ));
                if let Some(action) = check["next_action"].as_str() {
                    text.push_str(&format!("  Next: {}\n", single_line(action)));
                }
            }
        }
        text
    }
}

fn single_line(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}
