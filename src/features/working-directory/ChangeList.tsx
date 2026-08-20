import type { FileChange } from "@/shared/types";
import { ChangeRow } from "./ChangeRow";

interface ChangeListProps {
  title: string;
  files: FileChange[];
  actionLabel: string;
  onAction: (path: string) => void;
  bulkLabel: string;
  onBulkAction: () => void;
  busy: boolean;
}

/** Presentational: props in, callbacks out. Rendered once for staged, once
 * for unstaged (design.md §11). */
export function ChangeList({
  title,
  files,
  actionLabel,
  onAction,
  bulkLabel,
  onBulkAction,
  busy,
}: ChangeListProps) {
  return (
    <section aria-label={title}>
      <div className="flex items-center gap-2 border-b border-t border-border px-2.5 py-1.5 text-[11px] font-medium tracking-wide text-faint uppercase">
        <span className="flex-1">
          {title} ({files.length})
        </span>
        <button
          onClick={onBulkAction}
          disabled={busy || files.length === 0}
          className="rounded border border-border-strong px-1.5 py-0.5 text-[10.5px] normal-case text-muted hover:bg-white/5 hover:text-fg disabled:opacity-40"
        >
          {bulkLabel}
        </button>
      </div>
      {files.length === 0 ? (
        <p className="px-2.5 py-2 text-muted">Nothing here</p>
      ) : (
        <ul>
          {files.map((file) => (
            <ChangeRow
              key={file.path}
              file={file}
              actionLabel={actionLabel}
              onAction={() => onAction(file.path)}
              disabled={busy}
            />
          ))}
        </ul>
      )}
    </section>
  );
}
