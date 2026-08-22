//! Locating and running the user's own `git` binary (design.md §2, §3, §4).
//!
//! Staging and unstaging are libgit2, in-process. Committing is not: it runs
//! through a plain `std::process::Command` spawn of the real `git` the user
//! has installed, because "a commit in Gitvisor means what a commit in your
//! terminal means" is a product rule, not an implementation detail — hooks,
//! signing, and identity precedence must be `git`'s own, never re-derived
//! here (M1, M5).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::error::{CoreError, Result};
use crate::model::GitProbe;

/// Environment variables removed from every `git` invocation this module
/// makes (design.md §2.2). Inherited, they would commit into a different
/// repository or index than the one on screen — a work-destruction path a
/// stray export, a hook, or `git rebase --exec` could otherwise leave behind.
const REMOVED_ENV_VARS: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_NAMESPACE",
];

/// Default commit timeout: 120 s, long enough for a real `pre-commit` hook
/// running a test suite, short enough that a genuine hang surfaces while the
/// user is still watching (design.md §3.3). Overridable for tests and for a
/// user who knows their hooks run long.
pub fn default_commit_timeout() -> Duration {
    let secs = std::env::var("GITVISOR_COMMIT_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(120);
    Duration::from_secs(secs)
}

/// A `git` binary this module has picked, but not yet validated (design.md
/// §4.1). Resolution and validation are deliberately separate: `resolve()` is
/// evaluated at every invocation and is never cached, since installing or
/// uninstalling `git` while Gitvisor is open must take effect immediately.
pub struct ResolvedGit {
    pub path: PathBuf,
}

/// Pick a `git` candidate: explicit override → `GITVISOR_GIT_PATH` → `PATH`
/// (via the `which` crate, for correct `PATHEXT`/`.exe` handling on Windows —
/// hand-rolling that is the part of this module nobody can test here, U8).
/// Never cached: no `OnceLock`, no field on `GitRepo` (design.md §4.1).
pub fn resolve(override_path: Option<&str>) -> Result<ResolvedGit> {
    if let Some(path) = override_path.filter(|p| !p.is_empty()) {
        return Ok(ResolvedGit {
            path: PathBuf::from(path),
        });
    }
    if let Ok(path) = std::env::var("GITVISOR_GIT_PATH") {
        if !path.is_empty() {
            return Ok(ResolvedGit {
                path: PathBuf::from(path),
            });
        }
    }
    if let Ok(path) = which::which("git") {
        return Ok(ResolvedGit { path });
    }
    Err(CoreError::GitUnavailable {
        looked_for: "git (explicit override, GITVISOR_GIT_PATH, then PATH) — none resolved"
            .to_string(),
    })
}

/// Resolve, then validate: exists, is a file (or a symlink to one), the
/// executable bit is set on Unix, and `<candidate> --version` exits `0` with
/// stdout beginning `git version ` (design.md §4.3). This is the check that
/// rejects an override pointed at something that is not `git` at all.
pub fn probe(override_path: Option<&str>) -> GitProbe {
    let Ok(resolved) = resolve(override_path) else {
        return GitProbe {
            available: false,
            path: None,
            version: None,
        };
    };
    let path_string = resolved.path.display().to_string();
    match validate(&resolved.path) {
        Ok(version) => GitProbe {
            available: true,
            path: Some(path_string),
            version: Some(version),
        },
        Err(_) => GitProbe {
            available: false,
            path: Some(path_string),
            version: None,
        },
    }
}

fn validate(candidate: &Path) -> std::result::Result<String, ()> {
    let symlink_meta = std::fs::symlink_metadata(candidate).map_err(|_| ())?;
    let is_file = if symlink_meta.file_type().is_symlink() {
        std::fs::metadata(candidate)
            .map(|m| m.is_file())
            .unwrap_or(false)
    } else {
        symlink_meta.is_file()
    };
    if !is_file {
        return Err(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(candidate)
            .map_err(|_| ())?
            .permissions()
            .mode();
        if mode & 0o111 == 0 {
            return Err(());
        }
    }
    let output = Command::new(candidate)
        .arg("--version")
        .output()
        .map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.starts_with("git version ") {
        return Err(());
    }
    Ok(stdout.trim().to_string())
}

/// The one shared command builder for both the identity pre-flight and the
/// commit spawn (design.md §5.1) — their env and cwd must never drift apart,
/// or the pre-flight answers a different question than the commit asks.
fn base_command(git_path: &Path, workdir: &Path) -> Command {
    let mut cmd = Command::new(git_path);
    cmd.current_dir(workdir);
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("GIT_EDITOR", ":");
    cmd.env("GIT_SEQUENCE_EDITOR", ":");
    for var in REMOVED_ENV_VARS {
        cmd.env_remove(var);
    }
    // `GIT_AUTHOR_*` / `GIT_COMMITTER_*` are deliberately left untouched — M5.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // A new process group, so the timeout ladder's group signal (§3.3)
        // reaches a blocked `gpg`/`pinentry` grandchild too, not just `git`.
        cmd.process_group(0);
    }
    cmd
}

/// `git var GIT_AUTHOR_IDENT`, through the identical builder the commit will
/// use. U10 (resolved 2026-08-22, design.md §5.1): parity was confirmed in
/// every case tested, so this is a hard refusal, not reporting-only.
pub(crate) fn identity(git_path: &Path, workdir: &Path) -> Result<()> {
    let mut cmd = base_command(git_path, workdir);
    cmd.arg("var")
        .arg("GIT_AUTHOR_IDENT")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = cmd.output().map_err(|_| CoreError::GitUnavailable {
        looked_for: git_path.display().to_string(),
    })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(CoreError::IdentityMissing)
    }
}

/// The outcome of one commit attempt: what the subprocess itself reported.
/// The caller (`repo::commit`) turns this — together with an independently
/// observed HEAD delta — into a `CommitOutcome` or a `CoreError` (design.md
/// §2.5). This struct never decides success on its own; exit code and HEAD
/// movement are combined by the caller, never here.
pub(crate) struct CommitAttempt {
    /// `None` when the process was killed by the timeout ladder, or died by
    /// signal for any other reason — `ExitStatus::code()` is `None` in both
    /// cases on Unix, and the caller's outcome table treats them the same
    /// way: HEAD is the only thing that decides what happened.
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Exact argv per design.md §2.1: `<git> -C <workdir> --no-pager commit
/// --file=- --cleanup=whitespace`. Message on stdin, then the handle is
/// dropped for EOF. Never `--no-verify`, never `-a`, never a shell string.
pub(crate) fn run_commit(
    git_path: &Path,
    workdir: &Path,
    message: &str,
    timeout: Duration,
) -> Result<CommitAttempt> {
    let mut cmd = base_command(git_path, workdir);
    cmd.arg("-C")
        .arg(workdir)
        .arg("--no-pager")
        .arg("commit")
        .arg("--file=-")
        .arg("--cleanup=whitespace")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|_| CoreError::GitUnavailable {
        looked_for: git_path.display().to_string(),
    })?;

    // Reader threads start immediately after spawn, before we write to
    // stdin — a hook that writes past the pipe buffer before consuming all
    // of stdin must not deadlock us against our own plumbing (design.md
    // §2.3).
    let mut stdout_pipe = child.stdout.take().expect("stdout is piped");
    let mut stderr_pipe = child.stderr.take().expect("stderr is piped");
    let stdout_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut buf);
        buf
    });
    let stderr_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buf);
        buf
    });

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(message.as_bytes());
        // `stdin` drops at the end of this block, closing the fd — EOF.
    }

    let deadline = Instant::now() + timeout;
    let mut status = poll_until(&mut child, deadline);

    if status.is_none() {
        // Timeout: SIGTERM to the child's own process group, a 5 s grace
        // period, then SIGKILL — the ladder M3 actually measured (SIGTERM),
        // never a substituted, unmeasured signal (design.md §3.3).
        escalate(&mut child, Signal::Term);
        let grace_deadline = Instant::now() + Duration::from_secs(5);
        status = poll_until(&mut child, grace_deadline);
        if status.is_none() {
            escalate(&mut child, Signal::Kill);
            status = child.wait().ok();
        }
    }

    let stdout_bytes = stdout_thread.join().unwrap_or_default();
    let stderr_bytes = stderr_thread.join().unwrap_or_default();

    Ok(CommitAttempt {
        exit_code: status.and_then(|s| s.code()),
        stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
        stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
    })
}

/// `try_wait()` every 50 ms against `deadline` — never `wait_with_output()`,
/// which blocks and consumes the `Child` (design.md §2.3, §3.3).
fn poll_until(
    child: &mut std::process::Child,
    deadline: Instant,
) -> Option<std::process::ExitStatus> {
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return Some(status);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

enum Signal {
    Term,
    Kill,
}

/// Send to the child's own process group (`-pid`), not just the child, so a
/// blocked `gpg`/`pinentry` grandchild receives it too (design.md §3.3, U2).
/// On non-Unix, `Child::kill()` is the only primitive `std` offers (U8) —
/// there is no SIGTERM equivalent to send first, so both rungs collapse to
/// one call. That is a stated, known Windows gap (this change does not claim
/// Windows support), not a silent no-op: the process is still killed.
#[cfg_attr(not(unix), allow(unused_variables))]
fn escalate(child: &mut std::process::Child, signal: Signal) {
    #[cfg(unix)]
    {
        let pid = child.id();
        let sig = match signal {
            Signal::Term => libc::SIGTERM,
            Signal::Kill => libc::SIGKILL,
        };
        // Safety: `kill(2)` with a negative pid targets the process group;
        // `pid` came from `Child::id()` and `-(pid as i32)` cannot overflow
        // for any real process id.
        unsafe {
            libc::kill(-(pid as i32), sig);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
}
