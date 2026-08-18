import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

import { useRepo } from "@/features/repo/store";
import { CommitRow } from "./CommitRow";
import { drawGraph, indexLongEdges } from "./drawGraph";
import { graphWidth, OVERSCAN, ROW_HEIGHT } from "./layout";

/**
 * The history pane.
 *
 * Rows are virtualised DOM nodes so text stays selectable and accessible, while
 * the lines behind them are painted on a single canvas overlay. Rendering
 * thousands of commits as DOM elements is what makes naive clients stutter.
 */
export function CommitGraph() {
  const graph = useRepo((state) => state.graph);
  const selectedId = useRepo((state) => state.selectedId);
  const select = useRepo((state) => state.select);

  const scrollRef = useRef<HTMLDivElement | null>(null);
  const observerRef = useRef<ResizeObserver | null>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);

  const rows = graph?.rows ?? [];
  const gutter = graphWidth(graph?.laneCount ?? 1);
  const longEdges = useMemo(() => (graph ? indexLongEdges(graph) : []), [graph]);
  const rowOf = useMemo(() => {
    const index = new Map<string, number>();
    rows.forEach((row, at) => index.set(row.commit.id, at));
    return index;
  }, [rows]);
  const selectedRow = selectedId ? (rowOf.get(selectedId) ?? null) : null;

  // A callback ref, deliberately not an effect. The scroll container only
  // mounts once the graph has loaded, so an effect with an empty dependency
  // list runs while the placeholder is still showing, finds no node, and never
  // retries — leaving the viewport height pinned at zero, which silently blanks
  // the canvas and renders only OVERSCAN + 1 rows. Do not "simplify" this back.
  const attachScroller = useCallback((element: HTMLDivElement | null) => {
    observerRef.current?.disconnect();
    observerRef.current = null;
    scrollRef.current = element;
    if (!element) return;

    // Seed from the mounted node so the first paint is already correct rather
    // than waiting for the observer's first callback.
    setViewportHeight(element.clientHeight);
    const observer = new ResizeObserver(([entry]) =>
      setViewportHeight(entry.contentRect.height),
    );
    observer.observe(element);
    observerRef.current = observer;
  }, []);

  useLayoutEffect(() => {
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context || !graph || viewportHeight === 0) return;

    // Back the canvas with device pixels so the curves are not fuzzy on Retina.
    const ratio = window.devicePixelRatio || 1;
    canvas.width = Math.round(gutter * ratio);
    canvas.height = Math.round(viewportHeight * ratio);
    canvas.style.width = `${gutter}px`;
    canvas.style.height = `${viewportHeight}px`;

    drawGraph(context, {
      graph,
      longEdges,
      scrollTop,
      width: gutter,
      height: viewportHeight,
      devicePixelRatio: ratio,
      selectedRow,
    });
  }, [graph, gutter, longEdges, scrollTop, viewportHeight, selectedRow]);

  // Follow the selection when it moves outside the viewport, which is what
  // makes clicking a branch in the sidebar jump to its tip.
  useEffect(() => {
    const element = scrollRef.current;
    if (!element || selectedRow === null) return;
    const top = selectedRow * ROW_HEIGHT;
    if (top < element.scrollTop) {
      element.scrollTo({ top });
    } else if (top + ROW_HEIGHT > element.scrollTop + element.clientHeight) {
      element.scrollTo({ top: top + ROW_HEIGHT - element.clientHeight });
    }
  }, [selectedRow]);

  if (!graph) return <Placeholder>Loading history…</Placeholder>;
  if (rows.length === 0) {
    return <Placeholder>No commits yet. Make your first one.</Placeholder>;
  }

  const first = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
  const last = Math.min(
    rows.length - 1,
    Math.ceil((scrollTop + viewportHeight) / ROW_HEIGHT) + OVERSCAN,
  );
  const visible = Array.from({ length: last - first + 1 }, (_, offset) => first + offset);

  const move = (delta: number) => {
    if (selectedRow === null) return;
    const next = Math.min(rows.length - 1, Math.max(0, selectedRow + delta));
    if (next !== selectedRow) void select(rows[next].commit.id);
  };

  return (
    <div className="relative flex min-h-0 flex-1 flex-col">
      <header
        style={{ paddingLeft: gutter }}
        className="flex h-7 shrink-0 items-center gap-2 border-b border-border pr-3 text-[11px] font-medium tracking-wide text-faint uppercase"
      >
        <span className="flex-1">Commit</span>
        <span className="w-[136px]">Author</span>
        <span className="w-28 text-right">When</span>
        <span className="w-14 text-right">SHA</span>
      </header>

      <div className="relative min-h-0 flex-1">
        <canvas
          ref={canvasRef}
          className="pointer-events-none absolute top-0 left-0 z-10"
          aria-hidden
        />
        <div
          ref={attachScroller}
          role="listbox"
          aria-label="Commit history"
          tabIndex={0}
          onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
          onKeyDown={(event) => {
            if (event.key === "ArrowDown") {
              event.preventDefault();
              move(1);
            } else if (event.key === "ArrowUp") {
              event.preventDefault();
              move(-1);
            }
          }}
          className="scroll-thin h-full overflow-y-auto outline-none"
        >
          <div style={{ height: rows.length * ROW_HEIGHT }} className="relative">
            {visible.map((at) => (
              <CommitRow
                key={rows[at].commit.id}
                row={rows[at]}
                top={at * ROW_HEIGHT}
                gutter={gutter}
                selected={at === selectedRow}
                onSelect={() => void select(rows[at].commit.id)}
              />
            ))}
          </div>
          {graph.truncated && (
            <p className="border-t border-border px-4 py-2 text-center text-muted">
              History truncated at {rows.length.toLocaleString()} commits.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}

const Placeholder = ({ children }: { children: React.ReactNode }) => (
  <div className="grid flex-1 place-items-center text-muted">{children}</div>
);
