//! Rebuild a named fixture recipe and write its `fixture.json` manifest.
//!
//! ```text
//! cargo run -p git-fixtures --bin build-fixture -- [--out-root <dir>] [--name <recipe>]
//! ```
//!
//! `--out-root` defaults to `target/e2e-fixtures` (unchanged); `--name`
//! selects a **recipe** (`history` or `writes`) and defaults to `history`
//! (unchanged) — so existing callers (`package.json`'s `e2e:mocks`,
//! `wdio.native.conf.ts`'s `onPrepare`) need no argument changes
//! (design.md §9.1). The fixture lands at `<out-root>/<name>/`. Already
//! covered by the repo's `.gitignore` `target/` entry, and removed + rebuilt
//! on every run — determinism makes that free.

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use git_core::repo::GitRepo;
use git_fixtures::{build, build_writes, spec, BuildResult};
use serde::Serialize;

/// The manifest is the single Rust↔TypeScript seam (design.md §2.4): no OID,
/// branch name, or row count is hardcoded in a `.spec.ts` file.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    name: String,
    path: String,
    head_oid: String,
    commit_count: usize,
    /// From git-core's real layout, not guessed.
    lane_count: u32,
    /// Mirrored from `src/features/graph/layout.ts`'s `ROW_HEIGHT`.
    row_height: u32,
    /// The commit alias real-graph row 0 belongs to — for Spec B's probe
    /// coordinate (design.md §5), so the lane it reads is never guessed.
    row0_alias: String,
    row0_lane: u32,
    commits: Vec<CommitManifest>,
    branches: Vec<String>,
    remotes: Vec<String>,
    tags: Vec<String>,
    /// The fixture's own `repo.status()`, serialised — every fixture gets
    /// this (design.md §9.4, tasks.md 6.3), so a write spec reads the path
    /// to stage from the manifest instead of hardcoding it.
    initial_status: git_core::model::WorkingStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommitManifest {
    alias: String,
    oid: String,
    short_id: String,
    summary: String,
    lane: u32,
}

struct Args {
    out_root: PathBuf,
    name: String,
}

fn parse_args() -> Args {
    let mut out_root = PathBuf::from("target/e2e-fixtures");
    let mut name = "history".to_string();

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out-root" => {
                out_root = args
                    .next()
                    .map(PathBuf::from)
                    .expect("--out-root needs a value")
            }
            "--name" => name = args.next().expect("--name needs a value"),
            other => panic!("unknown argument: {other}"),
        }
    }

    Args { out_root, name }
}

fn main() {
    let args = parse_args();
    let out_dir = args.out_root.join(&args.name);

    let (result, commit_specs, branches, remotes, tags): (
        BuildResult,
        &[spec::CommitSpec],
        Vec<String>,
        Vec<String>,
        Vec<String>,
    ) = match args.name.as_str() {
        "history" => {
            let result = build(&out_dir).expect("fixture build failed");
            (
                result,
                spec::COMMITS,
                spec::LOCAL_BRANCHES
                    .iter()
                    .map(|(name, _)| name.to_string())
                    .collect(),
                spec::REMOTE_REFS
                    .iter()
                    .map(|(name, _)| name.to_string())
                    .collect(),
                vec![spec::TAG.0.to_string()],
            )
        }
        "writes" => {
            let result = build_writes(&out_dir).expect("fixture build failed");
            (
                result,
                spec::WRITES_COMMITS,
                spec::WRITES_LOCAL_BRANCHES
                    .iter()
                    .map(|(name, _)| name.to_string())
                    .collect(),
                vec![],
                vec![],
            )
        }
        other => panic!("unknown fixture recipe \"{other}\" (known: history, writes)"),
    };

    // Real lane layout, computed by the same algorithm the product uses —
    // not re-derived or guessed here.
    let repo = GitRepo::open(&out_dir).expect("open the freshly built fixture");
    let graph = repo.graph(None).expect("walk the fixture graph");
    let lane_of: HashMap<String, u32> = graph
        .rows
        .iter()
        .map(|row| (row.commit.id.clone(), row.lane))
        .collect();

    let row0 = graph
        .rows
        .first()
        .expect("fixture graph has at least one row");
    let row0_alias = result
        .commit_oids
        .iter()
        .find(|(_, oid)| oid.to_string() == row0.commit.id)
        .map(|(alias, _)| alias.clone())
        .expect("row 0's commit is one of the fixture's own commits");
    let row0_lane = row0.lane;

    let commits = result
        .commit_oids
        .iter()
        .map(|(alias, oid)| {
            let id = oid.to_string();
            let summary = commit_specs
                .iter()
                .find(|c| c.alias == alias)
                .map(|c| c.summary.to_string())
                .unwrap_or_default();
            CommitManifest {
                short_id: id[..id.len().min(7)].to_string(),
                lane: lane_of.get(&id).copied().unwrap_or(0),
                alias: alias.clone(),
                oid: id,
                summary,
            }
        })
        .collect();

    let initial_status = repo.status().expect("initial working status");

    let manifest = Manifest {
        name: args.name.clone(),
        path: out_dir
            .canonicalize()
            .expect("canonicalize fixture path")
            .to_string_lossy()
            .to_string(),
        head_oid: result.head_oid.to_string(),
        commit_count: result.commit_oids.len(),
        lane_count: graph.lane_count,
        row_height: 28,
        row0_alias,
        row0_lane,
        commits,
        branches,
        remotes,
        tags,
        initial_status,
    };

    let manifest_path = out_dir.join("fixture.json");
    let json = serde_json::to_string_pretty(&manifest).expect("serialize fixture manifest");
    std::fs::write(&manifest_path, json).expect("write fixture manifest");

    println!("fixture \"{}\" built at {}", args.name, out_dir.display());
    println!("manifest: {}", manifest_path.display());
}
