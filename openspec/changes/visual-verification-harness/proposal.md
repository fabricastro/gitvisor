# Proposal: visual-verification-harness

Give the agent, CI, and the human a way to **see the real app**. This change formalises the spike into a
supported end-to-end harness that drives the actual Tauri binary in the actual WKWebView via
`@wdio/tauri-service`, produces screenshots, and asserts on what is really rendered. It ships with a
regression test that **deliberately fails today** because of defect F1 — the harness must be shown to catch a
real bug before anyone is asked to trust it.

**This change does not fix F1 or F2.** Those belong to a later change, working name `fix-graph-viewport`.

---

## 1. Why now

The project has four verification gates — `tsc --noEmit`, `vite build`, `cargo clippy`, `cargo fmt`, and 5
`cargo test -p git-core` unit tests. All five pass on a build in which **the commit graph does not render at
all and only 7 of 16 commits are listed** (`findings.md`, F1). Every existing gate is blind to the thing the
product actually is: pixels on a screen showing a repository's history.

That blindness is not theoretical. It was measured: the very first native screenshot taken during the spike
exposed two defects, one severe, that had survived the entire existing gate stack.

Three routes were eliminated by direct experiment before this proposal (see `explore.md`): the Claude Chrome
extension is not connected on this machine; `osascript`/`screencapture` of the native window hangs on macOS
Accessibility and Screen Recording permission prompts and is therefore unusable unattended; and the official
`tauri-driver` genuinely does not support macOS.

## 2. What is already settled — by experiment, not argument

The spike ran. The result collapses the option set, and these points are **not reopened by this proposal**:

| Question | Answer | Evidence |
|---|---|---|
| Can WebDriver drive the real Tauri app on macOS? | Yes, via `@wdio/tauri-service` + the embedded provider | `Running: webkit (v605.1.15) on macos`, 1 passing in 8.4s |
| Can it produce a screenshot? | Yes | `browser.saveScreenshot()` → 2880×1800 PNG of the real app |
| Is the toolchain proprietary? | No | `tauri-plugin-wdio-webdriver` 1.3.0 on crates.io, 50,050 downloads, updated 2026-08-03; `@wdio/tauri-service` 1.3.0 on npm |
| Do we need a custom HTTP bridge crate? | **No — cancelled** | The service ships browser mode + `invoke()` mocking as maintained features |
| Do we need an `api.ts` transport shim? | **No — cancelled** | Same |

The bridge and the shim were the exploration's recommendation. The spike made them unnecessary. Building them
now would mean owning a Cargo crate, a second serialization surface, a drift risk, a symmetry test, and dead
code in every user's bundle — in exchange for a *weaker* result than the native path already demonstrated.
**There is no second command surface in this design, so there is nothing to keep in sync.**

## 3. Product scope check

`openspec/config.yaml` sets `product_scope.decision: "viewer + safe writes"`.

**Result: compatible, and actively constrained by it.** This change ships no user-facing capability — no new
Tauri command, no UI, no git write. It appears on neither `in_scope` nor `out_of_scope`, because those lists
enumerate *product* capabilities and this is developer infrastructure.

That is not a free pass. Two constraints fall directly out of the boundary:

1. **Fixtures and specs exercise only `in_scope` operations.** A fixture generator can trivially produce a
   rebase or a cherry-pick history. It will not. The fixture covers history graph, branches, merges, tags,
   diffs, and working-directory status — nothing that would quietly normalise a deferred capability into the
   test suite and from there into the product.
2. **The harness must add nothing to the shipped app.** A viewer that also embeds a remote-control server is
   not the product that was scoped. This is why §5.1 is a hard requirement rather than a nicety.

## 4. Scope

### In scope

- Formalise the spike into a maintained harness: `wdio.conf.ts` (native), a browser-mode config, TS config,
  spec layout, and `package.json` scripts.
- **Make the embedded WebDriver plugin structurally impossible to ship in a release binary**, plus an
  automated check that proves it, run in CI.
- A **deterministic fixture repository generator** in Rust, regenerated fresh on every run.
- One **green** native smoke spec (proves the harness itself works).
- One **red** native regression spec asserting the graph renders and the row count tracks viewport height —
  this is the test that fails today because of F1.
- A browser-mode fast-iteration loop with `invoke()` mocks whose payloads are **generated from the real
  backend**, not hand-authored.
- GitHub Actions workflows (greenfield — no `.github/` exists today).
- Contributor documentation: every command needed to run the harness locally, all free and open source.

### Out of scope — explicitly

| Excluded | Why |
|---|---|
| **Fixing F1 and F2** | Separate change (`fix-graph-viewport`). This change's job is to *prove the harness catches F1*, not to make it green. |
| The native folder-picker dialog (`@tauri-apps/plugin-dialog`) | A non-DOM, OS-native surface. WebDriver drives the webview, not `NSOpenPanel`. Sidestepped by launching the binary with the fixture path as `argv[1]`, which the existing `startup_path` command already supports. |
| A custom HTTP bridge crate | Cancelled by the spike. See §2. |
| An `api.ts` transport shim | Cancelled by the spike. See §2. |
| A symmetry test between two command surfaces | There is only one command surface. |
| Vitest or any unit-test runner | The problem is "cannot see the UI", not "no unit coverage". Separate, already-recorded gap in `config.yaml`. |
| Committed pixel baselines | See §5.4 — a deliberate position, not an omission. |
| Windows (WebView2) coverage | Deferred; the spike proved macOS only, and Linux is documented-but-unproven here. |

## 5. Approach and the decisions it commits to

### 5.1 Release safety: the plugin must not exist in a shipped binary

**The problem, stated plainly.** `tauri-plugin-wdio-webdriver` currently sits in `[dependencies]` in
`src-tauri/Cargo.toml`. The *registration* is gated behind `#[cfg(debug_assertions)]`, so the server never
starts in release — but the crate still compiles and links into the release binary. An embedded WebDriver
server is a remote-control surface. "It's debug only" written in a comment is documentation, not a guarantee.

Two mechanisms that do **not** work, ruled out up front so nobody spends an afternoon on them:

- `[target.'cfg(debug_assertions)'.dependencies]` — Cargo explicitly refuses `cfg(debug_assertions)`,
  `cfg(test)`, and `cfg(feature = "…")` in target-specific dependency tables.
- `[dev-dependencies]` — those are available to tests, examples, and benches. The artifact under test here is
  the real `gitvisor` binary, which does not see them.

**Decision: an opt-in Cargo feature over an optional dependency.**

```toml
[dependencies]
tauri-plugin-wdio-webdriver = { version = "1.3", optional = true }

[features]
e2e-webdriver = ["dep:tauri-plugin-wdio-webdriver"]
```

Registration is double-gated: `#[cfg(all(feature = "e2e-webdriver", debug_assertions))]`. The feature is
**not** in `default`. Without it, the crate is not in the dependency graph at all — not compiled, not linked,
not present.

**The capability file must move too.** `src-tauri/capabilities/default.json` currently grants
`wdio-webdriver:default` *and* `core:window:default`. Both are harness needs: `rg` confirms **no file under
`src/` imports `@tauri-apps/api/window`** — the app itself never calls a window API, so `core:window:default`
is pure harness surface widening the shipped app's ACL for no product reason. Both permissions move into a
separate `capabilities/e2e.json`, activated only in the e2e build. Leading candidate mechanism: a
`tauri.e2e.conf.json` overlay passed via `tauri build --config`, selecting capabilities through
`app.security.capabilities`. Final mechanism selection belongs to design; the *requirement* — a release build
resolves neither permission — belongs here and is non-negotiable.

**How absence is verified — two independent proofs, because one is not enough:**

| Proof | Command | Runs |
|---|---|---|
| **Build graph** — the crate is not a dependency | `cargo tree -p gitvisor -e normal --release` contains no `tauri-plugin-wdio-webdriver`; and *with* `--features e2e-webdriver` it does | Every push (blocking, seconds, no GUI deps) |
| **Artifact** — the compiled binary contains no trace | String/symbol scan of the release binary for the plugin's IPC name (`wdio-webdriver`) asserting absence; the same scan asserts *presence* on an e2e build, so a scan that silently stops matching anything cannot pass by accident | Release/tag pipeline |

The second check's inverted assertion is the important half. A grep-for-absence that has quietly stopped
working looks exactly like a grep-for-absence that passes. Asserting the positive case on the e2e binary in
the same script is what makes the negative case mean something.

### 5.2 Deterministic fixtures: a `git2` Rust helper, regenerated every run

`explore.md` compared a `git2`-based Rust helper against a `git fast-import` stream and declined to pick.
**Picking the Rust helper.**

The justification that decides it is not verbosity or reviewability — it is that **the helper can prove its own
determinism, and the stream cannot**. With author time, committer time, UTC offset, author identity,
committer identity, branch names, tag names, and every blob's bytes pinned as literals, commit OIDs become a
pure function of the input. So the fixture crate carries a `cargo test` asserting `HEAD` equals a hardcoded
OID constant. Any accidental leak of ambient state — a `git config` value, a clock read, a platform line
ending — fails in `cargo test` in under a second, with a diff of two hashes, instead of surfacing weeks later
as a screenshot that "looks slightly different on CI". A fast-import stream cannot self-check without shelling
out to the `git` CLI, which reintroduces exactly the ambient dependency being eliminated.

Supporting reasons:

- `git2` is already a workspace dependency with `vendored-libgit2` — zero new dependencies, zero system
  packages in CI, no dependence on the contributor's `git` binary version or their global `init.defaultBranch`.
- The fixture must exercise graph-layout cases (merges, diverging and reconverging branches, a tag on a
  non-tip commit). Those shapes are easier to express, name, and comment as code than as a stream of
  `commit`/`from`/`merge` directives.
- Contributors are already fluent in this codebase's `git2` usage (`crates/git-core/src/repo.rs`).

**What is pinned** — the full list from `explore.md`, all of it: author and committer time including UTC
offset; author and committer name and email; branch and tag names; every file path and its exact byte
content; and the deliberate graph shape.

**Location:** a new workspace member at `tools/git-fixtures` — path chosen so the tree says "tooling, not
product" out loud, while workspace membership keeps it inside `cargo clippy --workspace --all-targets` and
`cargo fmt --all --check`. It is depended on by nothing; `git-core` and `src-tauri` never reference it.

**Materialisation: regenerated fresh into a gitignored directory (`target/e2e-fixtures/<name>`) on every run.**
No committed `.git` directory, no committed `git bundle`. Generation is sub-second; a checked-in artifact
would need regeneration discipline anyway and buys nothing.

### 5.3 CI: browser mode broadly, native mode narrowly

Native mode needs `webkit2gtk` plus a virtual display on Linux and a full Tauri compile. Browser mode needs
neither. Cost differs by more than an order of magnitude, so the triggers differ.

| Trigger | Job | Runner | Blocking |
|---|---|---|---|
| Every push and PR | `cargo test`, clippy, fmt, `pnpm build`, **release-safety build-graph gate** (§5.1), **browser-mode e2e** | `ubuntu-latest` | Yes |
| PR to `main`, push to `main` | **Native-mode e2e** (WebKitGTK + `xvfb`) | `ubuntu-latest` | **No — see §5.5** |
| Nightly, `workflow_dispatch`, release tags | **Native-mode e2e on the engine users actually get** (WKWebView) + artifact scan (§5.1) | `macos-latest` | No |

Reasoning: the fast, cheap signal runs on everything, so contributors are never waiting minutes for feedback
on a CSS change. The expensive, high-fidelity signal runs where it pays for itself — before merge to `main`
and on a schedule. macOS runner minutes are materially more expensive than Linux for an open-source project,
which is why WKWebView coverage is scheduled rather than per-push, despite macOS being the primary platform.

**Honest caveat:** the spike proved the embedded provider on **macOS only**. Linux WebKitGTK support is
documented but unverified on this project. The first CI task is to establish whether the Linux native job
works at all; if it does not, the fallback is macOS-only native coverage on the narrow trigger, and the Linux
row above is dropped rather than fudged.

**Browser-mode mocks are generated, not written.** The exploration's fatal objection to mock-based testing was
that hand-authored fixture JSON drifts from the real types with nothing to catch it. That objection is
answered by deriving the mock payloads from the real backend: `crates/git-core/examples/dump.rs` already
exists and reads a real repository through `GitRepo`. The e2e mock payloads are produced by running it against
the generated fixture, and CI regenerates and diffs them — a drifted payload fails as a diff, not as a false
green. Browser mode is a fast iteration loop, not the correctness authority; native mode is the authority.

### 5.4 Screenshots: artifacts, not assertions. **No committed pixel baselines.**

**Position: this change commits zero baseline images and performs zero pixel comparison.**

`explore.md` warns that pixel-diffing is only safe within a single engine. This harness spans three rendering
contexts by design — WKWebView on macOS, WebKitGTK on Linux, Chrome in browser mode — across two DPI regimes
(the spike's 2880×1800 Retina capture versus a CI virtual display). A baseline captured in any one of those is
noise in the other two. Add locale-dependent text rendering (which is precisely F2's root cause) and the
absolute fixture path, which differs per machine and can surface in rendered text, and byte-identical images
are unattainable rather than merely inconvenient.

A pixel baseline that gets regenerated every time it fails is not a test. It is a ritual that produces green
checkmarks.

So:

- **Assertions are structural**: DOM queries, rendered row counts, canvas dimensions, canvas pixel *presence*
  at computed coordinates, text content, ref badge attachment.
- **Screenshots are evidence**: written to `target/e2e-artifacts/` (gitignored), uploaded as CI artifacts,
  attached to PRs, and readable by the agent. That is the "give the agent eyes" deliverable, and it does not
  require a baseline to work.
- The spike's committed `e2e/__screenshots__/native-welcome.png` is removed; that directory becomes a
  gitignored artifact path.
- Pixel baselines are not banned forever — they become defensible as their own future change, pinned to one
  engine and one runner image, once one such pair has proven stable. Not now, and not implicitly.

### 5.5 The failing test: what it asserts, and why it fails for the right reason

Two native specs land together, and the pairing is the point.

**Spec A — `e2e/native/smoke.spec.ts` — MUST BE GREEN.**
Launches the real binary with the fixture path as `argv[1]`, and asserts the app boots, the window title is
correct, the sidebar lists the fixture's branches and tags **by name**, and the header shows the fixture repo
name. Without a passing native spec, a red suite is indistinguishable from a broken harness — this is what
makes Spec B's failure mean something.

**Spec B — `e2e/native/regressions/graph-viewport.spec.ts` — EXPECTED RED (defect F1).**
The window is resized to a fixed logical size so the arithmetic is deterministic. Then:

| # | Assertion | Today |
|---|---|---|
| 1 | `[role="listbox"][aria-label="Commit history"]` exists and its measured `clientHeight` is > 0 | passes |
| 2 | With a 16-commit fixture and a viewport tall enough for more than 16 rows, **all 16 rows are in the DOM** | **fails: 7 rows** |
| 3 | Shrink the window so fewer rows fit; the rendered count **changes accordingly**, matching `min(total, ceil((scrollTop + clientHeight) / 28) + 6 + 1)` computed from the *measured* height | **fails: still 7** |
| 4 | The canvas overlay is sized to device pixels: `canvas.height === Math.round(viewportHeight × devicePixelRatio)` and > 0 | **fails: default 150** |
| 5 | The canvas has actually drawn: `getImageData` finds a non-transparent pixel in the graph gutter at the first commit's row-Y (the node dot) | **fails: blank** |

Assertion 3 is the one that matters most and the one `findings.md` explicitly asks for: it proves the row
count **tracks viewport height**, rather than merely being "more than 7". A test that only asserted "≥ 16 rows"
would pass against a fix that hardcodes a large constant. Assertion 5 distinguishes "a canvas element exists"
from "the graph was drawn" — the failure mode F1 actually produces.

**Acceptance criterion for this change:** Spec B fails with assertion 2's message (`expected 16 rows, got 7`),
**not** with a launch error, a timeout, or a selector-not-found. A red test that is red for an uninteresting
reason proves nothing. The captured failing output is the evidence this change is done.

**CI handling of a deliberately red test.** The native job lands with `continue-on-error: true` and a comment
naming `fix-graph-viewport` as the change that removes the flag. Blocking from day one: the browser-mode job
and the release-safety gate. This is stated as a temporary, tracked condition with a named owner-change — not
a permanently tolerated red.

### 5.6 Locale is pinned — and that is a tradeoff, not a win

E2E runs pin the locale (`LANG=en_US.UTF-8`) so text assertions and screenshots are comparable across
machines. The cost, stated openly: **pinning the locale means this harness cannot catch locale-dependent
layout breaks — which is exactly the class F2 belongs to** (`hace 29 minutos` wrapping and colliding rows).
F2 remains a product decision about whether the app follows the OS locale at all, per `findings.md`. The
harness is not pretending to cover it.

## 6. What this harness does NOT verify

The native path closes most of `explore.md`'s "CANNOT catch" list — WebKit rendering, font metrics, real Tauri
IPC and the capability/ACL system are all genuinely exercised now, because the thing under test is the real
binary in the real webview. What remains, precisely:

| Gap | Status |
|---|---|
| **The native folder-picker dialog** | Hard boundary. `plugin-dialog`'s `open({directory:true})` is a non-DOM OS surface; WebDriver cannot see or click it. Bypassed via the `argv[1]` startup path. Not closable by any amount of harness improvement in this shape. |
| **Windows (WebView2)** | Uncovered. No runner, no spike. |
| **Linux (WebKitGTK)** | Documented, unproven here. §5.3 says what happens if it does not work. |
| **Native window dragging** (`data-tauri-drag-region`) | The attribute is present in the real window, but WebDriver cannot perform an OS-level window drag. Whether dragging moves the window is untested. |
| **Traffic-light overlap** (`TITLEBAR_INSET`) | The *branch selection* is testable. Whether 78px is visually correct against real traffic lights depends on whether `saveScreenshot` captures native chrome or webview-only — **not yet known**, and not claimed either way. Design phase resolves it from the spike's 2880×1800 image. |
| **The binary users actually get** | Tests run against a **debug** build with an extra plugin registered. Anything that appears only in an optimised, signed, notarised, bundled `.app` — CSP or asset-protocol differences, packaging issues — is out of reach. This is inherent to the approach and worth repeating in PRs. |
| **Remote operations** (fetch/pull/push) | The fixture is local-only. No remote is exercised. |
| **Locale-dependent layout** | Deliberately excluded by §5.6. |
| **Pixel-level appearance** | No baselines, by decision (§5.4). Screenshots are for humans and the agent to look at, not for a machine to compare. |

**The line not to cross:** a green run of this harness means "the real app, in the real engine, correctly
rendered a real repository's history". It does **not** mean "the shipped macOS app looks right". Any PR that
presents a passing screenshot as the latter is overselling it.

## 7. Rollback plan

Required by `rules.proposal`. Rollback is cheap by construction — nothing here is load-bearing for the product.

**Trigger conditions:** `tauri-plugin-wdio-webdriver` proves unstable or is abandoned; the e2e suite becomes
flaky enough to be ignored; the release-safety verification (§5.1) cannot be made to hold; native CI cost
outgrows its value.

| Layer | How to remove | Blast radius |
|---|---|---|
| Cargo feature + optional dependency | Delete the `[features]` entry, the optional dep, and the `cfg` block in `lib.rs` | None — release builds already exclude it, so removal is a no-op for shipped artifacts |
| `capabilities/e2e.json` + config overlay | Delete both files | None — never referenced by the default build |
| JS devDependencies (`@wdio/*`) | Remove from `devDependencies`, `pnpm install` | None — devDependencies only; no runtime dependency, no bundle impact |
| `pnpm-workspace.yaml` `allowBuilds` entries | Revert the `edgedriver`/`geckodriver` lines | None |
| `wdio.conf.ts`, `tsconfig.wdio.json`, `e2e/` | Delete | None — no `src/` file imports them |
| `tools/git-fixtures` | Remove the workspace member | None — depended on by nothing |
| CI workflows | Delete or disable jobs | Reverts to today's state (no `.github/` exists) |
| The red Spec B | Delete, or leave with `continue-on-error` | The known defect is still recorded in `findings.md` regardless |

**The one-line escape hatch:** disabling the whole harness needs no code change at all — the e2e jobs are
separate workflow jobs and can be switched off in CI while the code stays in the tree, since nothing in the
product build path touches any of it.

**Crucially, rollback does not resurrect anything.** Because this design adds no bridge crate, no transport
shim, and no second command surface, there is no production code to un-thread and no user-facing behaviour to
restore. The spike's pre-existing state is additionally backed up under a temporary directory
`spike-backup/`.

## 8. Success criteria

- [ ] `cargo tree` proves `tauri-plugin-wdio-webdriver` is absent from a default/release build and present with `--features e2e-webdriver`; both directions asserted in CI.
- [ ] `core:window:default` and `wdio-webdriver:default` are gone from the shipped app's capabilities.
- [ ] The fixture generator produces byte-identical commit OIDs on two different machines, asserted by `cargo test`.
- [ ] Native Spec A is **green** on macOS.
- [ ] Native Spec B is **red**, failing on the row-count assertion with a message naming the expected and actual counts.
- [ ] Browser-mode suite runs on `ubuntu-latest` in CI with no webkit2gtk and no Tauri build.
- [ ] Screenshots land in CI artifacts and are readable by the agent.
- [ ] `README` (or `CONTRIBUTING`) documents every command a contributor needs; all are free and open source.
- [ ] No committed baseline images anywhere in the tree.

## 9. Open questions for design

1. The exact Tauri v2 mechanism for feature-conditional capabilities — the `tauri.e2e.conf.json` overlay is the leading candidate (§5.1), not a confirmed one.
2. Whether `saveScreenshot` captures native window chrome or webview-only, which decides whether the traffic-light inset is verifiable (§6).
3. Whether the embedded provider works on Linux WebKitGTK under `xvfb`; the Linux native CI row depends on it (§5.3).
4. The exact fixture graph shape — how many commits, which merges, which tags — chosen to exercise `git-core`'s lane-assignment cases.
5. Where browser-mode mock payloads live and the exact regenerate-and-diff CI step (§5.3).

## 10. Next step

`sdd-spec` and `sdd-design` (parallel). The follow-up change `fix-graph-viewport` fixes F1 and F2 and turns
Spec B green, removing the `continue-on-error` flag as its own acceptance criterion.
