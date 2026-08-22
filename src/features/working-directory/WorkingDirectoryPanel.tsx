import { useRepo } from "@/features/repo/store";
import { ChangeList } from "./ChangeList";
import { CommitBox } from "./CommitBox";
import { RefusalNotice } from "./RefusalNotice";

/**
 * Container. Reads `status`/`staging`/`gitProbe` from the store; owns no
 * markup decisions (design.md §11).
 *
 * Rendered inside the sidebar rather than as its own column. As a fourth
 * column it left roughly 530px for the history pane at 1440px, truncating
 * commit messages mid-word (finding F3).
 */
export function WorkingDirectoryPanel() {
  const status = useRepo((state) => state.status);
  const staging = useRepo((state) => state.staging);
  const stagePaths = useRepo((state) => state.stagePaths);
  const unstagePaths = useRepo((state) => state.unstagePaths);
  const gitProbe = useRepo((state) => state.gitProbe);
  const commitWarning = useRepo((state) => state.commitWarning);
  const createCommit = useRepo((state) => state.createCommit);
  const info = useRepo((state) => state.info);

  const staged = status?.staged ?? [];
  const unstaged = status?.unstaged ?? [];

  return (
    <section
      aria-label="Working directory"
      className="flex shrink-0 flex-col border-b border-border"
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

      <CommitBox
        stagedCount={staged.length}
        gitProbe={gitProbe}
        busy={staging.busy}
        detachedHead={info?.head?.detached ?? false}
        commitWarning={commitWarning}
        onCommit={(message) => void createCommit(message)}
      />
    </section>
  );
}
