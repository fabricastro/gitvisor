//! Deterministic git repository builder for the e2e verification harness.
//!
//! History is built without an index and without a worktree: every commit,
//! signature, tree and blob is a git2 object created from literal data, so
//! nothing ambient (system clock, `user.name`, `init.defaultBranch`,
//! `core.autocrlf`) can leak into the resulting OIDs. Working-directory dirt
//! (one staged file, one unstaged modification) is written *after* history
//! construction and a real checkout, and is excluded from the determinism
//! test's subject — see `determinism.rs`.

pub mod oids;
pub mod spec;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use git2::build::CheckoutBuilder;
use git2::{ObjectType, Oid, Repository, RepositoryInitOptions, Signature, Time};

/// Literal epoch. Every commit's timestamp is `EPOCH + index * 60`, offset `0`
/// (UTC) — never `Signature::now()`, which would read the system clock.
pub const EPOCH: i64 = 1_700_000_000;

pub const AUTHOR_NAME: &str = "Fixture Author";
pub const AUTHOR_EMAIL: &str = "fixture-author@example.com";
pub const COMMITTER_NAME: &str = "Fixture Committer";
pub const COMMITTER_EMAIL: &str = "fixture-committer@example.com";
pub const TAGGER_NAME: &str = "Fixture Tagger";
pub const TAGGER_EMAIL: &str = "fixture-tagger@example.com";

/// Everything a caller needs after a successful build: the real OIDs, not
/// re-derived from the manifest that gets written from them.
pub struct BuildResult {
    pub path: PathBuf,
    pub head_oid: Oid,
    pub head_tree_oid: Oid,
    pub tag_oid: Oid,
    /// `(alias, commit OID)`, in the same order as `spec::COMMITS`.
    pub commit_oids: Vec<(String, Oid)>,
}

/// Build the fixture repository at `out_dir`, removing anything already there.
///
/// `out_dir` must not be a path any other test or process depends on
/// concurrently — the directory is deleted and rebuilt on every call.
pub fn build(out_dir: &Path) -> Result<BuildResult, git2::Error> {
    if out_dir.exists() {
        std::fs::remove_dir_all(out_dir).expect("remove stale fixture directory");
    }
    std::fs::create_dir_all(out_dir).expect("create fixture directory");

    let mut init_opts = RepositoryInitOptions::new();
    init_opts.initial_head("main");
    let repo = Repository::init_opts(out_dir, &init_opts)?;

    let mut oids: HashMap<&'static str, Oid> = HashMap::new();
    // Cumulative file table per alias: union of every parent's files, plus
    // this commit's own file. Gives every commit a real, non-trivial tree
    // diff without hand-maintaining full trees.
    let mut files: HashMap<&'static str, BTreeMap<String, Oid>> = HashMap::new();
    let mut order: Vec<(String, Oid)> = Vec::with_capacity(spec::COMMITS.len());

    for (index, commit_spec) in spec::COMMITS.iter().enumerate() {
        let when = Time::new(EPOCH + (index as i64) * 60, 0);
        let author = Signature::new(AUTHOR_NAME, AUTHOR_EMAIL, &when)?;
        let committer = Signature::new(COMMITTER_NAME, COMMITTER_EMAIL, &when)?;

        let mut file_table = BTreeMap::new();
        for parent_alias in commit_spec.parents {
            if let Some(parent_files) = files.get(parent_alias) {
                file_table.extend(parent_files.iter().map(|(p, o)| (p.clone(), *o)));
            }
        }
        let blob_oid = repo.blob(commit_spec.content)?;
        file_table.insert(commit_spec.file.to_string(), blob_oid);

        let tree_oid = build_tree(&repo, &file_table)?;
        let tree = repo.find_tree(tree_oid)?;

        let parent_commits: Vec<git2::Commit<'_>> = commit_spec
            .parents
            .iter()
            .map(|alias| repo.find_commit(oids[alias]))
            .collect::<Result<_, _>>()?;
        let parent_refs: Vec<&git2::Commit<'_>> = parent_commits.iter().collect();

        // `update_ref: None` — branch refs are written explicitly below, once
        // every commit exists, rather than incrementally during the walk.
        let commit_oid = repo.commit(
            None,
            &author,
            &committer,
            commit_spec.summary,
            &tree,
            &parent_refs,
        )?;

        oids.insert(commit_spec.alias, commit_oid);
        files.insert(commit_spec.alias, file_table);
        order.push((commit_spec.alias.to_string(), commit_oid));
    }

    for (branch, alias) in spec::LOCAL_BRANCHES {
        repo.reference(
            &format!("refs/heads/{branch}"),
            oids[alias],
            true,
            "fixture: local branch",
        )?;
    }
    for (remote_ref, alias) in spec::REMOTE_REFS {
        repo.reference(
            &format!("refs/remotes/{remote_ref}"),
            oids[alias],
            true,
            "fixture: remote-tracking ref",
        )?;
    }

    let (tag_name, tag_alias) = spec::TAG;
    let tag_target_oid = oids[tag_alias];
    let tag_target = repo.find_object(tag_target_oid, Some(ObjectType::Commit))?;
    let tag_when = Time::new(EPOCH + (spec::COMMITS.len() as i64) * 60, 0);
    let tagger = Signature::new(TAGGER_NAME, TAGGER_EMAIL, &tag_when)?;
    let tag_oid = repo.tag(
        tag_name,
        &tag_target,
        &tagger,
        "fixture: annotated tag on a non-tip commit",
        false,
    )?;

    repo.set_head("refs/heads/main")?;
    // Sanctioned per design.md §1.3: this binary's entire job is writing a
    // throwaway fixture working tree, never a user repository.
    #[allow(clippy::disallowed_methods)]
    repo.checkout_head(Some(CheckoutBuilder::new().force()))?;

    // Working-directory dirt, sequenced after the checkout so it cannot
    // participate in any commit OID above.
    write_working_directory_dirt(&repo, out_dir)?;

    let head_oid = oids[spec::HEAD_ALIAS];
    let head_commit = repo.find_commit(head_oid)?;
    let head_tree_oid = head_commit.tree_id();

    Ok(BuildResult {
        path: out_dir.to_path_buf(),
        head_oid,
        head_tree_oid,
        tag_oid,
        commit_oids: order,
    })
}

/// Build the `writes` fixture at `out_dir` (design.md §9.2): three linear
/// commits, a real checkout, then working-directory dirt shaped for
/// stage/unstage/commit specs — nothing staged, two unstaged modifications,
/// one untracked file. Deliberately independent of `build()` above rather
/// than sharing its commit-building loop: `history`'s determinism guarantee
/// (`determinism.rs`) must not be able to drift because of a refactor made
/// for this recipe's sake.
///
/// `BuildResult.tag_oid` is `Oid::zero()` — a sentinel, never a real object
/// id — since this recipe has no tag.
pub fn build_writes(out_dir: &Path) -> Result<BuildResult, git2::Error> {
    if out_dir.exists() {
        std::fs::remove_dir_all(out_dir).expect("remove stale fixture directory");
    }
    std::fs::create_dir_all(out_dir).expect("create fixture directory");

    let mut init_opts = RepositoryInitOptions::new();
    init_opts.initial_head("main");
    let repo = Repository::init_opts(out_dir, &init_opts)?;

    // Pin what the local machine would otherwise supply, in the fixture's
    // own *local* config — the same "nothing ambient leaks in" rule this
    // crate already documents for OIDs, extended to commit behaviour
    // (design.md §9.2): a CI runner with no global identity, a developer
    // whose global `gpgsign = true`, or a global `core.hooksPath` must not
    // change what a spec against this fixture observes.
    let mut config = repo.config()?;
    config.set_str("user.name", AUTHOR_NAME)?;
    config.set_str("user.email", AUTHOR_EMAIL)?;
    config.set_bool("commit.gpgsign", false)?;
    let empty_hooks_dir = out_dir.join(".githooks-empty");
    std::fs::create_dir_all(&empty_hooks_dir).expect("create empty hooks directory");
    config.set_str("core.hooksPath", &empty_hooks_dir.to_string_lossy())?;

    let mut oids: HashMap<&'static str, Oid> = HashMap::new();
    let mut files: HashMap<&'static str, BTreeMap<String, Oid>> = HashMap::new();
    let mut order: Vec<(String, Oid)> = Vec::with_capacity(spec::WRITES_COMMITS.len());

    for (index, commit_spec) in spec::WRITES_COMMITS.iter().enumerate() {
        let when = Time::new(EPOCH + (index as i64) * 60, 0);
        let author = Signature::new(AUTHOR_NAME, AUTHOR_EMAIL, &when)?;
        let committer = Signature::new(COMMITTER_NAME, COMMITTER_EMAIL, &when)?;

        let mut file_table = BTreeMap::new();
        for parent_alias in commit_spec.parents {
            if let Some(parent_files) = files.get(parent_alias) {
                file_table.extend(parent_files.iter().map(|(p, o)| (p.clone(), *o)));
            }
        }
        let blob_oid = repo.blob(commit_spec.content)?;
        file_table.insert(commit_spec.file.to_string(), blob_oid);

        let tree_oid = build_tree(&repo, &file_table)?;
        let tree = repo.find_tree(tree_oid)?;

        let parent_commits: Vec<git2::Commit<'_>> = commit_spec
            .parents
            .iter()
            .map(|alias| repo.find_commit(oids[alias]))
            .collect::<Result<_, _>>()?;
        let parent_refs: Vec<&git2::Commit<'_>> = parent_commits.iter().collect();

        let commit_oid = repo.commit(
            None,
            &author,
            &committer,
            commit_spec.summary,
            &tree,
            &parent_refs,
        )?;

        oids.insert(commit_spec.alias, commit_oid);
        files.insert(commit_spec.alias, file_table);
        order.push((commit_spec.alias.to_string(), commit_oid));
    }

    for (branch, alias) in spec::WRITES_LOCAL_BRANCHES {
        repo.reference(
            &format!("refs/heads/{branch}"),
            oids[alias],
            true,
            "fixture: local branch",
        )?;
    }

    repo.set_head("refs/heads/main")?;
    // Sanctioned per design.md §1.3: this binary's entire job is writing a
    // throwaway fixture working tree, never a user repository.
    #[allow(clippy::disallowed_methods)]
    repo.checkout_head(Some(CheckoutBuilder::new().force()))?;

    // Dirt, sequenced after the checkout so it cannot participate in any
    // commit OID above: nothing staged, two unstaged modifications, one
    // untracked file (design.md §9.2's exact shape).
    std::fs::write(
        out_dir.join("tracked-a.txt"),
        b"tracked a, MODIFIED in the working directory, never staged\n",
    )
    .expect("write unstaged modification (tracked-a.txt)");
    std::fs::write(
        out_dir.join("tracked-b.txt"),
        b"tracked b, MODIFIED in the working directory, never staged\n",
    )
    .expect("write unstaged modification (tracked-b.txt)");
    std::fs::write(out_dir.join("untracked.txt"), b"never added\n")
        .expect("write untracked fixture file");

    let head_oid = oids[spec::WRITES_HEAD_ALIAS];
    let head_commit = repo.find_commit(head_oid)?;
    let head_tree_oid = head_commit.tree_id();

    Ok(BuildResult {
        path: out_dir.to_path_buf(),
        head_oid,
        head_tree_oid,
        tag_oid: Oid::zero(),
        commit_oids: order,
    })
}

/// `TreeBuilder::insert` is flat — it does not accept a `/`-separated path.
/// This groups a `path -> blob` table by top-level segment and recurses so
/// paths like `notes/c2.txt` land in a real `notes` subtree, not an entry
/// literally named `notes/c2.txt`.
fn build_tree(repo: &Repository, files: &BTreeMap<String, Oid>) -> Result<Oid, git2::Error> {
    let mut dirs: BTreeMap<String, BTreeMap<String, Oid>> = BTreeMap::new();
    let mut builder = repo.treebuilder(None)?;

    for (path, oid) in files {
        match path.split_once('/') {
            None => {
                builder.insert(path.as_str(), *oid, 0o100_644)?;
            }
            Some((dir, rest)) => {
                dirs.entry(dir.to_string())
                    .or_default()
                    .insert(rest.to_string(), *oid);
            }
        }
    }

    for (dir, entries) in &dirs {
        let subtree_oid = build_tree(repo, entries)?;
        builder.insert(dir.as_str(), subtree_oid, 0o040_000)?;
    }

    builder.write()
}

/// One staged addition, one unstaged modification to a file already tracked
/// at HEAD. Written after `checkout_head`, so it cannot affect any commit OID.
fn write_working_directory_dirt(repo: &Repository, out_dir: &Path) -> Result<(), git2::Error> {
    let staged_path = out_dir.join("staged-addition.txt");
    std::fs::write(&staged_path, b"queued for the next commit\n")
        .expect("write staged fixture file");
    // Same fixture-only sanction as `checkout_head` above: this binary never
    // touches a user repository, so `GitRepo::with_fresh_index`'s freshness
    // guarantee (M2) is not the concern here — this repo was just created.
    #[allow(clippy::disallowed_methods)]
    let mut index = repo.index()?;
    index.add_path(Path::new("staged-addition.txt"))?;
    index.write()?;

    let tracked_at_head = spec::COMMITS[0].file;
    let modified_path = out_dir.join(tracked_at_head);
    std::fs::write(
        &modified_path,
        b"modified in the working directory, never staged\n",
    )
    .expect("write unstaged fixture modification");

    Ok(())
}
