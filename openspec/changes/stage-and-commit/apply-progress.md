# Apply Progress: stage-and-commit — `stage-unstage` half

Scope for this run, per the 2026-08-20 delivery split at the top of
`tasks.md`: the U3 spike, structured errors, `with_fresh_index` (M2),
stage / unstage / bulk stage / bulk unstage, the changed-files UI, and the
harness delta needed to test them. Everything commit-shaped (`git`
resolution, the commit subprocess, the timeout ladder, hooks, signing,
identity via `git var`, HEAD-delta reporting, `CommitBox`) was explicitly
out of scope and was not started.

## Status

29/59 tasks in `tasks.md` are checked (Phase 0.1, Phases 1–3 in full, Phase 5
minus `CommitBox`/`gitProbe`/`createCommit`/the `git_probe` mock, Phase
6.1–6.3). The 30 unchecked tasks break down as: **19 in Phase 4a/4b/4c**
(`git` resolution, the commit subprocess, the timeout ladder, hooks,
signing, the commit test suite) — the entire commit unit, never attempted,
exactly as instructed; **4 in Phase 0.2–0.5** — commit-path spikes, deferred
with it; **7 across Phase 5/6** (`CommitBox`, the `git_probe` mock, the
native write spec and its config/CI on both platforms) — commit-shaped for
the reasons detailed below. None were silently dropped; every skipped task
has an inline reason in `tasks.md`.

## Phase-by-phase

### Phase 0 — Spike gate
- **0.1 (U3) — done.** `cargo clippy -p git-core -- -D clippy::disallowed-methods`
  errors on a throwaway `self.inner.index()` call with the exact configured
  reason, and passes clean once removed. Clippy's `disallowed-methods` does
  resolve paths to a foreign crate's inherent methods — the lint is the
  enforcement mechanism, no source-scan fallback (`index_discipline.rs`) was
  needed or written. Recorded in `design.md`'s U3 register entry.
- **0.2–0.5 — deferred**, each annotated in `tasks.md`: all four are
  commit-path spikes (Tauri freeze check, `git var` identity parity, real
  pinentry behaviour, the unmeasurable post-object hang) that ship with the
  `commit` follow-up change.

### Phase 1 (Unit 1) — Structured errors + wire shape — done
All 9 new `CoreError` variants, `code()`, and a hand-written `Serialize`
(`{ code, message, details? }`) landed in `crates/git-core/src/error.rs`.
The three pre-existing variants keep byte-identical `Display` text; `message`
is always `self.to_string()`, so the seven existing commands' rendered UX is
unchanged by construction. `describe()`/`asCoreError()`/`CoreErrorWire` added
to `src/features/repo/store.ts`, wire-shape branch checked first.
- Test: `cargo test -p git-core error::` → `code_is_distinct_per_variant` green.

### Phase 2 (Unit 2) — Paths + index guard + clippy gate (M2 replay) — done
- `clippy.toml` at the workspace root, full denylist from design §1.3.
- `crates/git-core/src/paths.rs::normalise_repo_path` — pure, no I/O, all of
  M4's four rows plus `a[b].txt`/`.git/config`/NUL as unit tests.
- `crates/git-core/src/repo/index_guard.rs` — `with_fresh_index`/`reload_index`.
  **Deviation from the literal task wording**: nested as a submodule of
  `repo` (`repo/mod.rs` + `repo/index_guard.rs`) rather than a top-level
  sibling of `repo.rs`. This keeps `GitRepo.inner` plain-private (visible only
  to `repo` and its descendants) instead of widening it to `pub(crate)`,
  which matches design §1.2's own claim — "nothing outside
  `crates/git-core::repo` can obtain an `Index`" — literally at the compiler
  level rather than approximately.
- **M2 replay — the non-negotiable requirement.** Two tests exist:
  - `repo::index_guard::tests::m2_external_git_add_survives_with_fresh_index`
    (in `src/repo/index_guard.rs`) calls `with_fresh_index` directly, real
    external `git add` subprocess. **This is the one that actually isolates
    the invariant.** Verified by hand: `index.read(true)?;` was temporarily
    deleted, the test went red (`b.txt` destroyed), then restored and
    re-confirmed green.
  - `crates/git-core/tests/index_freshness.rs` replays the same scenario
    through the public `GitRepo::stage` API. **Finding, recorded in
    `design.md`**: this black-box version does *not* go red when
    `read(true)` is removed, because `stage`'s own pre-flight calls
    `status()` first, and `Repository::statuses()` has an incidental side
    effect of soft-syncing libgit2's cached index. A soft sync is exactly
    what M2's own caveat warns is unreliable (mtime granularity, same-tick
    writes), so this is not evidence `read(true)` is unnecessary — it is why
    the isolated test above exists and is the one that satisfies the "must
    fail if read(true) is removed" requirement. Both tests are kept: one
    proves the mechanism, one replays the spec.md scenario literally through
    the real API.
- `repo::index_guard::tests::err_from_closure_leaves_on_disk_index_untouched`
  — a closure returning `Err` leaves the on-disk index byte-identical.

### Phase 3 (Unit 3) — Stage / unstage + thin commands — done
`GitRepo::stage`/`unstage` (both `&[String]`, never a glob), shared
`preflight_write()` (bare repo, conflicted paths), `WriteOutcome`/
`SkippedPath`/`SkipReason` in `model.rs`, `stage_paths`/`unstage_paths`
Tauri commands. 8 tests in `tests/stage_unstage.rs` replay spec.md's
scenarios literally, including the conflicted-path refusal proving the
on-disk index is untouched, and a write-outcome-vs-`status()` ordering
consistency check. One deviation: the "and (stubbed) commit entry points"
clause of 3.6 was dropped — no commit entry point exists this run.

### Phase 5 (Unit 5) — UI panel + store + browser specs — done, minus commit UI
Built: `api.ts` (`stagePaths`/`unstagePaths`), store additions (`staging`,
`refreshStatus`, `stagePaths`/`unstagePaths` actions), `WorkingDirectoryPanel`
+ `ChangeList`/`ChangeRow` (container/presentational), `RefusalNotice`
(switches on `code`). 4 browser-mode wdio specs, all green, expected
payloads derived in-spec from the generated `working_status` mock.
**Not built**, all commit-shaped and explicitly out of scope: `CommitBox`,
`gitProbe` (store field + `api.ts` wrapper), `createCommit`, the
`git_probe` `dump-mocks` entry, and every checklist item in 5.7/5.8 that
depends on a commit control (disabled-when-`git`-unavailable, hook/signer
stderr rendering, `Committing…` state and its 10-second note).

### Phase 6 (Unit 6) — Harness delta — partially done
Built and verified: `build-fixture --out-root/--name` (defaults unchanged,
`history` untouched byte-for-byte — `determinism.rs` still green, head OID
unchanged), the `writes` recipe (`spec::WRITES_COMMITS` + an independent
`build_writes()` — deliberately not sharing `build()`'s loop, so a refactor
for this recipe's sake cannot touch `history`'s determinism guarantee), and
`initialStatus` on every fixture's manifest (verified: `writes` shows
nothing staged / two `modified` unstaged / one `untracked`; `history`'s
existing dirt now reports through the same field).

**Not built**: `wdio.native.writes.conf.ts`, the native write spec, and both
platform CI runs (6.4–6.8). Reason, not a time-boxing shortcut:
`specs/e2e-verification-harness/spec.md`'s own requirement text is singular
and explicit — *"Exactly one native spec MUST prove the end-to-end path —
stage, **commit**, and the new commit appearing in the graph."* Building a
staging-only native spec would not satisfy that requirement (it proves a
narrower claim under the same name), and shipping one anyway risked exactly
the scope-confusion the delivery split exists to prevent. The fixture this
spec will need already exists and is verified; the spec itself belongs with
the `commit` follow-up change.

## Non-negotiables from the apply brief — status

1. **M2 replayed as a test using a real external `git` subprocess, verified
   to go red without the fix.** Done — see Phase 2 above. Two tests; the
   isolated one is the one that actually satisfies this.
2. **`with_fresh_index` is the only way any write path obtains an `Index`.**
   Done, and enforced two ways: compiler (private field, `repo`-tree-only
   visibility) and clippy (`disallowed-methods`, crate-level `deny`).
3. **Bulk stage/unstage operate on exactly the caller's paths, never a
   glob.** Done — `stage`/`unstage` take `&[String]`; `Index::add_all` etc.
   are clippy-denied; `stage_all_stages_only_what_was_listed` proves an
   un-listed untracked artifact stays untracked.
4. **Path validation in `git-core` before `add_path`, structured code.**
   Done — `normalise_repo_path` runs before any path reaches libgit2;
   `PathOutsideRepo` is a distinct `code()`.
5. **No `Repository::signature` usage, no identity pre-flight.** Honoured —
   `signature` is clippy-denied (added proactively, not yet reachable from
   anywhere) and no identity check exists in this run.

## Definition of done — checked

- `cargo test --workspace`: green (18 + 1 + 8 git-core tests, 2 git-fixtures
  determinism tests, 0 elsewhere).
- `cargo clippy --workspace --all-targets`: clean.
- `cargo fmt --all --check`: clean.
- `pnpm build` (`tsc --noEmit` + `vite build`): clean.
- M2 replay test: passes, and was hand-verified to fail red with the fix
  removed (see Phase 2).
- Browser-mode specs: `e2e/browser/welcome.spec.ts` (pre-existing) and
  `e2e/browser/working-directory.spec.ts` (new, 4 specs) — all green.
- Native suite: `pnpm e2e:native:smoke` and `pnpm e2e:native:regressions`
  both still green against a fresh `pnpm run e2e:build` (run with the
  sandbox disabled — the real WKWebView session needs socket/display access
  the default sandbox denies; this is an environment requirement of the
  harness itself, not a regression).
- Release-safety gates: `cargo tree -p gitvisor -e normal` excludes `wdio`;
  the same command with `--features e2e-webdriver` includes it (positive
  control). `compile_error!` gate untouched.
- `pnpm run e2e:mocks` regenerated against the (unchanged) `history` build
  path and diffed clean against the committed `e2e/mocks/history.json` — the
  `mocks-drift` CI job would still pass.

## Files changed

| File | Action |
|---|---|
| `clippy.toml` | Created |
| `crates/git-core/src/error.rs` | Modified — 9 refusal variants, `code()`, wire `Serialize` |
| `crates/git-core/src/lib.rs` | Modified — crate-level `deny(clippy::disallowed_methods)`, `pub mod paths` |
| `crates/git-core/src/model.rs` | Modified — `WriteOutcome`, `SkippedPath`, `SkipReason` |
| `crates/git-core/src/paths.rs` | Created |
| `crates/git-core/src/repo.rs` → `crates/git-core/src/repo/mod.rs` | Renamed + modified — `stage`, `unstage`, `preflight_write`, helpers |
| `crates/git-core/src/repo/index_guard.rs` | Created |
| `crates/git-core/Cargo.toml` | Modified — `tempfile` dev-dependency |
| `crates/git-core/tests/support/mod.rs` | Created |
| `crates/git-core/tests/index_freshness.rs` | Created |
| `crates/git-core/tests/stage_unstage.rs` | Created |
| `src-tauri/src/commands.rs` | Modified — `stage_paths`, `unstage_paths` |
| `src-tauri/src/lib.rs` | Modified — command registration |
| `src/shared/types.ts` | Modified — `SkipReason`, `SkippedPath`, `WriteOutcome` mirrors |
| `src/features/repo/api.ts` | Modified — `stagePaths`, `unstagePaths` |
| `src/features/repo/store.ts` | Modified — `CoreErrorWire`, `asCoreError`, `staging`, `refreshStatus`, write actions |
| `src/features/working-directory/WorkingDirectoryPanel.tsx` | Created |
| `src/features/working-directory/ChangeList.tsx` | Created |
| `src/features/working-directory/ChangeRow.tsx` | Created |
| `src/features/working-directory/RefusalNotice.tsx` | Created |
| `src/app/App.tsx` | Modified — mounts `WorkingDirectoryPanel` |
| `e2e/browser/working-directory.spec.ts` | Created |
| `tools/git-fixtures/src/spec.rs` | Modified — `WRITES_COMMITS`, `WRITES_HEAD_ALIAS`, `WRITES_LOCAL_BRANCHES` |
| `tools/git-fixtures/src/lib.rs` | Modified — `build_writes()`, clippy allows |
| `tools/git-fixtures/src/bin/build-fixture.rs` | Modified — `--out-root`/`--name`, `initialStatus` |
| `openspec/changes/stage-and-commit/design.md` | Modified — U3 resolution, M2 masking finding, exemption list |
| `openspec/changes/stage-and-commit/tasks.md` | Modified — checkboxes + deviation notes throughout |

## Next recommended

`sdd-verify` for the completed scope (Phases 0.1, 1–3, 5-minus-commit,
6.1–6.3). `sdd-apply` again — as the `commit` follow-up change — for
everything deferred here: `git` resolution, the commit subprocess, the
timeout ladder, hooks, signing, `git var` identity, HEAD-delta reporting,
`CommitBox`, `wdio.native.writes.conf.ts`, the native write spec, and its
CI wiring on both platforms.
