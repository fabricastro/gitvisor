//! Tauri commands: the only entry points the UI can call.
//!
//! Each one is a thin translation layer. All logic lives in `git-core`.

use git_core::model::{
    CommitDetail, CommitOutcome, CommitRequest, GitProbe, Graph, RefEntry, RepoInfo, WorkingStatus,
    WriteOutcome,
};
use git_core::Result;
use tauri::State;

use crate::state::RepoRegistry;

/// Repository passed on the command line, so `gitvisor .` works like an editor.
///
/// Returns `None` when the argument is missing or is not a real path.
#[tauri::command]
pub fn startup_path() -> Option<String> {
    std::env::args()
        .skip(1)
        .find(|arg| !arg.starts_with('-'))
        .and_then(|arg| std::fs::canonicalize(arg).ok())
        .map(|path| path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn open_repository(path: String, repos: State<'_, RepoRegistry>) -> Result<RepoInfo> {
    repos.with(&path, |repo| repo.info())
}

#[tauri::command]
pub fn close_repository(path: String, repos: State<'_, RepoRegistry>) {
    repos.close(&path);
}

#[tauri::command]
pub fn list_refs(path: String, repos: State<'_, RepoRegistry>) -> Result<Vec<RefEntry>> {
    repos.with(&path, |repo| repo.refs())
}

#[tauri::command]
pub fn commit_graph(
    path: String,
    limit: Option<usize>,
    repos: State<'_, RepoRegistry>,
) -> Result<Graph> {
    repos.with(&path, |repo| repo.graph(limit))
}

#[tauri::command]
pub fn commit_detail(
    path: String,
    id: String,
    repos: State<'_, RepoRegistry>,
) -> Result<CommitDetail> {
    repos.with(&path, |repo| repo.commit_detail(&id))
}

#[tauri::command]
pub fn working_status(path: String, repos: State<'_, RepoRegistry>) -> Result<WorkingStatus> {
    repos.with(&path, |repo| repo.status())
}

#[tauri::command]
pub fn stage_paths(
    path: String,
    paths: Vec<String>,
    repos: State<'_, RepoRegistry>,
) -> Result<WriteOutcome> {
    repos.with(&path, |repo| repo.stage(&paths))
}

#[tauri::command]
pub fn unstage_paths(
    path: String,
    paths: Vec<String>,
    repos: State<'_, RepoRegistry>,
) -> Result<WriteOutcome> {
    repos.with(&path, |repo| repo.unstage(&paths))
}

/// Whether `git` is available for the commit step (design.md §4.4). A hint,
/// not authoritative — `create_commit` resolves and validates `git` again,
/// because a probe result is allowed to go stale between an install and the
/// next commit.
#[tauri::command]
pub fn git_probe(
    path: String,
    git_override: Option<String>,
    repos: State<'_, RepoRegistry>,
) -> Result<GitProbe> {
    repos.with(&path, |repo| Ok(repo.probe(git_override.as_deref())))
}

#[tauri::command]
pub fn create_commit(
    path: String,
    message: String,
    git_override: Option<String>,
    repos: State<'_, RepoRegistry>,
) -> Result<CommitOutcome> {
    let outcome = repos.with(&path, |repo| {
        repo.commit(CommitRequest {
            message: message.clone(),
            git_override: git_override.clone(),
            timeout: None,
        })
    })?;
    // Only after success (design.md §12) — a refused or failed commit never
    // touched the repository, so the cached handle stays valid.
    repos.invalidate(&path);
    Ok(outcome)
}
