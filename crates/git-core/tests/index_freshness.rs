//! Black-box M2 replay through the *public* API (design.md §1.4, tasks.md
//! 2.5, spec.md "External Staging Is Never Destroyed"): a `GitRepo` held
//! open — exactly what `RepoRegistry` does — must not destroy a `git add`
//! performed externally, in a real subprocess, while it was open.
//!
//! **This test alone does not isolate `with_fresh_index`'s `read(true)`.**
//! While writing it, `Repository::statuses()` — which `stage`'s own
//! pre-flight calls before ever reaching `with_fresh_index` — was found to
//! have an incidental side effect of soft-syncing libgit2's cached index,
//! which masks a missing `read(true)` here (this test stayed green even with
//! that line deleted). The test that actually isolates the mechanism and
//! goes red without it is
//! `repo::index_guard::tests::m2_external_git_add_survives_with_fresh_index`
//! in `src/repo/index_guard.rs`, which calls `with_fresh_index` directly with
//! nothing in between that could refresh the cache as a side effect. That
//! isolation was verified by hand: the `read(true)?;` line was temporarily
//! deleted, that test went red (the on-disk index ended up missing `b.txt`),
//! and the line was restored.
//!
//! This test is kept anyway: it replays spec.md's scenario literally, end to
//! end, through the same `stage()` a caller actually uses.

mod support;

use git_core::GitRepo;

#[test]
fn external_git_add_survives_a_gitvisor_stage() {
    let sandbox = support::init_repo_with_commit("README.md", b"initial\n");
    let path = sandbox.path();

    // Open once and hold it — the same shape as `RepoRegistry::with`, which
    // keeps a `GitRepo` alive across multiple commands against one
    // repository.
    let repo = GitRepo::open(path).expect("open");

    // A first write is what actually warms libgit2's own cached index handle
    // on this `Repository` (measurements.md M2: the freshness problem only
    // appears once the repository's index has already been loaded once —
    // an idle, never-touched `Repository` reads fresh on its first access
    // regardless of `read(true)`, so a baseline `status()` alone does not
    // reproduce the condition; a prior write does).
    std::fs::write(path.join("a.txt"), b"a\n").expect("write a.txt");
    repo.stage(&["a.txt".to_string()])
        .expect("stage a.txt to warm the cached index handle");

    std::fs::write(path.join("b.txt"), b"b\n").expect("write b.txt");
    std::fs::write(path.join("c.txt"), b"c\n").expect("write c.txt");

    // The external add must be a real subprocess — an in-process
    // `index.add_path` would share libgit2's in-memory state and would not
    // reproduce the measured condition.
    support::external_git(path, &["add", "b.txt"]);

    let outcome = repo
        .stage(&["c.txt".to_string()])
        .expect("stage c.txt through GitRepo");
    assert!(outcome.skipped.is_empty());

    let staged: Vec<&str> = outcome
        .status
        .staged
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    assert_eq!(
        staged,
        vec!["a.txt", "b.txt", "c.txt"],
        "the external `git add b.txt` did not survive the Gitvisor stage of c.txt"
    );
}
