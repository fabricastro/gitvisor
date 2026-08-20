//! The fixture graph shape, as data.
//!
//! 16 commits, chosen to exercise `git_core::graph`'s lane allocator while
//! staying inside `product_scope.in_scope` (no rebase, cherry-pick, or
//! force-push shapes):
//!
//! - a linear root run (`c1`→`c2`→`c3`→`base`), lane reuse
//! - two branches diverging from `base` and both surviving to the tip
//!   (`feature-a`, `feature-b`) — lane allocation under contention
//! - one branch that diverges from `main` and reconverges via a merge
//!   commit (`refactor/parser`, merged at `merge1`) — lane release
//! - an annotated tag on a non-tip commit (`v0.1.0` on `m1`)
//! - a remote-tracking ref ahead of its local branch (`origin/feature-a`
//!   ahead of local `feature-a`)
//!
//! One merge with three parents (octopus) is deliberately excluded — not in
//! the product's daily path, and it adds layout risk for no coverage gain.
//!
//! A "long edge" spanning more than `SHORT_EDGE_SPAN` (120 rows, see
//! `src/features/graph/layout.ts`) is *not* exercised here: it is
//! mathematically impossible in a 16-commit graph, and both `spec.md`'s
//! acceptance criteria and Spec B's assertions are pinned to exactly 16
//! commits. Recorded as a deviation from design.md §2.4's illustrative list
//! in this change's apply-progress notes.

/// One commit in the fixture, described declaratively.
pub struct CommitSpec {
    /// Stable identifier used by tests and the manifest; never a real OID.
    pub alias: &'static str,
    /// Parent aliases, in order (first parent is the "staying" branch for a merge).
    pub parents: &'static [&'static str],
    /// Path of the one file this commit adds/changes, relative to the worktree root.
    pub file: &'static str,
    /// Literal file content. LF only.
    pub content: &'static [u8],
    pub summary: &'static str,
}

pub const COMMITS: &[CommitSpec] = &[
    CommitSpec {
        alias: "c1",
        parents: &[],
        file: "README.md",
        content: b"# history fixture\n\nDeterministic repository used by the e2e harness.\n",
        summary: "root commit",
    },
    CommitSpec {
        alias: "c2",
        parents: &["c1"],
        file: "notes/c2.txt",
        content: b"commit c2\n",
        summary: "second commit on the linear run",
    },
    CommitSpec {
        alias: "c3",
        parents: &["c2"],
        file: "notes/c3.txt",
        content: b"commit c3\n",
        summary: "third commit on the linear run",
    },
    CommitSpec {
        alias: "base",
        parents: &["c3"],
        file: "notes/base.txt",
        content: b"commit base\n",
        summary: "base commit for the branch fan-out",
    },
    CommitSpec {
        alias: "m1",
        parents: &["base"],
        file: "notes/m1.txt",
        content: b"commit m1\n",
        summary: "main continues past the fan-out",
    },
    CommitSpec {
        alias: "fa1",
        parents: &["base"],
        file: "notes/fa1.txt",
        content: b"commit fa1\n",
        summary: "feature-a diverges from base",
    },
    CommitSpec {
        alias: "rp1",
        parents: &["m1"],
        file: "notes/rp1.txt",
        content: b"commit rp1\n",
        summary: "refactor/parser diverges from main",
    },
    CommitSpec {
        alias: "m2",
        parents: &["m1"],
        file: "notes/m2.txt",
        content: b"commit m2\n",
        summary: "main continues under lane contention",
    },
    CommitSpec {
        alias: "fa2",
        parents: &["fa1"],
        file: "notes/fa2.txt",
        content: b"commit fa2\n",
        summary: "feature-a continues",
    },
    CommitSpec {
        alias: "rp2",
        parents: &["rp1"],
        file: "notes/rp2.txt",
        content: b"commit rp2\n",
        summary: "refactor/parser continues",
    },
    CommitSpec {
        alias: "fb1",
        parents: &["base"],
        file: "notes/fb1.txt",
        content: b"commit fb1\n",
        summary: "feature-b diverges from base",
    },
    CommitSpec {
        alias: "merge1",
        parents: &["m2", "rp2"],
        file: "notes/merge1.txt",
        content: b"commit merge1\n",
        summary: "merge refactor/parser into main",
    },
    CommitSpec {
        alias: "m3",
        parents: &["merge1"],
        file: "notes/m3.txt",
        content: b"commit m3\n",
        summary: "main continues after the merge",
    },
    CommitSpec {
        alias: "fa3",
        parents: &["fa2"],
        file: "notes/fa3.txt",
        content: b"commit fa3\n",
        summary: "feature-a, one commit ahead of its local branch tip",
    },
    CommitSpec {
        alias: "fb2",
        parents: &["fb1"],
        file: "notes/fb2.txt",
        content: b"commit fb2\n",
        summary: "feature-b tip",
    },
    CommitSpec {
        alias: "m4",
        parents: &["m3"],
        file: "notes/m4.txt",
        content: b"commit m4\n",
        summary: "main tip",
    },
];

pub const HEAD_ALIAS: &str = "m4";

/// `(local branch name, alias it points at)`.
pub const LOCAL_BRANCHES: &[(&str, &str)] = &[
    ("main", "m4"),
    ("feature-a", "fa2"),
    ("feature-b", "fb2"),
    ("refactor/parser", "rp2"),
];

/// `(remote-tracking ref name under refs/remotes/, alias it points at)`.
///
/// `origin/feature-a` points one commit ahead of local `feature-a` (`fa3` vs
/// `fa2`) so `RefEntry.ahead`/`.behind` has something real to report.
pub const REMOTE_REFS: &[(&str, &str)] = &[("origin/main", "m4"), ("origin/feature-a", "fa3")];

/// `(annotated tag name, alias it points at)`. `m1` is not the tip of any
/// branch, so this exercises a tag drawn mid-graph.
pub const TAG: (&str, &str) = ("v0.1.0", "m1");

// ---------------------------------------------------------------------------
// The `writes` recipe (design.md §9.2, §12). Dedicated to write-path specs —
// staging, unstaging, and (later) committing — so `history` above stays
// exactly what it always was: shared, read-only, asserted byte-for-byte by
// `determinism.rs`. Three linear commits, no fan-out, no merge, no tag —
// graph-layout coverage is `history`'s job, not this one's.

pub const WRITES_COMMITS: &[CommitSpec] = &[
    CommitSpec {
        alias: "w1",
        parents: &[],
        file: "README.md",
        content: b"# writes fixture\n\nDeterministic repository used by write-path e2e specs.\n",
        summary: "root commit",
    },
    CommitSpec {
        alias: "w2",
        parents: &["w1"],
        file: "tracked-a.txt",
        content: b"tracked a, version 1\n",
        summary: "add tracked-a.txt",
    },
    CommitSpec {
        alias: "w3",
        parents: &["w2"],
        file: "tracked-b.txt",
        content: b"tracked b, version 1\n",
        summary: "add tracked-b.txt",
    },
];

pub const WRITES_HEAD_ALIAS: &str = "w3";

/// `(local branch name, alias it points at)`. `writes` has exactly one
/// branch — nothing here exercises branch fan-out.
pub const WRITES_LOCAL_BRANCHES: &[(&str, &str)] = &[("main", "w3")];
