# Tasks: Stage and Commit

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

- [ ] 0.1 **U3** — `clippy.toml` `disallowed-methods` path-resolution spike (5 min, design §1.3): add `clippy.toml`, add a throwaway `let _ = self.inner.index();` in `repo.rs`, run `cargo clippy -p git-core`, confirm it errors, delete the line. **This decides Unit 2's shape**: if resolution works, the lint is the enforcement mechanism; if not, fall back to `crates/git-core/tests/index_discipline.rs` — a source-scan test asserting `.index()` appears in exactly one file (same technique as the harness change's release-artifact byte scan). Record the result in design.md's unverified register.
- [ ] 0.2 **U7** — long synchronous Tauri command / webview freeze check (2 min, design §12): point a throwaway repo's `pre-commit` hook at `sleep 30`, commit through a minimal stand-in command (or wait until 4b exists and re-run then if sequencing is easier — but the decision must land before 4b is written, not after), try to scroll the graph. **This decides whether Unit 4b's `create_commit` command needs `async fn` + `spawn_blocking`** — a one-line change, but on evidence, not belief about Tauri's threading model.
- [ ] 0.3 **U10** — `git var GIT_AUTHOR_IDENT` vs. `git commit` identity-refusal parity (5 min, design §5.1): isolated `HOME`, no config, no env identity — compare exit codes of `git var GIT_AUTHOR_IDENT` and `git commit`; repeat with a gecos name available but no email. **This decides Unit 4b's identity pre-flight strictness**: if `git var` is stricter, the pre-flight becomes reporting-only rather than a hard refusal, per §5.1's stated asymmetry.
- [ ] 0.4 **U1/U2** — residual pinentry experiment, §3.5 P1/P2/P3, folding in U2's `pgrep` survivor check: real `gpg` + `pinentry-curses` with no controlling terminal (P1), with an inherited controlling terminal (P2), and SSH signing with a passphrased key and no agent (P3). Throwaway repo, `commit.gpgsign=true`, throwaway `GNUPGHOME`, `gpgconf --kill gpg-agent`. Record: blocked or not, elapsed, whether the SIGTERM ladder terminated it, `pgrep -l 'pinentry|gpg-agent|ssh-keygen'` survivors after the kill, whether HEAD moved. Run on **macOS and Linux**. Nothing in Units 4b/4c structurally depends on the outcome (the timeout + always-read-HEAD handles any hang shape), but a surviving pinentry or an unexpected HEAD state would change the timeout UI copy in Unit 5, so this runs before that copy is written.
- [ ] 0.5 **U9** — record as non-actionable rather than silently dropped: "does a hang *after* the commit object exists behave like M3's?" has no cheap deliberate-staging check (design §13: "Hard to stage deliberately"). The design's mitigation is structural (§3.2 — HEAD is always re-read after every terminal outcome, never assumed), not a spike result. No task beyond this acknowledgement; carried into Unit 4b's design as the reason the outcome table has no third branch.

---

## Phase 1 (Unit 1): Structured errors + wire shape [Requirement: Commit and Staging Refusals Use Distinct, Machine-Readable Codes]

- [ ] 1.1 `crates/git-core/src/error.rs`: add 9 refusal variants (`GitUnavailable`, `CommitFailed`, `CommitTimedOut`, `IdentityMissing`, `ConflictsPresent`, `IndexLocked`, `BareRepository`, `NothingStaged`, `PathOutsideRepo`) per design §5.1. Keep the existing 3 variants' `Display` text byte-identical — no rewording.
- [ ] 1.2 `error.rs`: `code()` returning the stable `camelCase` `&'static str` for all 12 variants.
- [ ] 1.3 `error.rs`: hand-written `Serialize` via `serialize_map` → `{ code, message, details? }`; `details` only for variants carrying structure (`commitFailed`, `commitTimedOut`, `conflictsPresent`, `indexLocked`, `gitUnavailable`, `pathOutsideRepo`). No `serde_json` dependency added.
- [ ] 1.4 `src/features/repo/store.ts`: add the `CoreErrorWire` type, `isWire()`, `asCoreError()`; update `describe()` to check the wire-shape branch **first**, string/Error/JSON fallbacks unchanged. Manually confirm one existing command's error path (e.g. `open_repository` on a bad path) still renders the identical string as before this change — `message` is `self.to_string()`, so it must be.
- [ ] 1.5 `cargo test -p git-core`: `code()` returns a distinct value for every variant (a simple enumeration test — this is what makes "never collapsed into one generic failure" mechanically checked, not just asserted in prose).

---

## Phase 2 (Unit 2): Paths + index guard + clippy gate — the M2 replay [Requirement: Stage a Single Working-Tree Path; External Staging Is Never Destroyed]

- [ ] 2.1 Add `clippy.toml` at the workspace root per design §1.3's full list (index/reset/checkout/add_all/remove_all/update_all/signature denials), using whichever mechanism 0.1 determined. Add `#![deny(clippy::disallowed_methods)]` to `crates/git-core/src/lib.rs`.
- [ ] 2.2 `crates/git-core/src/paths.rs`: pure `normalise_repo_path(input) -> Result<String>` per design §6.2 — reject NUL/empty, reject absolute paths, fold `.`/`..` (empty-stack pop → `PathOutsideRepo`), reject an empty result, reject a first component that lowercases to `.git`, re-join on `/`. No I/O.
- [ ] 2.3 Unit tests for the normaliser (no repository on disk, microseconds each): M4's four rows (`inside.txt` accepted, `../outside.txt` refused, `/etc/hosts` refused, `sub/../inside.txt` accepted — corrects the raw libgit2 `NotFound` M4 recorded), plus `a[b].txt`, `.git/config`, and a NUL byte.
- [ ] 2.4 `crates/git-core/src/index_guard.rs` (private module): `with_fresh_index()` — `Repository::index()`, then **hard** `index.read(true)` before the closure runs, `mutate(&repo, &mut index)`, `index.write()` on the success path only (§1.1). `reload_index()` — forces the index back in sync without writing.
- [ ] 2.5 `crates/git-core/tests/index_freshness.rs` — **the M2 replay, real subprocess**: create repo, commit A, write `b.txt` and `c.txt`, run a **real `git add b.txt`** as a subprocess (not an in-process `index.add_path` — an in-process write shares libgit2's in-memory state and would not reproduce the measured condition), then `GitRepo::stage(["c.txt"])`, assert `status().staged == ["b.txt", "c.txt"]`.
- [ ] 2.6 `cargo test -p git-core`: a closure that returns `Err` mid-mutation leaves the on-disk index byte-identical to before the call (proves §1.5's "refuse before mutating anything" property is structural, not a discipline).

---

## Phase 3 (Unit 3): Stage / unstage + thin commands [Requirement: Stage a Single Working-Tree Path; Unstage a Single Working-Tree Path; Bulk Stage and Unstage Operate Only on Listed Entries; Listings Are Deterministically Ordered]

- [ ] 3.1 `repo.rs::stage(&self, paths: &[String])`: validate every path via `normalise_repo_path` first (whole batch fails before `write()` on any bad path — §7.2), then inside one `with_fresh_index` call: `Index::add_path` for files present on disk, `Index::remove_path` for files gone from disk but present in the index/HEAD (staging a deletion), skip-and-report (`SkipReason::Vanished`) for files in neither disk, index, nor HEAD.
- [ ] 3.2 `repo.rs::unstage(&self, paths: &[String])`: manual index-entry restoration per §8 — HEAD exists and path is in the HEAD tree → rebuild the `IndexEntry` from the tree entry (stat fields zeroed) and `index.add(&entry)`; HEAD unborn or path absent from the HEAD tree → `index.remove_path`. **No** `reset_default`, `checkout_*`. The file on disk is never read, written, or deleted.
- [ ] 3.3 `model.rs`: `WriteOutcome { status: WorkingStatus, skipped: Vec<SkippedPath> }`, `SkippedPath { path, reason }`, `SkipReason::Vanished` — `skipped` sorted at construction (D9 applies here too).
- [ ] 3.4 Shared pre-flight (used by stage, unstage, and — later — commit): conflicted paths present → `ConflictsPresent { paths }`; bare repository → `BareRepository`; both refuse before any mutation.
- [ ] 3.5 `src-tauri/src/commands.rs`: `stage_paths`, `unstage_paths` — each one `repos.with(&path, …)` line, no branching, no message building.
- [ ] 3.6 `cargo test -p git-core`:
  - a modified file stages and nothing else is touched (Requirement 1's scenario);
  - a staged file with local edits unstages and its on-disk content is byte-identical before/after (hash comparison — Requirement 2's scenario);
  - "stage all" over 3 listed paths + 1 untracked, un-ignored build artifact stages exactly the 3 and leaves the artifact untracked (Requirement 3's scenario, replayed literally);
  - `a[b].txt` stages and unstages without touching a sibling (§6.3/§8, glob-safety proof);
  - unstage on an unborn branch removes the entry rather than erroring;
  - a batch containing one vanished path is skipped-and-reported, not a batch failure;
  - conflicted-path refusal fires before any mutation for stage, unstage, and (stubbed) commit entry points.
- [ ] 3.7 `cargo test -p git-core`: the same staged/unstaged/conflicted listing read on a synthetic case-sensitive vs. case-insensitive path ordering scenario returns identical order (Requirement 12), via the existing sorted `status()` — no raw index iteration introduced anywhere in 3.1–3.2.

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

- [ ] 5.1 `src/features/repo/api.ts`: `stagePaths`, `unstagePaths`, `createCommit`, `gitProbe` invoke wrappers — `api.ts` stays the single `invoke()` call site.
- [ ] 5.2 `src/features/repo/store.ts`: `gitProbe: GitProbe | null`, `staging: { busy, error: CoreErrorWire | null }`, `refreshStatus()` (status-only — no graph re-walk, no reselect), `stagePaths`/`unstagePaths`/`createCommit` actions. Guard: `refreshStatus()` must never fire while a commit is in flight (§3.4/§12 — a queued refresh landing late is indistinguishable from a freeze).
- [ ] 5.3 `src/features/working-directory/WorkingDirectoryPanel.tsx` — container, reads `status`/`gitProbe`/`staging` from the store, owns no markup decisions.
- [ ] 5.4 `src/features/working-directory/ChangeList.tsx` + `ChangeRow.tsx` — presentational, props in / callbacks out, rendered for both staged and unstaged lists.
- [ ] 5.5 `src/features/working-directory/CommitBox.tsx` — message textarea, commit button, `Committing…` disabled state, a plain frontend `setTimeout` 10-second inline note (no backend plumbing, no Tauri event). **No "committing as …" line** — M5 makes any author Gitvisor could compute untrustworthy unless it comes from `git var` (§5.1, §11).
- [ ] 5.6 `src/features/working-directory/RefusalNotice.tsx` — switches on `code`, never on message text; renders hook/signer stderr in a quoted, attributed block ("Output from git and your hooks").
- [ ] 5.7 Manual verification pass (no JS/TS runner — per `config.yaml`): `pnpm build` clean; exercise commit control shown-and-disabled with the "git not found" message when `git_probe.available === false`; exercise each refusal `code` rendering its specific message; exercise hook/signer stderr rendering quoted and attributed; exercise the `Committing…` state and its 10-second note; exercise stage-all/unstage-all button enablement; exercise status refresh after a stage and a full refresh after a commit.
- [ ] 5.8 Browser-mode wdio specs (mocked `invoke`, milliseconds): commit control disabled + reason when `git_probe.available === false`; each refusal `code` → its specific rendered message; hook/signer stderr rendered quoted and attributed; the `Committing…` state and its note; stage-all/unstage-all enablement; status refresh after a write. Expected write payloads are derived **in the spec** by transforming the generated `working_status` (move entry X from `unstaged` to `staged`) — never hardcoded.
- [ ] 5.9 `tools/git-fixtures/src/bin/dump-mocks.rs`: add a `git_probe` mock entry, produced by **reading** (never by executing `stage_paths`/`unstage_paths`/`create_commit` — mock generation mutating a repository would be a fabrication), with its machine-specific path/version behind the existing `{{FIXTURE_PATH}}`-style token substitution. `mocks-drift` CI job keeps working unchanged.

---

## Phase 6 (Unit 6): Harness — `writes` fixture + native write spec [Requirement: e2e-verification-harness delta — Write Specs Use Dedicated, Isolated Fixtures; Write-Path Test Coverage Is Split by Speed and Fidelity; Write Specs Never Assert on Rendered Date Text]

- [ ] 6.1 `tools/git-fixtures/src/bin/build-fixture.rs`: parameterise `--name` as a **recipe** selector (currently hardcoded `let name = "history";` at line 57) — flags matching `dump-mocks.rs`'s existing `--repo`/`--out` style. Both defaults stay today's values (`--out-root target/e2e-fixtures`, `--name history`), so existing callers (`package.json`'s `e2e:mocks`, `wdio.native.conf.ts`'s `onPrepare`) need no argument changes.
- [ ] 6.2 `tools/git-fixtures/src/spec.rs`: register a `writes` recipe builder alongside `history`'s. `history`'s builder is not touched by a single byte (its OIDs are asserted by `determinism.rs`). `writes` content: three linear commits, a real checkout, nothing staged, two unstaged modifications, one untracked file. Pin `user.name`/`user.email`, `commit.gpgsign = false`, `core.hooksPath` → an empty in-fixture directory — all in the fixture's own **local** config, per the "nothing ambient leaks in" rule `git-fixtures` already documents for OIDs.
- [ ] 6.3 Manifest gains `initialStatus` (the fixture's own `repo.status()`, serialised) for **every** fixture — `history` gets the field for free.
- [ ] 6.4 `wdio.native.writes.conf.ts` — a **separate config**, not per-spec `appArgs` on the shared `wdio.native.conf.ts` (sidesteps U4, which is unverified and the project has already been burned once by betting on `@wdio/tauri-service` internals — see the harness change's U3). Own `specs: ["./e2e/native/writes/**"]`, own `onPrepare` building the `writes` fixture and setting `appArgs`, must call `clearRememberedRepoStorage()` (more load-bearing here — a write run leaves `gitvisor:last-repo` pointing at `writes`, and a later `history` run would otherwise open the wrong repository). Add `pnpm e2e:native:writes` script and its own CI job.
- [ ] 6.5 `e2e/native/writes/*.spec.ts` — the **single** native write spec: real binary built with `pnpm run e2e:build` (never a plain `cargo build` — `onPrepare` refuses it), against the dedicated `writes` fixture: stage a file, commit through the UI, assert the new commit appears in the rendered graph. Assertions on commit message, author, and graph position only — **zero assertions on rendered date text** (H2).
- [ ] 6.6 Run the write spec on macOS; in the same suite invocation, run the existing `history`-fixture read spec afterward and confirm it shows no staged/unstaged/committed changes introduced by the write spec (Requirement's own scenario, replayed literally).
- [ ] 6.7 Run the write spec on Linux (same job class as the already-promoted `e2e-native-linux-probe.yml`) — both platforms are required by the Requirement text, not optional coverage.
- [ ] 6.8 *(Optional, non-blocking, only relevant if someone later wants to collapse the two wdio configs — design §9.3 C1, 5 min):* add `beforeSession` logging to `wdio.native.conf.ts` and run it over both existing specs with no `--spec` filter; one session per spec file would make per-spec `appArgs` viable and the separate config unnecessary. Nothing in this change waits on the result.

---

## Cross-cutting verification (run after every unit lands, per `openspec/config.yaml`)

- `cargo test -p git-core`
- `cargo clippy --workspace --all-targets`
- `cargo fmt --all --check`
- `pnpm build`
- `crates/git-core` still carries no Tauri/React imports (boundary check)
