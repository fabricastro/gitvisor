//! Keeps opened repositories alive between commands.
//!
//! `git2::Repository` is `Send` but not `Sync`, so the registry is guarded by a
//! mutex. Reopening on every call would also work, but caching keeps the object
//! database warm, which matters when the UI polls the working-directory status.

use std::collections::HashMap;
use std::sync::Mutex;

use git_core::{GitRepo, Result};

#[derive(Default)]
pub struct RepoRegistry {
    open: Mutex<HashMap<String, GitRepo>>,
}

impl RepoRegistry {
    /// Run `action` against the repository at `path`, opening it on first use.
    pub fn with<T>(&self, path: &str, action: impl FnOnce(&GitRepo) -> Result<T>) -> Result<T> {
        let mut open = self
            .open
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !open.contains_key(path) {
            open.insert(path.to_string(), GitRepo::open(path)?);
        }
        action(&open[path])
    }

    /// Drop a cached repository, e.g. when its tab is closed.
    pub fn close(&self, path: &str) {
        self.open
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(path);
    }
}
