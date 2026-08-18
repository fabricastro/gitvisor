import { create } from "zustand";

import type {
  CommitDetail,
  Graph,
  RefEntry,
  RepoInfo,
  WorkingStatus,
} from "@/shared/types";
import * as api from "./api";

/** Remembers the last repository so a restart lands straight back in it. */
const LAST_REPO_KEY = "gitvisor:last-repo";

interface RepoState {
  info: RepoInfo | null;
  graph: Graph | null;
  refs: RefEntry[];
  status: WorkingStatus | null;
  selectedId: string | null;
  detail: CommitDetail | null;
  loading: boolean;
  error: string | null;

  open: (path: string) => Promise<void>;
  refresh: () => Promise<void>;
  select: (id: string | null) => Promise<void>;
  dismissError: () => void;
}

const describe = (error: unknown): string =>
  typeof error === "string"
    ? error
    : error instanceof Error
      ? error.message
      : JSON.stringify(error);

export const rememberedRepo = (): string | null =>
  localStorage.getItem(LAST_REPO_KEY);

export const useRepo = create<RepoState>((set, get) => ({
  info: null,
  graph: null,
  refs: [],
  status: null,
  selectedId: null,
  detail: null,
  loading: false,
  error: null,

  open: async (path) => {
    set({ loading: true, error: null });
    try {
      const info = await api.openRepository(path);
      localStorage.setItem(LAST_REPO_KEY, info.path);
      set({
        info,
        graph: null,
        refs: [],
        status: null,
        selectedId: null,
        detail: null,
      });
      await get().refresh();
    } catch (error) {
      localStorage.removeItem(LAST_REPO_KEY);
      set({ error: describe(error), info: null });
    } finally {
      set({ loading: false });
    }
  },

  refresh: async () => {
    const { info, selectedId } = get();
    if (!info) return;
    set({ loading: true });
    try {
      const [graph, refs, status] = await Promise.all([
        api.commitGraph(info.path),
        api.listRefs(info.path),
        api.workingStatus(info.path),
      ]);
      set({ graph, refs, status, error: null });

      // Keep the selection if it survived the refresh, otherwise take the tip.
      const stillThere =
        selectedId && graph.rows.some((row) => row.commit.id === selectedId);
      const next = stillThere ? selectedId : (graph.rows[0]?.commit.id ?? null);
      if (next !== selectedId || !get().detail) await get().select(next);
    } catch (error) {
      set({ error: describe(error) });
    } finally {
      set({ loading: false });
    }
  },

  select: async (id) => {
    const { info } = get();
    set({ selectedId: id });
    if (!info || !id) {
      set({ detail: null });
      return;
    }
    try {
      const detail = await api.commitDetail(info.path, id);
      // Guard against a slow response for a commit the user already left.
      if (get().selectedId === id) set({ detail });
    } catch (error) {
      set({ error: describe(error) });
    }
  },

  dismissError: () => set({ error: null }),
}));
