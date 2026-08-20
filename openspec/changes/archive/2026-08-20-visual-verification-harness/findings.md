# Defects found by the harness spike

Found on the first native screenshot, before the harness was formalised. Every
one of these passes `tsc --noEmit`, `vite build`, `cargo clippy`,
`cargo fmt --all --check` and all 5 `cargo test -p git-core` tests.

These are **not** part of the visual-verification-harness change. They are
recorded here as the evidence that motivated it, and should be scoped into their
own change.

## F1 — The commit graph never renders, and only 7 rows are listed (severe)

**File:** `src/features/graph/CommitGraph.tsx`

**Symptom:** The history pane shows commit text but no graph at all — no lanes,
no nodes, no edges. Independently, only 7 rows appear for a 16-commit fixture.

**Root cause — one defect, both symptoms:**

The `ResizeObserver` is installed in a `useLayoutEffect` with an empty
dependency array:

```ts
useLayoutEffect(() => {
  const element = scrollRef.current;
  if (!element) return;          // <- taken on first render
  const observer = new ResizeObserver(...);
  observer.observe(element);
  return () => observer.disconnect();
}, []);                          // <- never runs again
```

On the first render `graph` is still `null`, so the component returns early:

```ts
if (!graph) return <Placeholder>Loading history…</Placeholder>;
```

The JSX carrying `ref={scrollRef}` is therefore never mounted, `scrollRef.current`
is `null` when the effect runs, the effect bails, and `[]` means it is never
retried once the graph arrives. `viewportHeight` stays `0` forever.

Both symptoms follow arithmetically:

- The canvas draw effect is guarded by `viewportHeight === 0` and returns
  without drawing. No graph.
- `last = Math.ceil((0 + 0) / 28) + OVERSCAN` = `0 + 6` = `6`, so rows `0..6`
  render — **exactly 7 rows**, matching the screenshot.

**Measured evidence** (probe run against the real app in WKWebView, not inferred
from reading the source):

```
PROBE_ROWS: 7
PROBE_CANVAS: {"canvas":"PRESENT","width":300,"height":150,"painted":0,"scrollerHeight":828}
```

| Measurement | Value | What it proves |
|---|---|---|
| Rendered rows | 7 | Exactly `OVERSCAN + 1`, matching the arithmetic above |
| Painted canvas pixels | 0 | The canvas has not a single non-transparent pixel |
| Canvas size | 300×150 | The HTML default — the draw effect never ran even once, so `canvas.width/height` were never assigned |
| Scroll container height | **828px** | The DOM element has real height while React state holds `0` |

The last row is the defect in one line: the container is **828px in the DOM**
and `viewportHeight` state is **0**. At 828px roughly 29 rows should render,
not 7.

**Fix direction:** do not early-return before the observed node exists. Either
render the scroll container unconditionally and put the placeholder inside it,
or attach the observer via a callback ref so it fires whenever the node mounts.
A regression test must assert the rendered row count tracks the container
height, not just that "a graph appears".

## F2 — Relative-time column overflows and rows collide (moderate)

**File:** `src/features/graph/CommitRow.tsx`, `src/shared/format.ts`

**Symptom:** In the screenshot the "When" column reads `hace 29 minutos`
wrapped across two lines, overlapping neighbouring rows.

**Root cause:** `Intl.RelativeTimeFormat(undefined, ...)` resolves to the
system locale, so string length is locale-dependent. The column is a fixed
`w-24` and rows are absolutely positioned at a fixed `ROW_HEIGHT` of 28px, so
any string that wraps overflows its row instead of expanding it.

**Note:** the app is currently English-only by convention, but its date
formatting silently follows the OS locale. That inconsistency is worth an
explicit product decision, not just a width fix.

**Fix direction:** the row must not be able to wrap — `truncate`/`whitespace-nowrap`
on the cell, a width that fits the longest expected string, or a shorter
formatter. Whichever is chosen, the fixed row height makes wrapping a layout
break, so wrapping must be structurally impossible rather than merely unlikely.


## H1 — Harness note, not a product defect: `withGlobalTauri`

The WebdriverIO run logs a repeated warning:

```
WARN tauri-service:window: Failed to get window states:
Error: Tauri core.invoke not available after 5s timeout
```

The service's window-state and frontend-log-forwarding features reach for
`window.__TAURI__.core.invoke`, which Tauri v2 only exposes when
`app.withGlobalTauri` is `true` in `tauri.conf.json`. It is not set in this
project.

Tests still pass and screenshots still work without it — the core WebDriver
session is unaffected. But the design phase should decide deliberately whether
to enable `withGlobalTauri` for debug builds only (gaining log capture and
window-state assertions) or to leave it off and accept the warnings. Enabling it
globally would widen the app's exposed surface in release builds, so if it is
enabled it must be debug-scoped like the WebDriver plugin itself.

## H2 — Fixture determinism covers commit OIDs, not rendered output

**Found by:** looking at a passing screenshot, 2026-08-18.

`tools/git-fixtures` pins `EPOCH = 1_700_000_000` (November 2023) and asserts
commit OIDs against hardcoded constants. That makes the *repository*
byte-identical across machines, which is what the design claims and what
`cargo test` verifies.

It does **not** make the *rendered UI* deterministic. The commit list shows
relative time (`Intl.RelativeTimeFormat`) computed against wall-clock now, so
the same fixture renders `hace 2 años` today and `hace 3 años` next year — on
the same machine, from the same commit, with no code change.

**Consequences:**

- Any assertion on rendered date text is time-dependent and will eventually
  fail for a reason unrelated to the code.
- If pixel comparison is ever adopted (the proposal currently rejects it), this
  is an additional reason it cannot work — the baseline decays on a calendar.
- Screenshots kept as evidence in a PR silently stop matching what a reader
  reproduces later.

**Not a defect in the fixture builder.** Determinism was specified and delivered
at the level it was specified — object IDs. This records that the guarantee
stops short of the pixels, so nobody later mistakes "deterministic fixtures" for
"deterministic screenshots".

**Options for whoever picks this up:** inject a fixed clock into the formatter
for E2E runs, render absolute dates when a test flag is set, or keep relative
time and simply never assert on it. The third is what the harness does today,
by accident rather than by decision.
