import { useEffect } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

import { CommitDetailPanel } from "@/features/commit-detail/CommitDetailPanel";
import { CommitGraph } from "@/features/graph/CommitGraph";
import { startupPath } from "@/features/repo/api";
import { rememberedRepo, useRepo } from "@/features/repo/store";
import { WelcomeScreen } from "@/features/repo/WelcomeScreen";
import { Sidebar } from "@/features/sidebar/Sidebar";
import { BranchIcon, FolderIcon, RefreshIcon } from "@/shared/ui/Icon";

/** macOS draws its traffic lights over the content, so the bar needs room. */
const TITLEBAR_INSET = navigator.userAgent.includes("Mac") ? 78 : 12;

export function App() {
  const info = useRepo((state) => state.info);
  const open = useRepo((state) => state.open);
  const refresh = useRepo((state) => state.refresh);
  const loading = useRepo((state) => state.loading);
  const error = useRepo((state) => state.error);
  const dismissError = useRepo((state) => state.dismissError);

  // A path on the command line wins; otherwise reopen whatever was last in
  // view, so launching lands you back at work.
  useEffect(() => {
    void startupPath().then((fromCli) => {
      const target = fromCli ?? rememberedRepo();
      if (target) void open(target);
    });
  }, [open]);

  const pick = async () => {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: "Open a git repository",
    });
    if (typeof selected === "string") await open(selected);
  };

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey)) return;
      if (event.key === "o") {
        event.preventDefault();
        void pick();
      } else if (event.key === "r") {
        event.preventDefault();
        void refresh();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  return (
    <div className="flex h-full flex-col bg-canvas">
      <header
        data-tauri-drag-region
        style={{ paddingLeft: TITLEBAR_INSET }}
        className="flex h-11 shrink-0 items-center gap-3 border-b border-border bg-elevated pr-3"
      >
        <span data-tauri-drag-region className="flex min-w-0 items-center gap-2">
          <span className="truncate font-semibold text-fg">
            {info?.name ?? "Gitvisor"}
          </span>
          {info?.head && (
            <span className="flex shrink-0 items-center gap-1 rounded border border-border-strong px-1.5 py-px text-muted">
              <BranchIcon className="text-faint" />
              {info.head.name}
              {info.head.detached && (
                <span className="text-tag" title="HEAD is detached">
                  detached
                </span>
              )}
            </span>
          )}
        </span>

        <span data-tauri-drag-region className="flex-1" />

        <button
          onClick={() => void refresh()}
          disabled={!info || loading}
          title="Refresh (⌘R)"
          className="rounded p-1.5 text-muted hover:bg-white/5 hover:text-fg disabled:opacity-40"
        >
          <RefreshIcon className={loading ? "animate-spin" : ""} />
        </button>
        <button
          onClick={() => void pick()}
          title="Open repository (⌘O)"
          className="flex items-center gap-1.5 rounded px-2 py-1 text-muted hover:bg-white/5 hover:text-fg"
        >
          <FolderIcon />
          Open
        </button>
      </header>

      {error && (
        <div className="flex items-center gap-3 border-b border-removed/30 bg-removed/10 px-4 py-2 text-removed">
          <span className="flex-1">{error}</span>
          <button onClick={dismissError} className="hover:underline">
            Dismiss
          </button>
        </div>
      )}

      {info ? (
        <main className="flex min-h-0 flex-1">
          <Sidebar />
          <CommitGraph />
          <CommitDetailPanel />
        </main>
      ) : (
        <WelcomeScreen />
      )}
    </div>
  );
}
