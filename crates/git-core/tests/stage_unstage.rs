//! Phase 3 (tasks.md 3.6, 3.7): stage/unstage behaviour, replayed literally
//! against spec.md's scenarios.

mod support;

use git_core::model::SkipReason;
use git_core::GitRepo;

#[test]
fn a_modified_file_stages_and_nothing_else_is_touched() {
    // spec.md "A modified file is staged"
    let sandbox = support::init_repo_with_commit("src/main.rs", b"fn main() {}\n");
    let path = sandbox.path();
    std::fs::write(path.join("src/main.rs"), b"fn main() { }\n").unwrap();

    let repo = GitRepo::open(path).unwrap();
    let outcome = repo.stage(&["src/main.rs".to_string()]).unwrap();

    assert_eq!(
        outcome
            .status
            .staged
            .iter()
            .map(|f| f.path.as_str())
            .collect::<Vec<_>>(),
        vec!["src/main.rs"]
    );
    assert!(outcome.status.unstaged.is_empty());
    assert!(outcome.skipped.is_empty());
}

#[test]
fn a_staged_file_with_local_edits_unstages_content_byte_identical() {
    // spec.md "A staged file is unstaged without touching its contents"
    let sandbox = support::init_repo_with_commit("tracked.txt", b"committed\n");
    let path = sandbox.path();

    std::fs::write(path.join("tracked.txt"), b"staged edit\n").unwrap();
    let repo = GitRepo::open(path).unwrap();
    repo.stage(&["tracked.txt".to_string()]).unwrap();

    let before = std::fs::read(path.join("tracked.txt")).unwrap();
    let outcome = repo.unstage(&["tracked.txt".to_string()]).unwrap();
    let after = std::fs::read(path.join("tracked.txt")).unwrap();

    assert_eq!(before, after, "unstage must never touch the working tree");
    assert!(outcome.status.staged.is_empty());
    assert_eq!(
        outcome
            .status
            .unstaged
            .iter()
            .map(|f| f.path.as_str())
            .collect::<Vec<_>>(),
        vec!["tracked.txt"]
    );
}

#[test]
fn stage_all_stages_only_what_was_listed() {
    // spec.md "Stage all stages only what was shown"
    let sandbox = support::init_repo_with_commit("README.md", b"root\n");
    let path = sandbox.path();

    std::fs::write(path.join("one.txt"), b"1\n").unwrap();
    std::fs::write(path.join("two.txt"), b"2\n").unwrap();
    std::fs::write(path.join("three.txt"), b"3\n").unwrap();
    // Untracked, un-ignored build artifact NOT in the listed set.
    std::fs::write(path.join("build-artifact.log"), b"noise\n").unwrap();

    let repo = GitRepo::open(path).unwrap();
    let outcome = repo
        .stage(&[
            "one.txt".to_string(),
            "two.txt".to_string(),
            "three.txt".to_string(),
        ])
        .unwrap();

    let staged: Vec<&str> = outcome
        .status
        .staged
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    assert_eq!(staged, vec!["one.txt", "three.txt", "two.txt"]);

    let unstaged: Vec<&str> = outcome
        .status
        .unstaged
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    assert_eq!(
        unstaged,
        vec!["build-artifact.log"],
        "the un-listed artifact must remain untracked, not staged"
    );
}

#[test]
fn bracketed_filename_stages_and_unstages_without_touching_a_sibling() {
    let sandbox = support::init_repo_with_commit("README.md", b"root\n");
    let path = sandbox.path();

    std::fs::write(path.join("a[b].txt"), b"bracket\n").unwrap();
    std::fs::write(path.join("ab.txt"), b"sibling\n").unwrap();

    let repo = GitRepo::open(path).unwrap();
    let outcome = repo.stage(&["a[b].txt".to_string()]).unwrap();
    let staged: Vec<&str> = outcome
        .status
        .staged
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    assert_eq!(staged, vec!["a[b].txt"]);
    let unstaged: Vec<&str> = outcome
        .status
        .unstaged
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    assert_eq!(unstaged, vec!["ab.txt"], "sibling must stay untouched");

    let outcome = repo.unstage(&["a[b].txt".to_string()]).unwrap();
    assert!(outcome.status.staged.is_empty());
    let unstaged: Vec<&str> = outcome
        .status
        .unstaged
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    assert_eq!(unstaged, vec!["a[b].txt", "ab.txt"]);
}

#[test]
fn unstage_on_an_unborn_branch_removes_the_entry_rather_than_erroring() {
    let sandbox = support::init_repo();
    let path = sandbox.path();
    std::fs::write(path.join("new.txt"), b"first ever\n").unwrap();

    let repo = GitRepo::open(path).unwrap();
    repo.stage(&["new.txt".to_string()]).unwrap();
    let outcome = repo.unstage(&["new.txt".to_string()]).unwrap();

    assert!(outcome.status.staged.is_empty());
    let unstaged: Vec<&str> = outcome
        .status
        .unstaged
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    assert_eq!(unstaged, vec!["new.txt"]);
}

#[test]
fn a_vanished_path_in_a_batch_is_skipped_and_reported_not_a_batch_failure() {
    let sandbox = support::init_repo_with_commit("README.md", b"root\n");
    let path = sandbox.path();
    std::fs::write(path.join("real.txt"), b"present\n").unwrap();

    let repo = GitRepo::open(path).unwrap();
    let outcome = repo
        .stage(&["real.txt".to_string(), "ghost.txt".to_string()])
        .unwrap();

    let staged: Vec<&str> = outcome
        .status
        .staged
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    assert_eq!(staged, vec!["real.txt"]);
    assert_eq!(outcome.skipped.len(), 1);
    assert_eq!(outcome.skipped[0].path, "ghost.txt");
    assert_eq!(outcome.skipped[0].reason, SkipReason::Vanished);
}

#[test]
fn conflicted_path_refusal_fires_before_any_mutation_for_stage_and_unstage() {
    // Build two divergent branches over the same file so a merge conflicts.
    let sandbox = support::init_repo_with_commit("shared.txt", b"base\n");
    let path = sandbox.path();
    support::external_git(path, &["branch", "other"]);
    support::commit_file(path, "shared.txt", b"main change\n", "main change");
    support::external_git(path, &["checkout", "other"]);
    support::commit_file(path, "shared.txt", b"other change\n", "other change");
    support::external_git(path, &["checkout", "master"]);
    // Merge is expected to conflict; ignore its exit status.
    let _ = std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["merge", "other", "--no-edit"])
        .output();

    let repo = GitRepo::open(path).unwrap();
    let status = repo.status().unwrap();
    assert!(
        !status.conflicted.is_empty(),
        "test setup must produce a conflict"
    );

    let index_before = std::fs::read(path.join(".git/index")).unwrap();

    let stage_result = repo.stage(&["shared.txt".to_string()]);
    assert!(matches!(
        stage_result,
        Err(git_core::CoreError::ConflictsPresent { .. })
    ));

    let unstage_result = repo.unstage(&["shared.txt".to_string()]);
    assert!(matches!(
        unstage_result,
        Err(git_core::CoreError::ConflictsPresent { .. })
    ));

    let index_after = std::fs::read(path.join(".git/index")).unwrap();
    assert_eq!(
        index_before, index_after,
        "a refused write must not mutate the index"
    );
}

#[test]
fn staged_and_unstaged_listings_come_through_the_same_sorted_status_path() {
    // Requirement "Listings Are Deterministically Ordered": every list this
    // change returns comes from the existing sorted `status()`, so a write
    // outcome's lists are sorted the same way a plain `status()` call is.
    let sandbox = support::init_repo_with_commit("README.md", b"root\n");
    let path = sandbox.path();
    std::fs::write(path.join("zeta.txt"), b"z\n").unwrap();
    std::fs::write(path.join("alpha.txt"), b"a\n").unwrap();

    let repo = GitRepo::open(path).unwrap();
    let outcome = repo
        .stage(&["zeta.txt".to_string(), "alpha.txt".to_string()])
        .unwrap();
    let via_write: Vec<&str> = outcome
        .status
        .staged
        .iter()
        .map(|f| f.path.as_str())
        .collect();

    let via_status = repo.status().unwrap();
    let via_status: Vec<&str> = via_status.staged.iter().map(|f| f.path.as_str()).collect();

    assert_eq!(via_write, via_status);
    assert_eq!(via_write, vec!["alpha.txt", "zeta.txt"]);
}
