# E2E Verification Harness Specification

## Purpose

Native + browser-mode Tauri E2E harness giving the agent, CI, and humans real eyes on the app: deterministic fixtures, a green smoke spec, a red F1 regression spec, generated browser-mode mocks, and release-safety guarantees for the WebDriver plugin. F1/F2 fixes stay out of scope.

## Requirements

### Requirement: Release Safety Verification

A release build MUST exclude `tauri-plugin-wdio-webdriver`, its ACL capabilities, and any trace of its IPC identifier. Verification MUST assert both the absent case (release) and the present case (`e2e-webdriver` feature), so a check that silently stops matching cannot pass by accident.

#### Scenario: Two independent proofs of absence and presence
- GIVEN `cargo tree -p gitvisor -e normal --release` and a scanned release binary
- WHEN inspected/scanned THEN both show no trace of the plugin (build graph, IPC string `wdio-webdriver`)
- AND WHEN the same checks run with `--features e2e-webdriver` THEN both show it present

#### Scenario: Shipped capabilities exclude harness permissions
- GIVEN the default/release capability set
- WHEN inspected
- THEN neither `wdio-webdriver:default` nor `core:window:default` is present

### Requirement: Deterministic Fixture Generation

The `tools/git-fixtures` builder MUST produce byte-identical commit OIDs across machines, verified by `cargo test` against hardcoded constants. Pinned: author/committer name+email+time+UTC-offset, branch/tag names, tree/blob content, graph shape (merges, diverging/reconverging branches, tag on non-tip commit).

#### Scenario: Two runs produce identical OIDs
- GIVEN the builder run on two different machines
- WHEN each run finishes
- THEN both HEAD OIDs equal the same hardcoded constant, asserted by `cargo test`

#### Scenario: Ambient state cannot leak in
- GIVEN local `git config`, system clock, or `init.defaultBranch` differ from pinned values
- WHEN the builder runs
- THEN resulting OIDs are unaffected

### Requirement: Native Smoke Spec Proves the Harness Is Alive

`e2e/native/smoke.spec.ts` MUST pass against the real binary in the real WKWebView, launched with the fixture path as `argv[1]` — proof a red suite isn't a broken harness.

#### Scenario: Smoke spec is green
- GIVEN the real binary launched with the fixture repo path
- WHEN the app boots
- THEN window title, sidebar branch/tag names, and header repo name are all correct

### Requirement: Native Regression Spec Detects F1

`graph-viewport.spec.ts` MUST assert rendered row count tracks viewport height across a resize, not merely that a minimum count exists, and MUST fail today specifically for F1 — never a launch error, timeout, or missing selector.

#### Scenario: Initial viewport under-renders rows
- GIVEN a 16-commit fixture and a window tall enough for more than 16 rows
- WHEN the commit list is measured
- THEN fewer than 16 rows are in the DOM (today: 7)

#### Scenario: Row count does not track a resize
- GIVEN the window is then shrunk so fewer rows should fit
- WHEN the rendered count is compared before/after
- THEN it is unchanged (today: still 7) — proving it does not track height

#### Scenario: Failure reason is F1, not harness breakage
- GIVEN the spec run completes
- WHEN output is inspected
- THEN it fails with the row-count message (expected 16, got 7), never a launch/timeout/selector error

### Requirement: Browser-Mode Mocks Are Generated and Diff-Checked

Browser-mode `invoke()` mocks MUST be generated from `crates/git-core/examples/dump.rs` against the fixture, never hand-authored; CI MUST regenerate and fail on any diff from committed copies.

#### Scenario: Unchanged model types produce no diff
- GIVEN `git-core` types unchanged since mocks were committed
- WHEN CI regenerates mocks
- THEN output matches committed mocks exactly

#### Scenario: A model type change is caught
- GIVEN a field is added to a `git-core` type
- WHEN CI regenerates and diffs mocks
- THEN the diff is non-empty and the job fails

### Requirement: CI Trigger Matrix

CI MUST run the fast gate (unit tests, clippy, fmt, `pnpm build`, the release-safety build-graph check, browser-mode e2e) blocking on every push/PR. Native WebKitGTK e2e MUST run blocking on push/PR toward `main`. Native WKWebView e2e plus the release-binary artifact scan MUST run nightly, on `workflow_dispatch`, and on release tags. No native job may run under `continue-on-error`.

#### Scenario: Fast gate runs on every push/PR
- GIVEN a push or pull request
- WHEN CI runs
- THEN tests, clippy, fmt, `pnpm build`, the build-graph check, and browser-mode e2e all run and block on failure

#### Scenario: Native e2e runs on slower triggers
- GIVEN a PR/push to `main` THEN WebKitGTK e2e runs and blocks on failure
- AND GIVEN nightly, dispatch, or a release tag THEN WKWebView e2e runs on macOS with the release-binary artifact scan

### Requirement: Contributor Commands Use Only Free, Open-Source Tooling

Every command a contributor needs to run the harness locally MUST be documented (README or CONTRIBUTING) and MUST require no proprietary or paid tooling.

#### Scenario: Contributor runs the full local loop
- GIVEN a fresh checkout
- WHEN the contributor follows the documented commands
- THEN the fixture, browser-mode e2e, and native-mode e2e all run using free/open-source packages only

---

**Amended 2026-08-20.** The CI Trigger Matrix originally described Native
WebKitGTK e2e as non-blocking, tracked pending `fix-graph-viewport`. That change
has landed (Spec B is green) and the Linux probe has passed on `ubuntu-latest`,
so the temporary allowance no longer applies and the requirement is tightened to
match the shipped workflows. Recorded here rather than silently rewritten: the
implementation exceeded this text, and the text was the thing that was wrong.
