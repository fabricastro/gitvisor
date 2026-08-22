//! Phase 4c (tasks.md 4c.1–4c.7): the commit test suite.
//!
//! Every test here spawns a real `git` subprocess (either the system `git`
//! or a throwaway fake script injected through `CommitRequest.git_override`
//! — design.md §10) against a real, isolated repository. None of this
//! asserts against `git2::Repository::signature()` or any other in-process
//! stand-in: the whole point of this suite is to catch what an in-process
//! assertion would miss (design.md §10, M5).

mod support;

use std::path::Path;
use std::time::Duration;

use git_core::model::CommitRequest;
use git_core::{CoreError, GitRepo};

/// Guards the one test in this file that mutates process-global env vars
/// (`HOME`, `GIT_AUTHOR_*`, `GIT_COMMITTER_*`) — required because
/// `git_binary::base_command` has no per-call env-override channel by
/// design (it inherits ambient env, matching what Gitvisor itself would be
/// launched under). No other test in this binary reads those specific vars.
static ENV_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn stage_one_file(repo: &GitRepo, dir: &Path, name: &str, content: &[u8]) {
    std::fs::write(dir.join(name), content).unwrap();
    repo.stage(&[name.to_string()]).unwrap();
}

fn request(message: &str) -> CommitRequest {
    CommitRequest {
        message: message.to_string(),
        git_override: None,
        timeout: None,
    }
}

fn fake_request(message: &str, git_path: &Path, timeout: Option<Duration>) -> CommitRequest {
    CommitRequest {
        message: message.to_string(),
        git_override: Some(git_path.to_string_lossy().to_string()),
        timeout,
    }
}

/// A fake `git` that answers `var GIT_AUTHOR_IDENT` successfully (so the
/// pre-flight never blocks the scenario under test) and otherwise runs
/// `$1`-dispatched behaviour supplied by the caller.
fn fake_git_script(identity_ok: bool, commit_behaviour: &str) -> String {
    let identity_line = if identity_ok {
        "echo 'Fake Author <fake@example.com> 1700000000 +0000'; exit 0"
    } else {
        "echo 'boom: no identity' >&2; exit 128"
    };
    format!("#!/bin/sh\nif [ \"$1\" = \"var\" ]; then\n  {identity_line}\nfi\n{commit_behaviour}\n")
}

// ---------------------------------------------------------------------------
// 4c.1 — hook-rejection regression, with a positive control.

#[test]
fn a_rejecting_pre_commit_hook_blocks_the_commit_with_a_positive_control() {
    // spec.md "A rejecting pre-commit hook blocks the commit".
    let sandbox = support::init_repo_with_commit("README.md", b"root\n");
    let path = sandbox.path();
    let hooks_dir = path.join(".git/hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    let hook_path = hooks_dir.join("pre-commit");
    std::fs::write(
        &hook_path,
        "#!/bin/sh\necho 'HOOK RAN — rejecting' >&2\nexit 1\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms).unwrap();
    }

    let repo = GitRepo::open(path).unwrap();
    let head_before = repo.info().unwrap().head.and_then(|h| h.oid);
    stage_one_file(&repo, path, "a.txt", b"hello\n");

    let err = repo.commit(request("blocked by hook")).unwrap_err();
    match err {
        CoreError::CommitFailed { stderr, .. } => {
            assert!(
                stderr.contains("HOOK RAN — rejecting"),
                "expected the hook's own stderr verbatim, got: {stderr}"
            );
        }
        other => panic!("expected CommitFailed, got {other:?}"),
    }
    let head_after = repo.info().unwrap().head.and_then(|h| h.oid);
    assert_eq!(
        head_before, head_after,
        "a rejected commit must not move HEAD"
    );

    // Positive control: the identical code path, a hook that passes, must
    // succeed — proving the refusal above is about the hook's exit code,
    // not a broken test setup.
    std::fs::write(&hook_path, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms).unwrap();
    }
    let outcome = repo
        .commit(request("passes through an identical hook path"))
        .expect("a passing hook must let the commit through");
    assert!(outcome.warning.is_none());
}

// ---------------------------------------------------------------------------
// 4c.2 — M1 replay: signing configuration is honoured.

#[test]
fn commit_is_signed_when_signing_is_required_m1_replay() {
    // Skip cleanly if this machine has no `gpg` at all — this test asserts
    // the *plumbing* (git, not libgit2, performs the commit and therefore
    // honours signing), not that every CI box ships GnuPG. Everything else
    // is self-contained: an ephemeral, no-passphrase key in a throwaway
    // `GNUPGHOME`, wired in through the repo's own LOCAL `gpg.program`
    // rather than the process environment, so this test needs no global
    // mutation and is safe under `cargo test`'s default parallel execution.
    if std::process::Command::new("gpg")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("skipping M1 replay: no `gpg` on PATH on this machine");
        return;
    }

    let gnupg_home = tempfile::tempdir().expect("throwaway GNUPGHOME");
    let keygen = std::process::Command::new("gpg")
        .env("GNUPGHOME", gnupg_home.path())
        .args([
            "--batch",
            "--pinentry-mode",
            "loopback",
            "--passphrase",
            "",
            "--quick-generate-key",
            "M1 Test <m1test@example.com>",
            "default",
            "default",
        ])
        .output()
        .expect("spawn gpg --quick-generate-key");
    if !keygen.status.success() {
        eprintln!(
            "skipping M1 replay: ephemeral key generation failed: {}",
            String::from_utf8_lossy(&keygen.stderr)
        );
        return;
    }
    let list = std::process::Command::new("gpg")
        .env("GNUPGHOME", gnupg_home.path())
        .args(["--list-secret-keys", "--with-colons"])
        .output()
        .expect("list secret keys");
    let listing = String::from_utf8_lossy(&list.stdout);
    let key_id = listing
        .lines()
        .find(|line| line.starts_with("sec:"))
        .and_then(|line| line.split(':').nth(4))
        .expect("expected a `sec:` line with a key id")
        .to_string();

    let sandbox = support::init_repo_with_commit("README.md", b"root\n");
    let path = sandbox.path();

    let wrapper_path = path.join("gpg-wrapper.sh");
    std::fs::write(
        &wrapper_path,
        format!(
            "#!/bin/sh\nexport GNUPGHOME=\"{}\"\nexec gpg --pinentry-mode loopback --passphrase '' \"$@\"\n",
            gnupg_home.path().display()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&wrapper_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&wrapper_path, perms).unwrap();
    }

    support::external_git(path, &["config", "user.signingkey", &key_id]);
    support::external_git(path, &["config", "commit.gpgsign", "true"]);
    support::external_git(
        path,
        &["config", "gpg.program", &wrapper_path.to_string_lossy()],
    );

    let repo = GitRepo::open(path).unwrap();
    stage_one_file(&repo, path, "a.txt", b"hello\n");
    let outcome = repo
        .commit(request("signed commit"))
        .expect("commit through git must honour commit.gpgsign, not silently drop it (M1)");

    let show = std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["log", "--show-signature", "-1", &outcome.id])
        .output()
        .unwrap();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&show.stdout),
        String::from_utf8_lossy(&show.stderr)
    );
    assert!(
        combined.contains("Good signature"),
        "expected `git log --show-signature` to report a good signature, got: {combined}"
    );
}

// ---------------------------------------------------------------------------
// 4c.3 — M5 replay: identity from GIT_AUTHOR_* must not false-refuse.

#[test]
fn identity_from_env_vars_only_succeeds_m5_replay() {
    // Real subprocess, real isolated HOME: no user.name/user.email at ANY
    // config level (not even the repo-local config `init_repo` normally
    // sets), identity supplied only through GIT_AUTHOR_*/GIT_COMMITTER_*.
    // This is the exact shape measurements.md's M5 measured
    // `Repository::signature()` getting wrong; the assertion here is that
    // `GitRepo::commit` — which goes through `git_binary::identity()`, never
    // `signature()` — succeeds.
    let dir = tempfile::tempdir().unwrap();
    let repo_dir = dir.path().join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();
    let isolated_home = dir.path().join("home");
    std::fs::create_dir_all(&isolated_home).unwrap();

    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo_dir)
            .args(args)
            .env("HOME", &isolated_home)
            .env_remove("GIT_AUTHOR_NAME")
            .env_remove("GIT_AUTHOR_EMAIL")
            .env_remove("GIT_COMMITTER_NAME")
            .env_remove("GIT_COMMITTER_EMAIL")
            .status()
            .expect("spawn git");
        assert!(status.success(), "git {args:?} failed");
    };
    git(&["init", "-q"]);
    std::fs::write(repo_dir.join("README.md"), b"root\n").unwrap();
    git(&["add", "README.md"]);

    // Commit the FIRST file via a real `git commit`, with identity supplied
    // only through the environment — establishes a real HEAD without ever
    // touching `user.name`/`user.email`.
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .args(["commit", "-m", "root"])
        .env("HOME", &isolated_home)
        .env("GIT_AUTHOR_NAME", "Env Author")
        .env("GIT_AUTHOR_EMAIL", "env-author@example.com")
        .env("GIT_COMMITTER_NAME", "Env Author")
        .env("GIT_COMMITTER_EMAIL", "env-author@example.com")
        .status()
        .expect("spawn git commit");
    assert!(status.success());

    // Now the real scenario under test: `GitRepo::commit` for a SECOND
    // change, identity only via the environment our own process runs
    // under — `git_binary::base_command` inherits it unmodified (M5,
    // design.md §2.2), and never touches GIT_AUTHOR_*/GIT_COMMITTER_*.
    let _guard = ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
    let original_home = std::env::var("HOME").ok();
    // SAFETY: serialised by `ENV_GUARD` — no other test in this binary reads
    // or writes HOME/GIT_AUTHOR_*/GIT_COMMITTER_* while this lock is held.
    unsafe {
        std::env::set_var("HOME", &isolated_home);
        std::env::set_var("GIT_AUTHOR_NAME", "Env Author");
        std::env::set_var("GIT_AUTHOR_EMAIL", "env-author@example.com");
        std::env::set_var("GIT_COMMITTER_NAME", "Env Author");
        std::env::set_var("GIT_COMMITTER_EMAIL", "env-author@example.com");
    }
    let result = {
        let repo = GitRepo::open(&repo_dir).unwrap();
        std::fs::write(repo_dir.join("a.txt"), b"second\n").unwrap();
        repo.stage(&["a.txt".to_string()]).unwrap();
        repo.commit(request("second commit, env-only identity"))
    };
    // SAFETY: restoring what this test itself set, still under `ENV_GUARD`.
    unsafe {
        match &original_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        std::env::remove_var("GIT_AUTHOR_NAME");
        std::env::remove_var("GIT_AUTHOR_EMAIL");
        std::env::remove_var("GIT_COMMITTER_NAME");
        std::env::remove_var("GIT_COMMITTER_EMAIL");
    }

    let outcome = result.expect(
        "a commit whose identity comes only from GIT_AUTHOR_*/GIT_COMMITTER_* must succeed \
         (M5) — a false refusal here would mean `IdentityMissing` is sourced from \
         `Repository::signature()` again, which measurements.md M5 showed disagrees with `git`",
    );
    assert!(outcome.warning.is_none());
}

// ---------------------------------------------------------------------------
// 4c.4 — unborn-branch first commit.

#[test]
fn commit_succeeds_on_an_unborn_branch() {
    // spec.md "First commit in a new repository".
    let sandbox = support::init_repo();
    let path = sandbox.path();
    let repo = GitRepo::open(path).unwrap();
    assert!(
        repo.info().unwrap().head.is_none(),
        "expected an unborn HEAD"
    );

    stage_one_file(&repo, path, "first.txt", b"the first ever commit\n");
    let outcome = repo
        .commit(request("first commit"))
        .expect("commit must succeed on an unborn branch");
    assert!(outcome.warning.is_none());

    let head = repo.info().unwrap().head.expect("HEAD must exist now");
    assert_eq!(head.oid.as_deref(), Some(outcome.id.as_str()));
}

// ---------------------------------------------------------------------------
// 4c.5 — detached-HEAD commit.

#[test]
fn commit_succeeds_on_a_detached_head() {
    // spec.md "A commit is made in detached HEAD state".
    let sandbox = support::init_repo_with_commit("README.md", b"root\n");
    let path = sandbox.path();
    let first_head = GitRepo::open(path)
        .unwrap()
        .info()
        .unwrap()
        .head
        .unwrap()
        .oid
        .unwrap();
    support::external_git(path, &["checkout", "--detach", &first_head]);

    let repo = GitRepo::open(path).unwrap();
    let info = repo.info().unwrap();
    assert!(info.head.unwrap().detached, "expected a detached HEAD");

    stage_one_file(&repo, path, "b.txt", b"detached change\n");
    let outcome = repo
        .commit(request("detached commit"))
        .expect("commit must succeed on a detached HEAD");

    let after = repo.info().unwrap();
    let head = after.head.unwrap();
    assert!(
        head.detached,
        "HEAD must still be detached — no branch ref moves"
    );
    assert_eq!(head.oid.as_deref(), Some(outcome.id.as_str()));

    // No branch ref moved: `master`/`main` still points at the first commit.
    let refs = repo.refs().unwrap();
    let branch = refs
        .iter()
        .find(|r| r.kind == git_core::model::RefKind::LocalBranch)
        .expect("expected a local branch");
    assert_eq!(
        branch.target, first_head,
        "the branch ref must not have moved"
    );
}

// ---------------------------------------------------------------------------
// 4c.6 — the timeout ladder, via fake `git` scripts.

#[test]
fn timeout_never_exits_head_unchanged_commit_timed_out() {
    let sandbox = support::init_repo_with_commit("README.md", b"root\n");
    let path = sandbox.path();
    let script = support::write_fake_git(path, "fake-git-sleep", &fake_git_script(true, "sleep 5"));

    let repo = GitRepo::open(path).unwrap();
    let head_before = repo.info().unwrap().head.unwrap().oid;
    stage_one_file(&repo, path, "a.txt", b"content\n");

    let err = repo
        .commit(fake_request(
            "should time out",
            &script,
            Some(Duration::from_millis(500)),
        ))
        .unwrap_err();
    match err {
        CoreError::CommitTimedOut { seconds, .. } => assert_eq!(seconds, 0),
        other => panic!("expected CommitTimedOut, got {other:?}"),
    }
    let head_after = repo.info().unwrap().head.unwrap().oid;
    assert_eq!(
        head_before, head_after,
        "a timed-out commit must not move HEAD"
    );
}

#[test]
fn timeout_after_head_moved_reports_a_warning_not_a_failure() {
    // The duplicate-commit bug, proven absent: a fake `git` that moves HEAD
    // (via a real nested `git commit --allow-empty`) and then hangs must be
    // reported as a SUCCESSFUL commit with a warning, never as a failure —
    // reporting failure here is exactly what would make the user retry and
    // create a duplicate (design.md §3.2).
    let sandbox = support::init_repo_with_commit("README.md", b"root\n");
    let path = sandbox.path();
    let real_git = which::which("git").expect("real git on PATH for this test");
    let script_body = fake_git_script(
        true,
        &format!(
            "\"{real}\" -C \"$2\" commit --allow-empty -m 'moved by fake git' >/dev/null 2>&1\nsleep 5\n",
            real = real_git.display()
        ),
    );
    let script = support::write_fake_git(path, "fake-git-move-then-hang", &script_body);

    let repo = GitRepo::open(path).unwrap();
    let head_before = repo.info().unwrap().head.unwrap().oid;
    stage_one_file(&repo, path, "a.txt", b"content\n");

    let outcome = repo
        .commit(fake_request(
            "moves head then hangs",
            &script,
            Some(Duration::from_millis(500)),
        ))
        .expect("HEAD moved — this must be Ok with a warning, never an Err");

    assert!(matches!(
        outcome.warning,
        Some(git_core::model::CommitWarning::TimedOutButHeadMoved { .. })
    ));
    let head_after = repo.info().unwrap().head.unwrap().oid;
    assert_ne!(
        head_before, head_after,
        "HEAD must actually have moved for this test to mean anything"
    );
}

#[test]
fn nonzero_exit_with_a_stderr_line_is_commit_failed() {
    // M3 row 1's shape: a signer/hook exits non-zero with an actionable
    // message, zero commits.
    let sandbox = support::init_repo_with_commit("README.md", b"root\n");
    let path = sandbox.path();
    let script = support::write_fake_git(
        path,
        "fake-git-fail",
        &fake_git_script(true, "echo 'gpg failed to sign the data' >&2\nexit 128"),
    );

    let repo = GitRepo::open(path).unwrap();
    let head_before = repo.info().unwrap().head.unwrap().oid;
    stage_one_file(&repo, path, "a.txt", b"content\n");

    let err = repo
        .commit(fake_request("should fail", &script, None))
        .unwrap_err();
    match err {
        CoreError::CommitFailed {
            exit_code, stderr, ..
        } => {
            assert_eq!(exit_code, Some(128));
            assert!(stderr.contains("gpg failed to sign the data"));
        }
        other => panic!("expected CommitFailed, got {other:?}"),
    }
    let head_after = repo.info().unwrap().head.unwrap().oid;
    assert_eq!(head_before, head_after);
}

#[test]
fn absent_override_path_is_git_unavailable() {
    let sandbox = support::init_repo_with_commit("README.md", b"root\n");
    let path = sandbox.path();
    let repo = GitRepo::open(path).unwrap();
    stage_one_file(&repo, path, "a.txt", b"content\n");

    let missing = path.join("no-such-file-here");
    let err = repo
        .commit(fake_request("should fail", &missing, None))
        .unwrap_err();
    assert!(matches!(err, CoreError::GitUnavailable { .. }));
}

#[test]
fn identity_script_failure_refuses_before_commit_is_spawned() {
    let sandbox = support::init_repo_with_commit("README.md", b"root\n");
    let path = sandbox.path();
    let sentinel = path.join("SENTINEL-commit-was-spawned");
    let script_body = format!(
        "#!/bin/sh\nif [ \"$1\" = \"var\" ]; then\n  echo 'boom' >&2\n  exit 128\nfi\ntouch \"{}\"\nexit 0\n",
        sentinel.display()
    );
    let script = support::write_fake_git(path, "fake-git-bad-identity", &script_body);

    let repo = GitRepo::open(path).unwrap();
    stage_one_file(&repo, path, "a.txt", b"content\n");

    let err = repo
        .commit(fake_request(
            "should refuse before spawning commit",
            &script,
            None,
        ))
        .unwrap_err();
    assert!(matches!(err, CoreError::IdentityMissing));
    assert!(
        !sentinel.exists(),
        "the commit subprocess must never be spawned when identity is refused"
    );
}

// ---------------------------------------------------------------------------
// 4c.7 — nothing staged refuses before any subprocess is invoked.

#[test]
fn nothing_staged_refuses_before_any_subprocess_is_invoked() {
    // spec.md "Nothing staged".
    let sandbox = support::init_repo_with_commit("README.md", b"root\n");
    let path = sandbox.path();
    let sentinel = path.join("SENTINEL-git-was-invoked");
    let script_body = format!("#!/bin/sh\ntouch \"{}\"\nexit 0\n", sentinel.display());
    let script = support::write_fake_git(path, "fake-git-sentinel", &script_body);

    let repo = GitRepo::open(path).unwrap();
    // Deliberately nothing staged.
    let err = repo
        .commit(fake_request("nothing staged", &script, None))
        .unwrap_err();
    assert!(matches!(err, CoreError::NothingStaged));
    assert!(
        !sentinel.exists(),
        "no subprocess — not even `git var` — may run when nothing is staged"
    );
}
