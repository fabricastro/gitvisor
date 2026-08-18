# Exploration: visual-verification-harness

## Problem Statement

The AI agent working on this project (and CI, and the human) currently cannot see the UI. The app is a Tauri v2 desktop client; its React frontend calls Rust exclusively through Tauri's `invoke()`. Outside the Tauri window there is no `invoke`, so the app renders only its welcome screen and every data-driven view — the commit graph, the sidebar, the commit-detail panel, working status — is unreachable to any automated or unattended verification path.

The concrete, reproduced failure: with the Vite dev server running alone (no Tauri shell), the browser console reports `pageerror: Cannot read properties of undefined (reading 'invoke')`, thrown from `startupPath()` in `src/app/App.tsx` via `src/features/repo/api.ts`. This is not a bug in the app — it is `@tauri-apps/api/core`'s `invoke()` doing `window.__TAURI_INTERNALS__.invoke(...)`, and `__TAURI_INTERNALS__` is `undefined` outside a real Tauri webview, so the property access throws exactly that message. This confirms `window.__TAURI_INTERNALS__` presence/absence is a reliable, already-idiomatic runtime discriminator — no new convention needs to be invented for detection.

Two independent viable routes were ruled out by direct experiment before this exploration began:

- **The Claude Chrome extension is not connected** on this machine — any design depending on it is dead on arrival.
- **Screenshotting the native macOS window is not viable unattended** — an `osascript`/System Events attempt hung waiting on an Accessibility permission prompt, and `screencapture` additionally needs Screen Recording permission. Neither is scriptable without a human present to click through a one-time OS dialog, which defeats the purpose for CI and for an unattended agent.

What **was** validated by direct experiment: Playwright headless (`playwright` + `chromium`) works cleanly against the running Vite dev server (`http://localhost:1420`) and produces a PNG the agent can read, with no macOS permission prompts and no browser extension needed. Chromium headless shell was downloaded to `~/Library/Caches/ms-playwright`; nothing was installed into the project itself, `package.json` is untouched. This is the chosen direction; the question this exploration answers is not "should we use Playwright" but "what exactly should the harness look like so the app renders real data for Playwright to look at."

The sketched direction under pressure-test: a small HTTP bridge binary exposing the same `git-core` operations as `src-tauri`'s Tauri commands, plus a frontend shim that routes `invoke()` over HTTP when not running inside Tauri, so Playwright can drive the real UI against the real Rust core rather than a mock.

## Current State (Codebase Facts)

`src/features/repo/api.ts` is the single chokepoint for every Rust call from the frontend — all 7 backend operations (`startup_path`, `open_repository`, `close_repository`, `list_refs`, `commit_graph`, `commit_detail`, `working_status`) go through it, and nothing else under `src/` imports `invoke` directly. Its own doc comment states this design intent: "The only place that talks to the Rust side... so swapping the transport touches one file." This is a strong existing seam to build the shim on — it was designed for exactly this kind of substitution, whether or not that was the original author's intent.

`src-tauri/src/commands.rs` commands are thin, 1-to-6-line `#[tauri::command]` wrappers. Each either reads process args directly (`startup_path`) or delegates to `RepoRegistry::with(&path, |repo| repo.<method>())` (`src-tauri/src/state.rs`), which lazily opens and mutex-caches a `GitRepo` per path. Every command ultimately calls one method on `GitRepo` (`crates/git-core/src/repo.rs`): `open`, `info`, `refs`, `graph`, `commit_detail`, `status`.

`crates/git-core` is confirmed to have zero UI or transport dependencies — `lib.rs` exposes `pub mod error; pub mod graph; pub mod model; pub mod repo;` and `pub use repo::GitRepo`, nothing Tauri-specific. `GitRepo` in `repo.rs` is a plain synchronous struct wrapping `git2::Repository`, entirely ignorant of Tauri, HTTP, or any transport. This is exactly the seam an HTTP bridge needs: both `src-tauri` and a hypothetical bridge binary are meant to be thin callers of the same `GitRepo` API, never reimplementations of its logic.

`Cargo.toml` (workspace root) declares `git2 = { version = "0.20", default-features = false, features = ["vendored-libgit2"] }` as a workspace dependency — no system libgit2 is required to build anything in this workspace, which matters materially for CI (below).

`package.json` dependencies confirm exactly two Tauri-coupled frontend packages: `@tauri-apps/api` (the `invoke` chokepoint already covered) and `@tauri-apps/plugin-dialog`. No JS/TS test runner exists — `openspec/config.yaml`'s `testing.frontend` block explicitly records `test_runner.command: null`, `framework: "none installed — no vitest, jest, or Playwright present in package.json"`, and layers `unit/integration/e2e` all `available: false`. Frontend verification today is `tsc --noEmit` + `vite build` only.

No `.github/workflows/` directory exists — this is a greenfield CI decision, not a modification of an existing pipeline.

## The Second Tauri-Only Surface: `@tauri-apps/plugin-dialog`

The sketched direction, as framed, treats `invoke()` as the only thing standing between the app and a browser. That is incomplete. Reading `src/app/App.tsx` and `src/features/repo/WelcomeScreen.tsx` shows a second, independent Tauri-only call site: both files import `open as openDialog` from `@tauri-apps/plugin-dialog` and call it directly to show the native "pick a folder" dialog:

```ts
// WelcomeScreen.tsx
import { open as openDialog } from "@tauri-apps/plugin-dialog";
...
const pick = async () => {
  const selected = await openDialog({ directory: true, multiple: false, title: "Open a git repository" });
  if (typeof selected === "string") await open(selected);
};
```

`App.tsx` has an identical `pick()` wired to the header's "Open" button and to the `⌘O` shortcut. This plugin has the same failure mode as `invoke()` outside Tauri — it depends on Tauri's IPC layer and will not work in a bare browser. A shim that only patches `api.ts`'s `invoke()` calls does **not** fix this; clicking "Open repository" in a Playwright-driven Chromium tab would still throw or hang.

The mitigating discovery: the app does not require the dialog to open a repository. `useRepo`'s `open()` action (`src/features/repo/store.ts`) is invoked automatically on load by `App.tsx`'s effect:

```ts
useEffect(() => {
  void startupPath().then((fromCli) => {
    const target = fromCli ?? rememberedRepo();
    if (target) void open(target);
  });
}, [open]);
```

`rememberedRepo()` (`store.ts`) is `localStorage.getItem("gitvisor:last-repo")`. This means a Playwright test can call `page.addInitScript()` (or `context.addCookies`/`localStorage` seeding before navigation) to set `localStorage["gitvisor:last-repo"]` to the fixture repo's absolute path, then navigate — the app will call `openRepository(path)` → `invoke("open_repository", ...)` and populate the whole view **without the test ever touching the dialog picker or the plugin-dialog surface at all**.

This is the recommended scope boundary: the visual-verification-harness change should explicitly descope the dialog-picker interaction (clicking "Open repository", exercising `@tauri-apps/plugin-dialog`) from its initial goal. That interaction is a genuinely different, OS-native UI that Playwright/Chromium cannot render or interact with by construction (no native folder-picker exists in a headless browser context) — it is not a "we'll get to it" gap, it is a hard boundary that a future change would need a completely different approach for (e.g. mocking the plugin at the JS level specifically for that one interaction, accepting it will never be a real end-to-end test of the native dialog).

## Options Considered

### Option A — HTTP bridge binary + runtime shim in `api.ts` (the sketched direction, refined)

A new thin Rust binary imports `git-core` directly and exposes the 7 operations as JSON HTTP endpoints. `api.ts` branches at runtime: if running inside Tauri, call `invoke()`; otherwise, `fetch()` the bridge. Playwright drives the real UI, real React code, real rendering, against real `git-core` behavior via the bridge.

**Pros:**
- Playwright exercises the actual production frontend bundle and actual `git-core` git-reading logic (topological sort, ref badges, diff stats, tracking counts) — not a stand-in. This is the only option that actually answers "did the real app correctly render real git history," which is the stated problem.
- `git-core` is already transport-agnostic (verified above), so the bridge is not fighting the architecture — it is exactly the kind of second thin consumer the crate was designed to support, matching `config.yaml`'s explicit design rule: "Preserve the git-core / src-tauri boundary: domain logic (libgit2, graph layout) stays in git-core; src-tauri only exposes thin commands."
- The existing `api.ts` chokepoint was explicitly designed ("so swapping the transport touches one file") to support exactly this kind of substitution.

**Cons:**
- Introduces a second binary target and a second network/serialization surface that must stay in lockstep with `commands.rs`'s argument shapes, response shapes, and error semantics. Two hand-maintained command surfaces will silently diverge over time without an explicit anti-drift mechanism (addressed below).
- Adds a runtime branch to `api.ts`, a file that ships in every production build, unless the dead branch is build-time-eliminated (tradeoff addressed below).
- Requires a new Cargo workspace member and a new `cargo run -p <bridge>` process to orchestrate in CI/local dev alongside `vite dev`/`vite preview` and Playwright.

**Effort: Medium.**

### Option B — Mock `invoke` entirely at the Playwright/test level (fake `window.__TAURI_INTERNALS__`)

Playwright's `page.addInitScript()` installs a fake `__TAURI_INTERNALS__.invoke` that returns hardcoded or fixture-derived JSON matching each command's response shape. No Rust bridge, no HTTP server, nothing new in the Cargo workspace.

**Pros:**
- Zero new Rust code, zero new binary process to manage, zero risk of the bridge's JSON drifting from `commands.rs`'s JSON, because there is no second surface — the test payload is static or generated once (e.g., dumped from a `cargo test` run) rather than served live.
- Fastest to build and simplest to run — one Playwright process, nothing else to start or orchestrate.
- No risk of the bridge itself having bugs that make screenshots "wrong" for reasons unrelated to the app.

**Cons — this is the fatal one — and why it is rejected:**
- It does not exercise real `git-core` behavior at all. A bug in `graph()`'s topological/lane layout, in `status()`'s diff-based staged/unstaged classification, in ref-badge attachment, or in tracking ahead/behind counts would never surface in a screenshot test, because the "backend" in this design is a static fixture the test author wrote by hand, not the actual libgit2-backed code path. The verification question this option answers is "does React correctly render this JSON I already know is correct," which is a meaningfully weaker guarantee than "does the real app correctly show the real state of a real repository" — and the latter is the actual stated problem (agent/CI/human cannot see the UI, implicitly: cannot see whether the UI correctly reflects real git state). Choosing this option would be quietly redefining the problem to something easier, not solving the stated one.
- Fixture JSON would need to be hand-authored or hand-regenerated any time `git-core`'s model types change shape, with no compiler or runtime check tying it back to the real types — a silent, easy-to-miss source of false-positive-passing tests (the fixture and the real types drift, and no test catches it, because the fixture bypasses the real serialization path).

**Effort: Low.** Rejected as the primary path despite being cheapest, because it weakens the guarantee below what the problem statement requires. It remains a legitimate secondary/incremental technique for *pure UI-state* tests (e.g., "does the error banner render and dismiss correctly") that intentionally don't care about real git data — but it should not be the backbone of this harness.

### Option C — Full native-window E2E via `tauri-driver` / WebdriverIO

Drive the actual compiled Tauri app — real WKWebView on macOS, real webview2/webkit2gtk elsewhere — through the W3C WebDriver protocol instead of Chromium.

**Pros:**
- Closes the fidelity gap completely: this is literally the same rendering engine end users get, so every concern raised in the fidelity section below (drag regions, `-webkit-` quirks, font metrics, native dialogs) disappears, because the test *is* the real app.

**Cons:**
- The official `tauri-driver` binary does not support macOS at all (verified against official documentation, see dedicated section below) — and macOS is this project's primary listed platform (`openspec/config.yaml` context: "macOS/Windows/Linux"). The only ways to get WebDriver-level testing on macOS today are a paid CrabNebula fork of `tauri-driver`, or WebdriverIO's newer embedded-provider support / third-party community projects (e.g. `tauri-webdriver`) — none of which is the vanilla, officially documented, zero-cost path. For an **open-source project** whose constraint is explicitly "contributors must be able to run the harness with the documented commands and no proprietary tooling," a paid fork is disqualifying, and a community/embedded-provider dependency is a materially higher-risk, less-stable foundation to build a first version of this harness on.
- Even setting macOS aside, this is a heavier lift: real window launch (needs a real or virtual display even in "headless" CI configurations, e.g. `xvfb` on Linux runners), slower startup per test, and a fundamentally different tooling stack (WebDriver clients, driver process lifecycle) from the Playwright spike that has already been validated to work today. It does not solve the immediate, stated urgency (agent/CI/human blind to the UI *right now*); it is a heavier, slower-to-land capability layered on top of a problem that Option A already solves for the cases that matter (real data rendering correctness).

**Effort: High.** Rejected for this change; recommended as a documented backlog/follow-up idea, explicitly flagged as macOS-disadvantaged so nobody rediscovers this gap the hard way later.

### Option D — Do nothing / rely on human-driven manual screenshots

**Pros:** zero engineering cost.
**Cons:** does not solve the stated problem — the agent and CI remain blind between now and whenever a human happens to look. Rejected outright; it is not a serious contender given the problem was explicitly framed as needing an automated/unattended answer.

## Recommendation

**Option A** — HTTP bridge binary + a runtime shim in `api.ts` — narrowly scoped as follows, with Option B's technique available as a secondary tool for pure-UI-state assertions, and Option C explicitly deferred to a documented backlog item rather than silently dropped.

Reasoning, restated plainly: Option A is the only choice that satisfies the actual problem (the agent/CI/human need to see whether the *real* app correctly renders *real* git data) without requiring proprietary tooling or a macOS-unsupported dependency chain. It costs more engineering time than Option B, but Option B answers a different, weaker question. It costs less risk and less setup than Option C, and Option C's core blocker (no macOS `tauri-driver`) is a documented, verified fact, not a guess.

## Duplication Analysis: `commands.rs` ↔ HTTP Bridge

The concern: `src-tauri/src/commands.rs` declares 7 thin `#[tauri::command]` wrappers. If an HTTP bridge binary re-declares the same 7 operations by hand, from memory, the two will drift over time — an argument gets renamed in one and not the other, an error case gets added to one and not the other, and nobody notices until a test (or a user) hits the stale surface.

**What is NOT justified:** a shared dispatch enum, a code-generation macro, or a `git-core`-level command-dispatch facade. Building that kind of shared-dispatch infrastructure is proportionate when a command surface is large, changes frequently, or has many call sites — none of which is true here. There are exactly 7 commands, all read-only (no mutation commands exist in `commands.rs` today — `open_repository`, `close_repository`, `list_refs`, `commit_graph`, `commit_detail`, `working_status`, `startup_path` are all reads or lifecycle no-ops), and they are already thin 1-to-6-line pass-throughs to `GitRepo` methods. Introducing a macro or enum to "generate" both surfaces would add a layer of indirection and a new thing contributors have to learn, in exchange for solving a drift problem that has a much cheaper direct solution.

**The concrete structure recommended:**

1. **Both surfaces import the same `git_core::model` types directly** — `RepoInfo`, `RefEntry`, `Graph`, `CommitDetail`, `WorkingStatus` — rather than either surface defining its own parallel request/response structs. Since these types already derive `serde::Serialize`/`Deserialize` (implied by their use across the Tauri IPC boundary, which requires JSON-serializability), the bridge binary can serialize the exact same types `commands.rs` returns, with zero duplication of type definitions. This alone eliminates the most common form of drift (a field renamed or added on one side and not the other) because there is only one definition of each type, in `crates/git-core/src/model.rs`, and the compiler enforces both consumers use it.
2. **The bridge's HTTP handlers are structured as 1:1 mirrors of `commands.rs`'s functions** — same function names minus the `#[tauri::command]` attribute, same argument order, same delegation to `RepoRegistry`-equivalent state (the bridge needs its own lightweight registry, structurally identical to `src-tauri/src/state.rs`'s `RepoRegistry`, since it is also a long-lived process holding open `GitRepo` handles across requests). This is intentional, visible duplication of *wiring*, not of *logic* — the logic lives once, in `GitRepo`.
3. **One symmetry test** — a test (either a `#[test]` in the bridge crate, or a small script run in CI) that opens the same fixture repository through both the Tauri command function directly (calling `commands::commit_graph(...)` etc. in-process, without a running Tauri app) and through an HTTP request to the bridge, and asserts the two JSON payloads are byte-identical (or structurally equal after parsing). This is cheap to write, cheap to run, and directly catches the actual failure mode (silent drift) without requiring speculative shared-dispatch infrastructure. This test should be listed as a required task in the eventual `tasks.md`, not left as an implicit expectation.
4. **A code comment in both `commands.rs` and the new bridge's handler module**, cross-referencing each other by file path, stating explicitly "if you change this operation's shape, update the mirror in `<other file>` and the symmetry test." This is deliberately low-tech — a proposal that reaches for a macro here would be solving imaginary future-scale problems at the cost of real present-day readability for a 7-command open-source project that wants contributors to be able to read the code without learning a bespoke macro DSL.

If the number of commands grows substantially in the future (e.g., past 15-20, or once write/mutation commands are introduced with more complex error handling), revisiting a shared-dispatch structure would become justified — but that is future work, not part of this change.

## Shim Placement and the Production-Bundle Tradeoff

`api.ts` is confirmed as the correct and only place for the shim — every `invoke()` call in the frontend already funnels through it, and its own doc comment states this was the design intent ("Everything else works with the types in `@/shared/types`, so swapping the transport touches one file").

**Detection expression recommended:** a runtime check, `typeof window !== "undefined" && "__TAURI_INTERNALS__" in window` (or equivalently checking `window.__TAURI_INTERNALS__ != null`), evaluated at call time inside each exported function in `api.ts`, or once at module load to select which implementation each export delegates to.

**Explicitly rejected: `import.meta.env.DEV`.** This was one of the options raised for consideration, and it is the wrong tool here. `import.meta.env.DEV` is `false` whenever Vite builds in production mode — including a `vite build` + `vite preview` run, which is what CI should realistically be testing (the actual shipped bundle, not a dev-server-only code path; see the Playwright-target discussion below). A `DEV`-gated shim would silently stop working the moment the harness is pointed at a production build, which is exactly the scenario CI should be validating. A runtime `window.__TAURI_INTERNALS__` check has no such blind spot — it is correct in dev, in preview, and in the real shipped app, unconditionally.

**The dead-code tradeoff, stated plainly and not minimized:** with a runtime check, the HTTP-fetch branch's code ships inside the real production bundle that end users download, even though it will never execute there (real Tauri users always have `__TAURI_INTERNALS__` defined). This is a real cost — bytes shipped to every user for a code path that only ever runs in CI/test contexts. The alternative, build-time elimination via a Vite conditional alias (e.g., aliasing `api.ts` to two separate files, `api.tauri.ts` and `api.http.ts`, selected by a Vite `resolve.alias` keyed on an environment variable set only in the test/CI build), would produce a zero-bytes-in-production result, at the cost of: two files to keep in sync (or one shared file plus two 10-line adapters), a Vite config change, and one more moving part for contributors to understand when reading how the app talks to its backend.

Given the actual size of the affected code (7 tiny wrapper functions, likely well under 1KB minified+gzipped even including a small `fetch`-based HTTP client), the recommendation is to **accept the small dead-code cost** and keep the runtime check. This should be stated as a conscious, explicit tradeoff in the proposal — not an oversight — and revisited only if bundle-size analysis later shows it actually matters, which is unlikely at this scale.

## Fidelity Gap: Chromium (Playwright) vs. WKWebView (real macOS Tauri) — Itemised, Unsoftened

### CAN catch

- **Layout correctness**: flexbox/grid positioning, Tailwind v4 utility class application, responsive behavior, element presence/absence and DOM structure, text content correctness, ARIA/accessibility tree issues.
- **Canvas-drawn structural correctness**: `src/features/graph/drawGraph.ts` renders the commit graph via HTML5 Canvas 2D (`ctx.lineTo`, `ctx.arc`, etc.). Shape positions, lane assignment, edge routing, and fill colors drawn by explicit draw calls will render consistently enough across Chromium and WKWebView to catch *logic* bugs: a commit in the wrong lane, a missing edge, a wrong color assignment, an incorrectly truncated graph.
- **Data-driven correctness end-to-end**: commit ordering, ref badge attachment (branches/tags/remotes shown on the right commits), working-directory status classification (staged/unstaged/conflicted), commit-detail file lists and insertion/deletion counts, error banners, loading spinners, empty states — this is the actual "did the real app render real git history correctly" class of bug the harness exists to catch, and Chromium-via-Option-A catches it fully because the data comes from real `git-core` execution.
- **Interaction correctness**: click handlers (Open/Refresh buttons), keyboard shortcuts (`⌘O`, `⌘R`, both wired in `App.tsx`'s `keydown` listener), selection state transitions when clicking a commit row.
- **The macOS titlebar-inset *logic* (not its appearance)**: `TITLEBAR_INSET` in `App.tsx` is computed from `navigator.userAgent.includes("Mac")`. Chromium headless can be made to report a Mac-like UA string via Playwright's browser-context options, so the 78px-vs-12px branch selection is testable as "does the app pick the right constant for a given UA" — but see below for what this does *not* prove.

### CANNOT catch — explicit, not softened

- **`-webkit-` specific rendering and any Blink/WebKit rendering-engine divergence.** Chromium is a Blink-based engine; WKWebView is genuine Apple WebKit. CSS behaviors, layout edge cases, and any rendering quirk specific to WebKit will not be exercised by a Chromium-only harness. A screenshot passing in this harness is not proof it looks correct in the real macOS app.
- **Font metrics, hinting, and subpixel anti-aliasing.** Text rendering — both DOM text and any text drawn inside the Canvas — differs at the pixel level between Chromium headless-shell and WKWebView, because font rasterization is engine- and OS-pipeline-specific. This means **pixel-diff screenshot comparison is unsafe across the Chromium/WKWebView boundary**, even though it is stable and trustworthy *within* Chromium-only CI runs (i.e., comparing this week's Chromium screenshot to last week's Chromium screenshot is fine; comparing a Chromium screenshot to what a human sees in the real macOS app is not).
- **`data-tauri-drag-region` behavior.** This attribute (`App.tsx`, on the header and two spans inside it) is a native-window-dragging affordance that only has meaning inside an actual OS window frame managed by Tauri. Playwright/Chromium headless has no OS window chrome to drag in the first place — there is no window titlebar for the region to interact with. This is untestable by this harness **by construction**, not by oversight; no amount of harness improvement closes this gap short of Option C.
- **The real macOS traffic-light overlap.** Even though the *branch selection* for `TITLEBAR_INSET` is testable (see above), whether 78px is visually correct against real traffic-light buttons is not — Playwright never renders inside a real macOS window frame, so there are no real traffic lights to be correct or incorrect relative to.
- **The native folder-picker dialog itself** (`@tauri-apps/plugin-dialog`). This is a genuinely different, OS-native UI surface. Playwright cannot render it, cannot interact with it, and — per the recommended scope — this harness is not attempting to. This is a hard boundary, not a partial gap, and the recommendation is to sidestep it entirely via `localStorage` fixture-seeding (see above) rather than pretend it is covered.
- **Tauri IPC/permission-model bugs.** Misconfiguration in `tauri.conf.json`'s capability/ACL system, or any bug specific to Tauri's actual IPC transport, is invisible to this harness by design, because the harness's whole purpose is to bypass that IPC transport with an HTTP bridge. A bug that only manifests through real Tauri IPC (e.g., a permission denied for a command in the real app) would not be caught here.

**Bottom line, to be stated plainly in the proposal and not softened for the sake of a cleaner pitch:** this harness verifies "does the frontend correctly render real git-core output" — which is the actual, stated problem. It does not verify, and must not be presented as verifying, "does this look and feel correct as a native macOS app." Any proposal or PR description that shows a passing screenshot test as evidence the macOS app "looks right" is overselling the guarantee this harness actually provides.

## Deterministic Fixtures

The current state (per the orchestrator's framing) is an ad hoc shell script built in a scratchpad, not committed anywhere, not reproducible across machines: different `git` binary versions have different defaults, the default branch name depends on global `git config`, commit timestamps default to "now" (meaning every run produces a different history unless explicitly overridden), and committer/author identity is read from ambient `user.name`/`user.email` config, which varies per machine and per CI runner. None of this is acceptable for a harness whose entire purpose is producing *stable, comparable* screenshots.

### What must be pinned, explicitly

- **Commit timestamps** (both author and committer time, including UTC offset) — must be fixed, not "now," for every commit in the fixture history. Any timestamp shown in the UI (commit dates, relative-time formatting) will otherwise differ run-to-run, breaking both screenshot stability and any assertions on rendered text.
- **Author and committer identity** (name and email) for every commit — must be hardcoded in the fixture builder, not read from ambient `git config user.name`/`user.email`, which varies by machine/CI runner and would otherwise make fixture-generation non-reproducible and machine-dependent.
- **Branch and tag names** — must be explicit and fixed in the builder (not relying on `git`'s configurable default branch name, e.g. `init.defaultBranch`, which varies by global config and by git version).
- **Commit content/tree state** (file contents, file paths, binary vs. text files) — must be fixed literal content in the builder, since diff stats (insertions/deletions) and file-change lists shown in the commit-detail panel depend on it.
- **Commit graph shape** (which commits are merges, which branches diverge/reconverge) — must be deliberately constructed to exercise the graph-layout algorithm's interesting cases (the same cases the 5 existing `cargo test -p git-core` unit tests on graph layout presumably already cover with synthetic in-memory `Commit` structs, but here needs to exist as an actual on-disk repository for the HTTP bridge/`GitRepo::open` path to read).

### Option 1 — `git2`-based Rust helper

A helper (e.g., a `#[cfg(test)]` module or a small binary target, likely inside `crates/git-core` since `git2` is already a dependency there) that programmatically builds a repository using `git2::Repository::init`, `git2::Signature::new(name, email, git2::Time::new(fixed_epoch_seconds, fixed_offset_minutes))` for every commit, and explicit tree/blob construction via `git2`'s index and commit APIs.

**Pros:** reuses a library already in the workspace (no new dependency), runs entirely in-process (no shelling out to an ambient `git` binary whose version/behavior might differ across contributor machines or CI images), and can be written as ordinary, testable Rust code with the same rigor as the rest of `git-core`.
**Cons:** more verbose to write than a shell script for anyone unfamiliar with `git2`'s lower-level commit-construction API (building trees/blobs by hand rather than via porcelain-level `git add`/`git commit`); the fixture's history "shape" is expressed as imperative Rust code rather than something a reviewer can skim as a diff.

### Option 2 — `git fast-import` stream

Define the exact fixture history as a `git fast-import` stream — a plain-text, deterministic format (`commit`, `author`, `committer`, `M`/file-content directives, `merge` for multi-parent commits) that is itself a static, committable, human-diffable artifact. A one-line `git fast-import < fixture.stream` (or piped from a small generator) rebuilds the exact same history byte-for-byte every time, since the stream format has no ambient-state dependencies (timestamps and identities are literal fields in the stream itself).

**Pros:** the fixture definition is data, not code — reviewable in a PR diff as a readable, mostly-declarative artifact; deterministic by construction since every field that would otherwise vary (time, identity) is an explicit literal in the stream; no `git2` API knowledge required to read or modify it, only familiarity with the fast-import format (well-documented, stable, part of core git).
**Cons:** requires either the `git` CLI to be present in CI/dev (an added implicit dependency, though `git` is essentially guaranteed present in any gitvisor contributor's environment and any git-related CI runner) or a `git2`-based fast-import stream consumer if avoiding the CLI matters; less familiar format to most contributors than plain Rust or plain shell.

**Recommendation for the proposal to decide (not resolved here):** both are legitimate and meaningfully better than the current ad hoc shell script; the proposal phase should pick one explicitly rather than default silently. A slight lean toward `git fast-import` for its reviewability-as-data property, but this is not a strong preference — the `git2` helper has the advantage of staying inside the existing Rust toolchain with zero new format to learn, which may matter more for an open-source contributor base already fluent in the codebase's `git2` usage in `repo.rs`. Either choice must produce the fixture fresh at test-setup time (via a build step or a `cargo run`/script invocation) rather than committing a literal `.git` directory into the repository, which raises its own complications (a nested `.git` inside a `.git`-tracked repo needs careful handling — likely via a `git bundle` file checked in and unbundled at setup time, or fully regenerated every run, rather than a raw `.git/` folder committed directly, which most tooling — including git itself — treats specially and inconsistently).

## Frontend Test Runner: Playwright Only, No Vitest — Reasoning

`openspec/config.yaml` already records the gap explicitly: no vitest/jest/Playwright installed, frontend verification today is `tsc --noEmit` + `vite build` only, and the gap note states this is "recorded, not fixed — no packages were installed by init."

The recommendation is Playwright only for this change, with vitest explicitly out of scope, for the following reasons:

- **The app has very little logic that benefits from isolated unit testing.** State management is a thin zustand store (`store.ts`) whose `open`/`refresh`/`select` actions are thin async wrappers around `api.ts` calls plus `set()` calls — testing these in isolation would mostly mean re-mocking `api.ts` and asserting the store shape changes, which is a weaker signal than an E2E test that proves the same behavior against real data. The genuinely algorithmic code (`graph.rs`'s topological layout) already has 5 unit tests via `cargo test -p git-core`, on the Rust side, where the logic actually lives — this is correctly placed today and doesn't need a JS-side duplicate.
- **Introducing a second test runner in the same change is scope creep relative to the stated problem.** The problem is "cannot see the UI," not "no unit test coverage" — the latter is a separate, already-independently-recorded gap in `config.yaml`, and conflating the two would make this change larger and harder to review without making it more responsive to what was actually asked.
- **If a future need for true unit-level testing emerges** (e.g., testing `layout.ts`'s lane-assignment math in isolation without spinning up a full page), that is better served as its own, separately-scoped proposal that can pick vitest deliberately, evaluate it against the specific units that need coverage, and not be bundled as an afterthought inside a visual-verification change.

This is not a "more is always better" default; it's a direct application of the stated problem's actual scope.

## CI Reachability on GitHub Actions

Confirmed reachable, and — notably — lighter than a full Tauri build, not just "possible":

- **The HTTP bridge (Option A) is a plain Rust binary with zero GUI/webview dependency.** Because the workspace already uses `git2`'s `vendored-libgit2` feature (`Cargo.toml`), building the bridge needs only a C toolchain — which `ubuntu-latest` GitHub-hosted runners ship by default. No `libgit2-dev` (or any system libgit2) apt package is required.
- **Because this harness never builds or launches the real Tauri binary** (`cargo tauri build`/`pnpm app:build`), it does not need any of Tauri's Linux GUI build dependencies — `webkit2gtk-4.1`, `libsoup-3.0`, `javascriptcoregtk`, `libayatana-appindicator3`, `librsvg2`, and the rest of the apt-get list a real `tauri-action`/`cargo tauri build` CI job requires on Linux. This is a meaningful, concrete simplification: the visual-verification CI job's dependency footprint is a strict subset of what a "build the real app" CI job needs, because it deliberately sidesteps the native webview entirely.
- **Playwright needs `npx playwright install --with-deps chromium`** (or the pnpm equivalent) on Linux runners to pull the OS-level shared libraries Chromium needs (`libnss3`, `libatk-bridge2.0-0`, `libxkbcommon0`, etc.) — this is Playwright's own standard, well-documented, actively-maintained mechanism, and works reliably on `ubuntu-latest`.
- **`pnpm install` + `pnpm dev`/`pnpm build && pnpm preview`** are unaffected by any of the above.
- **Net CI shape**: checkout → set up pnpm/Node → set up Rust (via `rustup`, matching the project's existing local setup where Rust is installed at `~/.cargo/bin`, not on default `PATH`) → `cargo build -p <bridge-crate>` → `pnpm install` → `pnpm exec playwright install --with-deps chromium` → start the bridge binary and Vite (dev or preview) in the background → run the Playwright spec suite → tear down. This does not require macOS runner minutes (unlike Option C, which would need `xvfb`/virtual-display handling on Linux or, for true macOS-engine fidelity, an actual macOS runner — GitHub-hosted macOS runners exist but are materially more expensive per minute than Linux runners, another point against Option C for an open-source project's CI budget).

## Verified Fact: `tauri-driver` Does Not Support macOS

The orchestrator's stated belief — that `tauri-driver` supports Linux and Windows but not macOS — was checked directly against the official Tauri v2 documentation this session, not assumed or re-litigated from memory.

**Source checked:** `https://v2.tauri.app/develop/tests/webdriver/` (Tauri v2 official docs, "Tests → WebDriver" page).

**Direct quote retrieved from that page:** *"Driven directly, only Windows and Linux are supported on desktop, as macOS has no WKWebView driver tool available."*

This **confirms** the orchestrator's claim exactly, as stated, with no correction needed. The only ways to get WebDriver-level testing on macOS today, per the same research pass, are (a) CrabNebula's cross-platform fork of `tauri-driver`, which requires a paid API key for macOS use, or (b) WebdriverIO's embedded-WebDriver-server support and/or third-party community projects (e.g., a `tauri-webdriver` project providing a W3C WebDriver implementation specifically for macOS WKWebView apps) — neither of which is the vanilla, zero-cost, officially-documented path, and both represent materially higher adoption risk and setup complexity for an open-source contributor base than what this project's constraints call for. This is the central reason Option C is deferred rather than adopted now.

## Open Questions the Proposal Must Answer

1. Exact bridge-crate name and its position in the Cargo workspace (a full `[workspace.members]` entry, e.g. `crates/http-bridge` or `tools/http-bridge`), and whether it should be picked up by `cargo clippy --workspace --all-targets` (per `openspec/config.yaml`'s verify command) or deliberately excluded from the default workspace-wide lint/build/test sweep to keep it clearly test-tooling-only.
2. Confirmation of the runtime-detection expression in `api.ts` and final sign-off on accepting the small dead-code cost in production bundles versus investing in a build-time Vite alias — the tradeoff is laid out above but the actual decision belongs to the proposal.
3. Whether Playwright should target `vite dev` (faster iteration, but tests dev-mode-only code paths that may differ subtly from what ships) or `vite build && vite preview` (slower, but tests the actual production artifact) for CI — recommend `vite preview` for CI and allow `vite dev` for local iteration, but this needs to be an explicit, stated decision, not an implicit default.
4. Final fixture-format choice: `git2`-based Rust helper vs. `git fast-import` stream — both are viable and meaningfully better than the current ad hoc shell script; the proposal must pick one and justify it, plus decide how the built fixture is materialized for CI/local use (regenerated fresh every run vs. a checked-in `git bundle`).
5. Final confirmation that the `@tauri-apps/plugin-dialog` "Open repository" interaction is explicitly out of scope for this change's initial version, relying on `localStorage`/`rememberedRepo()` seeding to reach the data-driven views instead.
6. The exact symmetry-check mechanism between `commands.rs` and the HTTP bridge — a `#[test]` that calls both in-process and compares JSON, versus a CI-only script — needs to be picked and written into `tasks.md` as a concrete, checkable task, not left as an implicit good intention.
7. Where Playwright specs/config live in the repository tree (e.g. `e2e/` at the root vs. `tests/e2e/`), and explicit acknowledgment that this change **will** require a `package.json` edit at apply time (adding `playwright` as a devDependency) — the exploration-phase constraint of "no installs, no `package.json` edits" was correctly honored during this investigation, but the proposal must not carry that constraint forward as if it still applies once implementation starts.

## Risks

- **Fidelity overselling risk.** If the proposal or any resulting PR presents passing Chromium screenshots as proof the app "looks right on macOS" without the itemised CAN/CANNOT list above, reviewers and future contributors will over-trust a guarantee this harness does not actually provide.
- **Drift risk between `commands.rs` and the HTTP bridge.** Without the symmetry test being an actual, tracked task (not just a mentioned idea), the two command surfaces will silently diverge as either evolves.
- **Production bundle pollution.** The runtime shim in `api.ts` ships to real end users' builds unless explicitly aliased away at build time; low severity given the tiny code size involved, but must be a stated, conscious decision in the proposal, not something discovered later during a bundle-size review.
- **Fixture non-determinism.** If timestamps, author/committer identity, or branch/tag naming are not explicitly pinned in whichever fixture-builder approach is chosen, screenshot comparisons and any text-content assertions become flaky across machines and CI runs, undermining the entire point of the harness.
- **Scope creep risk.** Pressure may arise during implementation to also add vitest "since we're touching testing anyway," or to solve the native dialog-picker flow in the same change; both should be treated as explicitly out of scope per this exploration's reasoning, and any push to include them should go back through a fresh proposal rather than silently expanding this one.
- **Fixture-format decision deferred.** Not a blocker to proceeding to `sdd-propose`, but it must be resolved by the design phase at the latest — the choice affects file layout, the toolchain contributors need locally, and how CI materializes the fixture before each run.
- **CI process orchestration complexity.** Running three concurrent processes in CI (the HTTP bridge, Vite, Playwright) requires reasonably careful startup-ordering/health-check logic (e.g., waiting for the bridge's port and Vite's port to be ready before starting Playwright) to avoid flaky "connection refused" failures; this is a solvable, well-understood CI pattern, but should be explicitly designed rather than assumed to "just work."

---

## Orchestrator Correction (2026-08-18): the macOS blocker on Option C is not what this document says

Everything above is the exploration agent's work, persisted verbatim. This section is added by the orchestrator and corrects one factual finding that materially changes the option set. It is appended rather than edited in place so the original reasoning stays auditable.

### What the document gets right

The quote is accurate. `https://v2.tauri.app/develop/tests/webdriver/` does say: *"only Windows and Linux are supported on desktop, as macOS has no WKWebView driver tool available."*

### What it gets wrong

That sentence describes driving **`tauri-driver` directly**. The same page presents the **WebdriverIO service as the recommended approach**, states it works on **Windows, Linux, and macOS** via an **embedded WebDriver server provider**, and directs macOS users to "use the service's embedded WebDriver server for macOS."

`https://webdriver.io/docs/desktop-testing/tauri` lists three providers — official `tauri-driver`, the CrabNebula driver, and the **embedded plugin, marked recommended**. The embedded route is the ordinary Cargo crate `tauri-plugin-wdio-webdriver = "1"`, registered in Rust. Platform support is listed as Windows (WebView2), **macOS (WKWebView)**, Linux (WebKitGTK).

So the embedded provider is **not proprietary and not undocumented**. The exploration collapsed it together with CrabNebula's paid fork and dismissed both in one clause. That is the load-bearing premise under "Option C: rejected", and it does not hold.

### Why this is decision-critical

Option C, *if it works on this machine*, removes the three costs Option A explicitly accepts:

| Cost Option A accepts | Under Option C |
|---|---|
| A second command surface mirroring `commands.rs` | No bridge exists |
| Drift between the two surfaces, plus a symmetry test to police it | Nothing to drift |
| A transport shim in `api.ts` shipping dead code to every user | No shim; the app under test *is* the app |

It also closes the entire "CANNOT catch" list above — WebKit rendering, font metrics, `data-tauri-drag-region`, traffic-light overlap, real Tauri IPC and the capability/ACL system — because the thing under test is the real binary in the real WKWebView.

Note the shape of this: the option that eliminates the two risks the exploration itself worries about most is the one it rejected on a false premise.

### What is still genuinely unresolved

These are open, and none of them is answered by documentation:

- **Screenshot support through the embedded provider is not documented** on either page. If it cannot produce an image, it cannot give the agent eyes, which is the entire point of this change.
- **`tauri-plugin-wdio-webdriver` is at major version 1.** Maturity, release cadence and breakage risk are unknown.
- **An embedded WebDriver server is a remote-control surface inside the application.** It MUST be feature-gated so it can never appear in a release build. This is a security requirement, not a preference, and the docs carry no warning about it — which makes it more likely, not less, that someone ships it by accident.
- **Cost per run.** A full app compile per test run against a dev-server loop that needs no Rust rebuild for frontend changes.
- **CI shape.** Linux CI would need the webkit2gtk dependency set the document correctly notes Option A avoids.

### Decision rule for the phases that follow

Option A is **validated working today** — a screenshot was produced and read this session. Option C is **documented but unproven here**.

The design phase MUST NOT select Option C on documentation alone, and MUST NOT dismiss it on the rejected premise above. It gets settled by a spike that either produces a screenshot of the real app on macOS or fails to, and the result gets written down either way.

The two options are also not mutually exclusive: A as the fast iteration loop, C as an occasional native fidelity check, is a legitimate outcome the design phase should weigh rather than assume away.

---

## Spike Result (2026-08-18, orchestrator): Option C works on macOS — with evidence

The correction above called for a spike rather than an argument. It was run. Result: **Option C works**, and the option set collapses.

### What was done

`tauri-plugin-wdio-webdriver` 1.3.0 was added to `src-tauri`, registered under `#[cfg(debug_assertions)]`, `wdio-webdriver:default` and `core:window:default` were added to `src-tauri/capabilities/default.json`, and `@wdio/tauri-service` + `@wdio/tauri-plugin` 1.3.0 were installed. A minimal `wdio.conf.ts` pointed at `./target/debug/gitvisor` with no `driverProvider` set.

### Registry verification (not documentation, the registries themselves)

| Package | Registry | Version | Signal |
|---|---|---|---|
| `@wdio/tauri-service` | npm | 1.3.0 | exists |
| `@wdio/tauri-plugin` | npm | 1.3.0 | exists |
| `tauri-plugin-wdio-webdriver` | crates.io | 1.3.0 | 50,050 downloads, updated 2026-08-03 |
| `tauri-plugin-wdio` | crates.io | 1.3.0 | 34,351 downloads, updated 2026-08-03 |

Actively maintained, not abandoned, not proprietary.

### Outcome

```
[webkit 605.1.15 macos #0-0] Running: webkit (v605.1.15) on macos
[webkit 605.1.15 macos #0-0]    ✓ launches the real app and captures a screenshot
1 passing (8.4s)
```

`browser.saveScreenshot()` produced a 2880×1800 PNG of the real application. **Real WKWebView, real binary, screenshots work, 8.4 seconds.**

### What this settles

- **`browser.saveScreenshot()` works** through the embedded provider. This was the decision-critical unknown; it is answered.
- **The security concern is addressed by the official docs**, which register the plugin under `#[cfg(debug_assertions)]`. The exploration's worry was legitimate; the answer already existed.
- **`@wdio/tauri-service` ships a "browser mode"** — "test the Tauri frontend in plain Chrome against a Vite dev server, no Tauri binary or driver required" — plus "Mocking support for Tauri's invoke API". This is a maintained implementation of exactly what Option A proposed to hand-build.

### Consequence for the design phase

The custom HTTP bridge crate and the `api.ts` transport shim are **no longer justified**. Both were designed to approximate, with bespoke code, what this service provides as a maintained feature. Adopting them now would mean owning a Cargo crate, a serialization surface, a drift risk, a symmetry test and production dead code — to get a *worse* result than the native path already demonstrated.

Recommended shape, to be confirmed by design:

- **Native mode** (`@wdio/tauri-service`, embedded provider) — the correctness and fidelity harness. Closes the entire "CANNOT catch" list above.
- **Browser mode** (same service) — the fast iteration loop, no Rust rebuild.
- **No custom bridge. No custom shim. No symmetry test** — there is no second surface to keep in sync.

Everything in this document about deterministic fixtures, pinning timestamps and identity, and CI orchestration **still stands** and is unaffected by the transport decision.

Open items the design phase must still resolve:

- `tauri-plugin-wdio-webdriver` currently sits in `[dependencies]`. The *registration* is `cfg`-gated, but the crate still links into release builds. It should be an optional/dev-gated dependency so release binaries never contain it at all.
- Linux CI needs the webkit2gtk dependency set for native mode. Browser mode does not. Decide which runs on every push and which runs on a narrower trigger.
- Screenshot baseline strategy and where images live.

### The harness paid for itself on its first run

Before any of it was formalised, the first native screenshot exposed a defect that `tsc --noEmit`, `vite build`, `cargo clippy`, `cargo fmt` and all 5 `cargo test -p git-core` tests pass straight through. See `findings.md` in this folder.
