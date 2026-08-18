import { memo } from "react";

import { fullDate, initials, timeAgo } from "@/shared/format";
import type { GraphRow, RefBadge } from "@/shared/types";
import { ROW_HEIGHT } from "./layout";

interface CommitRowProps {
  row: GraphRow;
  top: number;
  /** Width of the graph gutter the text has to clear. */
  gutter: number;
  selected: boolean;
  onSelect: () => void;
}

/** One line of history. Purely presentational. */
export const CommitRow = memo(function CommitRow({
  row,
  top,
  gutter,
  selected,
  onSelect,
}: CommitRowProps) {
  const { commit } = row;
  return (
    <div
      role="option"
      aria-selected={selected}
      onClick={onSelect}
      style={{ top, height: ROW_HEIGHT, paddingLeft: gutter }}
      className={`absolute inset-x-0 flex items-center gap-2 pr-3 text-[12.5px] ${
        selected ? "bg-accent/15" : "hover:bg-white/[0.04]"
      }`}
    >
      {commit.refs.map((ref) => (
        <Badge key={`${ref.kind}:${ref.name}`} badge={ref} />
      ))}

      <span className="min-w-0 flex-1 truncate" title={commit.summary}>
        {commit.summary || <span className="text-faint">(no message)</span>}
      </span>

      <span
        className="flex items-center gap-1.5 text-muted"
        title={`${commit.author.name} <${commit.author.email}>`}
      >
        <span className="grid h-4 w-4 place-items-center rounded-full bg-border-strong text-[8px] font-semibold text-fg">
          {initials(commit.author.name)}
        </span>
        <span className="w-28 truncate">{commit.author.name}</span>
      </span>

      <span
        className="w-28 shrink-0 truncate text-right text-muted tabular-nums"
        title={fullDate(commit.author.timestamp)}
      >
        {timeAgo(commit.author.timestamp)}
      </span>

      <span className="w-14 shrink-0 text-right font-mono text-faint">
        {commit.shortId}
      </span>
    </div>
  );
});

const BADGE_STYLES: Record<RefBadge["kind"], string> = {
  localBranch: "bg-accent/20 text-accent border-accent/30",
  remoteBranch: "bg-white/5 text-muted border-border-strong",
  tag: "bg-tag/15 text-tag border-tag/30",
};

function Badge({ badge }: { badge: RefBadge }) {
  return (
    <span
      className={`flex max-w-40 shrink-0 items-center gap-1 truncate rounded border px-1.5 py-px text-[11px] leading-4 ${
        BADGE_STYLES[badge.kind]
      }`}
      title={badge.name}
    >
      {badge.isHead && (
        <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-current" aria-label="checked out" />
      )}
      <span className="truncate">{badge.name}</span>
    </span>
  );
}
