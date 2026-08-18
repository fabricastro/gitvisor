# Tasks: fix-graph-viewport

## Phase 1: Observe the failure

- [x] 1.1 Run Spec B and confirm it fails with both assertions, so the fix is
      proven against an observed red rather than an assumed one.

## Phase 2: F1 — the frozen viewport height

- [x] 2.1 Replace the empty-dependency `useLayoutEffect` in
      `src/features/graph/CommitGraph.tsx` with a callback ref that attaches the
      `ResizeObserver` when the scroll container mounts and disconnects when it
      unmounts.
- [x] 2.2 Seed `viewportHeight` from `clientHeight` at attach time, so the first
      paint after the graph loads is already correct rather than waiting a frame.
- [x] 2.3 Leave a comment explaining *why* it is a callback ref, so nobody
      "simplifies" it back into an effect.

## Phase 3: F2 — the wrapping date column

- [x] 3.1 Make the "When" cell structurally unable to wrap in
      `src/features/graph/CommitRow.tsx`; widen it and its header to match.
      Do **not** change the locale — that is out of scope per the proposal.

## Phase 4: Verify

- [x] 4.1 Spec B passes.
- [x] 4.2 Spec A still passes.
- [x] 4.3 `pnpm build`, `cargo test --workspace`, `cargo clippy --workspace
      --all-targets`, `cargo fmt --all --check` all clean.
- [x] 4.4 Capture a screenshot of the running app and **look at it**. Green
      assertions are not the same as a drawn graph.
- [x] 4.5 Remove `scripts/expect-red.sh` and note in the harness change that CI
      Phase 6 must run Spec B as an ordinary blocking test.
