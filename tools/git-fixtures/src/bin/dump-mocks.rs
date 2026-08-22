//! Generate browser-mode `invoke()` mocks from the deterministic fixture.
//!
//! ```text
//! cargo run -p git-fixtures --bin dump-mocks -- \
//!   --repo target/e2e-fixtures/history --out e2e/mocks/history.json
//! ```
//!
//! Reads the fixture through `git_core::GitRepo` — the *same* type
//! `src-tauri/src/commands.rs` calls — and serialises the *same* model
//! structs from `crates/git-core/src/model.rs`, keyed by Tauri command name.
//! There is no second serialization surface to keep in sync; the compiler
//! owns the shape (design.md §4.1).
//!
//! `crates/git-core/examples/dump.rs` is a separate, untouched, documented
//! command (`openspec/config.yaml`'s `reference_commands`) — deliberately
//! not reused here. See design.md §4.1 for why.

use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;

use git_core::repo::GitRepo;
use serde::Serialize;
use serde_json::Value;

/// Replaces `RepoInfo.path`, which is an absolute path that differs per
/// machine. `e2e/support/mocks.ts` substitutes a browser-safe value on load
/// (design.md §4.1) — a raw dump would make the CI diff fail on every run.
const FIXTURE_PATH_TOKEN: &str = "{{FIXTURE_PATH}}";

/// `git_probe`'s `path`/`version` are read from whatever `git` the machine
/// building the mocks actually has (design.md §9.4) — both machine-specific
/// and would make the CI diff fail on every runner with a different `git`
/// installed. Tokenised the same way `FIXTURE_PATH_TOKEN` is.
const GIT_PATH_TOKEN: &str = "{{GIT_PATH}}";
const GIT_VERSION_TOKEN: &str = "{{GIT_VERSION}}";

/// Mocks keyed by Tauri command name (`src-tauri/src/commands.rs`). Plain
/// field names, not `camelCase` — the outer keys are literal `invoke()`
/// command strings the frontend passes, not model data.
#[derive(Serialize)]
struct Mocks {
    startup_path: String,
    open_repository: Value,
    commit_graph: Value,
    list_refs: Value,
    working_status: Value,
    /// Keyed by commit OID — `open_repository` returns one repo, but a user
    /// can select any commit, so this is a map, not a single payload.
    commit_detail: BTreeMap<String, Value>,
    /// `close_repository` returns nothing; recorded anyway so every command
    /// `commands.rs` exposes has a mock entry.
    close_repository: Value,
    /// `git` resolution is machine-specific and never executes a write —
    /// this is the one entry `dump-mocks` produces by *reading* the building
    /// machine's own `git`, never by mutating a repository (design.md §9.4).
    /// `stage_paths`/`unstage_paths`/`create_commit` are deliberately absent:
    /// their payloads are write outcomes, derived per-spec from
    /// `working_status`, never generated here.
    git_probe: Value,
}

struct Args {
    repo: PathBuf,
    out: PathBuf,
}

fn parse_args() -> Args {
    let mut repo = PathBuf::from("target/e2e-fixtures/history");
    let mut out = PathBuf::from("e2e/mocks/history.json");

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--repo" => {
                repo = args
                    .next()
                    .map(PathBuf::from)
                    .expect("--repo needs a value")
            }
            "--out" => out = args.next().map(PathBuf::from).expect("--out needs a value"),
            other => panic!("unknown argument: {other}"),
        }
    }

    Args { repo, out }
}

fn main() {
    let args = parse_args();

    let repo = GitRepo::open(&args.repo).unwrap_or_else(|error| {
        panic!(
            "failed to open fixture at {}: {error} — run `cargo run -p git-fixtures --bin build-fixture` first",
            args.repo.display()
        )
    });

    let mut open_repository =
        serde_json::to_value(repo.info().expect("repo info")).expect("serialize RepoInfo");
    let real_path = open_repository["path"]
        .as_str()
        .expect("RepoInfo.path is a string")
        .to_string();
    open_repository["path"] = Value::String(FIXTURE_PATH_TOKEN.to_string());

    let graph = repo.graph(None).expect("commit graph");
    let list_refs = repo.refs().expect("refs");
    let working_status = repo.status().expect("working status");

    let mut git_probe =
        serde_json::to_value(git_core::git_binary::probe(None)).expect("serialize GitProbe");
    if let Some(path) = git_probe.get_mut("path") {
        if !path.is_null() {
            *path = Value::String(GIT_PATH_TOKEN.to_string());
        }
    }
    if let Some(version) = git_probe.get_mut("version") {
        if !version.is_null() {
            *version = Value::String(GIT_VERSION_TOKEN.to_string());
        }
    }

    let commit_detail: BTreeMap<String, Value> = graph
        .rows
        .iter()
        .map(|row| {
            let detail = repo
                .commit_detail(&row.commit.id)
                .expect("commit detail for a row in the row's own graph");
            (
                row.commit.id.clone(),
                serde_json::to_value(detail).expect("serialize CommitDetail"),
            )
        })
        .collect();

    let mocks = Mocks {
        startup_path: FIXTURE_PATH_TOKEN.to_string(),
        open_repository,
        commit_graph: serde_json::to_value(&graph).expect("serialize Graph"),
        list_refs: serde_json::to_value(&list_refs).expect("serialize Vec<RefEntry>"),
        working_status: serde_json::to_value(&working_status).expect("serialize WorkingStatus"),
        commit_detail,
        close_repository: Value::Null,
        git_probe,
    };

    if let Some(parent) = args.out.parent() {
        std::fs::create_dir_all(parent).expect("create mocks output directory");
    }
    let json = serde_json::to_string_pretty(&mocks).expect("serialize mocks");
    std::fs::write(&args.out, json).expect("write mocks file");

    println!(
        "mocks written to {} (real fixture path was {})",
        args.out.display(),
        real_path
    );
}
