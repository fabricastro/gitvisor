import { useMemo, useState } from "react";

import { useRepo } from "@/features/repo/store";
import { BranchIcon, ChevronIcon, CloudIcon, TagIcon } from "@/shared/ui/Icon";
import type { RefEntry } from "@/shared/types";

/** Branches, remotes and tags, grouped the way people actually think of them. */
export function Sidebar() {
  const refs = useRepo((state) => state.refs);
  const status = useRepo((state) => state.status);
  const select = useRepo((state) => state.select);

  const groups = useMemo(() => {
    const local = refs.filter((ref) => ref.kind === "localBranch");
    const tags = refs.filter((ref) => ref.kind === "tag");
    const remotes = new Map<string, RefEntry[]>();
    for (const ref of refs) {
      if (ref.kind !== "remoteBranch") continue;
      const remote = ref.name.split("/")[0];
      const bucket = remotes.get(remote) ?? [];
      bucket.push(ref);
      remotes.set(remote, bucket);
    }
    return { local, tags, remotes };
  }, [refs]);

  const pending =
    (status?.staged.length ?? 0) +
    (status?.unstaged.length ?? 0) +
    (status?.conflicted.length ?? 0);

  return (
    <nav className="scroll-thin flex w-60 shrink-0 flex-col overflow-y-auto border-r border-border bg-surface py-2">
      {pending > 0 && (
        <div className="mx-2 mb-2 rounded border border-border-strong bg-elevated px-2.5 py-2">
          <p className="text-[11px] font-medium tracking-wide text-faint uppercase">
            Working directory
          </p>
          <p className="mt-1 text-muted">
            {status?.staged.length ?? 0} staged · {status?.unstaged.length ?? 0} unstaged
            {(status?.conflicted.length ?? 0) > 0 && (
              <span className="text-removed"> · {status?.conflicted.length} conflicted</span>
            )}
          </p>
        </div>
      )}

      <Section title="Branches" count={groups.local.length} defaultOpen>
        {groups.local.map((ref) => (
          <RefRow key={ref.fullName} entry={ref} onSelect={() => void select(ref.target)}>
            <BranchIcon className={ref.isHead ? "text-accent" : "text-faint"} />
          </RefRow>
        ))}
      </Section>

      {[...groups.remotes].map(([remote, branches]) => (
        <Section key={remote} title={remote} count={branches.length}>
          {branches.map((ref) => (
            <RefRow
              key={ref.fullName}
              entry={ref}
              // The remote prefix is already the section title.
              label={ref.name.slice(remote.length + 1)}
              onSelect={() => void select(ref.target)}
            >
              <CloudIcon className="text-faint" />
            </RefRow>
          ))}
        </Section>
      ))}

      {groups.tags.length > 0 && (
        <Section title="Tags" count={groups.tags.length}>
          {groups.tags.map((ref) => (
            <RefRow key={ref.fullName} entry={ref} onSelect={() => void select(ref.target)}>
              <TagIcon className="text-tag" />
            </RefRow>
          ))}
        </Section>
      )}
    </nav>
  );
}

interface SectionProps {
  title: string;
  count: number;
  defaultOpen?: boolean;
  children: React.ReactNode;
}

function Section({ title, count, defaultOpen = false, children }: SectionProps) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <section className="mb-1">
      <button
        onClick={() => setOpen((was) => !was)}
        className="flex w-full items-center gap-1.5 px-2.5 py-1 text-[11px] font-medium tracking-wide text-faint uppercase hover:text-muted"
      >
        <ChevronIcon className={`transition-transform ${open ? "rotate-90" : ""}`} />
        <span className="flex-1 text-left">{title}</span>
        <span className="tabular-nums">{count}</span>
      </button>
      {open && <div>{children}</div>}
    </section>
  );
}

interface RefRowProps {
  entry: RefEntry;
  label?: string;
  onSelect: () => void;
  children: React.ReactNode;
}

function RefRow({ entry, label, onSelect, children }: RefRowProps) {
  return (
    <button
      onClick={onSelect}
      title={entry.upstream ? `${entry.name} → ${entry.upstream}` : entry.name}
      className={`flex w-full items-center gap-2 py-1 pr-2 pl-6 text-left hover:bg-white/[0.05] ${
        entry.isHead ? "font-medium text-fg" : "text-muted"
      }`}
    >
      {children}
      <span className="min-w-0 flex-1 truncate">{label ?? entry.name}</span>
      {entry.ahead > 0 && (
        <span className="shrink-0 text-[11px] text-added tabular-nums">↑{entry.ahead}</span>
      )}
      {entry.behind > 0 && (
        <span className="shrink-0 text-[11px] text-removed tabular-nums">↓{entry.behind}</span>
      )}
    </button>
  );
}
