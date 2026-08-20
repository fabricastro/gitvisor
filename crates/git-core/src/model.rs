//! Serializable value objects crossing the boundary towards the UI.
//!
//! Nothing here knows about Tauri or any transport. These are plain data
//! structures so the domain stays testable on its own.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Signature {
    pub name: String,
    pub email: String,
    /// Seconds since the Unix epoch.
    pub timestamp: i64,
    /// Timezone offset in minutes, as recorded by git.
    pub offset_minutes: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RefKind {
    LocalBranch,
    RemoteBranch,
    Tag,
}

/// A ref label drawn on top of a commit node in the graph.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefBadge {
    pub name: String,
    pub kind: RefKind,
    /// True when HEAD currently points at this ref.
    pub is_head: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub id: String,
    pub short_id: String,
    pub summary: String,
    pub body: String,
    pub author: Signature,
    pub committer: Signature,
    pub parents: Vec<String>,
    pub refs: Vec<RefBadge>,
}

impl Commit {
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }
}

/// A connection drawn from a commit down to one of its parents.
///
/// `to_lane` is the lane the parent will actually be rendered in, so the
/// renderer never has to guess where a line lands.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub from_lane: u32,
    pub to_lane: u32,
    /// Row index of the parent, or `None` when the parent lies outside the
    /// walked window (truncated history or a shallow clone).
    pub to_row: Option<u32>,
    pub color: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRow {
    pub commit: Commit,
    /// Horizontal track this commit is drawn on.
    pub lane: u32,
    /// Stable colour index for the line this commit belongs to.
    pub color: u32,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Graph {
    pub rows: Vec<GraphRow>,
    /// Widest number of simultaneous lanes, used to size the canvas.
    pub lane_count: u32,
    /// True when the walk stopped at the requested limit.
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefEntry {
    /// Short display name, e.g. `main` or `origin/main`.
    pub name: String,
    /// Fully qualified name, e.g. `refs/heads/main`.
    pub full_name: String,
    pub kind: RefKind,
    pub target: String,
    pub is_head: bool,
    /// Configured upstream for a local branch, e.g. `origin/main`.
    pub upstream: Option<String>,
    /// Commits on this branch missing from its upstream.
    pub ahead: u32,
    /// Commits on the upstream missing from this branch.
    pub behind: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadInfo {
    /// Branch name, or the short id when HEAD is detached.
    pub name: String,
    pub oid: Option<String>,
    pub detached: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub path: String,
    pub name: String,
    pub head: Option<HeadInfo>,
    pub is_empty: bool,
    pub remotes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChange,
    Untracked,
    Conflicted,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    /// Previous path when the file was renamed or copied.
    pub old_path: Option<String>,
    pub status: ChangeStatus,
    pub insertions: u32,
    pub deletions: u32,
    pub is_binary: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetail {
    pub commit: Commit,
    pub files: Vec<FileChange>,
    pub insertions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingStatus {
    pub staged: Vec<FileChange>,
    pub unstaged: Vec<FileChange>,
    pub conflicted: Vec<String>,
}

/// Why a requested path was skipped rather than staged or unstaged, instead
/// of failing the whole batch (design.md §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SkipReason {
    /// Present in neither the working tree, the index, nor `HEAD` — already
    /// committed elsewhere, or an untracked file that was deleted.
    Vanished,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedPath {
    pub path: String,
    pub reason: SkipReason,
}

/// The result of any write (`stage`, `unstage`). `status` is the existing
/// sorted listing, recomputed after the write; `skipped` names paths the
/// caller asked for that no longer existed anywhere (D9: sorted at
/// construction, same as every other list this crate returns).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteOutcome {
    pub status: WorkingStatus,
    pub skipped: Vec<SkippedPath>,
}
