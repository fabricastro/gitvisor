//! Domain core for the git viewer.
//!
//! Reads a repository through libgit2 and turns its history into a layout the
//! UI can draw directly. Deliberately free of any UI or transport concern.

pub mod error;
pub mod graph;
pub mod model;
pub mod repo;

pub use error::{CoreError, Result};
pub use repo::GitRepo;
