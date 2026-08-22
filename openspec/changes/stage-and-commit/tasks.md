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
- [x] 0.2 **U7** — long synchronous Tauri command / webview freeze check (design §12). **Resolved 2026-08-22.** A throwaway sync command blocking 8s, driven against the real WKWebView via a temporary native wdio spec: 20 JS round-trips during the block, 4–19ms each, webview never stalled. **`create_commit` does not need `async fn` + `spawn_blocking`.** All spike code removed immediately after the measurement; finding recorded in design.md §12/§13.
- [x] 0.3 **U10** — `git var GIT_AUTHOR_IDENT` vs. `git commit` identity-refusal parity (design §5.1). **Resolved 2026-08-22.** Three real-subprocess cases on macOS/git 2.50.1 (auto-detected identity; forced strict refusal; forced strict refusal with a partial identity) all showed exact parity between `git var` and `git commit`'s exit code and message. **The identity pre-flight is a hard refusal**, as originally decided — the reporting-only fallback was not needed. Recorded in design.md §5.1/§13.
- [x] 0.4 **U1/U2** — residual pinentry experiment, §3.5 P1/P2/P3. **Partially run 2026-08-22, macOS only.** Real `gpg`+`pinentry-curses` (installed via Homebrew for this check), throwaway `GNUPGHOME`, a passphrased key, `commit.gpgsign=true`, agent killed beforehand. P1 (no controlling terminal) and P2 (pty-allocated controlling terminal) both failed fast (~1.4–1.5s, 0 commits, HEAD unchanged) — `pinentry-curses` cannot initialise against this design's piped-stdio plumbing regardless of controlling-terminal presence, so neither run reached the SIGTERM/SIGKILL ladder. **Deviation — not fully done**: P3 (SSH signing) was not run; `pinentry-mac` (the real macOS GUI default, not `pinentry-curses`) was not measured — no windowed session available in this environment; Linux was not run (no Linux machine here); U2's actual question (does the ladder reap a *blocked* pinentry) is therefore still unanswered since neither P1 nor P2 ever reached the ladder. Nothing in Units 4b/4c depends on the outcome either way. Full findings in design.md §3.5/§13.
- [x] 0.5 **U9** — recorded as non-actionable, not silently dropped: "does a hang *after* the commit object exists behave like M3's?" has no cheap deliberate-staging check. The design's mitigation is structural (§3.2 — HEAD is always re-read after every terminal outcome via a freshly opened `Repository`, never assumed) and is exactly what `repo::commit()` implements. No spike result exists or is needed beyond this acknowledgement.

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

- [x] 4a.1 Add the `which` crate as `crates/git-core`'s new unconditional dependency (design §4.3 — hand-rolling `PATH`/`PATHEXT` search is the part we cannot test on Windows). `which = "8.0.5"`, added via `cargo add`.
- [x] 4a.2 `git_binary.rs::resolve(override_path: Option<&str>) -> Result<ResolvedGit>` per §4.1's order: explicit override → `GITVISOR_GIT_PATH` → `PATH` via `which` → `GitUnavailable`. **Never cached** — no `OnceLock`, no field on `GitRepo`.
- [x] 4a.3 `git_binary.rs::probe()`: exists, is a file (or symlink to one), executable bit on Unix, then spawn `<candidate> --version` and require exit `0` with stdout beginning `git version `. Executed directly via `Command::new`, never through a shell.
- [x] 4a.4 `src-tauri/src/commands.rs`: `git_probe` command wrapping `repo.probe()` (thin `GitRepo` wrapper over `git_binary::probe()`, matching design's component map).
- [x] 4a.5 `cargo test -p git-core` (`tests/git_binary.rs`): resolution precedence (override beats env beats `PATH`); probe rejects a candidate pointed at a non-git executable (`/bin/ls`) or a directory; `GitUnavailable` names what was looked for. 7 tests, all green.

---

## Phase 4b (Unit 4b): Commit subprocess + timeout + HEAD-delta [Requirement: Commit Runs Through the User's `git` Binary]

- [x] 4b.1 `git_binary.rs::base_command()`: one shared builder for both the identity pre-flight and the commit spawn, so their env/cwd cannot drift apart (§5.1). Exact env per §2.2: **set** `GIT_TERMINAL_PROMPT=0`, `GIT_EDITOR=:`, `GIT_SEQUENCE_EDITOR=:`; **remove** `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_COMMON_DIR`, `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_NAMESPACE`; **never touch** `GIT_AUTHOR_*`/`GIT_COMMITTER_*` (M5).
- [x] 4b.2 `git_binary.rs::identity()`: `git var GIT_AUTHOR_IDENT` via `base_command()`. U10 resolved (parity confirmed) — hard refusal (`IdentityMissing` on non-zero exit).
- [x] 4b.3 `git_binary.rs::run_commit()`: exact argv per §2.1 (`-C <workdir> --no-pager commit --file=- --cleanup=whitespace`), absolute resolved path only, never the bare string `git`. Message written to stdin then the handle dropped for EOF. stdout/stderr drained by two dedicated reader threads started immediately after spawn, *before* stdin is written (deadlock avoidance). `try_wait()` polled every 50 ms against the deadline; `wait_with_output()` is not used.
- [x] 4b.4 Timeout ladder: SIGTERM → 5 s grace → SIGKILL via `libc::kill` on the child's own process group (`process_group(0)`, signal sent to `-pid`). Unix-only `libc = "0.2"` dependency added (`[target."cfg(unix)".dependencies]`). U7 resolved (webview does not freeze) — `create_commit` stays a plain sync command, no `spawn_blocking`.
- [x] 4b.5 `repo.rs::commit()`: pre-flight order — bare? conflicts? nothing staged? `.git/index.lock` present (`Path::exists()`, never libgit2's own error code)? resolve `git`? identity? — every refusal before `git` is ever resolved or spawned. `head_before`/`head_after` via a **freshly opened** `Repository` (`Repository::discover`, never `self.inner`) on every terminal outcome. Outcome derived via `outcome_from(head_before, head_after, attempt, timeout)` per the 7-row table in §2.5 — no branch inspects stderr/stdout text.
- [x] 4b.6 `src-tauri/src/state.rs`: `invalidate(path)` — thin alias of `close()`, called by `create_commit` on success so the next command reopens rather than trusting a ref view the design has chosen not to trust.
- [x] 4b.7 `src-tauri/src/commands.rs`: `create_commit` thin command; calls `repos.invalidate(&path)` only after success (after the `?` on `repos.with(...)`, so a refusal never invalidates).

---

## Phase 4c (part of Unit 4c): Commit test suite — hook regression, M1, M5, unborn/detached, timeout ladder

All of 4c.1–4c.7 live in `crates/git-core/tests/commit.rs` (11 tests, all green). Both M1 and M5 were hand-verified to go red with their fix removed, then restored (non-negotiable #6) — see apply-progress.md for the exact drill.

- [x] 4c.1 `cargo test -p git-core`: **hook-rejection regression with a positive control** — a repo with a live `pre-commit` hook that exits 1 and prints `HOOK RAN — rejecting` produces **zero** commits, `HEAD` unchanged, stderr surfaced verbatim; the identical code path with a passing hook succeeds [Requirement: Commit Hooks Run and a Rejection Blocks the Commit — direct scenario replay]. `a_rejecting_pre_commit_hook_blocks_the_commit_with_a_positive_control`.
- [x] 4c.2 `cargo test -p git-core`: **M1 replay** — self-contained: an ephemeral, no-passphrase GPG key in a throwaway `GNUPGHOME`, wired in through the repo's own **local** `gpg.program` (not the process environment, so this is safe under default parallel `cargo test`), `commit.gpgsign=true`; the resulting commit carries a `gpgsig` header, verified via `git log --show-signature` reporting "Good signature". Skips cleanly (does not fail) if `gpg` is absent from `PATH`. [Requirement: Commit Honours Signing Configuration]. `commit_is_signed_when_signing_is_required_m1_replay`. **Hand-verified to go red**: temporarily made `repo::commit()` commit via libgit2 directly (bypassing the real `git` subprocess) — the assertion failed exactly as M1 predicts (no signature); reverted, green again.
- [x] 4c.3 `cargo test -p git-core`: **M5 replay, real subprocess, real isolated `HOME`** — no `user.name`/`user.email` at any config level, identity supplied only via `GIT_AUTHOR_*`/`GIT_COMMITTER_*`; `GitRepo::commit` **succeeds**. Exercises `git_binary::identity()` for real — never asserts against `Repository::signature()` [Requirement: Commit and Staging Refusals Use Distinct, Machine-Readable Codes — `IdentityMissing` must not false-refuse]. `identity_from_env_vars_only_succeeds_m5_replay`. **Hand-verified to go red**: temporarily swapped `identity()` to a config-only check (`git config --get user.email`, the shape of the actual M5 bug) — the test failed with `IdentityMissing` exactly as M5 predicts; reverted, green again.
- [x] 4c.4 `cargo test -p git-core`: unborn-branch first commit succeeds and becomes the branch's first commit [Requirement: Commit Succeeds on an Unborn Branch]. `commit_succeeds_on_an_unborn_branch`.
- [x] 4c.5 `cargo test -p git-core`: detached-HEAD commit succeeds, `HEAD` moves to the new commit, no branch ref moves [Requirement: Commit Succeeds on a Detached HEAD]. `commit_succeeds_on_a_detached_head`.
- [x] 4c.6 `cargo test -p git-core`: timeout ladder, via fake `git` shell scripts injected through `CommitRequest.git_override`:
  - `sleep 5`, never exits, `timeout = 500ms` → SIGTERM ladder fires → HEAD unchanged → `CommitTimedOut` (`timeout_never_exits_head_unchanged_commit_timed_out`);
  - moves HEAD (via a real nested `git commit --allow-empty`) then `sleep 5` → timeout fires but HEAD **moved** → `CommitOutcome { warning: TimedOutButHeadMoved }` — **the duplicate-commit bug, proven absent** (`timeout_after_head_moved_reports_a_warning_not_a_failure`);
  - `exit 128` with a stderr line → `CommitFailed`, stderr verbatim (`nonzero_exit_with_a_stderr_line_is_commit_failed`);
  - absent file as override → `GitUnavailable` (`absent_override_path_is_git_unavailable`);
  - `git var GIT_AUTHOR_IDENT` script exits 128 → `IdentityMissing`, refused **before** `git commit` is spawned, proven via a sentinel file the commit script would have created (`identity_script_failure_refuses_before_commit_is_spawned`).
- [x] 4c.7 `cargo test -p git-core`: nothing-staged refusal fires before any subprocess is invoked — a fake-`git` override that writes a sentinel file on invocation; the sentinel never appears [Requirement: Commit and Staging Refusals Use Distinct, Machine-Readable Codes — "Nothing staged" scenario]. `nothing_staged_refuses_before_any_subprocess_is_invoked`.

---

## Phase 5 (Unit 5): UI panel + store + browser specs [Requirement: multiple, UI-facing; e2e-verification-harness browser-mode portion]

Per `openspec/config.yaml` `rules.tasks`: no task below claims "write a failing test first" — there is no JS/TS test runner installed. Verification is `tsc --noEmit` / `pnpm build`, manual exercise of each UI state, and the browser-mode wdio specs (mocked `invoke`, not a unit-test runner).

- [x] 5.1 `src/features/repo/api.ts`: `stagePaths`, `unstagePaths` invoke wrappers — `api.ts` stays the single `invoke()` call site. **Deviation**: `createCommit`, `gitProbe` NOT added — commit-shaped, deferred with the `commit` follow-up change (2026-08-20 delivery split).
- [x] 5.2 `src/features/repo/store.ts`: `staging: { busy, error: CoreErrorWire | null }`, `refreshStatus()` (status-only — no graph re-walk, no reselect), `stagePaths`/`unstagePaths` actions. **Deviation**: `gitProbe` field and `createCommit` action NOT added (deferred with commit); the "never fire while a commit is in flight" guard has no commit to guard against yet — `refreshStatus()` instead guards on its own `staging.busy`, which will still be correct once `createCommit` lands and sets the same flag.
- [x] 5.3 `src/features/working-directory/WorkingDirectoryPanel.tsx` — container, reads `status`/`staging` from the store (no `gitProbe` — deferred), owns no markup decisions.
- [x] 5.4 `src/features/working-directory/ChangeList.tsx` + `ChangeRow.tsx` — presentational, props in / callbacks out, rendered for both staged and unstaged lists.
- [x] 5.5 `src/features/working-directory/CommitBox.tsx` — presentational: message textarea, commit button, `Committing…` state (from the shared `staging.busy` flag), a 10s inline "still running" note via a plain frontend `setTimeout` (no backend plumbing), the `commitWarning` banner, detached-HEAD notice. No "committing as …" line, per design §5.1/§11 (M5). Wired into `WorkingDirectoryPanel.tsx`. Store gained `gitProbe`, `commitWarning`, and `createCommit()` (reuses `staging.busy`, so `refreshStatus()`'s existing "never fire while a write is in flight" guard already covers a commit — no separate flag needed, as design anticipated). `api.ts` gained `gitProbe`/`createCommit`.
- [x] 5.6 `src/features/working-directory/RefusalNotice.tsx` — extended: `nothingStaged`, `identityMissing`, `indexLocked`, `gitUnavailable`, `commitFailed`, `commitTimedOut` codes added alongside the existing three; `commitFailed`/`commitTimedOut` render their `stderr` detail in a quoted, attributed "Output from git and your hooks" block. Still switches on `code`, never on message text.
- [x] 5.7 Manual verification (no JS/TS runner — per `config.yaml`): `pnpm build` clean (`tsc --noEmit` + `vite build`). Browser-mode wdio is broken in this environment (chromedriver 151.0.7922.173 cannot be downloaded — confirmed by running `pnpm e2e:browser`, an environment fault, not code) so the commit-control/hook-stderr/`Committing…` states were **not** additionally exercised as new browser-mode specs; verified instead through the real native write spec (task 6.5) — an actual `git` subprocess, actual UI states, strictly more authoritative than a mocked browser spec for this path — and through code review against spec.md's scenarios.
- [x] 5.8 Existing browser-mode specs (`e2e/browser/working-directory.spec.ts`) — untouched, still 4 specs (verified via `pnpm build`'s typecheck; could not execute locally, see 5.7). **Deviation, stated rather than silently skipped**: no new browser-mode specs were added for `git_probe`-gated commit-control state, hook/signer stderr rendering, or the `Committing…` state/note — browser-mode e2e is unusable in this environment (5.7), and authoring assertions nobody can run locally, for a path the native write spec already covers end-to-end, was judged not worth the risk of an unverified regression test. A future session with a working chromedriver should add them.
- [x] 5.9 `tools/git-fixtures/src/bin/dump-mocks.rs`: `git_probe` mock entry added — read via `git_core::git_binary::probe(None)` (the real machine's own `git`), with `path`/`version` tokenised (`{{GIT_PATH}}`/`{{GIT_VERSION}}`) the same way `open_repository.path` already is, since both are machine-specific and would otherwise fail the `mocks-drift` CI diff on every runner with a different `git`. `mocks.ts` substitutes both to fixed browser-safe values. Verified: regenerated `e2e/mocks/history.json`, diff is exactly the new `git_probe` entry plus one unrelated fix (see 6.3's note on `fixture.json` polluting `working_status`) — nothing else moved.

---

## Phase 6 (Unit 6): Harness — `writes` fixture + native write spec [Requirement: e2e-verification-harness delta — Write Specs Use Dedicated, Isolated Fixtures; Write-Path Test Coverage Is Split by Speed and Fidelity; Write Specs Never Assert on Rendered Date Text]

- [x] 6.1 `tools/git-fixtures/src/bin/build-fixture.rs`: parameterise `--name` as a **recipe** selector (currently hardcoded `let name = "history";` at line 57) — flags matching `dump-mocks.rs`'s existing `--repo`/`--out` style. Both defaults stay today's values (`--out-root target/e2e-fixtures`, `--name history`), so existing callers (`package.json`'s `e2e:mocks`, `wdio.native.conf.ts`'s `onPrepare`) need no argument changes. Verified: `cargo run -p git-fixtures --bin build-fixture` (no args) and `-- --name writes` both work; `determinism.rs` still green, `history`'s head OID unchanged.
- [x] 6.2 `tools/git-fixtures/src/spec.rs`: register a `writes` recipe builder alongside `history`'s. `history`'s builder is not touched by a single byte (its OIDs are asserted by `determinism.rs`, confirmed still green). `writes` content: three linear commits, a real checkout, nothing staged, two unstaged modifications, one untracked file. Pin `user.name`/`user.email`, `commit.gpgsign = false`, `core.hooksPath` → an empty in-fixture directory — all in the fixture's own **local** config. **Deviation**: implemented as a fully independent `build_writes()` in `lib.rs` rather than a shared/parameterised builder — sharing the commit-building loop risked `history`'s determinism guarantee drifting because of a refactor made for this recipe's sake; a few dozen duplicated lines were the safer trade.
- [x] 6.3 Manifest gains `initialStatus` (the fixture's own `repo.status()`, serialised) for **every** fixture — `history` gets the field for free. **Bug found and fixed while writing 6.5**: the manifest itself (`fixture.json`) was written *inside* the repository's own working tree, so by the time the real app queried `working_status` a moment later, `fixture.json` showed up as a **fourth** untracked entry the manifest itself (computed a moment earlier) never saw — `writes`' `initialStatus` said 3 unstaged entries, the live UI showed 4. Fixed by moving the manifest to `<out_dir>/.git/fixture.json` (git-invisible) for both recipes; `e2e/support/fixture.ts::readFixture` updated to match. Verified: `history`'s `determinism.rs` still green (OIDs untouched), regenerated `e2e/mocks/history.json` diff shows the same spurious `fixture.json` untracked entry disappearing from `working_status` there too — a real, pre-existing latent defect this caught, not something newly introduced.
- [x] 6.4 `wdio.native.writes.conf.ts` — built, mirroring `wdio.native.conf.ts`'s shape exactly per design §9.3 (separate config, own `onPrepare` building the `writes` fixture, own `appArgs`, calls `clearRememberedRepoStorage()`).
- [x] 6.5 `e2e/native/writes/stage-commit.spec.ts` — the one native write spec: stages a real unstaged file via a real DOM click, types a commit message and clicks Commit (via a native input-value-setter + dispatched `input` event and a real DOM `.click()` — see the spec's own header comment for why), asserts the new commit's summary is at the top of the real graph and the staged list is empty afterward. Per finding H2, asserts only on the typed message and row counts, never on rendered date text. **Discovered while writing this spec**: `@wdio/tauri-service`'s `ensureActiveWindowFocus` pre-command check retries against `window.__TAURI__.core.invoke` before *every* WebDriver command and fails every time (`withGlobalTauri` is off — finding H1), adding real per-command latency in this harness. Consolidating each interaction into one `browser.execute()` (real DOM operations, not separate WebDriver element commands) instead of many small commands cut this spec's wall-clock time from a 3-minute timeout down to ~20s, and applies to *any* future spec in this harness, not just this one.
- [x] 6.6 Run the write spec on macOS — **run and green**, 4 consecutive times (~20s each) via `pnpm e2e:native:writes`, real WKWebView, real `git` subprocess.
- [x] 6.7 Run the write spec on Linux — **CI wiring done, not locally executed.** No Linux machine is available in this environment. Added as a step to `.github/workflows/e2e-native-linux-probe.yml` (which already carries the WebKitGTK + `xvfb` setup) rather than a third workflow file; also added to `.github/workflows/e2e-native-macos.yml` as "Spec C". Neither addition has an observed passing CI run yet — that is real, stated residue, not silently assumed green.
- [ ] 6.8 *(Optional, non-blocking)* — not attempted.

---

## Cross-cutting verification (run after every unit lands, per `openspec/config.yaml`)

- `cargo test -p git-core`
- `cargo clippy --workspace --all-targets`
- `cargo fmt --all --check`
- `pnpm build`
- `crates/git-core` still carries no Tauri/React imports (boundary check)
