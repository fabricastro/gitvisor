# Verify Report: visual-verification-harness

**Date**: 2026-08-20
**Verifier**: sdd-verify (executor)
**Method**: Every claim below was established by *running* the command, not by
reading `apply-progress.md` and trusting it. Exit codes and captured output
are reported inline. Where I could not run something (a real GitHub Actions
trigger), I say so explicitly rather than reporting it as verified.

## Completeness (tasks.md)

32/33 tasks checked `[x]`. The one exception, task 6.5 (run the Linux
`workflow_dispatch` probe), carries the `[~]` marker with body text that
literally reads "Not run — cannot be" — but a later section of the same file,
"Linux probe resolved (2026-08-18)", states the probe *was* run three times
and that Linux native e2e was promoted to a blocking `push`/`pull_request`
trigger. I confirmed the promoted trigger is real (see CI section below), so
the substance is done. The checkbox/body text at line 99 was never updated to
match — a reader scanning only the numbered task list would see a stale "not
run" claim. **WARNING**, not CRITICAL: the correct, fuller story is present in
the same file, just not at the canonical task location.

## Build / test / lint — all run, all green

| Command | Result |
|---|---|
| `cargo test --workspace` | ✅ 5 git-core + 2 git-fixtures + 0 doctests, all pass |
| `cargo clippy --workspace --all-targets` | ✅ clean, exit 0 |
| `cargo fmt --all --check` | ✅ clean, exit 0 |
| `pnpm build` (`tsc --noEmit && vite build`) | ✅ 47 modules, built in ~450–580ms |

## Release-safety gates — both directions run

| Check | Result |
|---|---|
| `cargo tree -p gitvisor -e normal \| rg -i wdio` | ✅ no match (absent) |
| `cargo tree -p gitvisor -e normal --features e2e-webdriver \| rg -i wdio` | ✅ `tauri-plugin-wdio-webdriver v1.3.0` present |
| `cargo check --release --manifest-path src-tauri/Cargo.toml --features e2e-webdriver` | ✅ fails to compile; real exit code **101**, error text is the `compile_error!` message from `src-tauri/src/lib.rs:2` |

Confirms the `compile_error!` double-`cfg` gate actually fires, not merely
that it's present in source.

## `scripts/release-scan.sh` — all three modes, against real artifacts

Built a real release bundle (`pnpm app:build` → `target/release/bundle/macos/Gitvisor.app`, `.dmg`) and a real e2e-webdriver debug binary (`pnpm run e2e:build`, the correct command — never a plain `cargo build`, confirmed devUrl-stripping worked since the resulting app doesn't depend on a dev server being up).

| Mode | Invocation | Result | Exit |
|---|---|---|---|
| Release artifact alone | `release-scan.sh target/release/bundle/macos/Gitvisor.app` | `absent` (string probe: no match; symbol probe: uninformative-on-stripped-binary, logged not treated as evidence) | 0 |
| E2E artifact alone | `release-scan.sh target/debug/gitvisor` | `present` — string probe found `wdio-webdriver`, symbol probe found `tauri_plugin_wdio_webdriver` | 1 |
| Positive control | `release-scan.sh --positive-control <release .app> <e2e binary>` | `PASS — absent from the release artifact, present in the known-positive e2e artifact` | 0 |

All three outcomes match what `apply-progress.md` reported for the earlier
run — reproduced independently here, not merely re-read.

## Config validation

`python3 scripts/validate-config.py` → 8/8 files OK (`.github/workflows/{ci,e2e-native-linux-probe,e2e-native-macos,release}.yml`, `openspec/config.yaml`, `package.json`, `tsconfig.json`, `tsconfig.wdio.json`), exit 0.

## Mock reproducibility

`pnpm run e2e:mocks` (rebuilds the fixture, regenerates `e2e/mocks/history.json` via `dump-mocks`) → `git diff --stat -- e2e/mocks` and `git status --short -- e2e/mocks` both empty. Byte-identical regeneration confirmed.

## Browser mode and both native specs — run against the real binary/browser

| Spec | Command | Result |
|---|---|---|
| Browser-mode (`welcome.spec.ts`) | `pnpm e2e:browser` | ✅ `1 passing (401ms)`, exit 0 |
| Native smoke (Spec A) | `pnpm e2e:native:smoke` | ✅ `1 passing (2m 35s)`, real WKWebView, real binary, real fixture |
| Native regression (Spec B, `graph-viewport.spec.ts`) | `pnpm e2e:native:regressions` | ✅ `2 passing (1m 19.8s)` — **F1 is fixed**, both assertions (initial viewport row count, resize tracking) now pass. Confirms the `fix-graph-viewport` amendment's premise is real, not assumed. |

Spec B being green (rather than red) is expected and correct: `tasks.md`'s
"Amendment after `fix-graph-viewport`" states F1 was fixed and Spec B is now
green, `scripts/expect-red.sh`/Phase 4 retired. I confirmed this by running
the spec, not by reading the amendment.

## CI workflow YAML — parsed and job-dependency-checked

Parsed all four workflow files (`ci.yml`, `release.yml`, `e2e-native-macos.yml`, `e2e-native-linux-probe.yml`) with the `yaml` npm package (run from inside the scratchpad's `pwtest/` so `node_modules` resolution worked). All four parse without error.

`release.yml`'s job graph, read back from the parsed AST:
- `build`: no `needs` (root)
- `scan`: `needs: ["build"]`
- `publish`: `needs: ["scan"]`

Confirms the chain `build → scan → publish` is real, not merely claimed in a comment. `rg -n "continue-on-error" .github/workflows/*.yml` matches only inside comment text explaining its *absence* — no live `continue-on-error:` key anywhere in any workflow.

**Trigger matrix cross-check against spec.md**, read from the live files:
- `ci.yml`: `on: push (main) / pull_request` — fast gate, blocking. Matches spec.
- `e2e-native-macos.yml`: `schedule` (nightly cron), `workflow_dispatch`, `push: tags: v*` — matches spec's "nightly, dispatch, release tags" for WKWebView.
- `e2e-native-linux-probe.yml`: **now `push (main)` / `pull_request` / `workflow_dispatch`**, promoted from `workflow_dispatch`-only per the "Linux probe resolved" amendment (probe passed 3rd attempt: `Running: WebKitGTK (v605.1.15) on linux`, Spec A green).

This is a genuine, but *documented*, deviation from `spec.md`'s literal
"CI Trigger Matrix" requirement text, which still reads: "Native WebKitGTK
e2e SHOULD run non-blocking toward `main`, tracked pending
`fix-graph-viewport`." That was true when `fix-graph-viewport` was still
open. It has since landed, the probe passed, and the job was promoted to a
blocking trigger — strictly more coverage than the requirement demands, not
less. `spec.md` itself was not amended to match the new state (only
`tasks.md`'s "Linux probe resolved" section documents it). **WARNING**: spec
text is stale relative to a superseding, better-than-spec implementation —
worth a follow-up spec amendment so a future reader isn't confused about which
document is authoritative.

## Contributor docs

Both `README.md` (Spanish primary) and `README.en.md` document `pnpm e2e:browser`, `pnpm run e2e:mocks`, and `scripts/release-scan.sh`'s positive-control discipline under their respective Testing/Pruebas sections. Every tool referenced (`@wdio/*`, `tauri-plugin-wdio-webdriver`, `git2`) is free/open-source — confirmed by inspection of `package.json`/`Cargo.toml` dependencies already exercised above; no paid/proprietary tooling anywhere in the commands actually run during this verification.

Note: `apply-progress.md`'s Phase 7 write-up describes editing a `## Testing`
section under an assumed-English README; the repository's README was later
split into `README.md` (Spanish) / `README.en.md` (English) by a subsequent
commit (`ecde71e`, outside this change's own commit set per `git log`). Both
files currently carry the documented commands under their respective
language's Testing heading, so the requirement is satisfied in the current
tree even though the historical narrative in `apply-progress.md` describes an
earlier README shape.

## Spec compliance matrix

| Requirement | Status | Evidence |
|---|---|---|
| Release Safety Verification | ✅ Satisfied | `cargo tree` both directions, `compile_error!` gate, capability split (`capabilities/{app,e2e}/`, unchanged from "already implemented" baseline, not re-verified line-by-line here since apply-progress documents it and file layout is unchanged) |
| Deterministic Fixture Generation | ✅ Satisfied | `cargo test --workspace` includes `fixture_oids_are_deterministic` and `ambient_state_cannot_leak_in`, both pass |
| Native Smoke Spec Proves the Harness Is Alive | ✅ Satisfied | Spec A run, green, real binary/WKWebView |
| Native Regression Spec Detects F1 | ✅ Satisfied (spec now green — F1 fixed) | Spec B run, 2/2 assertions pass; this is the expected post-`fix-graph-viewport` state per the amendment, not a harness regression |
| Browser-Mode Mocks Are Generated and Diff-Checked | ✅ Satisfied | Regeneration produced zero diff; `mocks-drift` CI job implements the diff-and-fail path (not independently triggered on GitHub, see Gaps below) |
| CI Trigger Matrix | ✅ Mostly satisfied, ⚠️ spec text stale | Fast gate, WKWebView triggers verified in file content; Linux job's promotion to blocking is real but undocumented in spec.md itself |
| Contributor Commands Use Only Free, Open-Source Tooling | ✅ Satisfied | All documented commands run above; no proprietary tooling |

## Gaps / things I could not verify by running them

- **Actual GitHub Actions execution.** I parsed and lint-adjacent-checked
  (`actionlint` was not available on this machine, matching the task
  instructions — `python3 scripts/validate-config.py` plus manual YAML parse
  substituted) all four workflow files, and independently re-ran the
  underlying command each CI step invokes (tests, clippy, fmt, build,
  `cargo tree`, browser e2e, mocks regeneration, release-scan). I did not and
  could not trigger a real workflow run on GitHub — the repository is not
  pushed from this session, consistent with the explicit "do not push"
  instruction recorded throughout `apply-progress.md`. The claim "the Linux
  probe passed three times on GitHub" is taken from `tasks.md`'s own written
  record (run URLs are not reproducible from this sandbox); I verified the
  *mechanism* the probe now exercises (`pnpm run e2e:build`'s devUrl-stripping)
  functions correctly by running it myself, which is the actual defect the
  probe's second failed attempt found and fixed.
- **`actionlint` was genuinely unavailable** on this machine (per the task's
  own environment note) — substituted with a direct `yaml` package parse of
  all four workflow files (all parsed without error) plus inspection of
  `release.yml`'s job graph for the `needs:` chain. This is a narrower check
  than `actionlint`'s schema-aware validation (it would not catch, e.g., an
  unknown GitHub Actions context or a bad step reference), so I report this as
  a partial substitute, not equivalent coverage.

## Issues

**CRITICAL**: none found.

**WARNING**:
1. Task 6.5's checkbox (`tasks.md` line 99) still carries `[~]` and body text
   reading "Not run — cannot be", contradicted by the later "Linux probe
   resolved" section in the same file which says the task is done. The
   canonical task-list location was never updated; only an addendum was.
   Recommend updating line 99 to `[x]` with a short pointer to the
   "Linux probe resolved" section, so `tasks.md`'s task list is
   self-consistent without needing to read to the end of the file.
2. `spec.md`'s "CI Trigger Matrix" requirement text ("Native WebKitGTK e2e
   SHOULD run non-blocking toward `main`, tracked pending
   `fix-graph-viewport`") is now stale — the actual, verified implementation
   is *stronger* (blocking `push`/`pull_request` on `main`, per the probe's
   success) but the spec document itself was never amended to say so.
   Recommend a short spec addendum (mirroring the tasks.md amendment
   pattern already used for the F1 fix) so `spec.md` and the shipped
   workflow don't visibly disagree.
3. `actionlint` was unavailable on this verification machine (same
   constraint the apply run itself recorded); YAML validity and the
   `needs:` chain were confirmed by a plain parse instead. This is a real,
   if narrower, gap versus the task instruction's original ask — flagging it
   rather than silently treating the plain parse as equivalent.

**SUGGESTION**:
1. `apply-progress.md`'s Phase 7 narrative describes editing a single
   `## Testing` section under an English-default README; the repository's
   README has since split into `README.md` (Spanish) / `README.en.md`
   (English) via a later, out-of-change commit. Both currently carry the
   documented commands, so nothing is broken, but a reader cross-referencing
   `apply-progress.md` against the current README shape may be briefly
   confused about which file the write-up refers to. No action required
   unless the docs shift again.
2. `apply-progress.md`'s Phase 6 write-up records a second bug (`cargo tree`
   has no `--release` flag) found and fixed during the *previous* apply run,
   with an explicit note that the earlier run's own Definition-of-done table
   had reported a false positive on that exact check. This verify pass
   re-ran the corrected invocations and confirms the fix holds; recording
   here only so the false-positive history stays visible for anyone auditing
   this change's trustworthiness later.

## Verdict

**PASS WITH WARNINGS.**

Every spec requirement has real, reproduced runtime evidence — not source
inspection alone. 32/33 tasks are genuinely complete; the 33rd (6.5) is
substantively complete too, just documented in the wrong place in
`tasks.md`. The two WARNING-level spec/tasks inconsistencies described above
are documentation drift, not implementation defects: in both cases the
underlying system does *more* than the stale text claims, not less. No
CRITICAL issues block archive.
