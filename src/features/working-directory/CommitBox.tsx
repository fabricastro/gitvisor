import { useEffect, useRef, useState } from "react";

import type { CommitWarning, GitProbe } from "@/shared/types";

interface CommitBoxProps {
  stagedCount: number;
  gitProbe: GitProbe | null;
  busy: boolean;
  detachedHead: boolean;
  commitWarning: CommitWarning | null;
  onCommit: (message: string) => void;
}

/**
 * Presentational: message textarea, commit button, `Committing…` state,
 * refusal block (design.md §11). No "committing as …" line — M5 makes any
 * prospective author computable in-process untrustworthy; identity comes
 * from `git var GIT_AUTHOR_IDENT` only at commit time (design.md §5.1).
 */
export function CommitBox({
  stagedCount,
  gitProbe,
  busy,
  detachedHead,
  commitWarning,
  onCommit,
}: CommitBoxProps) {
  const [message, setMessage] = useState("");
  const [showSlowNote, setShowSlowNote] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Plain frontend timer — no backend plumbing, no Tauri event, no streaming
  // (design.md §3.4).
  useEffect(() => {
    if (busy) {
      timerRef.current = setTimeout(() => setShowSlowNote(true), 10_000);
    } else {
      if (timerRef.current) clearTimeout(timerRef.current);
      setShowSlowNote(false);
    }
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [busy]);

  const gitUnavailable = gitProbe !== null && !gitProbe.available;
  const canCommit = stagedCount > 0 && !busy && !gitUnavailable && message.trim().length > 0;

  const submit = () => {
    if (!canCommit) return;
    onCommit(message);
    setMessage("");
  };

  return (
    <div className="border-t border-border px-2.5 py-2">
      {detachedHead && (
        <p className="mb-1.5 text-[11px] text-faint">
          HEAD is detached — this commit will not move any branch.
        </p>
      )}
      {commitWarning && (
        <div className="mb-1.5 rounded border border-tag/30 bg-tag/10 px-2 py-1.5 text-[11.5px] text-tag">
          {commitWarning.kind === "timedOutButHeadMoved"
            ? "git was stopped after taking too long, but the commit was created. A hook that runs after the commit may not have finished."
            : `git exited with status ${commitWarning.exitCode}, but the commit was created.`}
        </div>
      )}
      <textarea
        value={message}
        onChange={(event) => setMessage(event.target.value)}
        placeholder="Commit message"
        rows={3}
        disabled={busy}
        className="w-full resize-none rounded border border-border-strong bg-black/20 px-2 py-1.5 text-[12.5px] text-fg placeholder:text-faint focus:outline-none disabled:opacity-60"
      />
      <div className="mt-1.5 flex items-center justify-between gap-2">
        <span className="text-[10.5px] text-faint">
          {gitUnavailable
            ? "git was not found — install it, or set the path in Settings"
            : `${stagedCount} staged`}
        </span>
        <button
          onClick={submit}
          disabled={!canCommit}
          className="rounded border border-border-strong bg-white/5 px-2.5 py-1 text-[11.5px] font-medium text-fg hover:bg-white/10 disabled:opacity-40"
        >
          {busy ? "Committing…" : "Commit"}
        </button>
      </div>
      {busy && showSlowNote && (
        <p className="mt-1.5 text-[11px] text-faint">
          Still running — your pre-commit hooks may be working. If a passphrase prompt is
          waiting in a terminal, it may be blocked.
        </p>
      )}
    </div>
  );
}
