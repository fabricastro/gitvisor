import { splitPath } from "@/shared/format";
import type { ChangeStatus, FileChange } from "@/shared/types";

const STATUS_MARK: Record<ChangeStatus, { label: string; className: string }> = {
  added: { label: "A", className: "bg-added/20 text-added" },
  modified: { label: "M", className: "bg-accent/20 text-accent" },
  deleted: { label: "D", className: "bg-removed/20 text-removed" },
  renamed: { label: "R", className: "bg-tag/20 text-tag" },
  copied: { label: "C", className: "bg-tag/20 text-tag" },
  typeChange: { label: "T", className: "bg-white/10 text-muted" },
  untracked: { label: "U", className: "bg-white/10 text-muted" },
  conflicted: { label: "!", className: "bg-removed/25 text-removed" },
};

interface ChangeRowProps {
  file: FileChange;
  actionLabel: string;
  onAction: () => void;
  disabled?: boolean;
}

/** Presentational: props in, callback out. Rendered for both staged and
 * unstaged lists (design.md §11). */
export function ChangeRow({ file, actionLabel, onAction, disabled }: ChangeRowProps) {
  const { dir, file: name } = splitPath(file.path);
  const mark = STATUS_MARK[file.status];
  return (
    <li className="flex items-center gap-2 px-2.5 py-1 hover:bg-white/[0.04]">
      <span
        className={`grid h-4 w-4 shrink-0 place-items-center rounded text-[10px] font-bold ${mark.className}`}
        title={file.status}
      >
        {mark.label}
      </span>
      <span
        className="min-w-0 flex-1 truncate"
        title={file.oldPath ? `${file.oldPath} → ${file.path}` : file.path}
      >
        <span className="text-faint">{dir}</span>
        <span className="text-fg">{name}</span>
      </span>
      <button
        onClick={onAction}
        disabled={disabled}
        className="shrink-0 rounded border border-border-strong px-1.5 py-0.5 text-[11px] text-muted hover:bg-white/5 hover:text-fg disabled:opacity-40"
      >
        {actionLabel}
      </button>
    </li>
  );
}
