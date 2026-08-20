//! Shared throwaway-repository helpers for `git-core`'s integration tests.
//!
//! These build repositories directly through `git2` (not through `GitRepo`),
//! which is fine here: this is test scaffolding, not a product write path,
//! and `crates/git-core`'s `#![deny(clippy::disallowed_methods)]` does not
//! apply to `tests/*.rs` — each is compiled as its own crate.

use std::path::Path;
use std::process::Command;

use git2::{Repository, Signature, Time};

pub const USER_NAME: &str = "Test User";
pub const USER_EMAIL: &str = "test@example.com";

/// A throwaway repository directory, deleted when the returned `TempDir`
/// drops.
pub struct Sandbox {
    pub dir: tempfile::TempDir,
}

impl Sandbox {
    pub fn path(&self) -> &Path {
        self.dir.path()
    }
}

/// An empty repository with git identity pinned locally, so no test depends
/// on ambient `user.name`/`user.email`.
pub fn init_repo() -> Sandbox {
    let dir = tempfile::tempdir().expect("create temp dir");
    let repo = Repository::init(dir.path()).expect("git2 init");
    let mut config = repo.config().expect("repo config");
    config
        .set_str("user.name", USER_NAME)
        .expect("set user.name");
    config
        .set_str("user.email", USER_EMAIL)
        .expect("set user.email");
    Sandbox { dir }
}

/// A repository with one commit adding `path` with `content`.
pub fn init_repo_with_commit(path: &str, content: &[u8]) -> Sandbox {
    let sandbox = init_repo();
    commit_file(sandbox.path(), path, content, "initial commit");
    sandbox
}

/// Write `path` (relative to `dir`) and commit it via `git2` directly — a
/// deterministic, in-process way to build fixture history without depending
/// on a real `git` subprocess for setup.
pub fn commit_file(dir: &Path, path: &str, content: &[u8], summary: &str) {
    let repo = Repository::open(dir).expect("open repo for commit_file");
    let full_path = dir.join(path);
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dirs");
    }
    std::fs::write(&full_path, content).expect("write file");

    // Test scaffolding, not a product write path — see design.md §1.3.
    #[allow(clippy::disallowed_methods)]
    let mut index = repo.index().expect("index");
    index.add_path(Path::new(path)).expect("add_path");
    index.write().expect("write index");
    let tree_oid = index.write_tree().expect("write_tree");
    let tree = repo.find_tree(tree_oid).expect("find_tree");

    let sig = Signature::new(USER_NAME, USER_EMAIL, &Time::new(1_700_000_000, 0))
        .expect("build signature");

    let parent_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let parents: Vec<git2::Commit<'_>> = parent_commit.into_iter().collect();
    let parent_refs: Vec<&git2::Commit<'_>> = parents.iter().collect();

    repo.commit(Some("HEAD"), &sig, &sig, summary, &tree, &parent_refs)
        .expect("commit");
}

/// Run a real `git` subprocess in `dir`. Used where the test specifically
/// needs staging (or another mutation) to happen *outside* the crate's own
/// libgit2 handle — an in-process `index.add_path` would share libgit2's
/// in-memory state and would not reproduce what this is testing.
pub fn external_git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", USER_NAME)
        .env("GIT_AUTHOR_EMAIL", USER_EMAIL)
        .env("GIT_COMMITTER_NAME", USER_NAME)
        .env("GIT_COMMITTER_EMAIL", USER_EMAIL)
        .status()
        .expect("spawn real `git` subprocess — is `git` on PATH?");
    assert!(
        status.success(),
        "external `git {}` failed in {}",
        args.join(" "),
        dir.display()
    );
}
