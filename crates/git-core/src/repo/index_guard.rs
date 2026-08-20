//! The only way a write path obtains an `Index`.
//!
//! M2 (`measurements.md`): a `Repository` held open by `RepoRegistry` returns
//! a STALE index after an external `git add`. Mutating that index and writing
//! it back DESTROYS the user's staging. `index.read(true)` below is the fix,
//! and this module exists so no future write method has to remember it.
//!
//! DO NOT delete this indirection. DO NOT add a variant that returns the
//! `Index` to the caller. See `design.md` §1.
//!
//! This is a private submodule of `repo`, not a sibling of it — `GitRepo`'s
//! `inner` field is private to the `repo` module tree, so only `repo` itself
//! and this submodule can reach it. Nothing outside `crates/git-core::repo`
//! can obtain an `Index` at all (design.md §1.2).

use git2::{Index, Repository};

use crate::error::Result;

use super::GitRepo;

impl GitRepo {
    /// The only way a write path obtains an index.
    ///
    /// `read(true)` is hard, not soft: a soft reload compares mtime/size, and
    /// mtime granularity can hide a same-second external write. `write()`
    /// runs only on the success path, so a closure that returns `Err` leaves
    /// the on-disk index byte-identical to before the call — "refuse before
    /// mutating anything" is therefore structural, not a discipline every
    /// write method has to maintain (design.md §1.5).
    #[allow(clippy::disallowed_methods)] // the one sanctioned Repository::index() call site
    pub(crate) fn with_fresh_index<T>(
        &self,
        mutate: impl FnOnce(&Repository, &mut Index) -> Result<T>,
    ) -> Result<T> {
        let mut index = self.inner.index()?;
        index.read(true)?; // M2
        let out = mutate(&self.inner, &mut index)?;
        index.write()?; // success path only
        Ok(out)
    }

    /// Force the repository's own index view back in sync with disk, without
    /// writing. Used before recomputing `status()` after a write, so the
    /// returned listing never depends on libgit2's mtime-based soft reload.
    #[allow(clippy::disallowed_methods)] // the other sanctioned Repository::index() call site
    pub(crate) fn reload_index(&self) -> Result<()> {
        let mut index = self.inner.index()?;
        index.read(true)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::error::CoreError;
    use crate::GitRepo;

    /// Task 2.6: a closure that returns `Err` mid-mutation leaves the on-disk
    /// index byte-identical to before the call. Proves §1.5's "refuse before
    /// mutating anything" is structural (write() is on the success path
    /// only), not a discipline every write method has to remember.
    #[test]
    fn err_from_closure_leaves_on_disk_index_untouched() {
        let dir = tempfile::tempdir().expect("create temp dir");
        #[allow(clippy::disallowed_methods)] // test scaffolding, not a write path
        let raw = git2::Repository::init(dir.path()).expect("git2 init");
        drop(raw);

        std::fs::write(dir.path().join("a.txt"), b"hello\n").expect("write a.txt");
        let git_repo = GitRepo::open(dir.path()).expect("open");
        git_repo
            .stage(&["a.txt".to_string()])
            .expect("real stage to produce a real on-disk index");

        let index_path = dir.path().join(".git/index");
        let before = std::fs::read(&index_path).expect("read index before");

        let result = git_repo
            .with_fresh_index(|_repo, _index| Err::<(), _>(CoreError::Invalid("boom".to_string())));
        assert!(result.is_err());

        let after = std::fs::read(&index_path).expect("read index after");
        assert_eq!(before, after, "on-disk index changed despite a closure Err");
    }

    /// Task 2.5 — the M2 replay, against a real external `git` subprocess.
    ///
    /// This targets `with_fresh_index` directly rather than going through
    /// `GitRepo::stage`. That distinction is load-bearing, not stylistic:
    /// `stage`'s own pre-flight calls `status()` before ever reaching
    /// `with_fresh_index`, and `Repository::statuses()` was found (while
    /// writing this test) to have an incidental side effect of soft-syncing
    /// libgit2's cached index — which would mask a *missing* `read(true)`
    /// here and make this test worthless. `read(true)` is a HARD reload
    /// specifically because a soft one is mtime-based and can miss a
    /// same-tick external write (measurements.md M2); relying on `status()`
    /// happening to run first everywhere a write occurs is exactly the
    /// "discipline instead of structure" this module exists to remove. This
    /// test isolates the one thing that must actually hold: `with_fresh_index`
    /// itself does not lose an external write, independent of any caller.
    ///
    /// Verified by hand while writing this test: temporarily deleting the
    /// `index.read(true)?;` line above turned this test red (the on-disk
    /// index ended up missing `b.txt` — the external `git add` was silently
    /// destroyed), and restoring the line turned it green again.
    #[test]
    fn m2_external_git_add_survives_with_fresh_index() {
        let dir = tempfile::tempdir().expect("create temp dir");
        #[allow(clippy::disallowed_methods)] // test scaffolding, not a write path
        let raw = git2::Repository::init(dir.path()).expect("git2 init");
        {
            let mut config = raw.config().expect("config");
            config.set_str("user.name", "Test User").unwrap();
            config.set_str("user.email", "test@example.com").unwrap();
        }
        drop(raw);

        let git_repo = GitRepo::open(dir.path()).expect("open");

        // First call: warms libgit2's cached index handle on this
        // Repository — the same shape `RepoRegistry` produces by holding a
        // `GitRepo` open across commands.
        std::fs::write(dir.path().join("a.txt"), b"a\n").expect("write a.txt");
        git_repo
            .with_fresh_index(|_repo, index| {
                index
                    .add_path(std::path::Path::new("a.txt"))
                    .map_err(CoreError::from)
            })
            .expect("first with_fresh_index call");

        // A real external `git add`, in a real subprocess — an in-process
        // `index.add_path` would share libgit2's in-memory state and would
        // not reproduce the measured condition.
        std::fs::write(dir.path().join("b.txt"), b"b\n").expect("write b.txt");
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["add", "b.txt"])
            .env("GIT_AUTHOR_NAME", "Test User")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .status()
            .expect("spawn real `git` subprocess — is `git` on PATH?");
        assert!(status.success(), "external `git add b.txt` failed");

        // Second call on the SAME `GitRepo`, immediately: nothing runs in
        // between that could refresh the cache as a side effect.
        std::fs::write(dir.path().join("c.txt"), b"c\n").expect("write c.txt");
        git_repo
            .with_fresh_index(|_repo, index| {
                index
                    .add_path(std::path::Path::new("c.txt"))
                    .map_err(CoreError::from)
            })
            .expect("second with_fresh_index call");

        #[allow(clippy::disallowed_methods)] // test assertion, not a write path
        let mut check_index = git2::Repository::open(dir.path())
            .expect("reopen")
            .index()
            .expect("index");
        check_index.read(true).expect("read index for assertion");
        let paths: Vec<String> = check_index
            .iter()
            .map(|entry| String::from_utf8_lossy(&entry.path).to_string())
            .collect();

        assert!(paths.contains(&"a.txt".to_string()));
        assert!(
            paths.contains(&"b.txt".to_string()),
            "external `git add b.txt` was destroyed by the second with_fresh_index call: {paths:?}"
        );
        assert!(paths.contains(&"c.txt".to_string()));
    }
}
