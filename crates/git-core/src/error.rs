use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    // existing — text unchanged, deliberately (design.md §5.2: the seven
    // existing commands' rendered UX is provably unchanged).
    #[error("{0}")]
    Git(#[from] git2::Error),
    #[error("{0}")]
    NotARepository(String),
    #[error("{0}")]
    Invalid(String),

    // new — commit and staging refusals (design.md §5.1). Each carries a
    // distinct `code()`; the UI branches on the code, never on this message.
    #[error("git executable not found (looked for {looked_for})")]
    GitUnavailable { looked_for: String },
    #[error("git exited with status {exit_code:?}")]
    CommitFailed {
        exit_code: Option<i32>,
        stderr: String,
        stdout: String,
    },
    #[error("git did not finish within {seconds} seconds and was stopped")]
    CommitTimedOut { seconds: u64, stderr: String },
    #[error("no author identity configured — set user.name and user.email")]
    IdentityMissing,
    #[error("this repository has unresolved conflicts")]
    ConflictsPresent { paths: Vec<String> },
    #[error("the index is locked by another git process ({lock_path})")]
    IndexLocked { lock_path: String },
    #[error("this is a bare repository and has no working directory")]
    BareRepository,
    #[error("nothing is staged")]
    NothingStaged,
    #[error("{path} is outside the repository")]
    PathOutsideRepo { path: String },
}

impl CoreError {
    /// Stable, `camelCase` wire code. The UI MUST branch on this, never on
    /// `Display`/`message` text (spec.md, Requirement "Commit and Staging
    /// Refusals Use Distinct, Machine-Readable Codes").
    pub fn code(&self) -> &'static str {
        match self {
            CoreError::Git(_) => "git",
            CoreError::NotARepository(_) => "notARepository",
            CoreError::Invalid(_) => "invalid",
            CoreError::GitUnavailable { .. } => "gitUnavailable",
            CoreError::CommitFailed { .. } => "commitFailed",
            CoreError::CommitTimedOut { .. } => "commitTimedOut",
            CoreError::IdentityMissing => "identityMissing",
            CoreError::ConflictsPresent { .. } => "conflictsPresent",
            CoreError::IndexLocked { .. } => "indexLocked",
            CoreError::BareRepository => "bareRepository",
            CoreError::NothingStaged => "nothingStaged",
            CoreError::PathOutsideRepo { .. } => "pathOutsideRepo",
        }
    }
}

/// Wire shape: `{ code, message, details? }` (design.md §5.2). `message` is
/// always `self.to_string()`, so the existing three variants serialise to a
/// byte-identical string as before this change; `details` is emitted only for
/// variants that carry structure.
impl Serialize for CoreError {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let has_details = matches!(
            self,
            CoreError::CommitFailed { .. }
                | CoreError::CommitTimedOut { .. }
                | CoreError::ConflictsPresent { .. }
                | CoreError::IndexLocked { .. }
                | CoreError::GitUnavailable { .. }
                | CoreError::PathOutsideRepo { .. }
        );
        let len = if has_details { 3 } else { 2 };
        let mut map = serializer.serialize_map(Some(len))?;
        map.serialize_entry("code", self.code())?;
        map.serialize_entry("message", &self.to_string())?;
        match self {
            CoreError::GitUnavailable { looked_for } => {
                map.serialize_entry("details", &GitUnavailableDetails { looked_for })?;
            }
            CoreError::CommitFailed {
                exit_code,
                stderr,
                stdout,
            } => {
                map.serialize_entry(
                    "details",
                    &CommitFailedDetails {
                        exit_code: *exit_code,
                        stderr,
                        stdout,
                    },
                )?;
            }
            CoreError::CommitTimedOut { seconds, stderr } => {
                map.serialize_entry(
                    "details",
                    &CommitTimedOutDetails {
                        seconds: *seconds,
                        stderr,
                    },
                )?;
            }
            CoreError::ConflictsPresent { paths } => {
                map.serialize_entry("details", &ConflictsPresentDetails { paths })?;
            }
            CoreError::IndexLocked { lock_path } => {
                map.serialize_entry("details", &IndexLockedDetails { lock_path })?;
            }
            CoreError::PathOutsideRepo { path } => {
                map.serialize_entry("details", &PathOutsideRepoDetails { path })?;
            }
            _ => {}
        }
        map.end()
    }
}

// Small, private, `camelCase` detail shapes — one per variant that carries
// structure. Hand-written rather than `serde_json::Value` so no `serde_json`
// dependency is added to `git-core` (design.md §5.2).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GitUnavailableDetails<'a> {
    looked_for: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommitFailedDetails<'a> {
    exit_code: Option<i32>,
    stderr: &'a str,
    stdout: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommitTimedOutDetails<'a> {
    seconds: u64,
    stderr: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConflictsPresentDetails<'a> {
    paths: &'a [String],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexLockedDetails<'a> {
    lock_path: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PathOutsideRepoDetails<'a> {
    path: &'a str,
}

pub type Result<T> = std::result::Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// "Never collapsed into one generic failure" (spec.md), mechanically
    /// checked: every variant's `code()` is distinct.
    #[test]
    fn code_is_distinct_per_variant() {
        let samples = [
            CoreError::NotARepository("x".into()),
            CoreError::Invalid("x".into()),
            CoreError::GitUnavailable {
                looked_for: "git".into(),
            },
            CoreError::CommitFailed {
                exit_code: Some(1),
                stderr: String::new(),
                stdout: String::new(),
            },
            CoreError::CommitTimedOut {
                seconds: 1,
                stderr: String::new(),
            },
            CoreError::IdentityMissing,
            CoreError::ConflictsPresent { paths: vec![] },
            CoreError::IndexLocked {
                lock_path: "x".into(),
            },
            CoreError::BareRepository,
            CoreError::NothingStaged,
            CoreError::PathOutsideRepo { path: "x".into() },
        ];
        let codes: HashSet<&'static str> = samples.iter().map(CoreError::code).collect();
        assert_eq!(codes.len(), samples.len(), "duplicate `code()` value found");
    }
}
