import { useRepo } from "@/features/repo/store";
import { ChangeList } from "./ChangeList";
import { RefusalNotice } from "./RefusalNotice";

/**
 * Container. Reads `status`/`staging` from the store; owns no markup
 * decisions (design.md §11). Committing is a later change (2026-08-20
 * delivery split) — this panel only stages and unstages.
 */
export function WorkingDirectoryPanel() {
  const status = useRepo((state) => state.status);
  const staging = useRepo((state) => state.staging);
  const stagePaths = useRepo((state) => state.stagePaths);
  const unstagePaths = useRepo((state) => state.unstagePaths);

  const staged = status?.staged ?? [];
  const unstaged = status?.unstaged ?? [];

  return (
    <aside
      aria-label="Working directory"
      className="scroll-thin flex w-72 shrink-0 flex-col overflow-y-auto border-r border-border bg-surface"
    >
      <div className="border-b border-border px-2.5 py-2">
        <h2 className="text-[11px] font-medium tracking-wide text-faint uppercase">
          Changes
        </h2>
      </div>

      {staging.error && <RefusalNotice error={staging.error} />}

      <ChangeList
        title="Staged"
        files={staged}
        actionLabel="Unstage"
        onAction={(path) => void unstagePaths([path])}
        bulkLabel="Unstage all"
        onBulkAction={() => void unstagePaths(staged.map((file) => file.path))}
        busy={staging.busy}
      />

      <ChangeList
        title="Unstaged"
        files={unstaged}
        actionLabel="Stage"
        onAction={(path) => void stagePaths([path])}
        bulkLabel="Stage all"
        onBulkAction={() => void stagePaths(unstaged.map((file) => file.path))}
        busy={staging.busy}
      />

      {status && status.conflicted.length > 0 && (
        <div className="mx-2.5 my-2 rounded border border-removed/30 bg-removed/10 px-2.5 py-2 text-removed">
          <p className="font-medium">Conflicted</p>
          <p className="mt-0.5 text-[12.5px] text-removed/90">
            {status.conflicted.length} path(s) have unresolved conflicts.
            Resolve them in a terminal.
          </p>
        </div>
      )}
    </aside>
  );
}
