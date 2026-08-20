# Explore: stage-and-commit

**Change**: `stage-and-commit`
**Scope**: File-level stage, unstage, and commit. Branches and remotes are out of scope for this change (separate later changes per `product_scope`).
**Status**: exploration complete, unverified items labelled explicitly

---

## 1. Problem

Gitvisor is read-only today. `crates/git-core::GitRepo` only opens repositories and reads them (`info`, `refs`, `graph`, `commit_detail`, `status`). This is the project's first write capability, and `openspec/config.yaml`'s `product_scope` explicitly frames the risk asymmetry: "a defect in anything on the `in_scope` list shows wrong pixels or refuses an action; a defect in rebase, force-push or conflict resolution destroys a user's work." Stage/unstage/commit sit on the safer side of that line **only if** the implementation does not silently diverge from what `git commit` on the command line would actually do. Six traps were named for investigation; all six are real, and two of them (hooks, signing) are severe enough to decide the shape of the whole change.

---

## 2. Current state

### `crates/git-core` (`crates/git-core/src/repo.rs`, `model.rs`, `lib.rs`)

- `GitRepo` wraps a single `git2::Repository` (`inner`), opened via `Repository::discover`. No write methods exist. `status()` already builds `WorkingStatus { staged, unstaged, conflicted }` via `StatusOptions`, but **does not set `update_index(true)`** — see §3.4.
- `error.rs`: `CoreError` wraps `git2::Error`, `NotARepository`, `Invalid`. Serializes to a plain string for the UI. No variant exists yet for "operation refused" (e.g. hooks present, signing required, identity missing) as a distinct, UI-actionable case — today everything collapses to a string message.
- Every model type derives `Serialize` with `#[serde(rename_all = "camelCase")]`, and this is the *only* serialization surface (confirmed by the `visual-verification-harness` change's `dump-mocks` binary, which reuses these exact structs for E2E mocks — there is no second command-name vocabulary to keep in sync).

### `src-tauri` (`commands.rs`, `state.rs`)

- Seven thin commands, each `repos.with(&path, |repo| repo.X())`.
- `RepoRegistry::with<T>(&self, path, action: impl FnOnce(&GitRepo) -> Result<T>)` holds a single `Mutex<HashMap<String, GitRepo>>` for **the lock's entire duration of the closure**, not per-repo. This means all commands across *all* open repositories are already globally serialized relative to each other — a write to repo A cannot race a read of repo A, but it also cannot run concurrently with a read of unrelated repo B. This is existing behaviour, not something this change introduces, but it becomes more consequential once writes exist (see §3.6, Ordering/concurrency note).
- `action` takes `&GitRepo`, not `&mut GitRepo`. This is compatible with adding write methods because `git2::Repository::index()` returns an **owned** `Index` value with its own `&mut self` methods (`add_path`, `write`, `read`) — the mutation happens on that owned value, not on `&self.inner`. No signature change to `RepoRegistry::with` is required for stage/unstage/commit.

### Frontend (`src/features/repo/store.ts`, `api.ts`, `src/features/sidebar/Sidebar.tsx`)

- `api.ts` is confirmed to be the single `invoke()` chokepoint — no other file imports `@tauri-apps/api/core` (design.md of the harness change independently confirmed this for `@tauri-apps/api/window`, and the same single-file pattern holds for `core`).
- `store.ts`'s `refresh()` re-fetches `graph`, `refs`, `status` after `open()`; there is no polling and no fine-grained "just re-fetch status" path yet. A stage/unstage/commit action will need at least a `refreshStatus()` (cheaper than a full `refresh()`, which also re-walks the commit graph and re-selects).
- `Sidebar.tsx` already renders `status.staged.length` / `status.unstaged.length` / `status.conflicted.length` as a pending-count badge — this is the only existing UI surface for working-directory state. There is no staging UI (checkboxes, stage/unstage buttons, commit message box) anywhere in the tree today; this change is greenfield UI as well as greenfield backend.

### E2E harness (`openspec/changes/visual-verification-harness/`)

- Fully implemented (`apply-progress.md` confirms all 33 tasks done, both native (WKWebView) and browser (Chrome + mocked `invoke`) modes working). Key facts that constrain this change's testing design:
  - The fixture is regenerated **once per suite run**, in `wdio.native.conf.ts`'s `onPrepare` (`execSync("cargo run -p git-fixtures --bin build-fixture")`), **not per spec file**. Every spec in a native run shares one fixture directory (`target/e2e-fixtures/history/`).
  - The fixture is built with `git2::TreeBuilder`, no index, no worktree, for history; working-directory dirt (one staged file, one unstaged modification) is written **after** `checkout_head`, via a real index add + a real unstaged file write.
  - `fixture.json` is the single Rust→TS data seam; no OID/name is hardcoded in a spec file.
  - Native specs must clear `~/Library/WebKit/gitvisor/WebsiteData` in `onPrepare` because `rememberedRepo()`'s `localStorage` key persists across wdio runs (a hard-won lesson from that change, not theoretical).
  - Browser mode mocks are generated from the real backend via `tools/git-fixtures/src/bin/dump-mocks.rs`, keyed by Tauri command name, committed and diffed in CI (`mocks-drift` job).

---

## 3. Investigation of the eight named traps

### 3.1 Hooks — libgit2 does not run them. Confirmed.

**Verified** (web search corroborated by multiple independent libgit2 issue threads, e.g. `libgit2/libgit2#964`, `libgit2/libgit2sharp#2145`, `libgit2/libgit2#2007`): libgit2 has no hook execution mechanism at all — not "disabled by default," not "off unless configured." `git2::Repository::commit()` (and the lower-level `commit_create`) write a commit object and move a ref. Nothing in that call path shells out to `.git/hooks/pre-commit`, `commit-msg`, or `prepare-commit-msg`.

**Consequence, stated plainly**: a repository with husky, lint-staged, a conventional-commit linter, or any `pre-commit`/`commit-msg` hook would have those hooks **silently skipped** by a libgit2-based commit. This is not a cosmetic gap — it means Gitvisor can produce a commit that `git commit` from the same working tree, at the same moment, would have **rejected** (failing lint, failing tests, malformed message). That is a correctness and trust problem, not just a missing nicety: the user did not opt out of their own hooks, the tool silently didn't run them.

**Detection is straightforward and cheap.** Two independent signals, both readable via `git2::Config` and `std::fs` without shelling out:
- `core.hooksPath` (falls back to `<repo>/.git/hooks` — or `<worktree>/.git/hooks` for a linked worktree, **unverified for this project since it has no worktree support to test against**) gives the directory to check.
- A hook is "active" if a file named exactly `pre-commit`, `commit-msg`, or `prepare-commit-msg` (no extension) exists in that directory. Git's default `git init` templates ship these as `*.sample` (inert), so checking for the exact non-`.sample` filename is a reliable signal on Unix.
- **Executable-bit checking is Unix-only and not fully reliable cross-platform.** On Windows, Git for Windows invokes hooks via its bundled shell regardless of the Windows ACL "executable" concept, so the presence check (not the executable-bit check) is the portable signal. **Unverified**: whether this project needs to special-case Windows hook detection at all in v1, or whether "file present" is a good-enough proxy on every target platform. Needs a design-phase decision, not an implementation guess.

**Options, with a position taken:**

| Option | Description | Pros | Cons |
|---|---|---|---|
| A. Shell out to `git commit` for the actual commit | Use libgit2 only for stage/unstage (index manipulation, which is safe — see below); invoke the system `git` binary as a subprocess for the commit itself, so hooks run exactly as they would from a terminal | Correct by construction — no hook gap, no signing gap (git itself handles `commit.gpgsign`) | Depends on the user having `git` on `PATH` (reasonable assumption for a git GUI, but not guaranteed); reintroduces "ambient state" the fixture-determinism work in the harness change deliberately eliminated for *fixtures* (not the same concern for *product* code, but worth naming); output parsing needed for errors; breaks "everything through libgit2" purity but that purity was never a stated goal |
| B. Run hooks manually after a libgit2 commit | Detect hook files, execute them via `std::process::Command` with the same env/argv git itself would pass, abort/rollback if `pre-commit` or `commit-msg` fails | Keeps libgit2 as the single git engine | **Reimplementing git's hook contract is a known trap.** Exact argv, exact env vars (`GIT_INDEX_FILE`, etc.), exact staged-file exposure via a temp index, and correct handling of a `prepare-commit-msg` hook that *rewrites the message* are all subtly different per hook and easy to get wrong in ways that look like they work until a specific hook type is tested. This is strictly more implementation surface than option A for a worse guarantee |
| C. Refuse to commit when hooks exist | Detect hook files (same detection as A/B); if any of the three commit-related hooks exist, refuse the commit with a clear message pointing at the terminal | Zero risk of a hook-skipping trust violation; simplest to implement and verify | Makes the feature useless for any repo using husky/lint-staged/commitlint — arguably a large fraction of professional repos, which is exactly the audience most likely to install a git GUI |
| D. Silently commit via libgit2, document the gap | Do nothing special; note "hooks are not run" in docs/README | **Rejected outright.** This is precisely the "confident claim that doesn't survive a five-minute check" failure mode this project has been bitten by before, except inverted — here the *silence* is the defect, not a false claim. A user relying on a `pre-commit` hook to block a broken commit gets no warning at all |

**Position: Option A (shell out to `git commit`) for the commit step, keep libgit2 for stage/unstage.** Reasoning:

1. Staging (`git add`/`git reset` equivalents) has no hook surface in stock git (`pre-commit` and friends only fire on `git commit`), so libgit2's index API is safe and correct there — no reason to shell out for staging.
2. For the commit step, hooks are not an edge case to detect-and-refuse; they are mainstream tooling (husky, lint-staged, commitlint, pre-commit-the-Python-tool all install `pre-commit`/`commit-msg` hooks by default in a large fraction of modern JS/Python repos). Option C would make the commit feature unusable for exactly the users most likely to want a visual git client with a diff-then-commit workflow.
3. Option A is not a purity violation given the product's own framing: `openspec/config.yaml`'s `out_of_scope_ux` already says the app "SHOULD say so plainly and point at the terminal" for deferred capabilities — the same philosophy (prefer honest behavior over a leaky abstraction) supports shelling out rather than faking hook support badly.
4. This does introduce a real new dependency: **`git` must be resolvable on `PATH`** (or via a configurable path) at commit time. This is the single biggest open question the proposal must answer — see §5.
5. **Fallback if `git` cannot be found**: refuse the commit with a clear message ("git executable not found; cannot commit safely") rather than falling back to a libgit2 commit that silently skips hooks. Never silently downgrade from "safe" to "unsafe."

This also directly answers the signing question (§3.2): shelling out to `git commit` makes `commit.gpgsign`/`gpg.format=ssh` correct for free, because the user's own `git` binary and their own signing config execute unchanged. It is the single decision that resolves both of the two most severe traps at once.

### 3.2 Commit signing — libgit2 cannot sign. Confirmed, and resolved by the same decision as §3.1.

**Verified**: `git2-rs` issue `rust-lang/git2-rs#507` ("How to correctly create GPG-signed commits?") confirms the *intended* pattern is `commit_create_buffer()` → sign the buffer bytes yourself (shell out to `gpg`/`ssh-keygen`, or link a signing library) → `commit_signed()` to write the signed object. **libgit2 has no built-in signing.** If Gitvisor called `Repository::commit()` (the convenience wrapper) directly, on a repo with `commit.gpgsign=true` or `gpg.format=ssh`, the result is a commit that is **unsigned**, with no error and no warning — the same silent-degradation shape as the hooks trap.

**Options** (same table shape as hooks, since it's the same underlying decision):

| Option | Pros | Cons |
|---|---|---|
| Shell out to `git commit` (git handles signing via its own `gpg-sign`/`gpg.ssh.program` config) | Correct by construction, zero new signing code, matches the option chosen for hooks | Same `PATH` dependency as §3.1 |
| Detect `commit.gpgsign`/`gpg.format` via `git2::Config`, refuse if set | Simple, no subprocess dependency at all | Also makes the feature unusable for any repo with signing enabled — plausibly a smaller population than hook users, but non-trivial (many orgs mandate signed commits) |
| Implement signing ourselves (`commit_create_buffer` + shell to `gpg`/`ssh-keygen`) | Avoids shelling to `git` itself | Strictly more code than the git-commit option, for a worse guarantee (only covers the mechanisms explicitly implemented; git's own `gpg.program`/`gpg.ssh.program` indirection is more general) |

**Position: same as §3.1 — shelling out to `git commit` closes this gap as a side effect.** No separate signing code is needed in `git-core` if the commit step is delegated to the `git` binary.

**If the shell-out design is ever rejected in the proposal phase** (e.g. because a `PATH`-dependent commit is judged unacceptable), the fallback position is: detect `commit.gpgsign`/`gpg.format` via config and **refuse** the commit with a message naming the config key, rather than silently committing unsigned. Never the silent path.

### 3.3 Author identity — where it comes from, and what "unset" means.

**CORRECTED 2026-08-20 — this paragraph's original "Verified" label was false; see `measurements.md` M5.** `git2::Repository::signature()` reads `user.name`/`user.email` from git **config only**. It does *not* honour `GIT_AUTHOR_NAME`/`GIT_AUTHOR_EMAIL`; `git` does. Measured: with identity supplied only through the environment, `signature()` fails with "config value 'user.name' was not found" while `git commit` succeeds and attributes the commit to the environment identity. A pre-flight built on `signature()` therefore falsely refuses valid commits. The original text follows, struck through in meaning but preserved so the error is auditable: ~~reads user.name/user.email from the effective git config, following the standard precedence: GIT_AUTHOR_NAME/GIT_AUTHOR_EMAIL env vars, then local → global → system config.~~ If either is missing, the underlying libgit2 call (`git_signature_default`) returns `GIT_ENOTFOUND`, which `git2-rs` surfaces as a normal `git2::Error` — **not** an empty or bogus signature. This means the "commit as empty identity" failure mode named in the task **cannot actually happen through `Repository::signature()`** — libgit2 already refuses. The trap would only materialize if the implementation constructed a `Signature` manually with empty strings as a fallback instead of propagating that error, which would be a self-inflicted bug, not a libgit2 limitation.

**Consequence for design**: the commit path (whichever option is chosen in §3.1) MUST call `Repository::signature()` (or, if shelling to `git`, simply let `git commit` fail naturally on missing identity — git itself refuses with `Please tell me who you are...`) and surface that specific error distinctly in the UI ("no author identity configured — set `user.name`/`user.email`"), rather than a generic failure message. This is a UX requirement for the design phase, not a new discovery about libgit2 behavior.

### 3.4 Cached repository state / stale index — real, and currently unmitigated.

**Verified via `git2-rs` docs** (`StatusOptions`, `Index`):
- `StatusOptions::no_refresh(bool)` — "Bypasses the default status behavior of doing a 'soft' index reload." This means the *current* `status()` implementation, which never calls `no_refresh(true)`, **already gets a soft reload of the on-disk index for free** on every `working_status` call. "Soft" (per `Index::read(force: bool)`'s docs) means: reload from disk only if the on-disk index's mtime/state changed since last load, discarding unwritten in-memory index changes if so. So `status()` today is **not** stale with respect to an external `git add` run in a terminal — this specific staleness concern for the *read* path is already handled by libgit2's default behavior. **This is worth confirming empirically in the design/apply phase** (unverified: whether `git_status_list_new`'s soft reload applies to the *same* `Repository`/`Index` object across multiple libgit2 calls within one process, or only across process restarts — the doc wording ("if it has changed since last time it was loaded") is consistent with either).
- **The real trap is the *write* path.** `Repository::index()` returns the repository's canonical `Index` object. Nothing in `git2-rs`'s design guarantees that calling `.index()` a second time (e.g. from a `stage()` command run after a `status()` command already touched the index) gives you data that reflects a `git add` a user ran in their terminal *between* those two Gitvisor commands, unless the code explicitly calls `index.read(true)` (or at minimum `false`) before mutating and writing it back. **If Gitvisor's stage/unstage/commit path reads a stale in-memory index, mutates it, and writes it back, it can silently discard an external `git add`/`git rm --cached`** — exactly the "staging the wrong thing" failure the task warns about.
- The `RepoRegistry`'s per-path caching of `GitRepo` (and by extension the underlying `git2::Repository`, and whatever `Index` object gets pulled from it) makes this more likely to bite than it would with a fresh `Repository::open()` per command, because the whole point of the cache is to keep the object database warm across calls — the same warmth that makes an index go stale.

**Recommendation for design**: every write command (`stage`, `unstage`, `commit`) MUST call `index.read(true)` (hard reload, discard any accidental in-memory staleness) as its **first** action before touching the index, and MUST call `index.write()` before returning. This is cheap (a stat + maybe a re-parse of a typically-small file) and removes the staleness class entirely, at the cost of not preserving any Gitvisor-internal unwritten index state across calls — which is fine, because nothing in this design holds index state in memory between commands anyway (every command opens-mutates-writes in one call).

**Also worth flagging** (unverified, cheap to check in design/apply): whether `git2::Repository::commit()`/manual index writes need the same `HEAD`-staleness treatment — i.e., if a terminal `git commit` moves `HEAD` while Gitvisor is open, does the cached `Repository`'s notion of `HEAD` (used for e.g. `commit_tree`'s parent lookup) observe that? `Repository`'s reference lookups in libgit2 generally re-read the ref file on each call (refs are not cached the way the index is), so this is **probably** fine, but "probably" is exactly the word this project's own history says not to trust — a five-minute check (open a repo, `git commit` externally, call `working_status`/`info` again, confirm HEAD moved) belongs in the design or apply phase, not left as an assumption.

### 3.5 Ordering — must extend to any new listing.

`GitRepo::status()` already sorts `staged`, `unstaged` (by path) and `conflicted` (via `.sort()`) explicitly, with a comment explaining libgit2 orders status entries by `core.ignorecase`, which follows the filesystem (case-insensitive on macOS, byte-wise on Linux) — so the same repo would report a different order on different platforms without the explicit sort. `commit_detail()` does the same for diff deltas.

**This applies directly to stage/unstage.** If the command surface returns an updated `WorkingStatus` after a stage/unstage operation (recommended — see §3.6), it goes through the same `status()` method and inherits the existing sort, so **no new ordering bug is introduced by construction**, provided the design does not add a second, separate listing path (e.g. a raw `git2::Index` iteration exposed directly to the UI) that bypasses `status()`'s sort. This is a constraint to state explicitly in the design: any new read surface added for this change must funnel through the existing sorted `status()` output, not a new unsorted iteration.

### 3.6 Granularity — file-level, not hunk-level, for v1.

**Recommendation: file-level staging only for this change.** Reasoning:

- **libgit2 support asymmetry.** File-level staging is a single, well-trodden API: `Index::add_path`/`Index::remove_path` (or `add_all`/`remove_all` for bulk) plus `write()`. Hunk-level staging requires either (a) applying a partial patch to the index via `git2::Index::add_frombuffer` after manually constructing a hunk-limited blob, or (b) shelling out to `git apply --cached` with a synthesized patch — there is no single libgit2 call for "stage this hunk." Both paths are meaningfully more implementation surface and more failure modes (a malformed synthetic patch that `git apply` rejects, or worse, applies wrong) than file-level staging.
- **Product-scope fit.** `product_scope.in_scope` lists "stage, unstage" without qualifying granularity, and the explicit exclusion list (rebase, cherry-pick, force-push, visual conflict resolution) is about *destructive* operations, not about granularity — but hunk-level staging is the single riskiest correctness surface in ordinary daily git use short of the excluded operations themselves: a wrong hunk boundary silently stages part of a change, which is a subtler and harder-to-notice defect than staging a whole wrong file (which the diff view would immediately show as "staged this file I didn't mean to"). Not assuming "more granularity is better" — for a first write feature, the failure mode that's easiest to *notice and undo* (file-level) is the right one to ship first.
- **Effort.** File-level: Low (a thin `stage(path)`/`unstage(path)` wrapper over `Index::add_path`/`remove_path` + `write()`, following the exact same thin-command pattern already used for the seven read commands). Hunk-level: Medium-High, and would need its own exploration for the patch-construction and partial-apply mechanism, plus new UI for hunk selection.
- Hunk-level staging is a reasonable **fast-follow**, not a rejected idea — it should be named in the proposal as explicitly deferred, the same way the harness change named octopus merges as excluded rather than silently dropped.

### 3.7 Testing writes — fixture isolation is a real, currently-unsolved problem for this change.

**Current state (verified by reading `wdio.native.conf.ts` and `apply-progress.md`)**: the fixture is built exactly once per `wdio run` invocation, in `onPrepare`, and every spec file in that run shares the same `target/e2e-fixtures/history/` directory. This was fine for the harness change because every existing spec is **read-only** — Spec A (smoke) and Spec B (graph-viewport regression) never mutate the fixture.

**This change breaks that assumption.** A write spec (e.g. "stage a file, assert it moves from unstaged to staged") mutates the fixture's index and, for a commit spec, its `HEAD`/ref state. If two write specs run against the same shared fixture in one suite invocation, they are **not** order-independent: a "commit spec" running after a "stage spec" would see already-staged files; a spec asserting "nothing is staged initially" would fail if it runs after any earlier spec staged something and didn't clean up.

**Options for design to resolve:**

| Option | Description | Pros | Cons |
|---|---|---|---|
| A. Rebuild fixture per spec file, via a `beforeSession`/`before` hook keyed to the spec path | Move fixture construction from suite-level `onPrepare` to a per-spec hook | True isolation, no spec ordering dependency | Slower (native mode already takes minutes per spec per `apply-progress.md`'s measured timings — e.g. Spec A alone: `2m 56.8s`; multiplying fixture rebuilds by spec count is real CI cost). Needs `wdio.native.conf.ts` restructuring |
| B. One dedicated, uniquely-named fixture per write spec (e.g. `stage-fixture`, `commit-fixture`), each built once, read-only specs keep sharing `history` | Keeps the fast shared-fixture path for existing/most specs; only write specs pay the isolation cost | Directly extends `build-fixture`'s existing `[out-dir]` argument (already supports a name — see `build-fixture.rs`'s `let name = "history"` — trivially parameterizable) and the harness's existing manifest-per-fixture pattern (`fixture.json` already lives at `<out-dir>/<name>/`) | Each write spec needs its own manifest read logic (already exists — `readFixture()` — just needs a name/path parameter); slightly more moving parts than A but far less CI cost |
| C. Read-only assertions only; simulate writes by asserting the *intent* (button state, disabled/enabled) without ever completing them in E2E, verify write correctness only via `cargo test` on `git-core`'s new methods | Cheapest, no fixture isolation problem at all | **Rejected** — this defeats the entire purpose of the harness, which exists specifically because "all existing gates passed while the graph didn't render" (`findings.md`, F1). A write feature with the same blind spot the harness was built to close would repeat that exact mistake for writes instead of reads |
| D. Reset the shared fixture's working-directory/index state between write specs (re-checkout, clean index) without a full `cargo run` rebuild | Cheaper than A, no naming scheme needed | Reimplements a small "known-good state" reset mechanism that overlaps with what `build-fixture` already does — extra code for a marginal speed win over B |

**Position: Option B.** It reuses the fixture builder's existing parameterization (`out_dir` argument) and manifest pattern almost unchanged, keeps the fast shared-fixture path for every spec that doesn't need it, and avoids A's CI-time blowup. The proposal/design phase needs to decide the **exact set of dedicated fixture names** (e.g. one per write scenario: staging, unstaging, committing, refuse-on-conflict, refuse-on-hooks) and whether they're built once per suite (still shared across *assertions within one spec file*, since a spec file typically drives one full user flow — stage then commit — sequentially) or per-`it()` block (more isolation, more cost). **This is an open question for the proposal, not resolved here** — see §5.

**Unverified**: whether `git2::opts::set_search_path`'s process-global mutation (used by the fixture's own `determinism.rs` tests, guarded with `std::sync::Once` per `apply-progress.md`'s Phase 2 writeup, because two test threads calling it concurrently caused a `SIGABRT`) has any interaction with a *native* E2E run's use of ambient git config for the write commands' own identity/hook detection. The E2E `gitvisor` binary process is separate from the fixture-builder process, so this is very likely a non-issue, but it has not been explicitly checked and is worth one line in the design phase rather than assumed.

### 3.8 Failure surfaces — enumerated, with a position on each.

| Failure surface | Detection | Recommended handling |
|---|---|---|
| **Locked index** (`.git/index.lock` exists — another git process, or a crashed previous run, is mid-write) | `Index::write()`/`add_path()` will surface this as a `git2::Error` with `ErrorClass::Index`/`ErrorCode::Locked` (**unverified exact error code — needs a design-phase check against the vendored libgit2 1.9.x+ behavior**, but libgit2 is well-known to fail cleanly on a lock file rather than silently proceeding) | Refuse with a message naming the lock file, not a generic error string. Do **not** attempt to remove the lock file automatically — that is exactly the kind of "helpful" auto-recovery that can corrupt a concurrent operation |
| **Read-only filesystem** | `Index::write()` fails with an I/O error surfaced through `git2::Error` | Refuse with the underlying I/O message. No special handling needed beyond not swallowing the error |
| **Detached HEAD** | `Repository::head_detached()` — already used in `GitRepo::info()` | Staging and unstaging work identically regardless of HEAD state (they operate on the index vs. `HEAD`'s tree, and `git2::Index` staging doesn't care whether `HEAD` is detached). **Committing in a detached HEAD is legal in git** (creates a commit with no branch ref update) — it should be **allowed**, not refused, but the UI should say plainly that no branch will move, mirroring `out_of_scope_ux`'s "say so plainly" philosophy for a state the user should understand, not a capability being withheld |
| **Empty repository / unborn branch** (`RepoInfo.is_empty` / `head` is `None` — already modeled in `model.rs`) | `Repository::head()` returns `Err` today (already handled as `None` in `info()`) | Staging works (there's an index even with no commits yet). **The first commit on an unborn branch needs its own libgit2 path** — `Repository::commit()`'s `update_ref` parameter is `Some("HEAD")` either way, but there is no parent commit to pass (`parents: &[]`), which is a real branch in the commit code that must be tested explicitly, not assumed to "just work" because the normal path (`parents: &[&parent_commit]`) does |
| **Nothing staged, user clicks commit** | Check `status().staged.is_empty()` before attempting a commit, or let `git2`/`git commit` fail naturally | Refuse client-side with a clear message before even attempting — cheaper and clearer than surfacing git's own "nothing to commit" error text, and works identically whether the commit step ends up shelling to `git` (§3.1) or not |
| **Conflicted paths present** | `status().conflicted` (already modeled) | Per `product_scope`'s exclusion of visual conflict resolution: staging/committing over unresolved conflicts should be **refused** with a message pointing at the terminal — this is the `out_of_scope_ux` pattern applied directly, not a new decision |
| **Bare repository** (no working directory) | `Repository::is_bare()` | Staging/committing require a working tree; `GitRepo::open` currently doesn't check this. Should refuse plainly — "no working directory" — rather than attempting an index operation with `workdir()` unwrap-panicking or erroring cryptically deep in libgit2 |

---

## 4. Affected areas

- `crates/git-core/src/repo.rs` — new methods on `GitRepo`: `stage(path)`, `unstage(path)`, `commit(message)` (and possibly a hooks-detection helper, an identity-check helper). Must stay free of Tauri/React imports (existing constraint, easy to hold given the crate's current shape).
- `crates/git-core/src/model.rs` — likely no new *read* model types needed (stage/unstage/commit responses can reuse `WorkingStatus` and a minimal commit-result type), but a new error variant or two in `error.rs` for "operation refused" cases (hooks present, identity missing, conflicts present, index locked) so the UI can distinguish "refused for a known reason" from "unexpected failure."
- `crates/git-core/src/error.rs` — extend `CoreError` with refusal variants carrying enough structure for the UI to render a specific message, not just a generic string (current `Serialize` impl flattens everything to a string, which is fine for display but loses the ability to branch UI behavior per refusal reason — worth revisiting in design).
- `src-tauri/src/commands.rs` — three new thin commands (`stage_path`, `unstage_path`, `commit`), following the exact existing pattern.
- `src-tauri/src/state.rs` — likely unchanged; `RepoRegistry::with`'s `&GitRepo` signature already supports mutation through owned `Index`/commit calls (see §2).
- `src-tauri/Cargo.toml` / capabilities — if the hooks/signing decision (§3.1/§3.2) lands on shelling out to `git`, this needs a `std::process::Command` call, which needs no new Tauri capability (it's plain Rust, not a Tauri plugin surface) but does need a decision on how the `git` binary is located (`PATH` search vs. a configurable setting) — a genuinely new dependency class for this project.
- `src/features/repo/store.ts` / `api.ts` — new `stagePath`/`unstagePath`/`commit` API functions and store actions; likely a `refreshStatus()` action cheaper than the full `refresh()`.
- `src/features/sidebar/Sidebar.tsx` and a new (not-yet-existing) staging/commit UI component — this change is greenfield UI, not a modification of existing staging UI (none exists).
- `openspec/changes/visual-verification-harness/` (read-only reference, not modified) — `wdio.native.conf.ts`, `tools/git-fixtures/src/bin/build-fixture.rs`, `e2e/support/fixture.ts` are the concrete extension points for §3.7's fixture-isolation design.

---

## 5. Open questions the proposal MUST answer

1. **Does the commit path shell out to `git`, or stay pure-libgit2 with a refuse-on-hooks/refuse-on-signing fallback?** This is the single highest-leverage decision in the whole change — it resolves both the hooks trap (§3.1) and the signing trap (§3.2) together, or neither. The proposal must take an explicit position and, if it chooses "shell out," must also decide: how `git` is located (`PATH` search, with what error if not found?), how stdout/stderr/exit-code are parsed into a `CoreError` variant, and whether staging still goes through libgit2 (recommended: yes) while only the commit step shells out.
2. **What does "operation refused" look like in the error model?** Does `CoreError` gain structured variants (e.g. `HooksPresent(Vec<String>)`, `SigningRequired`, `IdentityMissing`, `ConflictsPresent(Vec<String>)`, `IndexLocked`) so the frontend can render distinct, actionable messages — or does everything stay a flat string, pushing the UI to parse message text (fragile)? Given `out_of_scope_ux`'s emphasis on saying things "plainly," structured refusal reasons seem worth the small extra surface, but this is a design-phase call informed by how much the frontend needs to branch on it.
3. **Exact index-freshness contract.** Does every write command call `index.read(true)` unconditionally as its first action (recommended in §3.4), and does `status()` gain the same treatment even though it currently benefits from libgit2's default soft-reload? Should this be documented as an explicit invariant in `repo.rs` (a doc comment, the way the existing sort-ordering comment documents *its* invariant) so a future contributor doesn't remove it as "seems unnecessary"?
4. **Fixture isolation scheme for write specs** (§3.7) — Option B's exact fixture-naming/count scheme, and whether write specs get their own `wdio.native.conf.ts` variant or share the existing one with per-spec fixture selection logic in a hook.
5. **First-commit-on-unborn-branch and detached-HEAD commit paths** — do these get their own explicit test coverage (unit tests in `git-core`, not just E2E), given they are real branches in the commit code that "look like" the normal path but exercise a different libgit2 parameter shape (`parents: &[]` vs `parents: &[&parent]`)?
6. **Where does the "is `git` on PATH" check happen**, and does its absence block the whole write feature (grey out stage/commit UI) or only the commit step specifically (allow stage/unstage via libgit2, refuse only commit)? Given §3.1's stance that staging has no hook surface, the latter seems right, but it's a UX decision, not just a technical one.

---

## 6. Recommendation

Ship stage/unstage via pure libgit2 (`Index::add_path`/`remove_path`/`write()`, with a mandatory `index.read(true)` at the start of every write command per §3.4), and ship commit by **shelling out to the system `git commit`** rather than `git2::Repository::commit()`, because that single decision closes both the hooks trap and the signing trap correctly and for free, at the cost of a new "`git` must be on `PATH`" dependency that must fail loudly (refuse, not degrade) when unmet. Keep granularity file-level for v1 (§3.6) — hunk-level is a real fast-follow, not a rejected idea, but it roughly doubles the implementation and testing surface of this change for a correctness-risk profile that's harder to notice when wrong. Extend the E2E harness with a per-write-spec dedicated fixture (Option B, §3.7) rather than rebuilding per spec file (too slow) or sharing the read-only fixture (not order-independent). Every write command must funnel its listing output through the existing sorted `status()` method (§3.5) so no new platform-ordering bug is introduced. Enumerate refusal cases (§3.8) as structured, not string-only, errors so the UI can be specific rather than generic.

---

## 7. Risks

- **`git`-on-`PATH` dependency** (if the shell-out recommendation is adopted) is a new failure class this project has not had before — a repository/environment where `git` is unreachable (unlikely in practice for a git GUI's user, but not impossible, e.g. a sandboxed/portable install) turns "commit" into a hard refusal. Must be tested explicitly, not assumed to always succeed.
- **Hunk-level staging deferral** may be perceived as a missing table-stakes feature by users coming from GitKraken/other GUIs that support it from day one; explicitly naming it as a fast-follow in the proposal (not silently dropped) mitigates the risk of it being rediscovered as a "gap" later.
- **Index-freshness contract (§3.4) is a discipline requirement, not a one-time fix.** If a future contributor adds a new write path without the same `index.read(true)` invariant, the staleness bug returns silently. Worth a doc comment and, ideally, a shared internal helper (`fn with_fresh_index(&self) -> Result<Index>`) that makes the correct behavior the path of least resistance rather than something every write method has to remember independently.
- **E2E fixture isolation (§3.7) is unresolved in this exploration** — it is named as an open question for the proposal/design phase, not a solved problem. If under-scoped, write specs could ship flaky or order-dependent, which is a worse outcome than not having write E2E coverage at all (a flaky-and-ignored red test is exactly the failure mode `visual-verification-harness/design.md`'s §5 argued against for Spec B).
- **Detached-HEAD and unborn-branch commit paths are real code branches that are easy to under-test** if the implementation and its unit tests only exercise the common "commit on a normal branch with a parent" case.
- **Windows hook detection is unverified** (§3.1) — the exact-filename-presence heuristic is reasoned from general git-for-Windows behavior, not measured on this project's target platforms. Should be an explicit design-phase or apply-phase check, not shipped on the strength of this exploration's reasoning alone.

---

## 8. Ready for Proposal

**Yes**, with the six open questions in §5 flagged as required inputs to `sdd-propose` (particularly #1, the shell-out-vs-refuse decision, which the proposal cannot avoid taking a position on — it determines almost everything else about the shape of the commit command).

---

## Orchestrator verification (2026-08-18): the hooks finding is confirmed by experiment

§3.1's claim is load-bearing — it decides the shape of the entire commit path — so it was
tested rather than accepted from documentation. A throwaway crate created a repository with
a `pre-commit` hook that always exits 1, staged a file, and committed twice: once through
`git2::Repository::commit()`, once through the `git` binary in the same repository.

```
RESULT:  libgit2 COMMITTED 6fd1c9ed… — the rejecting pre-commit hook did NOT run
CONTROL: `git commit` exit=Some(1) stderr=HOOK RAN — rejecting
```

Same repository, same hook, same moment. libgit2 ignored it; `git` honoured it.

The control matters as much as the result: without it, "libgit2 committed" could equally have
meant the hook was never executable, never found, or silently broken. The control proves the
hook was live and blocking, so libgit2's success is the finding rather than an artefact of a
badly built experiment.

**Confirmed, and severe.** A `git2::Repository::commit()` implementation would produce commits
that the user's own tooling would have rejected — silently, with no warning, on repositories
where a `pre-commit` hook is the thing standing between a broken change and history.

This raises §5's question 1 from "the highest-leverage decision" to a decision the proposal
cannot resolve any other way without accepting a known, demonstrated defect. The exploration's
recommendation — libgit2 for staging, the `git` binary for the commit step — stands on measured
evidence.

**Still unverified** and left for design: the signing claim (§3.2) is reasoned from the same
libgit2 limitation but was not separately measured; the index-freshness behaviour (§3.4); and
the Windows hook-detection heuristic (§3.1). Each keeps the "unverified" label the exploration
gave it.
