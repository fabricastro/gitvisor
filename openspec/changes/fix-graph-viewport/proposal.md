# Proposal: fix-graph-viewport

## Why

The commit graph does not render. Not "renders incorrectly" — the canvas
contains zero non-transparent pixels, and the history list shows 7 rows for a
16-commit repository. This is the application's core feature.

It survived `tsc --noEmit`, `vite build`, `cargo clippy`, `cargo fmt --all
--check` and all Rust tests. It was found by looking at the screen.

Full root-cause analysis and measurements:
`openspec/changes/visual-verification-harness/findings.md` (F1, F2).

## Process note

This change deliberately skips `explore` and `design`. The root cause is already
measured, the fix is a handful of lines, and an acceptance test already exists
and is already failing for the right reason. Running the full ceremony here
would be process theatre, and process nobody believes in is process people
route around.

## Scope

### In scope

| # | Defect | Fix |
|---|---|---|
| F1 | `viewportHeight` frozen at `0`: the canvas never draws and only `OVERSCAN + 1` rows render | Attach the `ResizeObserver` through a **callback ref** so it binds whenever the scroll container mounts, instead of an effect with an empty dependency list that runs while the placeholder is showing |
| F2 | The "When" column wraps and rows visually collide | Make wrapping structurally impossible in a fixed-height row |

### Out of scope — deliberately

- **The locale question.** `Intl.RelativeTimeFormat(undefined, …)` follows the
  OS locale, so an English-only UI shows Spanish dates on a Spanish machine.
  That is a *product* decision, not a defect, and this change fixes the layout
  break without smuggling a product change in behind a bug fix. Recorded in
  `findings.md` F2; it needs its own decision.
- Anything in `openspec/backlog.md`.

## Acceptance

`e2e/native/regressions/graph-viewport.spec.ts` (Spec B) currently fails with:

```
1) expected 16 rows, got 7
2) expected row count to track the resized viewport
   (~13 rows for a 178px-tall scroller), got 7 (was 7 before resizing)
```

This change is done when Spec B passes, Spec A still passes, and a screenshot of
the running application shows the graph drawn.

The second assertion is the one that matters: it resizes the window and requires
the row count to *follow*. It cannot be satisfied by hardcoding a row count,
which is exactly why it was written that way.

## Consequence for the harness change

`scripts/expect-red.sh` exists only to assert Spec B stays red. Once Spec B is
green the guard has no purpose and is removed, and the deferred CI phase
(`visual-verification-harness` Phase 6) must run Spec B as an ordinary blocking
test rather than wrapping it.

## Rollback

Both fixes are confined to two files with no API or data-model change. Reverting
`src/features/graph/CommitGraph.tsx` and `src/features/graph/CommitRow.tsx`
restores the previous behaviour exactly. No migration, no persisted state.
