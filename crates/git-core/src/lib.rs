//! Domain core for the git viewer.
//!
//! Reads a repository through libgit2 and turns its history into a layout the
//! UI can draw directly. Deliberately free of any UI or transport concern.

// The index-freshness invariant (M2 / design.md §1) and the path/unstage
// safety invariants (design.md §6, §8) are enforced by `clippy.toml`'s
// `disallowed-methods`, denied here so the check travels with the crate and
// fires in a contributor's editor before CI. `clippy::` tool lints are
// accepted and ignored by plain `rustc`, so this does not affect `cargo
// build`. Sanctioned exemptions are each an `#[allow]` with a comment, in
// `repo/index_guard.rs`.
#![deny(clippy::disallowed_methods)]

pub mod error;
pub mod git_binary;
pub mod graph;
pub mod model;
pub mod paths;
pub mod repo;

pub use error::{CoreError, Result};
pub use repo::GitRepo;
