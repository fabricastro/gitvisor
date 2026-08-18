//! Asserts the fixture builder produces byte-identical OIDs, checked against
//! hardcoded constants in `src/oids.rs` — not just `HEAD`, but the full
//! alias→OID map, so a drift points at *which* commit changed.
//!
//! Ambient config is made unreadable for the duration of this process via
//! `git2::opts::set_search_path`, pointed at an empty scratch directory, so
//! a developer's local `~/.gitconfig` (or CI's) cannot participate even by
//! accident. This is process-global and `unsafe`; `design.md` §2.2 marks it
//! optional for the `build-fixture` binary and recommended for this test.

use std::path::PathBuf;
use std::sync::Once;

use git_fixtures::{build, oids};

static ISOLATE_AMBIENT_CONFIG: Once = Once::new();

/// `git2::opts::set_search_path` mutates process-global libgit2 state and is
/// not safe to call concurrently from multiple test threads (observed:
/// `SIGABRT` under the default parallel test runner). `Once` makes the
/// redirect happen exactly one time no matter how many tests in this binary
/// call this function.
fn isolate_ambient_config() {
    ISOLATE_AMBIENT_CONFIG.call_once(|| {
        let empty = std::env::temp_dir().join("git-fixtures-empty-config");
        std::fs::create_dir_all(&empty).expect("create empty config scratch dir");
        for level in [
            git2::ConfigLevel::System,
            git2::ConfigLevel::Global,
            git2::ConfigLevel::XDG,
            git2::ConfigLevel::ProgramData,
        ] {
            // SAFETY: called at most once per process (via `Once`), before
            // any repository is opened by this test binary, and only
            // redirects libgit2's config search paths — it does not touch
            // any file outside `empty`.
            unsafe {
                git2::opts::set_search_path(level, &empty).expect("redirect config search path");
            }
        }
    });
}

#[test]
fn fixture_oids_are_deterministic() {
    isolate_ambient_config();

    // A scratch directory under `target/`, distinct from the runtime output
    // path (`target/e2e-fixtures/<name>/`) that `build-fixture` writes to.
    let out_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("determinism-fixture");
    let result = build(&out_dir).expect("fixture build succeeds");

    let mut mismatches = Vec::new();

    assert_eq!(
        result.commit_oids.len(),
        oids::COMMIT_OIDS.len(),
        "commit count drifted: fixture built {} commits, oids.rs expects {}",
        result.commit_oids.len(),
        oids::COMMIT_OIDS.len()
    );

    for (alias, oid) in &result.commit_oids {
        let expected = oids::commit_oid(alias);
        let actual = oid.to_string();
        if expected != actual {
            mismatches.push(format!(
                "commit `{alias}`: expected {expected}, got {actual}"
            ));
        }
    }

    let actual_tag = result.tag_oid.to_string();
    if oids::TAG_V0_1_0 != actual_tag {
        mismatches.push(format!(
            "tag `v0.1.0`: expected {}, got {actual_tag}",
            oids::TAG_V0_1_0
        ));
    }

    let actual_head_tree = result.head_tree_oid.to_string();
    if oids::HEAD_TREE != actual_head_tree {
        mismatches.push(format!(
            "HEAD tree: expected {}, got {actual_head_tree}",
            oids::HEAD_TREE
        ));
    }

    assert!(
        mismatches.is_empty(),
        "fixture OIDs drifted from src/oids.rs:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn ambient_state_cannot_leak_in() {
    // Two independent builds, in two independent directories, must agree —
    // proving the result does not depend on process-local mutable state
    // (e.g. an accidental `set_search_path` ordering bug).
    isolate_ambient_config();
    let first = build(&PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ambient-a"))
        .expect("first build succeeds");
    let second = build(&PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ambient-b"))
        .expect("second build succeeds");

    assert_eq!(first.head_oid, second.head_oid);
    assert_eq!(first.head_tree_oid, second.head_tree_oid);
    assert_eq!(first.tag_oid, second.tag_oid);
    assert_eq!(first.commit_oids, second.commit_oids);
}
