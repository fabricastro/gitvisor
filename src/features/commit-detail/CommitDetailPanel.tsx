import { useRepo } from "@/features/repo/store";
import { fullDate, splitPath, timeAgo } from "@/shared/format";
import type { ChangeStatus, FileChange } from "@/shared/types";

/** Everything about the selected commit: metadata plus what it touched. */
export function CommitDetailPanel() {
  const detail = useRepo((state) => state.detail);
  const select = useRepo((state) => state.select);

  if (!detail) {
    return (
      <aside className="grid w-[380px] shrink-0 place-items-center border-l border-border bg-surface text-muted">
        Select a commit
      </aside>
    );
  }

  const { commit, files, insertions, deletions } = detail;

  return (
    <aside className="scroll-thin flex w-[380px] shrink-0 flex-col overflow-y-auto border-l border-border bg-surface">
      <div className="border-b border-border p-4">
        <h2 className="text-[14px] leading-snug font-semibold text-fg">
          {commit.summary || "(no message)"}
        </h2>
        {commit.body && (
          <pre className="mt-2 font-sans text-[12.5px] whitespace-pre-wrap text-muted">
            {commit.body.trim()}
          </pre>
        )}

        <dl className="mt-4 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 text-muted">
          <dt className="text-faint">Commit</dt>
          <dd className="font-mono break-all text-fg select-text">{commit.id}</dd>

          <dt className="text-faint">Author</dt>
          <dd className="truncate" title={commit.author.email}>
            {commit.author.name}
          </dd>

          <dt className="text-faint">Date</dt>
          <dd title={fullDate(commit.author.timestamp)}>
            {timeAgo(commit.author.timestamp)}
          </dd>

          {commit.parents.length > 0 && (
            <>
              <dt className="text-faint">
                {commit.parents.length > 1 ? "Parents" : "Parent"}
              </dt>
              <dd className="flex flex-wrap gap-2">
                {commit.parents.map((parent) => (
                  <button
                    key={parent}
                    onClick={() => void select(parent)}
                    className="font-mono text-accent hover:underline"
                  >
                    {parent.slice(0, 7)}
                  </button>
                ))}
              </dd>
            </>
          )}
        </dl>
      </div>

      <div className="flex items-center gap-2 border-b border-border px-4 py-2 text-[11px] font-medium tracking-wide text-faint uppercase">
        <span className="flex-1">
          {files.length} {files.length === 1 ? "file" : "files"}
        </span>
        <span className="text-added tabular-nums">+{insertions}</span>
        <span className="text-removed tabular-nums">−{deletions}</span>
      </div>

      <ul>
        {files.map((file) => (
          <FileRow key={file.path} file={file} />
        ))}
      </ul>
    </aside>
  );
}

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

function FileRow({ file }: { file: FileChange }) {
  const { dir, file: name } = splitPath(file.path);
  const mark = STATUS_MARK[file.status];
  return (
    <li className="flex items-center gap-2 px-4 py-1 hover:bg-white/[0.04]">
      <span
        className={`grid h-4 w-4 shrink-0 place-items-center rounded text-[10px] font-bold ${mark.className}`}
        title={file.status}
      >
        {mark.label}
      </span>
      <span className="min-w-0 flex-1 truncate" title={file.oldPath ? `${file.oldPath} → ${file.path}` : file.path}>
        <span className="text-faint">{dir}</span>
        <span className="text-fg">{name}</span>
      </span>
      {file.isBinary ? (
        <span className="shrink-0 text-[11px] text-faint">binary</span>
      ) : (
        <span className="shrink-0 text-[11px] tabular-nums">
          <span className="text-added">+{file.insertions}</span>{" "}
          <span className="text-removed">−{file.deletions}</span>
        </span>
      )}
    </li>
  );
}
