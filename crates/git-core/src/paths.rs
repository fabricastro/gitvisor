//! Path safety for staging and unstaging (design.md §6, M4).
//!
//! M4 measured that libgit2 already refuses path escapes (`../outside.txt`,
//! `/etc/hosts`) — that safety property is inherited, not invented here. What
//! it also measured is that both refusals arrive as `GenericError`,
//! distinguishable only by English message text, which `spec.md` forbids the
//! UI from branching on. So `git-core` validates first and emits
//! `PathOutsideRepo`, a code the UI can branch on; libgit2's own refusal
//! stays as a deliberate, redundant backstop at the call site (design.md
//! §6.1) — do not delete either.

use crate::error::{CoreError, Result};

/// Normalise a caller-supplied, repository-relative path and reject anything
/// that would escape the repository once normalised.
///
/// Pure: no I/O, no repository. Rejects paths that *escape after
/// normalisation*, not paths that merely contain `..` — `sub/../inside.txt`
/// normalises to `inside.txt` and is accepted (M4's nuance: a naive
/// "contains `..`" check would refuse legitimate input).
pub fn normalise_repo_path(input: &str) -> Result<String> {
    if input.contains('\0') {
        return Err(outside(input));
    }
    if input.is_empty() {
        return Err(outside(input));
    }
    if input.starts_with('/') || input.starts_with('\\') || has_windows_drive_prefix(input) {
        return Err(outside(input));
    }

    let mut stack: Vec<&str> = Vec::new();
    for component in input.split(['/', '\\']) {
        match component {
            "" | "." => continue,
            ".." => {
                if stack.pop().is_none() {
                    return Err(outside(input));
                }
            }
            other => stack.push(other),
        }
    }

    if stack.is_empty() {
        return Err(outside(input));
    }
    if stack[0].eq_ignore_ascii_case(".git") {
        return Err(outside(input));
    }

    Ok(stack.join("/"))
}

fn has_windows_drive_prefix(input: &str) -> bool {
    let bytes = input.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn outside(input: &str) -> CoreError {
    CoreError::PathOutsideRepo {
        path: input.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // M4's four rows, replayed as a pure-function normalisation instead of
    // depending on libgit2's `GenericError` message text.
    #[test]
    fn inside_path_is_accepted() {
        assert_eq!(normalise_repo_path("inside.txt").unwrap(), "inside.txt");
    }

    #[test]
    fn parent_escape_is_refused() {
        assert!(matches!(
            normalise_repo_path("../outside.txt"),
            Err(CoreError::PathOutsideRepo { .. })
        ));
    }

    #[test]
    fn absolute_path_is_refused() {
        assert!(matches!(
            normalise_repo_path("/etc/hosts"),
            Err(CoreError::PathOutsideRepo { .. })
        ));
    }

    #[test]
    fn internal_dotdot_that_stays_inside_is_accepted() {
        // M4 recorded this row failing as `NotFound` (because `sub/` did not
        // exist on disk), not because of the `..`. The pure normaliser fixes
        // that: a `..` that stays inside the repository is legitimate.
        assert_eq!(
            normalise_repo_path("sub/../inside.txt").unwrap(),
            "inside.txt"
        );
    }

    #[test]
    fn bracketed_filename_is_accepted_literally() {
        assert_eq!(normalise_repo_path("a[b].txt").unwrap(), "a[b].txt");
    }

    #[test]
    fn dot_git_first_component_is_refused() {
        assert!(matches!(
            normalise_repo_path(".git/config"),
            Err(CoreError::PathOutsideRepo { .. })
        ));
    }

    #[test]
    fn nul_byte_is_refused() {
        assert!(matches!(
            normalise_repo_path("inside\0.txt"),
            Err(CoreError::PathOutsideRepo { .. })
        ));
    }

    #[test]
    fn empty_stack_pop_is_refused() {
        assert!(matches!(
            normalise_repo_path(".."),
            Err(CoreError::PathOutsideRepo { .. })
        ));
    }

    #[test]
    fn empty_string_is_refused() {
        assert!(matches!(
            normalise_repo_path(""),
            Err(CoreError::PathOutsideRepo { .. })
        ));
    }

    #[test]
    fn dot_alone_is_refused() {
        assert!(matches!(
            normalise_repo_path("."),
            Err(CoreError::PathOutsideRepo { .. })
        ));
    }
}
