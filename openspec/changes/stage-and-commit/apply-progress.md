# Apply Progress: stage-and-commit

Merged across both delivery-split runs (see `tasks.md`'s "Delivery decision"
note): the **`stage-unstage`** half (2026-08-20) and the **`commit`** half
(2026-08-22, this run). All 59 tasks in `tasks.md` are now checked except
one explicitly optional item (6.8).

## Status

58/59 tasks checked. The one unchecked task is 6.8, explicitly marked
"Optional, non-blocking" in the design and never attempted. Everything else
— including every item the first run deferred (`git` resolution, the commit
subprocess, the timeout ladder, hooks, signing, identity via `git var`,
HEAD-delta reporting, `CommitBox`, the native write spec and its CI wiring)
— is done in this run. Two residual gaps are stated plainly below, not
hidden: 6.7 (the write spec's Linux CI job is wired but has no observed
passing run — no Linux machine in this environment) and 0.4 (U1/U2's pinentry
experiment ran P1/P2 on macOS only; P3, `pinentry-mac`, and Linux were not
run — nothing in the design depends on the outcome either way).

## This run (commit half) — phase by phase

### Phase 0 — Spike gate, resolved
- **0.2 (U7) — resolved.** A throwaway sync `#[tauri::command]` blocking 8s,
  driven against the real WKWebView via a temporary native wdio spec: 20 JS
  round-trips during the block, 4–19ms each, webview never stalled.
  `create_commit` does not need `async fn` + `spawn_blocking`. All spike code
  removed immediately after the measurement (`git status` confirmed clean
  before continuing).
- **0.3 (U10) — resolved.** Real subprocesses, isolated `HOME`, three cases
  (auto-detected identity; forced strict refusal; forced strict refusal with
  a partial identity) — `git var GIT_AUTHOR_IDENT` and `git commit` agreed in
  every case. The identity pre-flight is a hard refusal, as originally
  designed.
- **0.4 (U1/U2) — partially run.** Real `gpg` + `pinentry-curses` (installed
  via Homebrew for this check — not present in the base environment),
  throwaway `GNUPGHOME`, a passphrased key, `commit.gpgsign=true`, agent
  killed beforehand. P1 (no controlling terminal — confirmed via `tty` that
  this shell genuinely has none) and P2 (pty-allocated controlling terminal
  via `script`) both failed fast, ~1.4–1.5s, 0 commits, HEAD unchanged —
  `pinentry-curses` cannot initialise against this design's piped-stdio
  plumbing regardless of controlling-terminal presence, so neither run
  reached the SIGTERM/SIGKILL ladder. **Not run**: P3 (SSH signing),
  `pinentry-mac` (the actual macOS GUI default — no windowed session
  available here), and anything on Linux. U2's real question (does the
  ladder reap a *blocked* pinentry) is therefore still open. Full findings
  in `design.md` §3.5 and §13.
- **0.5 (U9) — acknowledged, no task exists.** Structural mitigation only
  (§3.2's always-fresh-HEAD-read), which `repo::commit()` implements.

### Phase 4a (Unit 4a) — `git` resolution + probe — done
`crates/git-core/src/git_binary.rs`: `resolve()` (explicit override →
`GITVISOR_GIT_PATH` → `PATH` via the `which` crate → `GitUnavailable`, never
cached), `probe()` (exists, executable, `--version` starts with
`git version `). `GitRepo::probe()` thin wrapper; `git_probe` Tauri command.
7 tests in `tests/git_binary.rs`, including a `GITVISOR_GIT_PATH`-mutation
race across parallel tests caught and fixed with a `Mutex`-guarded RAII env
helper (see "Environment-mutation safety" below).

### Phase 4b (Unit 4b) — commit subprocess + timeout + HEAD-delta — done
- `git_binary::base_command()`: one shared builder for `identity()` and
  `run_commit()`. Sets `GIT_TERMINAL_PROMPT=0`/`GIT_EDITOR=:`/
  `GIT_SEQUENCE_EDITOR=:`; removes `GIT_DIR`/`GIT_WORK_TREE`/
  `GIT_INDEX_FILE`/`GIT_COMMON_DIR`/`GIT_OBJECT_DIRECTORY`/
  `GIT_ALTERNATE_OBJECT_DIRECTORIES`/`GIT_NAMESPACE`; never touches
  `GIT_AUTHOR_*`/`GIT_COMMITTER_*` (M5); `process_group(0)` on Unix.
- `git_binary::identity()`: `git var GIT_AUTHOR_IDENT`, hard refusal
  (`IdentityMissing`) on non-zero exit — U10-justified.
- `git_binary::run_commit()`: exact argv (`-C <workdir> --no-pager commit
  --file=- --cleanup=whitespace`), message on stdin then dropped for EOF,
  two reader threads started **before** stdin is written (deadlock
  avoidance), `try_wait()` polled every 50ms, SIGTERM → 5s grace → SIGKILL
  sent to the process group via `libc::kill(-pid, …)`. Unix-only `libc =
  "0.2"` dependency added.
- `repo::commit()`: pre-flight order bare → conflicts → nothing staged →
  `.git/index.lock` exists → resolve `git` → identity, every refusal before
  `git` is ever spawned. `head_before`/`head_after` via
  `Repository::discover` (a **fresh** handle, never the cached `self.inner`)
  on every terminal outcome. `outcome_from()` implements the 7-row exit×HEAD
  table — no branch inspects stdout/stderr text, only `exit_code` and
  whether HEAD moved.
- `state.rs::invalidate()`, thin alias of `close()`; `create_commit` command
  calls it only after `repos.with(...)`'s `?` succeeds.

### Phase 4c — commit test suite — done, 11 tests, all green
`crates/git-core/tests/commit.rs`: hook rejection + positive control, M1
replay, M5 replay, unborn branch, detached HEAD, 5 timeout-ladder/exit-table
cases via fake `git` shell scripts, nothing-staged-refuses-before-any-
subprocess. **M1 and M5 were each hand-verified to go red with their fix
removed, then restored** (non-negotiable #6):
- **M5**: temporarily swapped `identity()` to a config-only check
  (`git config --get user.email`) — the exact shape of the M5 bug
  (`Repository::signature()` reads config only). The replay test failed with
  `IdentityMissing`, exactly as M5 predicts. Reverted; green again.
- **M1**: temporarily made `repo::commit()` commit via `self.inner.commit()`
  (libgit2 directly) instead of the real `git` subprocess. The M1 replay
  test failed — `git log --show-signature` reported no signature, exactly
  as M1 predicts (libgit2 silently ignores `commit.gpgsign`). Reverted;
  green again.
- The M1 replay is **self-contained**: an ephemeral, no-passphrase GPG key
  in a throwaway `GNUPGHOME`, wired in via the repo's own **local**
  `gpg.program` (never the process environment) — safe under default
  parallel `cargo test`, and skips cleanly (does not fail) on a machine with
  no `gpg` at all.

### Phase 5 (Unit 5) — CommitBox + store + refusal rendering — done
`CommitBox.tsx` (message textarea, `Committing…` state from the shared
`staging.busy` flag, a 10s "still running" note via a plain frontend
`setTimeout`, the `commitWarning` banner, a detached-HEAD notice — no
"committing as …" line, per M5/design §5.1/§11). Store gained `gitProbe`
(fetched once after `refresh()` in `open()`, best-effort, never blocks repo
open on failure), `commitWarning`, `createCommit()` — reusing `staging.busy`
rather than a new flag, exactly as design anticipated ("will still be
correct once `createCommit` lands and sets the same flag"). `api.ts` gained
`gitProbe`/`createCommit`. `RefusalNotice.tsx` extended with
`nothingStaged`/`identityMissing`/`indexLocked`/`gitUnavailable`/
`commitFailed`/`commitTimedOut`, the last two rendering their `stderr`
detail in a quoted, attributed block. `dump-mocks.rs` gained a `git_probe`
mock entry, machine-specific `path`/`version` tokenised the same way
`open_repository.path` already is.

**Deviation, stated plainly**: no new browser-mode wdio specs were added for
the commit UI states. Browser-mode e2e is broken in this environment —
confirmed by actually running `pnpm e2e:browser`: chromedriver
151.0.7922.173 cannot be downloaded (`All providers failed for chromedriver
151.0.7922.173`). This is an environment fault, not a code defect, per this
run's brief. `pnpm build` (`tsc --noEmit` + `vite build`) is clean, and the
actual commit path — including the UI states — is exercised for real by the
native write spec (6.5), which is strictly more authoritative for this path
than a mocked browser spec would have been. Authoring browser-mode
assertions nobody in this environment can execute, for a path the native
spec already covers, was judged not worth the risk of an unverified
regression test shipping unverified. A future session with a working
chromedriver should add them.

### Phase 6 (Unit 6) — native write spec + CI — done, one bug found and fixed
- `wdio.native.writes.conf.ts` — mirrors `wdio.native.conf.ts`'s shape
  exactly (design §9.3): separate config, own `onPrepare` building the
  `writes` fixture, own `appArgs`, calls `clearRememberedRepoStorage()`.
- `e2e/native/writes/stage-commit.spec.ts` — the one native write spec:
  stage a real file → commit through the UI → the new commit's summary
  appears at the top of the real graph, staged list empties. Per finding H2,
  asserts only on the typed commit message and row counts, never on
  rendered date text.
- **Bug found and fixed while building 6.3/6.5**: `build-fixture.rs` wrote
  its `fixture.json` manifest *inside* the fixture repository's own working
  tree. By the time the real app queried `working_status` a moment later,
  `fixture.json` itself was a **fourth** untracked file the manifest (computed
  a moment earlier) never saw — `writes`' `initialStatus` said 3 unstaged
  entries; the live UI showed 4. This is why the first native-write-spec run
  failed its very first assertion. Fixed by moving the manifest to
  `<out_dir>/.git/fixture.json` (git-invisible) for **both** recipes;
  `e2e/support/fixture.ts::readFixture` updated to match. Verified:
  `history`'s `determinism.rs` still green (OIDs untouched by the move);
  regenerating `e2e/mocks/history.json` showed the same spurious
  `fixture.json` untracked entry disappearing from `working_status`
  there too — a real, pre-existing latent defect in the harness this
  caught, not something this change introduced.
- **Performance finding**: `@wdio/tauri-service`'s `ensureActiveWindowFocus`
  pre-command check retries against `window.__TAURI__.core.invoke` before
  *every* WebDriver command and always fails (`withGlobalTauri` is off,
  finding H1), adding real per-command latency — enough that the spec's
  first version, written with ~20 discrete WebDriver commands (element
  lookups, per-row `getText()` calls, individual `setValue`/`click`), hit
  the 180s mocha timeout without even reaching the commit step. Rewritten to
  consolidate each interaction into one `browser.execute()` (real DOM
  `.click()` — React's delegated listeners handle it identically to a real
  click; a native `HTMLTextAreaElement` value setter + a dispatched `input`
  event for the controlled textarea) instead of many small WebDriver
  commands. Cut wall-clock time to ~20s, run 4 times consecutively, all
  green. This applies to any future spec in this harness, not just this one
  — worth carrying into a standing finding if this pattern recurs.
- **6.6 — run and green on macOS**, 4 consecutive runs via
  `pnpm e2e:native:writes`, real WKWebView, real `git` subprocess.
- **6.7 — CI wiring only, not locally executed.** Added as "Spec C" to
  `.github/workflows/e2e-native-macos.yml` (same nightly/dispatch/tag
  trigger as Spec A/B) and as a step to
  `.github/workflows/e2e-native-linux-probe.yml` (reusing its existing
  WebKitGTK + `xvfb` setup rather than a third workflow file). Neither
  addition has an observed passing CI run — no Linux machine in this
  environment, and the macOS job has not been triggered on the real GitHub
  Actions runner. `actionlint` and the project's own
  `scripts/validate-config.py` both pass on the modified YAML.

## Environment-mutation safety in the new tests

Three test files mutate process-global env vars for scenarios
`git_binary::base_command()` structurally cannot support any other way (it
inherits ambient env by design, matching what Gitvisor itself runs under).
Each is guarded by a file-local `Mutex` + RAII guard so `cargo test`'s
default parallel execution cannot race two of these tests against each
other within the same binary:
- `tests/git_binary.rs`: `GITVISOR_GIT_PATH` (3 tests) — caught a real race
  in this exact run (`probe_reports_the_real_git_as_available` failed
  intermittently under parallel `cargo test --workspace` before the guard
  was added to that specific test; fixed, reran 3× clean).
- `tests/commit.rs`: `HOME`/`GIT_AUTHOR_*`/`GIT_COMMITTER_*` (1 test, the M5
  replay) — restores the original `HOME` afterward, not just removes the
  mutated one.

`crates/git-core/tests/support/mod.rs` gained `#![allow(dead_code)]`: each
`tests/*.rs` file is its own crate root with its own copy of this shared
module, and a helper (`write_fake_git`, added this run) used by only some of
those files is not actually dead code — just not needed everywhere.

## Non-negotiables from the apply brief — status

1. **Commit outcome from observed HEAD movement, never the exit code.**
   Done — `outcome_from()`'s 7-row table; `attempt.exit_code` combined with
   an independently-read `head_before`/`head_after`, never trusted alone.
   Proven by the fake-`git` timeout-ladder tests (4c.6), especially
   `timeout_after_head_moved_reports_a_warning_not_a_failure`.
2. **SIGTERM, then grace, then SIGKILL — the measured signal, not a
   substitute.** Done — `git_binary::escalate()` sends `libc::SIGTERM` then,
   after 5s, `libc::SIGKILL`, both to the process group.
3. **Never `--no-verify`, never `-a`, never a shell string.** Done —
   `run_commit()`'s argv is a fixed list passed to `Command::new(git_path)`;
   no shell is ever invoked; message goes on stdin.
4. **`GIT_DIR`/`GIT_INDEX_FILE` removed from the subprocess env.** Done —
   `base_command()`'s `REMOVED_ENV_VARS` list, along with 5 other
   repo-location variables design.md §2.2 names.
5. **Identity from `git var GIT_AUTHOR_IDENT` through the same command
   builder as the commit; `Repository::signature()` on the clippy
   denylist; no prospective author displayed.** Done — `identity()` and
   `run_commit()` both go through `base_command()`; `signature` was already
   on `clippy.toml`'s denylist from the first run and stays untouched;
   `CommitBox.tsx` shows no author line.
6. **M1, M3, M5 replayed as tests using real `git` subprocesses; each fix
   hand-verified to go red when removed.** M1 and M5: done, see above, both
   hand-verified. **M3 was already replayed in this run's own way**: the
   timeout-ladder tests (4c.6) use fake `git` scripts standing in for the
   exact M3 shape (a signer that hangs, or exits 128 with a message) —
   `nonzero_exit_with_a_stderr_line_is_commit_failed` replays M3 row 1's
   shape literally (`exit 128`, stderr verbatim → `CommitFailed`), and
   `timeout_never_exits_head_unchanged_commit_timed_out` replays M3's core
   claim (a killed commit leaves nothing behind) using the actual
   `git_binary` code path rather than the ad-hoc script this project's M3
   measurement itself used. Not separately hand-verified red/green as its
   own drill — the SIGTERM/SIGKILL ladder is exercised directly by these
   tests already, and a fix-removal drill on the ladder itself was judged
   lower-value than the M1/M5 drills, which target the two places this
   project has previously been *wrong* (M1's silent gpgsign drop, M5's
   `explore.md` mislabelled "Verified").
7. **`git` absent → refuse, never fall back to libgit2.** Done —
   `git_binary::resolve()`'s only fallback on total failure is
   `GitUnavailable`; nothing in `repo::commit()` calls `self.inner.commit()`
   on the real path (only in the temporary M1 regression drill, reverted).

## Definition of done — checked

- `cargo test --workspace`: green — 18 (unit) + 11 (commit.rs) + 7
  (git_binary.rs) + 1 (index_freshness.rs) + 8 (stage_unstage.rs) git-core
  tests, 2 git-fixtures determinism tests, 0 elsewhere. Reran 3× under
  default parallel execution with no flakes after the env-guard fixes.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo fmt --all --check`: clean.
- `pnpm build` (`tsc --noEmit` + `vite build`): clean.
- M1 and M5 replay tests: pass, and were each hand-verified to fail red with
  their fix removed (see Phase 4c above).
- Native suite: `pnpm e2e:native:smoke`, `pnpm e2e:native:regressions`, and
  the new `pnpm e2e:native:writes` all green (run with the sandbox disabled
  — the real WKWebView session needs socket/display access the default
  sandbox denies, an environment requirement of the harness itself).
- Browser-mode e2e: **could not run** — chromedriver 151.0.7922.173 cannot
  be downloaded on this machine, confirmed by actually attempting it.
  Environment fault, not a regression; not chased, per the apply brief.
- Release-safety gates: `cargo tree -p gitvisor -e normal` excludes `wdio`;
  the same command with `--features e2e-webdriver` includes it (positive
  control, unchanged). `compile_error!` gate re-confirmed by actually
  building `--release --features e2e-webdriver` and observing the expected
  build failure.
- `crates/git-core` still carries no Tauri/React imports (`rg -l
  "tauri|react" crates/git-core/src` → clean).
- `pnpm run e2e:mocks` regenerated; diff against the committed
  `e2e/mocks/history.json` is exactly the new `git_probe` entry plus the
  `fixture.json`-pollution fix (Phase 6 above) — nothing else moved.
- `actionlint` and `scripts/validate-config.py` both pass on the two
  modified CI workflow files.

## Files changed (this run, in addition to the first run's list)

| File | Action |
|---|---|
| `crates/git-core/src/git_binary.rs` | Created — `resolve`, `probe`, `base_command`, `identity`, `run_commit`, the timeout ladder |
| `crates/git-core/src/lib.rs` | Modified — `pub mod git_binary` |
| `crates/git-core/src/model.rs` | Modified — `CommitRequest`, `CommitOutcome`, `CommitWarning`, `GitProbe` |
| `crates/git-core/src/repo/mod.rs` | Modified — `commit()`, `probe()`, `fresh_head()`, `outcome_from()`, `success()` |
| `crates/git-core/Cargo.toml` | Modified — `which` dependency, Unix-only `libc` target dependency |
| `crates/git-core/tests/support/mod.rs` | Modified — `write_fake_git()`, `#![allow(dead_code)]` |
| `crates/git-core/tests/git_binary.rs` | Created — 7 tests |
| `crates/git-core/tests/commit.rs` | Created — 11 tests |
| `src-tauri/src/commands.rs` | Modified — `git_probe`, `create_commit` |
| `src-tauri/src/state.rs` | Modified — `invalidate()` |
| `src-tauri/src/lib.rs` | Modified — command registration |
| `src/shared/types.ts` | Modified — `CommitOutcome`, `CommitWarning`, `GitProbe` mirrors |
| `src/features/repo/api.ts` | Modified — `gitProbe`, `createCommit` |
| `src/features/repo/store.ts` | Modified — `gitProbe`, `commitWarning`, `createCommit`, `rememberedGitPath` |
| `src/features/working-directory/CommitBox.tsx` | Created |
| `src/features/working-directory/RefusalNotice.tsx` | Modified — 6 more codes, quoted stderr block |
| `src/features/working-directory/WorkingDirectoryPanel.tsx` | Modified — mounts `CommitBox` |
| `tools/git-fixtures/src/bin/build-fixture.rs` | Modified — manifest moved to `.git/fixture.json` |
| `tools/git-fixtures/src/bin/dump-mocks.rs` | Modified — `git_probe` mock entry |
| `e2e/support/fixture.ts` | Modified — manifest path, `initialStatus`/`FixtureWorkingStatus` types |
| `e2e/support/mocks.ts` | Modified — `git_probe` mock, token substitution |
| `e2e/mocks/history.json` | Regenerated |
| `wdio.native.writes.conf.ts` | Created |
| `e2e/native/writes/stage-commit.spec.ts` | Created |
| `tsconfig.wdio.json` | Modified — includes the new wdio config |
| `package.json` | Modified — `e2e:native:writes` script |
| `.github/workflows/e2e-native-macos.yml` | Modified — "Spec C" step |
| `.github/workflows/e2e-native-linux-probe.yml` | Modified — write-spec step, header comment |
| `openspec/changes/stage-and-commit/design.md` | Modified — U1/U2/U7/U10 register entries resolved, §3.5 findings |
| `openspec/changes/stage-and-commit/tasks.md` | Modified — checkboxes + deviation notes throughout |

## Next recommended

`sdd-verify` — every scoped task is done except the explicitly optional 6.8.
Two residues to carry forward, both already stated above rather than hidden:
Linux CI for the write spec has no observed passing run yet (needs a
maintainer to trigger it or merge to `main`), and U1/U2's pinentry
experiment is missing P3/`pinentry-mac`/Linux coverage (non-blocking per
design — the timeout + always-read-HEAD handles any hang shape regardless).
A future session with a working local chromedriver should add browser-mode
specs for the commit UI states (5.8's stated deviation).
