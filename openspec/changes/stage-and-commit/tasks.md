# Tasks: Stage and Commit

## Delivery decision (orchestrator, 2026-08-20) — RESOLVED: split into two changes

`ask-on-risk` fired on a ~3,000–3,400 line forecast. The user chose to **split
the change in two** rather than chain 8–9 units inside one.

| | Scope | Size | Ships |
|---|---|---|---|
| **This run** — `stage-unstage` | `with_fresh_index` (M2), structured errors, stage / unstage / bulk, the changed-files UI, fixtures, harness delta | ~1,200 | A usable feature: mark files from the app, commit in the terminal |
| **Follow-up** — `commit` | `git` resolution, the subprocess, the timeout ladder, hooks, signing, identity via `git var` (M5), HEAD-delta reporting | ~2,000 | The rest |

**Why here.** The risk is not spread evenly across this change. Everything that
can modify a repository in a way the user did not ask for — a subprocess, a
timeout that might kill a half-written commit, hook and signing behaviour,
identity precedence — is in the second half. Staging is libgit2 in-process with
one file written, and `with_fresh_index` makes "refuse before mutating"
structural.

Splitting lets the lower-risk half land, be used, and settle before the higher-risk
half is written against it. It also keeps the commit subprocess out of a PR that
would otherwise have already spent its review budget on UI.

**The planning artifacts are NOT split.** `proposal.md`, `design.md` and
`specs/` cover both halves and stay as they are — the commit half is fully
designed and its evidence (M1, M3, M4, M5) is already measured. Only *delivery*
is split. The follow-up change references these documents rather than
re-deriving them; re-planning designed work would be the waste this split is
trying to avoid.

**Spikes.** Only **U3** (clippy path resolution for a foreign inherent method,
with a source-scan fallback) gates this run — it decides how `with_fresh_index`
is enforced. **U7** (long synchronous Tauri command freezing the webview) and
**U10** (`git var` refusal parity) are commit-path concerns and defer with it.

---

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | **~3,000–3,400** (Rust incl. tests ~2,000; frontend TS/TSX ~850; harness/config/wdio ~450) |
| 400-line budget risk | **High** — several individual work units exceed 400 lines *before* chaining, once the design's mandated real-subprocess replay tests (M2, M5, hook regression, timeout ladder) are counted with the code they verify |
| Chained PRs recommended | **Yes** |
| Suggested split | See **Suggested Work Units** below — **8–9 units**, not the 5 design §15 sketches. §15's slice 3 ("git_binary + commit") alone bundles resolution, probe, subprocess, timeout ladder, HEAD-delta reporting, the hook regression, M1/M5 replays, and unborn/detached coverage; with its required tests it is ~700–800 lines and needs its own internal split |
| Delivery strategy | `ask-on-risk` |
| Decision needed before apply | **Yes.** Total forecast is well over 400 lines with no natural single-PR cut under budget. The orchestrator should confirm chain strategy and unit count with the user before `sdd-apply`, the same way it did for `visual-verification-harness` |

### Why design §15's 5-slice sketch is evaluated, not adopted as-is

§15 groups "errors + paths + index guard" as one unit and "git_binary + commit" as another. Both undercount once tests are included:

- **Slice 1** (errors/paths/index guard): `error.rs` variants + `Serialize` change + `describe()` update is a self-contained ~220-line unit on its own — it does not need `paths.rs`/`index_guard.rs` to compile or to be reviewed, and splitting it out keeps the M2-replay unit (paths + index guard + clippy gate, ~360 lines) under budget instead of combining to ~580.
- **Slice 3** (git_binary + commit): resolution/probe (~200 lines, no dependency on the commit subprocess) is naturally separable from the subprocess/timeout/HEAD-delta mechanism (~280 lines) and separable again from its test suite (hook regression + M1 + M5 + unborn + detached + fake-`git` timeout-ladder tests, ~500 lines by itself, per the `work-unit-commits` rule that tests stay with the behaviour they verify — meaning the *behaviour* commit that owns those tests is the one that grows, not a rule to strip tests elsewhere).

Slices 2, 4, and 5 (stage/unstage, UI panel, harness) hold up close to as-proposed and are kept.

### Suggested Work Units

| Unit | Goal | Depends on | Parallelizable with | Rough lines | Focused test command | Runtime harness | Rollback boundary |
|------|------|------------|----------------------|-------------|-----------------------|------------------|--------------------|
| 0 | Spike gate (U3, U7, U10, U1/U2 residual) | — | all of 0 internally parallel | ~0 shipped (throwaway experiment code; findings recorded in design.md's unverified register) | N/A — experiments, not product code | N/A | Nothing to roll back; findings are documentation |
| 1 | Structured errors + wire shape | Unit 0 (U3 informs nothing here directly; independent) | Unit 2 | ~220 | `cargo test -p git-core error::` | N/A | Revert `error.rs`, `Serialize` impl, `describe()` change; no caller yet |
| 2 | Paths + index guard + clippy gate (M2 replay) | Unit 0.1 (U3) | Unit 1 | ~360 | `cargo test -p git-core index_freshness` | N/A | Remove `paths.rs`, `index_guard.rs`, `clippy.toml`; unused until Unit 3 |
| 3 | Stage / unstage + commands | Units 1, 2 | — | ~420 | `cargo test -p git-core stage:: unstage::` | N/A (no UI yet) | Remove `stage`/`unstage` from `repo.rs`, the two commands; index guard stays inert |
| 4a | `git` resolution + probe | Unit 1 | Unit 3 | ~200 | `cargo test -p git-core git_binary::resolve` | N/A | Remove `git_binary.rs` resolve/probe; `which` dep removable |
| 4b | Commit subprocess + timeout + HEAD-delta | Unit 4a; Unit 0.2 (U7), Unit 0.3 (U10) | — | ~280 | `cargo test -p git-core commit::` (happy path) | N/A yet | Remove `run_commit`, `repo.rs::commit`, `state.rs::invalidate`, `create_commit` command; `libc` dep removable |
| 4c | Commit test suite: hook regression + M1 signing + M5 identity + unborn/detached + timeout-ladder replays | Unit 4b | — | ~500 | `cargo test -p git-core` (full) | N/A | Test-only; delete the added test files |
| 5 | UI panel + store + browser specs | Units 3, 4a, 4b | — | ~850 | `pnpm build` (tsc + vite) + manual verification checklist (Phase 4.7) | Browser-mode wdio specs, mocked `invoke` | Remove `src/features/working-directory/`, revert `api.ts`/`store.ts` additions; backend inert without a caller |
| 6 | Harness: `writes` fixture + native write spec | Unit 5 | — | ~450 | `cargo test -p git-fixtures` (recipe/determinism) | `pnpm e2e:native:writes` — real binary, real WebKitGTK/WKWebView, macOS **and** Linux | Remove `wdio.native.writes.conf.ts`, `e2e/native/writes/`, the `writes` recipe; `history` untouched |

---

## Phase 0: Spike Gate — resolve the load-bearing unverifieds before they reshape the plan

Per design §13, ten items are unverified. Five are load-bearing enough that discovering them mid-implementation would force a replan; they run first, before any product code.

- [x] 0.1 **U3** — `clippy.toml` `disallowed-methods` path-resolution spike (5 min, design §1.3): add `clippy.toml`, add a throwaway `let _ = self.inner.index();` in `repo.rs`, run `cargo clippy -p git-core`, confirm it errors, delete the line. **This decides Unit 2's shape**: if resolution works, the lint is the enforcement mechanism; if not, fall back to `crates/git-core/tests/index_discipline.rs` — a source-scan test asserting `.index()` appears in exactly one file (same technique as the harness change's release-artifact byte scan). Record the result in design.md's unverified register. **Result: resolution works.** `cargo clippy -p git-core -- -D clippy::disallowed-methods` errored on the throwaway call with the exact configured reason, then passed clean once removed. Lint is the enforcement mechanism; no source-scan fallback written.
- [ ] 0.2 **U7** *(deferred — commit-path concern per the 2026-08-20 delivery split; ships with the `commit` follow-up change)* — long synchronous Tauri command / webview freeze check (2 min, design §12): point a throwaway repo's `pre-commit` hook at `sleep 30`, commit through a minimal stand-in command (or wait until 4b exists and re-run then if sequencing is easier — but the decision must land before 4b is written, not after), try to scroll the graph. **This decides whether Unit 4b's `create_commit` command needs `async fn` + `spawn_blocking`** — a one-line change, but on evidence, not belief about Tauri's threading model.
- [ ] 0.3 **U10** *(deferred — commit-path concern per the 2026-08-20 delivery split; ships with the `commit` follow-up change)* — `git var GIT_AUTHOR_IDENT` vs. `git commit` identity-refusal parity (5 min, design §5.1): isolated `HOME`, no config, no env identity — compare exit codes of `git var GIT_AUTHOR_IDENT` and `git commit`; repeat with a gecos name available but no email. **This decides Unit 4b's identity pre-flight strictness**: if `git var` is stricter, the pre-flight becomes reporting-only rather than a hard refusal, per §5.1's stated asymmetry.
- [ ] 0.4 **U1/U2** *(deferred — commit-path concern per the 2026-08-20 delivery split; ships with the `commit` follow-up change)* — residual pinentry experiment, §3.5 P1/P2/P3, folding in U2's `pgrep` survivor check: real `gpg` + `pinentry-curses` with no controlling terminal (P1), with an inherited controlling terminal (P2), and SSH signing with a passphrased key and no agent (P3). Throwaway repo, `commit.gpgsign=true`, throwaway `GNUPGHOME`, `gpgconf --kill gpg-agent`. Record: blocked or not, elapsed, whether the SIGTERM ladder terminated it, `pgrep -l 'pinentry|gpg-agent|ssh-keygen'` survivors after the kill, whether HEAD moved. Run on **macOS and Linux**. Nothing in Units 4b/4c structurally depends on the outcome (the timeout + always-read-HEAD handles any hang shape), but a surviving pinentry or an unexpected HEAD state would change the timeout UI copy in Unit 5, so this runs before that copy is written.
- [ ] 0.5 **U9** *(deferred — commit-path concern per the 2026-08-20 delivery split; ships with the `commit` follow-up change)* — record as non-actionable rather than silently dropped: "does a hang *after* the commit object exists behave like M3's?" has no cheap deliberate-staging check (design §13: "Hard to stage deliberately"). The design's mitigation is structural (§3.2 — HEAD is always re-read after every terminal outcome, never assumed), not a spike result. No task beyond this acknowledgement; carried into Unit 4b's design as the reason the outcome table has no third branch.

---

## Phase 1 (Unit 1): Structured errors + wire shape [Requirement: Commit and Staging Refusals Use Distinct, Machine-Readable Codes]

- [x] 1.1 `crates/git-core/src/error.rs`: add 9 refusal variants (`GitUnavailable`, `CommitFailed`, `CommitTimedOut`, `IdentityMissing`, `ConflictsPresent`, `IndexLocked`, `BareRepository`, `NothingStaged`, `PathOutsideRepo`) per design §5.1. Keep the existing 3 variants' `Display` text byte-identical — no rewording.
- [x] 1.2 `error.rs`: `code()` returning the stable `camelCase` `&'static str` for all 12 variants.
- [x] 1.3 `error.rs`: hand-written `Serialize` via `serialize_map` → `{ code, message, details? }`; `details` only for variants carrying structure (`commitFailed`, `commitTimedOut`, `conflictsPresent`, `indexLocked`, `gitUnavailable`, `pathOutsideRepo`). No `serde_json` dependency added.
- [x] 1.4 `src/features/repo/store.ts`: add the `CoreErrorWire` type, `isWire()`, `asCoreError()`; update `describe()` to check the wire-shape branch **first**, string/Error/JSON fallbacks unchanged. Manually confirm one existing command's error path (e.g. `open_repository` on a bad path) still renders the identical string as before this change — `message` is `self.to_string()`, so it must be. **Confirmed by construction**: the three pre-existing variants' `#[error("{0}")]` text is untouched and `message` is always `self.to_string()`, so `describe()`'s wire branch returns the byte-identical string; `pnpm build` (`tsc --noEmit`) passes.
- [x] 1.5 `cargo test -p git-core`: `code()` returns a distinct value for every variant (a simple enumeration test — this is what makes "never collapsed into one generic failure" mechanically checked, not just asserted in prose). `error::tests::code_is_distinct_per_variant`.

---

## Phase 2 (Unit 2): Paths + index guard + clippy gate — the M2 replay [Requirement: Stage a Single Working-Tree Path; External Staging Is Never Destroyed]

- [x] 2.1 Add `clippy.toml` at the workspace root per design §1.3's full list (index/reset/checkout/add_all/remove_all/update_all/signature denials), using whichever mechanism 0.1 determined. Add `#![deny(clippy::disallowed_methods)]` to `crates/git-core/src/lib.rs`.
- [x] 2.2 `crates/git-core/src/paths.rs`: pure `normalise_repo_path(input) -> Result<String>` per design §6.2 — reject NUL/empty, reject absolute paths, fold `.`/`..` (empty-stack pop → `PathOutsideRepo`), reject an empty result, reject a first component that lowercases to `.git`, re-join on `/`. No I/O.
- [x] 2.3 Unit tests for the normaliser (no repository on disk, microseconds each): M4's four rows (`inside.txt` accepted, `../outside.txt` refused, `/etc/hosts` refused, `sub/../inside.txt` accepted — corrects the raw libgit2 `NotFound` M4 recorded), plus `a[b].txt`, `.git/config`, and a NUL byte.
- [x] 2.4 `crates/git-core/src/index_guard.rs` (private module): `with_fresh_index()` — `Repository::index()`, then **hard** `index.read(true)` before the closure runs, `mutate(&repo, &mut index)`, `index.write()` on the success path only (§1.1). `reload_index()` — forces the index back in sync without writing. **Deviation**: nested at `crates/git-core/src/repo/index_guard.rs` (a submodule of `repo`, which became `repo/mod.rs`) rather than a top-level sibling of `repo.rs`. `GitRepo.inner` stays a plain-private field, visible only to `repo` and its descendant modules — matching design §1.2's own claim ("nothing outside `crates/git-core::repo` can obtain an `Index`") literally at the compiler level, rather than widening `inner` to `pub(crate)`, which would have let *any* module in the crate reach it.
- [x] 2.5 `crates/git-core/tests/index_freshness.rs` — **the M2 replay, real subprocess**: create repo, commit A, write `b.txt` and `c.txt`, run a **real `git add b.txt`** as a subprocess, then `GitRepo::stage(["c.txt"])`, assert both survive. **Finding recorded in design.md**: replayed through the public `stage()` API this way, the test cannot isolate `read(true)` alone — `stage`'s own pre-flight calls `status()` first, and `Repository::statuses()` has an incidental soft-sync side effect that masks a missing `read(true)` here. The test that actually isolates and proves the invariant — calls `with_fresh_index` directly, goes red without `read(true)`, green with it — is `repo::index_guard::tests::m2_external_git_add_survives_with_fresh_index` in `src/repo/index_guard.rs`. `tests/index_freshness.rs` is kept as the black-box, public-API replay of spec.md's literal scenario.
- [x] 2.6 `cargo test -p git-core`: a closure that returns `Err` mid-mutation leaves the on-disk index byte-identical to before the call (proves §1.5's "refuse before mutating anything" property is structural, not a discipline). `repo::index_guard::tests::err_from_closure_leaves_on_disk_index_untouched`.

---

## Phase 3 (Unit 3): Stage / unstage + thin commands [Requirement: Stage a Single Working-Tree Path; Unstage a Single Working-Tree Path; Bulk Stage and Unstage Operate Only on Listed Entries; Listings Are Deterministically Ordered]

- [x] 3.1 `repo.rs::stage(&self, paths: &[String])`: validate every path via `normalise_repo_path` first (whole batch fails before `write()` on any bad path — §7.2), then inside one `with_fresh_index` call: `Index::add_path` for files present on disk, `Index::remove_path` for files gone from disk but present in the index/HEAD (staging a deletion), skip-and-report (`SkipReason::Vanished`) for files in neither disk, index, nor HEAD.
- [x] 3.2 `repo.rs::unstage(&self, paths: &[String])`: manual index-entry restoration per §8 — HEAD exists and path is in the HEAD tree → rebuild the `IndexEntry` from the tree entry (stat fields zeroed) and `index.add(&entry)`; HEAD unborn or path absent from the HEAD tree → `index.remove_path`. **No** `reset_default`, `checkout_*`. The file on disk is never read, written, or deleted.
- [x] 3.3 `model.rs`: `WriteOutcome { status: WorkingStatus, skipped: Vec<SkippedPath> }`, `SkippedPath { path, reason }`, `SkipReason::Vanished` — `skipped` sorted at construction (D9 applies here too).
- [x] 3.4 Shared pre-flight (used by stage, unstage, and — later — commit): conflicted paths present → `ConflictsPresent { paths }`; bare repository → `BareRepository`; both refuse before any mutation. Implemented as `preflight_write()`, called at the top of both `stage` and `unstage`; the (later) commit pre-flight will call the same helper.
- [x] 3.5 `src-tauri/src/commands.rs`: `stage_paths`, `unstage_paths` — each one `repos.with(&path, …)` line, no branching, no message building.
- [x] 3.6 `cargo test -p git-core` (`tests/stage_unstage.rs`):
  - a modified file stages and nothing else is touched (Requirement 1's scenario);
  - a staged file with local edits unstages and its on-disk content is byte-identical before/after (hash comparison — Requirement 2's scenario);
  - "stage all" over 3 listed paths + 1 untracked, un-ignored build artifact stages exactly the 3 and leaves the artifact untracked (Requirement 3's scenario, replayed literally);
  - `a[b].txt` stages and unstages without touching a sibling (§6.3/§8, glob-safety proof);
  - unstage on an unborn branch removes the entry rather than erroring;
  - a batch containing one vanished path is skipped-and-reported, not a batch failure;
  - conflicted-path refusal fires before any mutation for stage and unstage, and the on-disk index is byte-identical before/after the refusal. **Deviation**: "and (stubbed) commit entry points" is dropped — commit is out of scope for this run (2026-08-20 delivery split); the shared `preflight_write()` will be reused by `commit()` when that change lands.
- [x] 3.7 `cargo test -p git-core`: `staged_and_unstaged_listings_come_through_the_same_sorted_status_path` — a write outcome's `status` and a direct `status()` call return the identical sorted order, via the existing sorted `status()` — no raw index iteration introduced anywhere in 3.1–3.2. **Deviation**: implemented as "the write path and the read path agree" rather than a synthetic case-sensitive-vs-insensitive filesystem harness (constructing two real filesystems with different case sensitivity is outside a unit test's reach); the case-sensitivity claim itself is already covered by `status()`'s own pre-existing behaviour, untouched by this change.

---

## Phase 4a (Unit 4a): `git` resolution + probe [Requirement: `git` Availability Gates Only the Commit Step]

- [ ] 4a.1 Add the `which` crate as `crates/git-core`'s new unconditional dependency (design §4.3 — hand-rolling `PATH`/`PATHEXT` search is the part we cannot test on Windows).
- [ ] 4a.2 `git_binary.rs::resolve(override_path: Option<&str>) -> Result<ResolvedGit>` per §4.1's order: explicit override → `GITVISOR_GIT_PATH` → `PATH` via `which` → `GitUnavailable`. **Never cached** — no `OnceLock`, no field on `GitRepo`.
- [ ] 4a.3 `git_binary.rs::probe()`: exists, is a file (or symlink to one), executable bit on Unix, then spawn `<candidate> --version` and require exit `0` with stdout beginning `git version `. Executed directly via `Command::new`, never through a shell.
- [ ] 4a.4 `src-tauri/src/commands.rs`: `git_probe` command wrapping `probe()`.
- [ ] 4a.5 `cargo test -p git-core`: resolution precedence (override beats env beats `PATH`); probe rejects a candidate pointed at a non-git executable (`/bin/ls`-equivalent) or a directory; `GitUnavailable` names what was looked for.

---

## Phase 4b (Unit 4b): Commit subprocess + timeout + HEAD-delta [Requirement: Commit Runs Through the User's `git` Binary]

- [ ] 4b.1 `git_binary.rs::base_command()`: one shared builder for both the identity pre-flight and the commit spawn, so their env/cwd cannot drift apart (§5.1). Exact env per §2.2: **set** `GIT_TERMINAL_PROMPT=0`, `GIT_EDITOR=:`, `GIT_SEQUENCE_EDITOR=:`; **remove** `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_COMMON_DIR`, `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_NAMESPACE`; **never touch** `GIT_AUTHOR_*`/`GIT_COMMITTER_*` (M5).
- [ ] 4b.2 `git_binary.rs::identity()`: `git var GIT_AUTHOR_IDENT` via `base_command()`. Apply 0.3's (U10) finding — hard refusal if parity was confirmed, reporting-only (letting `git commit`'s own refusal be authoritative) if `git var` proved stricter.
- [ ] 4b.3 `git_binary.rs::run_commit()`: exact argv per §2.1 (`-C <workdir> --no-pager commit --file=- --cleanup=whitespace`), absolute resolved path only, never the bare string `git`. Message written to stdin then the handle dropped for EOF. stdout/stderr drained by two dedicated reader threads started immediately after spawn (deadlock avoidance — a hook writing past the pipe buffer must not block us). `try_wait()` polled every 50 ms against the deadline; `wait_with_output()` is not used.
- [ ] 4b.4 Timeout ladder: SIGTERM → 5 s grace → SIGKILL via `libc::kill` on the child's own process group (`process_group(0)`, signal sent to `-pid`). Add the Unix-only `libc = "0.2"` dependency (`[target.'cfg(unix)'.dependencies]`). Apply 0.2's (U7) finding to decide whether `create_commit` needs `async fn` + `spawn_blocking`.
- [ ] 4b.5 `repo.rs::commit()`: pre-flight order — bare? conflicts? nothing staged? `.git/index.lock` present (`Path::exists()`, never libgit2's own error code)? resolve `git`? identity? — every refusal before the index is touched. Then `head_before` via a **freshly opened** `Repository` (never the cached one — §2.4), run the commit, `head_after` via a **freshly opened** `Repository` on **every** terminal outcome (success, non-zero exit, timeout, signal death). Outcome derived from `f(head_before, head_after, exit)` per the 7-row table in §2.5 — no branch inspects stderr/stdout text.
- [ ] 4b.6 `src-tauri/src/state.rs`: `invalidate(path)` — thin alias of `close()`, called by `create_commit` on success so the next command reopens rather than trusting a ref view the design has chosen not to trust.
- [ ] 4b.7 `src-tauri/src/commands.rs`: `create_commit` thin command; calls `repos.invalidate(&path)` only after success.

---

## Phase 4c (part of Unit 4c): Commit test suite — hook regression, M1, M5, unborn/detached, timeout ladder

- [ ] 4c.1 `cargo test -p git-core`: **hook-rejection regression with a positive control** — a repo with a live `pre-commit` hook that exits 1 and prints `HOOK RAN — rejecting` produces **zero** commits, `HEAD` unchanged, stderr surfaced verbatim; a control repo with a passing hook succeeds through the identical code path [Requirement: Commit Hooks Run and a Rejection Blocks the Commit — direct scenario replay].
- [ ] 4c.2 `cargo test -p git-core`: **M1 replay** — `commit.gpgsign=true`, commit through this path, resulting commit carries a `gpgsig` header, verified via `git log --show-signature` [Requirement: Commit Honours Signing Configuration].
- [ ] 4c.3 `cargo test -p git-core`: **M5 replay, real subprocess, real isolated `HOME`** — no `user.name`/`user.email` at any config level, identity supplied only via `GIT_AUTHOR_*`/`GIT_COMMITTER_*`; assert `GitRepo::commit` **succeeds**. Must exercise `git_binary::identity()`; an in-process assertion against `Repository::signature()` would reproduce the bug rather than catch it, per design §10 explicitly [Requirement: Commit and Staging Refusals Use Distinct, Machine-Readable Codes — `IdentityMissing` must not false-refuse].
- [ ] 4c.4 `cargo test -p git-core`: unborn-branch first commit succeeds and becomes the branch's first commit [Requirement: Commit Succeeds on an Unborn Branch].
- [ ] 4c.5 `cargo test -p git-core`: detached-HEAD commit succeeds, `HEAD` moves to the new commit, no branch ref moves [Requirement: Commit Succeeds on a Detached HEAD].
- [ ] 4c.6 `cargo test -p git-core`: timeout ladder, via fake `git` shell scripts injected through `CommitRequest.git_override` (this is what makes the whole path testable in ~2 seconds, not minutes):
  - `sleep 5`, never exits, `timeout = 1s` → SIGTERM ladder fires → HEAD unchanged → `CommitTimedOut`;
  - moves HEAD then `sleep 5` → timeout fires but HEAD **moved** → `CommitOutcome { warning: TimedOutButHeadMoved }` — **the duplicate-commit bug, proven absent**;
  - `exit 128` with a stderr line → `CommitFailed`, stderr verbatim (M3 row 1's shape);
  - absent file as override → `GitUnavailable`;
  - `git var GIT_AUTHOR_IDENT` script exits 128 → `IdentityMissing`, refused **before** `git commit` is spawned.
- [ ] 4c.7 `cargo test -p git-core`: nothing-staged refusal fires before any subprocess is invoked — assert via a fake-`git` override that writes a sentinel file on invocation, and confirm the sentinel never appears [Requirement: Commit and Staging Refusals Use Distinct, Machine-Readable Codes — "Nothing staged" scenario].

---

## Phase 5 (Unit 5): UI panel + store + browser specs [Requirement: multiple, UI-facing; e2e-verification-harness browser-mode portion]

Per `openspec/config.yaml` `rules.tasks`: no task below claims "write a failing test first" — there is no JS/TS test runner installed. Verification is `tsc --noEmit` / `pnpm build`, manual exercise of each UI state, and the browser-mode wdio specs (mocked `invoke`, not a unit-test runner).

- [x] 5.1 `src/features/repo/api.ts`: `stagePaths`, `unstagePaths` invoke wrappers — `api.ts` stays the single `invoke()` call site. **Deviation**: `createCommit`, `gitProbe` NOT added — commit-shaped, deferred with the `commit` follow-up change (2026-08-20 delivery split).
- [x] 5.2 `src/features/repo/store.ts`: `staging: { busy, error: CoreErrorWire | null }`, `refreshStatus()` (status-only — no graph re-walk, no reselect), `stagePaths`/`unstagePaths` actions. **Deviation**: `gitProbe` field and `createCommit` action NOT added (deferred with commit); the "never fire while a commit is in flight" guard has no commit to guard against yet — `refreshStatus()` instead guards on its own `staging.busy`, which will still be correct once `createCommit` lands and sets the same flag.
- [x] 5.3 `src/features/working-directory/WorkingDirectoryPanel.tsx` — container, reads `status`/`staging` from the store (no `gitProbe` — deferred), owns no markup decisions.
- [x] 5.4 `src/features/working-directory/ChangeList.tsx` + `ChangeRow.tsx` — presentational, props in / callbacks out, rendered for both staged and unstaged lists.
- [ ] 5.5 `src/features/working-directory/CommitBox.tsx` — **NOT built.** Commit-shaped UI, explicitly out of scope per the 2026-08-20 delivery split ("Everything commit-shaped defers"). Ships with the `commit` follow-up change.
- [x] 5.6 `src/features/working-directory/RefusalNotice.tsx` — switches on `code`, never on message text. **Deviation**: hook/signer stderr quoted-block rendering NOT built — that path (`commitFailed`/`commitTimedOut` details) has no producer yet in this run; `RefusalNotice` covers the codes `stage`/`unstage` can actually raise today (`conflictsPresent`, `bareRepository`, `pathOutsideRepo`, plus a generic fallback for any other code).
- [x] 5.7 Manual verification pass (no JS/TS runner — per `config.yaml`): `pnpm build` clean (`tsc --noEmit` + `vite build`); exercised via the browser-mode specs below rather than a separate manual pass, since they drive the same UI states against the same mocked `invoke`. **Deviation**: commit-control, hook/signer stderr, and `Committing…` checklist items dropped — no commit UI exists yet.
- [x] 5.8 Browser-mode wdio specs (mocked `invoke`, milliseconds) — `e2e/browser/working-directory.spec.ts`, 4 specs, all green: stage-all/unstage-all enablement; a single stage updates the lists from the write outcome; bulk unstage empties the staged list; `conflictsPresent` refusal renders by code. Expected write payloads are derived **in the spec** by transforming the generated `working_status` — never hardcoded. **Deviation**: `git_probe`-gated commit-control state, hook/signer stderr rendering, and the `Committing…` state/note are NOT covered — no producer exists yet in this run.
- [ ] 5.9 `tools/git-fixtures/src/bin/dump-mocks.rs`: `git_probe` mock entry — **NOT added.** `git` resolution is commit-shaped and explicitly deferred; nothing in this run's UI reads a `git_probe` mock.

---

## Phase 6 (Unit 6): Harness — `writes` fixture + native write spec [Requirement: e2e-verification-harness delta — Write Specs Use Dedicated, Isolated Fixtures; Write-Path Test Coverage Is Split by Speed and Fidelity; Write Specs Never Assert on Rendered Date Text]

- [x] 6.1 `tools/git-fixtures/src/bin/build-fixture.rs`: parameterise `--name` as a **recipe** selector (currently hardcoded `let name = "history";` at line 57) — flags matching `dump-mocks.rs`'s existing `--repo`/`--out` style. Both defaults stay today's values (`--out-root target/e2e-fixtures`, `--name history`), so existing callers (`package.json`'s `e2e:mocks`, `wdio.native.conf.ts`'s `onPrepare`) need no argument changes. Verified: `cargo run -p git-fixtures --bin build-fixture` (no args) and `-- --name writes` both work; `determinism.rs` still green, `history`'s head OID unchanged.
- [x] 6.2 `tools/git-fixtures/src/spec.rs`: register a `writes` recipe builder alongside `history`'s. `history`'s builder is not touched by a single byte (its OIDs are asserted by `determinism.rs`, confirmed still green). `writes` content: three linear commits, a real checkout, nothing staged, two unstaged modifications, one untracked file. Pin `user.name`/`user.email`, `commit.gpgsign = false`, `core.hooksPath` → an empty in-fixture directory — all in the fixture's own **local** config. **Deviation**: implemented as a fully independent `build_writes()` in `lib.rs` rather than a shared/parameterised builder — sharing the commit-building loop risked `history`'s determinism guarantee drifting because of a refactor made for this recipe's sake; a few dozen duplicated lines were the safer trade.
- [x] 6.3 Manifest gains `initialStatus` (the fixture's own `repo.status()`, serialised) for **every** fixture — `history` gets the field for free. Verified by inspecting both generated manifests: `writes` shows `staged: []`, two `modified` unstaged entries, one `untracked` entry; `history`'s existing staged/unstaged dirt is now also reported through the same field.
- [ ] 6.4 `wdio.native.writes.conf.ts` — **NOT built.**
- [ ] 6.5 `e2e/native/writes/*.spec.ts` — **NOT built.**
- [ ] 6.6 Run the write spec on macOS — **NOT run** (no spec exists).
- [ ] 6.7 Run the write spec on Linux — **NOT run** (no spec exists).
- [ ] 6.8 *(Optional, non-blocking)* — not attempted; not needed while 6.4 doesn't exist.

**Why 6.4–6.8 are deferred, not attempted-and-cut-down.** `specs/e2e-verification-harness/spec.md`'s own requirement text is explicit and singular: *"Exactly one native spec MUST prove the end-to-end path — stage, **commit**, and the new commit appearing in the graph — against a dedicated fixture, on both macOS and Linux."* The scenario it's built from is "stages a file **and commits through the UI**". Commit does not exist in this run — no `create_commit` command, no `CommitBox` — per the 2026-08-20 delivery split ("Everything commit-shaped defers"). A spec that only stages would not satisfy this requirement's text (it proves a different, narrower claim), and building one anyway risked exactly the kind of scope confusion the split was meant to prevent: a "native write spec" that exists but doesn't mean what the spec says it means. The fixture and manifest infrastructure it would need (`writes` recipe, `--name` selector, `initialStatus`) is built and verified above; the spec itself, its wdio config, and both platform CI runs belong with the `commit` follow-up change, against this same fixture.

---

## Cross-cutting verification (run after every unit lands, per `openspec/config.yaml`)

- `cargo test -p git-core`
- `cargo clippy --workspace --all-targets`
- `cargo fmt --all --check`
- `pnpm build`
- `crates/git-core` still carries no Tauri/React imports (boundary check)
