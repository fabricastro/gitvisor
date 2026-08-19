//! Adapter over libgit2.
//!
//! Everything that touches an actual repository lives here. The rest of the
//! crate works on plain data, which keeps the graph algorithm testable without
//! creating repositories on disk.

use std::collections::HashMap;
use std::path::Path;

use git2::{BranchType, Delta, DiffOptions, Repository, Sort, StatusOptions};

use crate::error::{CoreError, Result};
use crate::graph;
use crate::model::{
    ChangeStatus, Commit, CommitDetail, FileChange, Graph, HeadInfo, RefBadge, RefEntry, RefKind,
    RepoInfo, Signature, WorkingStatus,
};

/// Commits read in one go when the caller does not ask for a specific limit.
pub const DEFAULT_COMMIT_LIMIT: usize = 5_000;

/// An opened repository. Cheap to keep around; not thread safe by itself, so
/// callers that share it across threads must guard it.
pub struct GitRepo {
    inner: Repository,
}

impl GitRepo {
    /// Open the repository containing `path`, walking up towards the root.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let inner = Repository::discover(path).map_err(|_| {
            CoreError::NotARepository(format!("No git repository found at {}", path.display()))
        })?;
        Ok(Self { inner })
    }

    pub fn info(&self) -> Result<RepoInfo> {
        let root = self
            .inner
            .workdir()
            .unwrap_or_else(|| self.inner.path())
            .to_path_buf();
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| root.to_string_lossy().to_string());

        let head = match self.inner.head() {
            Ok(reference) => {
                let detached = self.inner.head_detached().unwrap_or(false);
                let oid = reference.target().map(|oid| oid.to_string());
                let name = if detached {
                    oid.as_deref().map(short).unwrap_or("HEAD").to_string()
                } else {
                    reference.shorthand().unwrap_or("HEAD").to_string()
                };
                Some(HeadInfo {
                    name,
                    oid,
                    detached,
                })
            }
            // An freshly initialised repository has an unborn HEAD.
            Err(_) => None,
        };

        let remotes = self
            .inner
            .remotes()?
            .iter()
            .flatten()
            .map(str::to_string)
            .collect();

        Ok(RepoInfo {
            path: root.to_string_lossy().to_string(),
            name,
            head,
            is_empty: self.inner.is_empty().unwrap_or(false),
            remotes,
        })
    }

    /// Local branches, remote branches and tags, with tracking counts.
    pub fn refs(&self) -> Result<Vec<RefEntry>> {
        let head_name = self
            .inner
            .head()
            .ok()
            .and_then(|r| r.shorthand().map(str::to_string));

        let mut entries = Vec::new();
        for reference in self.inner.references()? {
            let reference = reference?;
            // Skip symbolic refs such as refs/remotes/origin/HEAD.
            if reference.symbolic_target().is_some() {
                continue;
            }
            let Some(full_name) = reference.name().map(str::to_string) else {
                continue;
            };
            let Some(name) = reference.shorthand().map(str::to_string) else {
                continue;
            };
            let kind = if reference.is_branch() {
                RefKind::LocalBranch
            } else if reference.is_remote() {
                RefKind::RemoteBranch
            } else if reference.is_tag() {
                RefKind::Tag
            } else {
                continue;
            };
            let Ok(commit) = reference.peel_to_commit() else {
                continue;
            };

            let (upstream, ahead, behind) = match kind {
                RefKind::LocalBranch => self.tracking(&name, commit.id()),
                _ => (None, 0, 0),
            };

            entries.push(RefEntry {
                is_head: kind == RefKind::LocalBranch && head_name.as_deref() == Some(&name),
                name,
                full_name,
                kind,
                target: commit.id().to_string(),
                upstream,
                ahead,
                behind,
            });
        }

        // Branches, then remotes, then tags; alphabetical within each group.
        entries.sort_by(|a, b| {
            (a.kind as u8)
                .cmp(&(b.kind as u8))
                .then_with(|| a.name.cmp(&b.name))
        });
        Ok(entries)
    }

    fn tracking(&self, branch: &str, local_oid: git2::Oid) -> (Option<String>, u32, u32) {
        let Ok(local) = self.inner.find_branch(branch, BranchType::Local) else {
            return (None, 0, 0);
        };
        let Ok(upstream) = local.upstream() else {
            return (None, 0, 0);
        };
        let name = upstream.name().ok().flatten().map(str::to_string);
        let Some(upstream_oid) = upstream.get().target() else {
            return (name, 0, 0);
        };
        let (ahead, behind) = self
            .inner
            .graph_ahead_behind(local_oid, upstream_oid)
            .unwrap_or((0, 0));
        (name, ahead as u32, behind as u32)
    }

    /// Walk the history from every ref and lay it out into a drawable graph.
    pub fn graph(&self, limit: Option<usize>) -> Result<Graph> {
        let limit = limit.unwrap_or(DEFAULT_COMMIT_LIMIT);
        let badges = self.ref_badges()?;

        let mut walk = self.inner.revwalk()?;
        // Topological order keeps children strictly above their parents, which
        // the lane algorithm depends on; time order breaks the remaining ties.
        walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
        self.seed(&mut walk)?;

        let mut commits = Vec::with_capacity(limit.min(4_096));
        let mut truncated = false;
        for oid in walk {
            if commits.len() == limit {
                truncated = true;
                break;
            }
            let oid = oid?;
            let commit = self.inner.find_commit(oid)?;
            let id = oid.to_string();
            commits.push(Commit {
                short_id: short(&id).to_string(),
                summary: commit.summary().unwrap_or_default().to_string(),
                body: commit.body().unwrap_or_default().to_string(),
                author: signature(&commit.author()),
                committer: signature(&commit.committer()),
                parents: commit.parent_ids().map(|p| p.to_string()).collect(),
                refs: badges.get(&id).cloned().unwrap_or_default(),
                id,
            });
        }

        Ok(graph::build(commits, truncated))
    }

    /// Push every ref tip onto the walk so no branch is left out of the graph.
    fn seed(&self, walk: &mut git2::Revwalk<'_>) -> Result<()> {
        let mut seeded = false;
        for reference in self.inner.references()? {
            let Ok(reference) = reference else { continue };
            if let Ok(commit) = reference.peel_to_commit() {
                walk.push(commit.id())?;
                seeded = true;
            }
        }
        // A detached HEAD is not covered by any ref.
        if let Ok(commit) = self.inner.head().and_then(|h| h.peel_to_commit()) {
            walk.push(commit.id())?;
            seeded = true;
        }
        if !seeded {
            // Empty repository: nothing to walk, and pushing nothing is fine.
        }
        Ok(())
    }

    fn ref_badges(&self) -> Result<HashMap<String, Vec<RefBadge>>> {
        let mut badges: HashMap<String, Vec<RefBadge>> = HashMap::new();
        for entry in self.refs()? {
            badges.entry(entry.target).or_default().push(RefBadge {
                name: entry.name,
                kind: entry.kind,
                is_head: entry.is_head,
            });
        }
        Ok(badges)
    }

    /// Metadata plus the changed-file list for a single commit.
    pub fn commit_detail(&self, id: &str) -> Result<CommitDetail> {
        let oid = git2::Oid::from_str(id)
            .map_err(|_| CoreError::Invalid(format!("Invalid commit id: {id}")))?;
        let commit = self.inner.find_commit(oid)?;
        let badges = self.ref_badges()?;

        let tree = commit.tree()?;
        // Compare against the first parent; a root commit is compared to nothing.
        let parent_tree = match commit.parent(0) {
            Ok(parent) => Some(parent.tree()?),
            Err(_) => None,
        };

        let mut options = DiffOptions::new();
        options.include_typechange(true);
        let mut diff =
            self.inner
                .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut options))?;
        diff.find_similar(None)?;

        let mut files = collect_file_changes(&diff)?;
        // Same platform-dependent ordering applies to diff deltas.
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let insertions = files.iter().map(|f| f.insertions).sum();
        let deletions = files.iter().map(|f| f.deletions).sum();

        let id = oid.to_string();
        // Bound to a local so the borrowed signatures drop before `commit` does.
        let detail = CommitDetail {
            commit: Commit {
                short_id: short(&id).to_string(),
                summary: commit.summary().unwrap_or_default().to_string(),
                body: commit.body().unwrap_or_default().to_string(),
                author: signature(&commit.author()),
                committer: signature(&commit.committer()),
                parents: commit.parent_ids().map(|p| p.to_string()).collect(),
                refs: badges.get(&id).cloned().unwrap_or_default(),
                id,
            },
            files,
            insertions,
            deletions,
        };
        Ok(detail)
    }

    /// Staged, unstaged and conflicted paths in the working directory.
    pub fn status(&self) -> Result<WorkingStatus> {
        let mut options = StatusOptions::new();
        options
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true)
            .include_ignored(false);

        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        let mut conflicted = Vec::new();

        for entry in self.inner.statuses(Some(&mut options))?.iter() {
            let path = entry.path().unwrap_or_default().to_string();
            let flags = entry.status();

            if flags.is_conflicted() {
                conflicted.push(path);
                continue;
            }
            if let Some(status) = index_status(flags) {
                staged.push(simple_change(path.clone(), status));
            }
            if let Some(status) = workdir_status(flags) {
                unstaged.push(simple_change(path, status));
            }
        }

        // libgit2 orders status entries using the repository's `core.ignorecase`,
        // which follows the filesystem: case-insensitive on macOS, byte-wise on
        // Linux. Sort explicitly so the same repository reports the same order
        // on every platform.
        staged.sort_by(|a, b| a.path.cmp(&b.path));
        unstaged.sort_by(|a, b| a.path.cmp(&b.path));
        conflicted.sort();

        Ok(WorkingStatus {
            staged,
            unstaged,
            conflicted,
        })
    }
}

fn collect_file_changes(diff: &git2::Diff<'_>) -> Result<Vec<FileChange>> {
    // Per-file line counts are only available through the line callback;
    // `Diff::stats` gives repository-wide totals only.
    let mut counts: HashMap<String, (u32, u32)> = HashMap::new();
    diff.foreach(
        &mut |_delta, _progress| true,
        None,
        None,
        Some(&mut |delta, _hunk, line| {
            let path = delta_path(&delta);
            let entry = counts.entry(path).or_default();
            match line.origin() {
                '+' => entry.0 += 1,
                '-' => entry.1 += 1,
                _ => {}
            }
            true
        }),
    )?;

    Ok(diff
        .deltas()
        .map(|delta| {
            let path = delta_path(&delta);
            let (insertions, deletions) = counts.get(&path).copied().unwrap_or((0, 0));
            let old_path = delta
                .old_file()
                .path()
                .map(|p| p.to_string_lossy().to_string())
                .filter(|old| *old != path);
            FileChange {
                is_binary: delta.new_file().is_binary() || delta.old_file().is_binary(),
                status: change_status(delta.status()),
                path,
                old_path,
                insertions,
                deletions,
            }
        })
        .collect())
}

fn delta_path(delta: &git2::DiffDelta<'_>) -> String {
    delta
        .new_file()
        .path()
        .or_else(|| delta.old_file().path())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn change_status(delta: Delta) -> ChangeStatus {
    match delta {
        Delta::Added => ChangeStatus::Added,
        Delta::Deleted => ChangeStatus::Deleted,
        Delta::Renamed => ChangeStatus::Renamed,
        Delta::Copied => ChangeStatus::Copied,
        Delta::Typechange => ChangeStatus::TypeChange,
        Delta::Untracked => ChangeStatus::Untracked,
        Delta::Conflicted => ChangeStatus::Conflicted,
        _ => ChangeStatus::Modified,
    }
}

fn index_status(flags: git2::Status) -> Option<ChangeStatus> {
    if flags.is_index_new() {
        Some(ChangeStatus::Added)
    } else if flags.is_index_deleted() {
        Some(ChangeStatus::Deleted)
    } else if flags.is_index_renamed() {
        Some(ChangeStatus::Renamed)
    } else if flags.is_index_typechange() {
        Some(ChangeStatus::TypeChange)
    } else if flags.is_index_modified() {
        Some(ChangeStatus::Modified)
    } else {
        None
    }
}

fn workdir_status(flags: git2::Status) -> Option<ChangeStatus> {
    if flags.is_wt_new() {
        Some(ChangeStatus::Untracked)
    } else if flags.is_wt_deleted() {
        Some(ChangeStatus::Deleted)
    } else if flags.is_wt_renamed() {
        Some(ChangeStatus::Renamed)
    } else if flags.is_wt_typechange() {
        Some(ChangeStatus::TypeChange)
    } else if flags.is_wt_modified() {
        Some(ChangeStatus::Modified)
    } else {
        None
    }
}

fn simple_change(path: String, status: ChangeStatus) -> FileChange {
    FileChange {
        path,
        old_path: None,
        status,
        insertions: 0,
        deletions: 0,
        is_binary: false,
    }
}

fn signature(sig: &git2::Signature<'_>) -> Signature {
    Signature {
        name: sig.name().unwrap_or_default().to_string(),
        email: sig.email().unwrap_or_default().to_string(),
        timestamp: sig.when().seconds(),
        offset_minutes: sig.when().offset_minutes(),
    }
}

/// Abbreviated object id, matching what git shows by default.
fn short(id: &str) -> &str {
    &id[..id.len().min(7)]
}
