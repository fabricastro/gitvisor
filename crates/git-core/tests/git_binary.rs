//! Phase 4a (tasks.md 4a.5): `git` resolution precedence and probe rejection.
//!
//! `resolve()`/`probe()` read `GITVISOR_GIT_PATH` from the process
//! environment (design.md §4.1) — process-global state that `cargo test`'s
//! default parallel execution would otherwise race across tests. `ENV_GUARD`
//! serialises just the tests in this file that touch it; every other test in
//! the crate is unaffected.

mod support;

use std::sync::Mutex;

use git_core::git_binary::{probe, resolve};
use git_core::CoreError;

static ENV_GUARD: Mutex<()> = Mutex::new(());

/// RAII guard: sets `GITVISOR_GIT_PATH` for the duration of one test, then
/// removes it — so a panic mid-test cannot leak the mutation into the next.
struct EnvVar {
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl EnvVar {
    fn set(value: &str) -> Self {
        let lock = ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
        // SAFETY: serialised by `ENV_GUARD` above; no other test in this
        // binary reads or writes `GITVISOR_GIT_PATH` outside that lock.
        unsafe { std::env::set_var("GITVISOR_GIT_PATH", value) };
        EnvVar { _lock: lock }
    }
}

impl Drop for EnvVar {
    fn drop(&mut self) {
        // SAFETY: see `set` above — still under `ENV_GUARD`.
        unsafe { std::env::remove_var("GITVISOR_GIT_PATH") };
    }
}

#[test]
fn explicit_override_beats_env_var() {
    let _env = EnvVar::set("/definitely/not/a/real/git/path/from/env");
    let resolved = resolve(Some("/definitely/not/a/real/git/path/from/override"))
        .expect("resolve with an explicit override never fails — validation is probe's job");
    assert_eq!(
        resolved.path.to_string_lossy(),
        "/definitely/not/a/real/git/path/from/override"
    );
}

#[test]
fn env_var_beats_path_when_no_override_given() {
    let _env = EnvVar::set("/definitely/not/a/real/git/path/from/env");
    let resolved = resolve(None).expect("resolve with only an env var never fails");
    assert_eq!(
        resolved.path.to_string_lossy(),
        "/definitely/not/a/real/git/path/from/env"
    );
}

#[test]
fn path_lookup_is_used_when_nothing_else_is_given() {
    let _lock = ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
    // SAFETY: serialised by `ENV_GUARD`.
    unsafe { std::env::remove_var("GITVISOR_GIT_PATH") };
    let resolved = resolve(None).expect("a real `git` must be on PATH for this test to run at all");
    assert!(
        resolved.path.file_name().is_some(),
        "expected a resolved path with a file name, got {:?}",
        resolved.path
    );
}

#[test]
fn probe_rejects_a_candidate_pointed_at_a_non_git_executable() {
    // `/bin/ls`-equivalent: a real, executable, non-`git` binary.
    let probed = probe(Some("/bin/ls"));
    assert!(
        !probed.available,
        "probe must reject a candidate whose --version output is not git's"
    );
}

#[test]
fn probe_rejects_a_candidate_pointed_at_a_directory() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let probed = probe(Some(dir.path().to_str().expect("utf8 path")));
    assert!(!probed.available, "probe must reject a directory");
}

#[test]
fn probe_reports_the_real_git_as_available() {
    let _lock = ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
    // SAFETY: serialised by `ENV_GUARD` — `probe(None)` below must not race
    // another test's `GITVISOR_GIT_PATH` mutation.
    unsafe { std::env::remove_var("GITVISOR_GIT_PATH") };

    let probed = probe(None);
    assert!(
        probed.available,
        "a real `git` must be on PATH for this test to run at all"
    );
    assert!(probed
        .version
        .as_deref()
        .is_some_and(|v| v.starts_with("git version ")));
}

#[test]
fn git_unavailable_names_what_was_looked_for() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let missing = dir.path().join("no-such-git-binary");
    let sandbox = support::init_repo_with_commit("README.md", b"root\n");
    std::fs::write(sandbox.path().join("a.txt"), b"content\n").unwrap();
    let repo = git_core::GitRepo::open(sandbox.path()).unwrap();
    repo.stage(&["a.txt".to_string()]).unwrap();

    let request = git_core::model::CommitRequest {
        message: "test".to_string(),
        git_override: Some(missing.to_string_lossy().to_string()),
        timeout: None,
    };
    let err = repo.commit(request).unwrap_err();
    match err {
        CoreError::GitUnavailable { looked_for } => {
            assert!(
                looked_for.contains("no-such-git-binary"),
                "expected the refusal to name the path that was looked for, got: {looked_for}"
            );
        }
        other => panic!("expected GitUnavailable, got {other:?}"),
    }
}
