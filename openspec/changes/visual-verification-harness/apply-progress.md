# Apply Progress: visual-verification-harness

**Scope for this run**: Phases 1–4 only (spike gate, deterministic fixtures, native specs,
expected-failure guard), per the resolved chained/stacked-to-main delivery decision recorded
in `tasks.md`. Phases 5 (browser mode), 6 (CI), 7 (docs) are deliberately **not started** —
stopped here as instructed, not because anything blocked.

**Mode**: Standard (project `strict_tdd: false`). Backend verification used the real
`cargo test -p git-fixtures` runner; frontend/E2E verification used real suite runs
(`pnpm exec wdio run ...`), not a pre-written failing unit test — per the environment
instructions for E2E work under `strict_tdd: false`.

**Skill Resolution**: `paths-injected` — the project's skill registry
loaded before writing any code, plus the phase skill definitions and
the phase skill definitions.

---

## Phase 1 — Spike Gate (U3), RESOLVED

**Task**: determine whether `@wdio/tauri-service` 1.3.0's `tauri:options.args` passes argv to
the app, by running it, not reading docs.

**Method**: built a throwaway spike (`e2e/spike-args.spec.ts` + `wdio.spike-args.conf.ts`,
both deleted after use — not part of the final tree), pointed at a scratch git repo
(a temporary directory), and asserted on the DOM (main view vs. WelcomeScreen,
header repo name) since `withGlobalTauri` is off and `invoke()` can't be called directly from
the test.

**First measurement was contaminated and had to be redone.** A run with `tauri:options.args`
set landed on the main view with header text `"fixture"` — looked like a pass. Investigation
found `~/Library/WebKit/gitvisor/WebsiteData/…/localstorage.sqlite3` already held a
`gitvisor:last-repo` entry from an earlier manual verification session the same day.
WebKit's localStorage persists across process launches and wdio runs; it is not reset by the
harness. After `rm -rf ~/Library/WebKit/gitvisor/WebsiteData` and re-running on a clean
state:

| Capability shape | `[role="listbox"]` renders | Header text |
|---|---|---|
| `"tauri:options": { application, args: [FIXTURE_PATH] }` | **false** | `"Gitvisor"` (WelcomeScreen) |
| `"tauri:options": { application }` + `"wdio:tauriServiceOptions": { appArgs: [FIXTURE_PATH] }` | **true** | `"spike-fixture"` (exact dir name) |

**Root cause** (read from `@wdio/tauri-service@1.3.0`'s bundled `dist/esm/index.js`): the
embedded-provider spawn path builds process args from `options.appArgs`, where `options` comes
from `mergeOptions(this.options, cap['wdio:tauriServiceOptions'])`. `cap['tauri:options'].args`
is read exactly once, only to be logged at debug level during capability validation — it is
dead as far as the actual `spawn()` call is concerned. This is a real gap/naming mismatch in
`@wdio/tauri-service` 1.3.0, not a docs-vs-behaviour ambiguity.

**Decision**: use `wdio:tauriServiceOptions.appArgs`. This still exercises the real
`startup_path` argv code path with zero product-code fallback (strictly better than the
`localStorage` fallback design.md documented). `wdio.native.conf.ts` uses this shape.
The `localStorage` fallback is demoted to "unused escape hatch" in design.md.

**Consequence recorded and acted on**: every native-mode wdio config must clear
`~/Library/WebKit/gitvisor/WebsiteData` before the suite runs, or a stale `localStorage`
entry silently masks a real regression. `wdio.native.conf.ts`'s `onPrepare` does this.

Full writeup: `design.md`, new section "U3 resolution (2026-08-18, apply phase Phase 1)".

---

## Phase 2 — Deterministic Fixture Builder (`tools/git-fixtures`)

New workspace member `tools/git-fixtures` (registered in root `Cargo.toml`), depending on
`git2` and `git-core` (never the reverse — `git-core` gained zero new dependencies).

- `src/spec.rs` — 16-commit graph as data. Shapes exercised: linear root run (`c1→c2→c3→base`),
  two branches diverging from `base` and both surviving to the tip (`feature-a`, `feature-b`),
  one branch diverging from main and reconverging via a merge (`refactor/parser` → `merge1`),
  an annotated tag on a non-tip commit (`v0.1.0` on `m1`), a remote-tracking ref one commit
  ahead of its local branch (`origin/feature-a` vs. local `feature-a`). No rebase, cherry-pick,
  or force-push shapes — `product_scope.out_of_scope` respected.
- `src/lib.rs` — `build()` constructs history with `git2` `TreeBuilder` + literal blobs, no
  index/no worktree during history construction; pinned author/committer/tagger signatures,
  epoch-based timestamps, `initial_head("main")`. A recursive `build_tree()` helper handles
  `TreeBuilder::insert`'s flat-path limitation (it does not accept `/`-separated paths — this
  was caught by the first `cargo test` run: `invalid name for a tree entry - notes/c2.txt`).
  Working-directory dirt (one staged file, one unstaged modification) is written *after*
  `checkout_head`, outside the OID-affecting path.
- `src/bin/build-fixture.rs` — rebuilds `target/e2e-fixtures/history/`, opens the fresh repo
  through `git_core::GitRepo` (the same type `src-tauri` uses) to get the *real* lane layout
  and row-0 alias/lane for `fixture.json`, never guessed.
- `tests/determinism.rs` — asserts the full alias→OID map (16 commits) plus the tag OID and
  HEAD tree OID against hardcoded constants in `src/oids.rs`, backfilled from a real run.
  Also isolates ambient git config via `git2::opts::set_search_path` and asserts two
  independent builds agree (`ambient_state_cannot_leak_in`).

**Bug found and fixed during this phase**: running both determinism tests under the default
parallel test runner caused a `SIGABRT` — `git2::opts::set_search_path` mutates process-global
libgit2 state and is not safe to call concurrently from two test threads. Fixed with
`std::sync::Once` so the redirect happens exactly once per test binary regardless of how many
`#[test]`s call it. Confirmed fixed: `cargo test -p git-fixtures` (default parallelism) and
`cargo test --workspace` are both green.

**Deviation from design.md §2.4, recorded there is no way around it**: one sub-bullet asks for
"one long edge spanning more than the short-edge window" (`SHORT_EDGE_SPAN` = 120 rows in
`layout.ts`). This is mathematically impossible in a 16-commit graph (max possible span is 15
rows), and `spec.md`'s acceptance criteria and Spec B's assertions are pinned to exactly 16
commits. Documented in `spec.rs`'s module doc and `tasks.md` 2.2 rather than silently dropped
or silently oversized.

Manifest measured: `commitCount: 16`, `laneCount: 4`, `row0Alias: "m4"`, `row0Lane: 0`.

---

## Phase 3 — Native Specs

- Deleted `e2e/spike.spec.ts`, `wdio.conf.ts`, `e2e/__screenshots__/native-welcome.png` (and
  the now-empty `e2e/__screenshots__/` directory).
- `wdio.shared.conf.ts` — pins `LANG=en_US.UTF-8` for the wdio/mocha process, shared
  framework/reporter/timeout options.
- `wdio.native.conf.ts` — `wdio:tauriServiceOptions.appArgs` capability shape (Phase 1's
  finding), `onPrepare` clears WebKit storage then runs `cargo run -p git-fixtures --bin
  build-fixture` and injects the manifest's absolute path into the capability before any
  worker starts.
- `e2e/support/fixture.ts` / `e2e/support/artifacts.ts` — typed manifest reader and
  gitignored artifact-path helper, matching `build-fixture.rs`'s JSON shape exactly (including
  the new `row0Alias`/`row0Lane` fields added to serve Spec B's probe-coordinate assertion —
  design.md's example manifest didn't have these; added because "read from the manifest's row
  0" (design.md §5) needs a field to read).
- `e2e/native/smoke.spec.ts` (Spec A) — window title, header repo name (== fixture name), all
  4 local branch names, and the tag name (after expanding the sidebar's collapsed "Tags"
  section — `Sidebar.tsx`'s `Section` doesn't render collapsed children at all, not just hides
  them, so this had to be a real click, not just a text search).
- `e2e/native/regressions/graph-viewport.spec.ts` (Spec B) — 5 truthful, un-inverted
  assertions across 2 `it()`s: canvas sanity, row count vs. `fixture.commitCount`, resize
  tracking (fractional `getBoundingClientRect` height, per design.md §5's precision note),
  canvas backing-store size derived from the manifest's `laneCount` and measured viewport
  height, and a painted-pixel probe at `laneX(row0Lane)`/`rowY(0)`. Layout constants
  (`ROW_HEIGHT`, `LANE_WIDTH`, etc.) are mirrored, not imported, from
  `src/features/graph/layout.ts` — the harness must not depend on the product module it is
  trying to catch a defect in.

**Measured**:
- Spec A: **green**. `1 passing (2m 56.8s)`. Sidebar rendered all 4 branches
  (`feature-b`, `feature-a`, `main`, `refactor/parser`) and the tag `v0.1.0` correctly.
- Spec B: **red, exactly for F1**. `2 failing (1m 11.7s)`, both failures inside the spec's own
  `assert.equal` calls (`graph-viewport.spec.ts:69` and `:97`), never a launch/timeout/selector
  error:
  - `AssertionError [ERR_ASSERTION]: expected 16 rows, got 7`
  - `AssertionError [ERR_ASSERTION]: expected row count to track the resized viewport (~13 rows for a 178px-tall scroller), got 7 (was 7 before resizing)`

  Root cause confirmed by the numbers themselves: `viewportHeight` React state in
  `CommitGraph.tsx` starts at `0` and its `useLayoutEffect`'s `ResizeObserver` callback is
  never observed to update it inside this harness, so the initial row-window math
  (`Math.ceil((0+0)/28) + OVERSCAN(6) = 6` → 7 rows, 0-indexed) stays fixed regardless of the
  real 828px-tall scroller, and the canvas is never resized past its un-sized browser default
  (matches the orchestrator's earlier probe: `canvas 300x150, painted 0`).

---

## Phase 4 — Expected-Failure Guard

- `scripts/expect-red.sh` — wraps a command (all args except the last), captures its exit code
  and combined stdout/stderr, and passes only when the exit is non-zero **and** the output
  contains the last argument as a literal substring. Fails closed on every other outcome
  (command passed → "F1 appears fixed" message; command failed for an unrelated reason →
  "not for the expected reason" message).
- `package.json` scripts: `e2e:native:smoke`, `e2e:native:regressions`.

**Measured** (all three outcomes the design's failure-mode table names):
1. `./scripts/expect-red.sh pnpm e2e:native:regressions "expected 16 rows, got 7"` →
   `expect-red.sh: PASS — expected failure confirmed: "expected 16 rows, got 7"`, exit **0**.
2. `./scripts/expect-red.sh true "expected 16 rows, got 7"` →
   `expect-red.sh: FAIL — the wrapped command passed. F1 appears fixed. Remove this guard and
   the CI step's wrapper (fix-graph-viewport).`, exit **1**.
3. `./scripts/expect-red.sh sh -c "echo boom; exit 3" "expected 16 rows, got 7"` →
   `expect-red.sh: FAIL — the wrapped command failed, but not for the expected reason.`,
   exit **1**.

---

## Definition-of-done verification (re-run at the end, not just once during development)

| Check | Command | Result |
|---|---|---|
| Workspace tests | `cargo test --workspace` | ✅ all green (5 git-core + 2 git-fixtures + 0 doctests, default parallelism) |
| Clippy | `cargo clippy --workspace --all-targets` | ✅ clean |
| Formatting | `cargo fmt --all --check` | ✅ clean (two `cargo fmt --all` passes needed during development; both re-verified clean afterward) |
| Frontend build | `pnpm build` (`tsc --noEmit && vite build`) | ✅ `47 modules transformed`, built in 398ms |
| G1 build graph, release | `cargo tree -p gitvisor -e normal --release \| rg wdio` | ✅ 0 matches |
| G1 build graph, e2e feature | `cargo tree -p gitvisor -e normal --features e2e-webdriver \| rg wdio` | ✅ 1 match (`tauri-plugin-wdio-webdriver v1.3.0`) |
| Release-safety compile gate | `cargo check --release --manifest-path src-tauri/Cargo.toml --features e2e-webdriver` | ✅ fails to compile with the `compile_error!` message, as designed |
| Plain build still works | `cargo build -p gitvisor` | ✅ |
| e2e-feature build still works | `cargo build --manifest-path src-tauri/Cargo.toml --features e2e-webdriver` | ✅ |
| Spec A | `pnpm exec wdio run wdio.native.conf.ts --spec ./e2e/native/smoke.spec.ts` | ✅ green |
| Spec B | `pnpm exec wdio run wdio.native.conf.ts --spec ./e2e/native/regressions/graph-viewport.spec.ts` | ✅ red for F1, exit 1 |
| Expected-failure guard | `./scripts/expect-red.sh pnpm e2e:native:regressions "expected 16 rows, got 7"` | ✅ passes |

**Known pre-existing gap, not a regression**: `pnpm exec tsc --noEmit -p tsconfig.wdio.json`
reports `TS2688: Cannot find type definition file for '@wdio/globals/types'` — a pnpm
hoisting/type-resolution issue unrelated to this change's code (the `types` array in
`tsconfig.wdio.json` was already `["node", "@wdio/globals/types", "@wdio/mocha-framework"]`
before this run; only `include` was edited). Not part of Definition of Done's required
commands (`cargo test`, `cargo clippy`, `cargo fmt`, `pnpm build`), and does not affect runtime
correctness — both native specs actually ran and produced correct pass/fail results via wdio's
own `tsx`-based transpilation, which does not do full type-checking. Left unfixed; flagging so
it isn't silently discovered later and mistaken for something this change broke.

---

## Files changed (Phases 1–4)

| File | Action |
|---|---|
| `Cargo.toml` | Modified — added `tools/git-fixtures` to workspace members |
| `tools/git-fixtures/Cargo.toml` | Created |
| `tools/git-fixtures/src/spec.rs` | Created |
| `tools/git-fixtures/src/lib.rs` | Created |
| `tools/git-fixtures/src/oids.rs` | Created |
| `tools/git-fixtures/src/bin/build-fixture.rs` | Created |
| `tools/git-fixtures/tests/determinism.rs` | Created |
| `e2e/spike.spec.ts` | Deleted |
| `wdio.conf.ts` | Deleted |
| `e2e/__screenshots__/native-welcome.png` | Deleted |
| `wdio.shared.conf.ts` | Created |
| `wdio.native.conf.ts` | Created |
| `tsconfig.wdio.json` | Modified — `include` updated for the new config file names |
| `e2e/support/fixture.ts` | Created |
| `e2e/support/artifacts.ts` | Created |
| `e2e/native/smoke.spec.ts` | Created (Spec A) |
| `e2e/native/regressions/graph-viewport.spec.ts` | Created (Spec B) |
| `scripts/expect-red.sh` | Created |
| `package.json` | Modified — added `e2e:native:smoke` / `e2e:native:regressions` scripts |
| `openspec/changes/visual-verification-harness/design.md` | Modified — added the U3 resolution section |
| `openspec/changes/visual-verification-harness/tasks.md` | Modified — Phases 1–4 checked off with evidence |

Not touched, per explicit instruction: `crates/git-core/examples/dump.rs`, `README.md`.

---

## Intended commit boundaries (working tree left uncommitted, per instruction)

Two work units, matching `tasks.md`'s "Suggested Work Units" table (Unit 1 and Unit 2), for
the `stacked-to-main` chain:

1. **`tools/git-fixtures` deterministic builder** (design.md §2, tasks Phase 2):
   `Cargo.toml`, `tools/git-fixtures/**`.
   - Focused test: `cargo test -p git-fixtures` → 2 passed.
   - Runtime harness: N/A — determinism is proven by `cargo test`, no UI involved (as
     `tasks.md`'s work-unit table states).
   - Rollback: remove `tools/git-fixtures` from the workspace members list and delete the
     directory; nothing else in the tree depends on it yet.

2. **Native specs (A green, B red) + wdio split + expect-red guard** (design.md §3, §5, tasks
   Phases 1, 3, 4): `wdio.shared.conf.ts`, `wdio.native.conf.ts`, `tsconfig.wdio.json`,
   `e2e/support/**`, `e2e/native/**`, `scripts/expect-red.sh`, `package.json`'s two new
   scripts, deletion of `e2e/spike.spec.ts`/`wdio.conf.ts`/the old screenshot, and the
   design.md U3-resolution section.
   - Focused test: `pnpm e2e:native:smoke` (green) then
     `./scripts/expect-red.sh pnpm e2e:native:regressions "expected 16 rows, got 7"` (passes).
   - Runtime harness: real `gitvisor` binary (`--features e2e-webdriver`), real WKWebView,
     Unit 1's fixture.
   - Rollback: delete `e2e/native/`, `wdio.native.conf.ts`, `wdio.shared.conf.ts`,
     `scripts/expect-red.sh`; this depends on Unit 1 but nothing later depends on it yet.

Both units are currently uncommitted in one working tree (git history in this repo is a single
empty "Initial commit" — nothing was tracked before this run). No commits were created; the
user has not asked for any.

---

## Stopped here as instructed

Phase 5 (browser mode), Phase 6 (CI), Phase 7 (docs) are **not started**. This is a deliberate
stop, not a blocker: the resolved delivery decision at the top of `tasks.md` scopes this apply
run to Phases 1–4 only, and Spec B now proves F1 with a captured failing message, which is the
gate for the next change (`fix-graph-viewport`) rather than for more harness infrastructure.

### Status
18/18 tasks complete for the assigned scope (Phases 1–4: 2+6+7+3). 15 tasks remain,
deliberately deferred (Phase 5: 5, Phase 6: 8, Phase 7: 2). Ready for `sdd-verify` against
this scope, or for `fix-graph-viewport` to start using Spec B as its acceptance test.
