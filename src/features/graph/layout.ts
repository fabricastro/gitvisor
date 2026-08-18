/** Geometry shared by the canvas renderer and the DOM rows drawn beside it. */

export const ROW_HEIGHT = 28;
export const LANE_WIDTH = 15;
export const GRAPH_PADDING = 14;
export const NODE_RADIUS = 4.5;

/** Rows kept rendered outside the viewport so scrolling never shows a gap. */
export const OVERSCAN = 6;

/**
 * Edges spanning at most this many rows are found by scanning backwards from
 * the viewport. Anything longer is indexed separately, so a branch spanning
 * thousands of commits costs nothing per frame.
 */
export const SHORT_EDGE_SPAN = 120;

/** Widest graph gutter before lanes start overlapping into the text column. */
const MAX_GRAPH_WIDTH = 260;

export const laneX = (lane: number) => GRAPH_PADDING + lane * LANE_WIDTH;

export const rowY = (row: number) => row * ROW_HEIGHT + ROW_HEIGHT / 2;

export const graphWidth = (laneCount: number) =>
  Math.min(GRAPH_PADDING * 2 + Math.max(laneCount - 1, 0) * LANE_WIDTH, MAX_GRAPH_WIDTH);
