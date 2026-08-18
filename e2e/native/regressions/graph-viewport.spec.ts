import assert from "node:assert/strict";

import { $, $$, browser } from "@wdio/globals";

import { artifactPath } from "../../support/artifacts";
import { readFixture } from "../../support/fixture";

/**
 * Spec B — the real regression test for F1: `viewportHeight` state in
 * `CommitGraph.tsx` starts at 0 and never observably updates from the
 * `ResizeObserver` callback in this harness, so both the DOM row count and
 * the canvas backing store stay pinned to their initial values regardless
 * of the real scroller height.
 *
 * Assertions are truthful and un-inverted (design.md §5, D7) — this file
 * MUST stay red until `fix-graph-viewport` lands. CI wraps only the step
 * Formerly wrapped by `scripts/expect-red.sh` while F1 was open. That guard
 * was removed once this spec went green; it is now an ordinary blocking
 * regression test.
 *
 * Mirrors, not imports, from `src/features/graph/layout.ts` — the harness
 * must not depend on the same product module it is trying to catch a defect
 * in. Values recorded so they can be diffed against a future change to
 * `layout.ts` deliberately, not silently.
 */
const ROW_HEIGHT = 28;
const OVERSCAN = 6;
const GRAPH_PADDING = 14;
const LANE_WIDTH = 15;
const MAX_GRAPH_WIDTH = 260;
const NODE_RADIUS = 4.5;

const graphWidth = (laneCount: number) =>
  Math.min(GRAPH_PADDING * 2 + Math.max(laneCount - 1, 0) * LANE_WIDTH, MAX_GRAPH_WIDTH);
const laneX = (lane: number) => GRAPH_PADDING + lane * LANE_WIDTH;
const rowY = (row: number) => row * ROW_HEIGHT + ROW_HEIGHT / 2;

const scrollerRect = () =>
  browser.execute(() => {
    const element = document.querySelector('[role="listbox"]') as HTMLElement | null;
    return element ? element.getBoundingClientRect().height : null;
  });

const canvasProbe = () =>
  browser.execute(() => {
    const canvas = document.querySelector("canvas") as HTMLCanvasElement | null;
    if (!canvas) return null;
    const context = canvas.getContext("2d");
    return { width: canvas.width, height: canvas.height, hasContext: Boolean(context) };
  });

describe("native regression: graph-viewport (F1)", () => {
  const fixture = readFixture();

  it("renders every row that fits the initial viewport", async () => {
    await $('[role="listbox"]').waitForExist({ timeout: 15000 });
    await browser.setWindowSize(1200, 900);
    // Let the ResizeObserver callback and the resulting render settle.
    await browser.pause(500);

    // Assertion 1 — sanity: the harness reached the graph pane at all. A
    // failure here means a broken harness, never F1.
    const canvasExists = await $("canvas").isExisting();
    assert.ok(canvasExists, "expected the graph canvas element to exist");

    // Assertion 2 — the actual regression. A window this tall, with a
    // 16-commit fixture, should render every commit as a DOM row.
    const rows = await $$('[role="option"]');
    const rowCount = rows.length;
    await browser.saveScreenshot(artifactPath("graph-viewport-failure.png"));
    assert.equal(
      rowCount,
      fixture.commitCount,
      `expected ${fixture.commitCount} rows, got ${rowCount}`,
    );
  });

  it("tracks the viewport across a resize", async () => {
    const before = (await $$('[role="option"]')).length;

    // Shrink well below the space 16 rows would need, so a correct
    // implementation renders fewer rows than before.
    await browser.setWindowSize(1200, 500);
    await browser.pause(500);

    const rect = await scrollerRect();
    assert.ok(rect !== null, "expected the commit-history scroller to exist");
    const height = rect as number;

    // Assertion 3 — expectation derived from the fractional height
    // `ResizeObserver` actually reports (`getBoundingClientRect`), not from
    // rounding `clientHeight`, per design.md §5's precision note.
    //
    // The window is the INCLUSIVE index range [first..last], so the row count
    // is `last - first + 1`. At scrollTop 0, `first` is 0 and `last` is
    // `ceil(height / ROW_HEIGHT) + OVERSCAN`, which makes the count that
    // expression **plus one**. Omitting the `+ 1` computes the last index and
    // calls it a count — an error that stays hidden whenever the commit-count
    // clamp happens to swallow it, which is why the full-height assertion
    // above agrees while this one is off by exactly one row.
    const expectedRows = Math.min(
      fixture.commitCount,
      Math.ceil(height / ROW_HEIGHT) + OVERSCAN + 1,
    );

    const after = (await $$('[role="option"]')).length;
    assert.equal(
      after,
      expectedRows,
      `expected row count to track the resized viewport (~${expectedRows} rows for a ` +
        `${height}px-tall scroller), got ${after} (was ${before} before resizing)`,
    );

    // Assertion 4 — canvas backing-store size, derived from the manifest's
    // lane count and the measured viewport height, not a guessed constant.
    const probe = await canvasProbe();
    assert.ok(probe, "expected the graph canvas element to exist");
    const dpr = await browser.execute(() => window.devicePixelRatio || 1);
    const expectedWidth = Math.round(graphWidth(fixture.laneCount) * dpr);
    const expectedHeight = Math.round(height * dpr);
    assert.equal(
      `${probe!.width}x${probe!.height}`,
      `${expectedWidth}x${expectedHeight}`,
      `expected the canvas backing store to be ${expectedWidth}x${expectedHeight} ` +
        `(laneCount=${fixture.laneCount}, viewport=${height}px, dpr=${dpr}), got ` +
        `${probe!.width}x${probe!.height}`,
    );

    // Assertion 5 — a painted pixel at row 0's node, ±NODE_RADIUS. Sourced
    // from the manifest's row0Lane, never guessed at which commit is on top.
    const x = Math.round(laneX(fixture.row0Lane) * dpr);
    const y = Math.round(rowY(0) * dpr);
    const tolerance = Math.ceil(NODE_RADIUS * dpr);
    const painted = await browser.execute(
      (px: number, py: number, radius: number) => {
        const canvas = document.querySelector("canvas") as HTMLCanvasElement | null;
        const context = canvas?.getContext("2d");
        if (!canvas || !context) return false;
        const left = Math.max(0, px - radius);
        const top = Math.max(0, py - radius);
        const size = radius * 2;
        if (left + size > canvas.width || top + size > canvas.height) return false;
        const { data } = context.getImageData(left, top, size, size);
        for (let i = 3; i < data.length; i += 4) {
          if (data[i] !== 0) return true;
        }
        return false;
      },
      x,
      y,
      tolerance,
    );
    assert.ok(
      painted,
      `expected a painted pixel near (${x}, ${y}) — row 0 / lane ${fixture.row0Lane}'s node`,
    );
  });
});
