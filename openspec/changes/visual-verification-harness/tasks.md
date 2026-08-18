# Tasks: Visual Verification Harness

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~1600 (Rust ~600, TS/config ~460, scripts ~140, workflows ~260, docs ~150; generated `e2e/mocks/*.json` excluded from authored count) |
| 400-line budget risk | High |
| Chained PRs recommended | Yes |
| Suggested split | PR1 fixtures → PR2 native specs / PR3 browser mocks (parallel) → PR4 CI+release → PR5 docs |
| Delivery strategy | ask-on-risk |
| Chain strategy | pending |

### Delivery decision (orchestrator, 2026-08-18) — RESOLVED

| Field | Value |
|---|---|
| Delivery strategy | `ask-on-risk` → **chained**, user asked and answered |
| Chain strategy | `stacked-to-main` — note the repo has **no git remote yet**, so these are sequential local work units, not GitHub PRs |
| **Apply scope for this run** | **Phases 1–4 ONLY** (spike gate, fixtures, native specs, expected-failure guard) |
| Deliberately deferred | Phases 5 (browser mode), 6 (CI), 7 (docs) |

**Why the chain is interrupted rather than run 1→5:** the only reason the red
test exists is to make fixing F1 possible. After Phase 4 the harness proves F1
with a failing test, and the right next move is the `fix-graph-viewport` change —
not browser mode, CI and documentation layered on top of an app whose commit
graph still renders nothing. Building the remaining infrastructure first would be
decoration over something broken, which is the same reasoning the user applied
when deferring the feature backlog earlier the same day.

Phases 5–7 resume after `fix-graph-viewport` lands.

Decision needed before apply: **RESOLVED — see above**
Chained PRs recommended: Yes
Chain strategy: pending
400-line budget risk: High

### Suggested Work Units

| Unit | Goal | Likely PR | Focused test command | Runtime harness | Rollback boundary |
|------|------|-----------|----------------------|-----------------|-------------------|
| 1 | `tools/git-fixtures` deterministic builder | PR 1 | `cargo test -p git-fixtures` | N/A — determinism is proven by `cargo test`, no UI involved | Remove the `tools/git-fixtures` workspace member; nothing depends on it |
| 2 | Native specs (A green, B red) + wdio split + expect-red guard | PR 2 (parallel with PR 3, both need PR 1) | `pnpm e2e:native:smoke` then `./scripts/expect-red.sh pnpm e2e:native:regressions "expected 16 rows, got 7"` | Real `gitvisor` binary, real WKWebView, PR 1's fixture | Delete `e2e/native/`, `wdio.native.conf.ts`, `wdio.shared.conf.ts`, `scripts/expect-red.sh` |
| 3 | Browser-mode mocks + spec | PR 3 (parallel with PR 2) | `pnpm e2e:browser` | Chrome + mocked `invoke()`, no Tauri build | Delete `e2e/browser/`, `e2e/mocks/`, `wdio.browser.conf.ts`, `dump-mocks.rs` |
| 4 | CI workflows + release-safety scan | PR 4 (needs PR 2 + PR 3) | `bash scripts/release-scan.sh` against a local `--features e2e-webdriver` debug build | `.github/workflows/ci.yml` triggered on its own push | Delete/disable workflow files; no product code touched |
| 5 | Contributor docs | PR 5 (needs PR 4) | N/A — docs only | Manually re-run every documented command | Delete/revert `README.md` |

## Already Implemented — verify / wire into CI, do not rewrite

`src-tauri/Cargo.toml` optional dep + `e2e-webdriver` feature, `src-tauri/src/lib.rs` double `cfg` gate + `compile_error!`, `src-tauri/build.rs` capability glob switch, `capabilities/{app,e2e}/` split. Measured working (design.md "Orchestrator verification"). Phase 6.1/6.6 below wire the already-measured commands into CI; they do not re-derive the mechanism.

## Phase 1: Spike Gate — resolve U3 first

- [x] 1.1 Spike whether `@wdio/tauri-service` 1.3.0 `tauri:options.args` passes argv to the app; assert via `invoke("startup_path")`. [seam for FX+SA, design D5]
- [x] 1.2 Record result in design.md's unverified register; pick `args` path or `localStorage` fallback for Phase 3.

## Phase 2: Deterministic Fixture Builder — `tools/git-fixtures` [Requirement: Deterministic Fixture Generation]

- [x] 2.1 Add `tools/git-fixtures` workspace member (`Cargo.toml`, `git2`/`git-core`/`serde` deps), register in root workspace `Cargo.toml`.
- [x] 2.2 `src/spec.rs`: 16-commit graph (linear run, diverging+reconverging branches, one long edge, non-tip annotated tag, remote-ahead ref); no rebase/cherry-pick/force-push shapes. **Deviation**: a >120-row long edge is mathematically impossible in a 16-commit graph (`SHORT_EDGE_SPAN` in `layout.ts` is 120); spec.md pins the fixture at exactly 16 commits, so this sub-bullet of design.md §2.4 is not satisfiable without breaking spec.md. Recorded in `spec.rs`'s module doc and here.
- [x] 2.3 `src/lib.rs`: `FixtureSpec`/`build()`/`Manifest`; pinned signatures/time/branch via `git2`, no index/no worktree for history construction. (`Manifest` lives in `src/bin/build-fixture.rs`, not `lib.rs` — it is the binary's serialization concern, not the builder's; `build()` returns a plain `BuildResult` of real OIDs instead.)
- [x] 2.4 `src/bin/build-fixture.rs`: rebuild `target/e2e-fixtures/<name>/`, write `fixture.json` manifest.
- [x] 2.5 `tests/determinism.rs`: assert full alias→OID map (not just HEAD) via `cargo test -p git-fixtures`.
- [x] 2.6 Run the test, backfill `src/oids.rs` constants from real output; confirm `target/e2e-fixtures/` is covered by the existing `.gitignore` `target/` entry.

## Phase 3: Native Specs [Requirement: Native Smoke Spec; Native Regression Spec Detects F1]

- [x] 3.1 Delete `e2e/spike.spec.ts`, `wdio.conf.ts`, `e2e/__screenshots__/native-welcome.png`.
- [x] 3.2 `wdio.shared.conf.ts` (LANG pin, artifact dir) + `wdio.native.conf.ts` (tauri capability per 1.2's decision, `onPrepare` runs `build-fixture`). Capability uses `wdio:tauriServiceOptions.appArgs`, not `tauri:options.args` — see design.md's U3 resolution. `onPrepare` also clears `~/Library/WebKit/gitvisor/WebsiteData` (macOS) so `rememberedRepo()`'s stale localStorage cannot mask a real regression.
- [x] 3.3 `e2e/support/fixture.ts` (reads `fixture.json`), `e2e/support/artifacts.ts` (screenshot paths).
- [x] 3.4 `e2e/native/smoke.spec.ts` — Spec A: window title, sidebar branch/tag names, header repo name.
- [x] 3.5 `e2e/native/regressions/graph-viewport.spec.ts` — Spec B: 5 assertions per design §5, incl. fractional-height (`getBoundingClientRect`), manifest-derived canvas size, probe coordinate.
- [x] 3.6 Run Spec A locally (`pnpm exec wdio run wdio.native.conf.ts --spec ./e2e/native/smoke.spec.ts`) — **green**: `1 passing (2m 56.8s)`. (`pnpm e2e:native:smoke` script itself is added in Phase 4.)
- [x] 3.7 Run Spec B locally — **red for F1**, exit code 1. Captured messages: `AssertionError [ERR_ASSERTION]: expected 16 rows, got 7` (assertion 2, `under-renders the initial viewport`) and `AssertionError [ERR_ASSERTION]: expected row count to track the resized viewport (~13 rows for a 178px-tall scroller), got 7 (was 7 before resizing)` (assertion 3, `does not track a resize`). Both failures are inside the spec's own assertions (`graph-viewport.spec.ts:69` and `:97`), not a launch/timeout/selector error — `2 failing (1m 11.7s)`, `Spec Files: 0 passed, 1 failed`.

## Phase 4: Expected-Failure Guard [Requirement: Native Regression Spec Detects F1]

- [x] 4.1 `scripts/expect-red.sh`: passes only on non-zero exit AND output containing the exact expected message; fails closed otherwise.
- [x] 4.2 Add `e2e:native:smoke` / `e2e:native:regressions` scripts to `package.json`.
- [x] 4.3 Run the guard against Spec B (must pass) and a stub always-passing command (must fail with the "appears fixed" message). Both measured:
  - `./scripts/expect-red.sh pnpm e2e:native:regressions "expected 16 rows, got 7"` → `expect-red.sh: PASS — expected failure confirmed: "expected 16 rows, got 7"`, exit 0.
  - `./scripts/expect-red.sh true "expected 16 rows, got 7"` → `expect-red.sh: FAIL — the wrapped command passed. F1 appears fixed. Remove this guard and the CI step's wrapper (fix-graph-viewport).`, exit 1.
  - Third case also measured (not required by 4.3 but cheap and closes the "fails closed" claim): a command that fails for an unrelated reason (`sh -c "echo boom; exit 3"`) → `expect-red.sh: FAIL — the wrapped command failed, but not for the expected reason.`, exit 1.

## Phase 5: Browser Mode [Requirement: Browser-Mode Mocks Are Generated and Diff-Checked]

- [x] 5.1 `tools/git-fixtures/src/bin/dump-mocks.rs`: serialise via `git_core::GitRepo`, keyed by Tauri command name, `{{FIXTURE_PATH}}` token substitution. `crates/git-core/examples/dump.rs` stays untouched (confirmed: `git diff` on that file is empty for this phase). Keys: `startup_path`, `open_repository`, `commit_graph`, `list_refs`, `working_status`, `commit_detail` (map keyed by commit OID — a user can select any commit, not just the one `open_repository` returns), `close_repository` (added beyond design.md's example JSON, for full command-surface coverage; `close_repository` returns nothing so its mock is `null`).
- [x] 5.2 Generate and commit `e2e/mocks/history.json`. Measured: `commit_graph` 16 rows / laneCount 4, `list_refs` 7 entries (4 local branches, 2 remote-tracking, 1 tag), `working_status` 1 staged / 2 unstaged / 0 conflicted, `commit_detail` 16 entries. Regenerating after `cargo fmt` produced byte-identical output (fixture determinism holds through the mock-dump path too).
- [x] 5.3 `e2e/support/mocks.ts` (loads + substitutes `{{FIXTURE_PATH}}` with a literal browser-mode placeholder path — browser mode never touches a real filesystem), `wdio.browser.conf.ts` (`mode: "browser"`, `devServerUrl: "http://localhost:1420"`, `devServer: "pnpm dev"` auto-managed). Also added `e2e/support/browser-tauri.d.ts` — a minimal ambient type for `browser.tauri`, since `@wdio/tauri-service`'s published `dist/` ships no global `WebdriverIO.Browser` augmentation (checked: no `declare global` anywhere in the package); editor/reader clarity only, never load-bearing given wdio's `tsx` transpile-only execution.
- [x] 5.4 `e2e/browser/welcome.spec.ts` — drives the real "Open repository" UI flow (button click → mocked `plugin:dialog|open` → mocked `open_repository`/`commit_graph`/`list_refs`/`working_status`/`commit_detail`), asserting header repo name, exact commit-row count, and exact local-branch names from the mocked `list_refs`. Does **not** rely on `startup_path` firing correctly on mount — documented in-file: `@wdio/tauri-service`'s browser-mode `before()` hook navigates to the dev server before any test code runs, so the app's mount-time `startup_path` call is unavoidably unmocked and fails silently (app lands on `WelcomeScreen`, same as a real backend with no remembered/argv repo). Driving the Open-repository button is the one path that fires every mocked command safely after mocks exist.
- [x] 5.5 Added `e2e:browser` script (`wdio run wdio.browser.conf.ts`) and `e2e:mocks` script (rebuild fixture + regenerate mocks in one command) to `package.json`; ran locally — **green**: `1 passing (321-367ms across 3 runs)`. **Negative control performed** (not required by 5.5, but closes the same "can a check that stopped matching still fail" concern §1.3 raises elsewhere): temporarily changed the row-count assertion's expectation by `+1` and reran — got a real `AssertionError [ERR_ASSERTION]: expected 16 commit rows, got 16`, confirming the spec is a genuine DOM assertion against real React rendering, not a tautology against the mock's own echoed data; reverted, reran, green again.

## Phase 6: CI [Requirement: Release Safety Verification; CI Trigger Matrix]

- [x] 6.1 `.github/workflows/ci.yml`: `cargo test`, clippy (`-D warnings`), fmt, `pnpm build`, G1 `cargo tree` both-directions check, browser-e2e, mocks-drift diff — all blocking, no `continue-on-error` anywhere. G1 is its own job (`release-safety-graph`), not a step inside the Rust job: `cargo tree` is metadata-only (no compilation, no webkit2gtk headers), so it is the fastest signal in the workflow and must not wait behind `cargo test --workspace`. `browser-e2e` and `mocks-drift` are separate jobs matching design.md §4.1's table exactly (`browser-e2e`: no Rust, no webkit; `mocks-drift`: needs Rust, no webkit).
- [x] 6.2 Added `scripts/validate-config.py` (parses `.github/**/*.{yml,yaml}`, `openspec/config.yaml`, `package.json`, `tsconfig*.json` — plain `yaml.safe_load_all`/`json.loads`, the exact class of check that would have caught this repo's earlier invalid-YAML incident) plus an `actionlint` step (schema-aware, catches bad `needs:`/context errors a plain parse would miss) in `ci.yml`'s new `validate-config` job. Both run locally clean: `actionlint .github/workflows/*.yml` — zero findings; `python3 scripts/validate-config.py` — 8/8 files OK.
- [x] 6.3 `.github/workflows/e2e-native-macos.yml`: nightly + `workflow_dispatch` + release tags. **Deviates from this task's original wording per the amendment below**: Spec B runs as an ordinary blocking step, not through `expect-red.sh` — that guard is deleted, F1 is fixed, Spec B is green. No `continue-on-error`.
- [x] 6.4 `.github/workflows/e2e-native-linux-probe.yml`: `workflow_dispatch`-only, Spec A only (embedded provider + `xvfb`), per design D9's decision rule. Not wired into any other trigger; nothing depends on it.
- [~] 6.5 **Not run — cannot be, under this session's constraints, and that is the correct outcome, not a shortcut.** Running the probe means triggering a real `workflow_dispatch` run on GitHub, which requires the workflow file to exist on the remote first; the orchestrator's instructions are explicit: **do not push**. The probe workflow (6.4) is written and lands disabled by default, exactly as design.md §8 specifies for the unrun state ("until the probe passes, no Linux native job is blocking, and nothing in the repository claims Linux native coverage exists"). No WebKitGTK job was added to `ci.yml` or any other trigger. This is the honest, undecided state — promoting it is a follow-up commit for whoever runs the probe after this lands, citing the run URL, per design.md §8.
- [x] 6.6 `scripts/release-scan.sh`: string scan (`rg -a --binary`, primary) + symbol scan (`nm -aU`, corroborating, never authoritative alone) per design §1.3's 4-outcome table (pass / plugin-shipped / scan-broken / inverted). **Bug found and fixed during verification**: the symbol probe's original `nm -aU "$file" | grep -q "$PLUGIN_SYMBOL"` produced a false "not found" under `set -o pipefail` — `grep -q` exits as soon as it finds a match, SIGPIPEs `nm` mid-write on a large symbol table, and `pipefail` propagates `nm`'s SIGPIPE exit code as the pipeline's status instead of `grep`'s success. Fixed by capturing `nm`'s output into a variable first, then `grep`-ing the variable (no live pipe, no SIGPIPE). Confirmed via `bash -x` trace before the fix and a clean rerun after.
- [x] 6.7 `.github/workflows/release.yml`: `build` (release bundle + e2e-webdriver positive-control binary, same job/runner, sha256 provenance manifest) → `scan` (needs: `[build]`; downloads artifacts, re-hashes and diffs against the build job's manifest, runs `release-scan.sh --positive-control`) → `publish` (needs: `[scan]`). No `continue-on-error` anywhere. The provenance re-hash/diff shell logic (path-prefix normalisation across the build/download directory-name mismatch) was simulated locally against the real bundle before being trusted in the workflow — see 6.8.
- [x] 6.8 Ran `release-scan.sh` against a real `--release` build (`cargo build --release --manifest-path src-tauri/Cargo.toml`, then the full `pnpm app:build` bundle at `target/release/bundle/macos/Gitvisor.app`) and a real `--features e2e-webdriver` debug build, in every mode the script supports:
  - Single-artifact scans: release → `absent` (exit 0); e2e → `present` (exit 1).
  - `--positive-control` (the real invocation shape `release.yml` uses) against the actual `.app` bundle directory tree and the e2e binary → `PASS`.
  - All three failure outcomes forced and confirmed: `present+present` → `FAIL — the plugin shipped`; `absent+absent` → `FAIL — scan produced no match on a known-positive artifact` (scan-broken); `present+absent` → `FAIL — inverted result`.
  - String probe survives `strip` (tested against a stripped copy of the e2e binary — string match: 29 occurrences; symbol match: none), confirming why the symbol probe alone is uninformative and the string probe must be primary.
  - The provenance re-hash/diff shell pipeline (`release.yml`'s `scan` job) was reproduced standalone against the real bundle: hashes matched byte-for-byte across the build-job-style and scan-job-style path prefixes once normalised on `/Contents/`.
  - **U5 → CLOSED.** `strings`-class scanning of a bundled release `.app` reliably surfaces the Rust `&'static str` IPC-identifier literal after `strip`, measured, not inferred.

## Phase 7: Contributor Docs [Requirement: Contributor Commands Use Only Free, Open-Source Tooling]

- [x] 7.1 **EDITED** the existing `README.md` at repo root (read first via the `Read` tool; did not overwrite). Extended the existing `## Testing` section — not `## Development`, which no longer exists in the current README; the file was substantially rewritten during publication after this task was originally written, and the harness commands already live under `## Testing` — with: `pnpm e2e:browser` (browser-mode fast loop, described as needing no Rust/WebKit), `pnpm run e2e:mocks` (fixture + mock regeneration, with the "run this after changing `git-core` model types" note), and a paragraph on `scripts/release-scan.sh`'s positive-control discipline placed next to the existing WebDriver-plugin-exclusion paragraph it extends. Every tool mentioned (`@wdio/*`, `tauri-plugin-wdio-webdriver`, `git2`) is free/open-source; no new tool was introduced. Confirmed exactly one `## Testing` heading exists (`rg -n "^## " README.md`) and the diff is purely additive (`git diff --stat README.md` → `17 insertions(+)`, 0 deletions).
- [x] 7.2 Re-ran every documented command in sequence, including the pre-existing ones this task didn't add: `cargo test --workspace` → `pnpm build` → `pnpm run e2e:mocks` (no drift) → `pnpm e2e:browser` (green, `1 passing`) → `cargo run -p git-core --example dump -- target/e2e-fixtures/history` (unchanged, confirmed via `git log -- crates/git-core/examples/dump.rs` still pointing at the original pre-harness commit) → `./scripts/release-scan.sh --positive-control <bundle> <e2e-binary>` (PASS). Manual verification, not a unit-test-first step (no JS/TS runner installed per `openspec/config.yaml`).


---

## Amendment after `fix-graph-viewport` (2026-08-18)

F1 is fixed and Spec B is green. Consequences for the deferred phases:

- `scripts/expect-red.sh` and Phase 4 are **obsolete and removed**. Phase 6 CI
  MUST run `e2e/native/regressions/graph-viewport.spec.ts` as an ordinary
  **blocking** test. Do not reintroduce an expected-failure wrapper.
- The `continue-on-error` allowance discussed in proposal §5.5 no longer applies
  to the native job. It existed only for the window while F1 was open.
- New finding H2 (see `findings.md`): fixture determinism covers commit OIDs but
  **not** rendered relative dates. Phase 6 must not add assertions on date text.
