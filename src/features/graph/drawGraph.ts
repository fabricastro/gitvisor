/**
 * Canvas renderer for the commit graph.
 *
 * Deliberately free of React: it takes a context plus a frame description and
 * draws. Rows are drawn in view coordinates via a single translate, so the
 * caller only has to hand over the current scroll offset.
 */

import type { Edge, Graph } from "@/shared/types";
import { GRAPH_BACKGROUND, laneColor } from "./colors";
import {
  laneX,
  NODE_RADIUS,
  OVERSCAN,
  ROW_HEIGHT,
  rowY,
  SHORT_EDGE_SPAN,
} from "./layout";

/** An edge long enough that it must be tracked outside the viewport scan. */
export interface LongEdge {
  row: number;
  edge: Edge;
}

/**
 * Collect edges spanning more than {@link SHORT_EDGE_SPAN} rows.
 *
 * Without this the renderer would have to scan the whole history every frame to
 * catch a branch line that starts far above the viewport and lands far below.
 */
export function indexLongEdges(graph: Graph): LongEdge[] {
  const long: LongEdge[] = [];
  graph.rows.forEach((graphRow, row) => {
    for (const edge of graphRow.edges) {
      if (edge.toRow !== null && edge.toRow - row > SHORT_EDGE_SPAN) {
        long.push({ row, edge });
      }
    }
  });
  return long;
}

export interface Frame {
  graph: Graph;
  longEdges: LongEdge[];
  scrollTop: number;
  /** CSS pixels. */
  width: number;
  height: number;
  devicePixelRatio: number;
  selectedRow: number | null;
}

export function drawGraph(ctx: CanvasRenderingContext2D, frame: Frame): void {
  const { graph, longEdges, scrollTop, width, height, devicePixelRatio, selectedRow } = frame;

  ctx.setTransform(devicePixelRatio, 0, 0, devicePixelRatio, 0, 0);
  ctx.clearRect(0, 0, width, height);
  ctx.save();
  ctx.translate(0, -scrollTop);
  ctx.lineCap = "round";
  ctx.lineJoin = "round";

  const first = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
  const last = Math.min(
    graph.rows.length - 1,
    Math.ceil((scrollTop + height) / ROW_HEIGHT) + OVERSCAN,
  );

  // Lines first so commit nodes always sit on top of them.
  const scanFrom = Math.max(0, first - SHORT_EDGE_SPAN);
  for (let row = scanFrom; row <= last; row += 1) {
    for (const edge of graph.rows[row].edges) {
      if (edge.toRow !== null && edge.toRow - row > SHORT_EDGE_SPAN) continue;
      drawEdge(ctx, row, edge);
    }
  }
  for (const { row, edge } of longEdges) {
    if (row > last || (edge.toRow !== null && edge.toRow < first)) continue;
    drawEdge(ctx, row, edge);
  }

  for (let row = first; row <= last; row += 1) {
    drawNode(ctx, row, graph, row === selectedRow);
  }

  ctx.restore();
}

function drawEdge(ctx: CanvasRenderingContext2D, row: number, edge: Edge): void {
  const x1 = laneX(edge.fromLane);
  const y1 = rowY(row);

  ctx.lineWidth = 1.75;
  ctx.strokeStyle = laneColor(edge.color);
  ctx.beginPath();

  if (edge.toRow === null) {
    // The parent lies outside the walked history: fade the line out.
    ctx.moveTo(x1, y1);
    ctx.lineTo(x1, y1 + ROW_HEIGHT * 0.9);
    ctx.globalAlpha = 0.3;
    ctx.stroke();
    ctx.globalAlpha = 1;
    return;
  }

  const x2 = laneX(edge.toLane);
  const y2 = rowY(edge.toRow);
  ctx.moveTo(x1, y1);

  if (x1 === x2) {
    ctx.lineTo(x2, y2);
  } else if (x2 > x1) {
    // Branching out: leave the node and swing into the new lane immediately,
    // which is what makes a merge read as "this came from over there".
    const bend = Math.min(y1 + ROW_HEIGHT, y2);
    ctx.bezierCurveTo(x1, (y1 + bend) / 2, x2, (y1 + bend) / 2, x2, bend);
    ctx.lineTo(x2, y2);
  } else {
    // Converging: keep the branch's own lane as long as possible, then curve in
    // just above the parent.
    const bend = Math.max(y2 - ROW_HEIGHT, y1);
    ctx.lineTo(x1, bend);
    ctx.bezierCurveTo(x1, (bend + y2) / 2, x2, (bend + y2) / 2, x2, y2);
  }

  ctx.stroke();
}

function drawNode(
  ctx: CanvasRenderingContext2D,
  row: number,
  graph: Graph,
  selected: boolean,
): void {
  const graphRow = graph.rows[row];
  const x = laneX(graphRow.lane);
  const y = rowY(row);
  const color = laneColor(graphRow.color);
  const isMerge = graphRow.commit.parents.length > 1;
  const isHead = graphRow.commit.refs.some((ref) => ref.isHead);

  if (selected) {
    ctx.beginPath();
    ctx.arc(x, y, NODE_RADIUS + 3.5, 0, Math.PI * 2);
    ctx.fillStyle = "rgba(255,255,255,0.16)";
    ctx.fill();
  }

  ctx.beginPath();
  ctx.arc(x, y, NODE_RADIUS, 0, Math.PI * 2);
  if (isMerge) {
    // Hollow, so a merge reads as a junction rather than another commit.
    ctx.fillStyle = GRAPH_BACKGROUND;
    ctx.fill();
    ctx.lineWidth = 2;
    ctx.strokeStyle = color;
    ctx.stroke();
  } else {
    ctx.fillStyle = color;
    ctx.fill();
  }

  if (isHead) {
    ctx.beginPath();
    ctx.arc(x, y, NODE_RADIUS + 2.5, 0, Math.PI * 2);
    ctx.lineWidth = 1.5;
    ctx.strokeStyle = "#f0f6fc";
    ctx.stroke();
  }
}
