import { open as openDialog } from "@tauri-apps/plugin-dialog";

import { FolderIcon } from "@/shared/ui/Icon";
import { useRepo } from "./store";

export function WelcomeScreen() {
  const open = useRepo((state) => state.open);
  const loading = useRepo((state) => state.loading);

  const pick = async () => {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: "Open a git repository",
    });
    if (typeof selected === "string") await open(selected);
  };

  return (
    <div className="grid flex-1 place-items-center">
      <div className="max-w-sm text-center">
        <h1 className="text-2xl font-semibold text-fg">Gitvisor</h1>
        <p className="mt-2 text-muted">
          Open a repository to see its branches, merges and history laid out
          visually.
        </p>
        <button
          onClick={() => void pick()}
          disabled={loading}
          className="mt-6 inline-flex items-center gap-2 rounded-md bg-accent px-4 py-2 font-medium text-canvas hover:brightness-110 disabled:opacity-50"
        >
          <FolderIcon />
          {loading ? "Opening…" : "Open repository"}
        </button>
      </div>
    </div>
  );
}
