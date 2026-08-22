import { create } from "zustand";

import type {
  CommitDetail,
  CommitWarning,
  GitProbe,
  Graph,
  RefEntry,
  RepoInfo,
  WorkingStatus,
} from "@/shared/types";
import * as api from "./api";

/** `git` path override, remembered the same way `rememberedRepo()` remembers
 * the last repository (design.md §4.2) — no new persistence layer. */
const GIT_PATH_KEY = "gitvisor:git-path";

const rememberedGitPath = (): string | undefined =>
  localStorage.getItem(GIT_PATH_KEY) ?? undefined;

/** Remembers the last repository so a restart lands straight back in it. */
const LAST_REPO_KEY = "gitvisor:last-repo";

interface StagingState {
  busy: boolean;
  error: CoreErrorWire | null;
}

interface RepoState {
  info: RepoInfo | null;
  graph: Graph | null;
  refs: RefEntry[];
  status: WorkingStatus | null;
  selectedId: string | null;
  detail: CommitDetail | null;
  loading: boolean;
  error: string | null;
  staging: StagingState;
  gitProbe: GitProbe | null;
  /** Set when a commit succeeded but `git` reported something unusual —
   * never a refusal (design.md §2.5). Cleared on the next commit attempt. */
  commitWarning: CommitWarning | null;

  open: (path: string) => Promise<void>;
  refresh: () => Promise<void>;
  /** Status only — no graph re-walk, no reselect. Cheap after a stage/unstage. */
  refreshStatus: () => Promise<void>;
  select: (id: string | null) => Promise<void>;
  dismissError: () => void;
  stagePaths: (paths: string[]) => Promise<void>;
  unstagePaths: (paths: string[]) => Promise<void>;
  /** Sets the same `staging.busy` flag stage/unstage use, so `refreshStatus()`'s
   * existing "never fire while a write is in flight" guard already covers a
   * commit too (design.md §3.4, §11) — no separate `committing` flag needed. */
  createCommit: (message: string) => Promise<void>;
}

/** The `{ code, message, details? }` shape every `CoreError` now serialises to
 * (design.md §5.2). `message` is always `self.to_string()`, so the three
 * pre-existing variants render byte-identically to before this change. */
export interface CoreErrorWire {
  code: string;
  message: string;
  details?: unknown;
}

const isWire = (error: unknown): error is CoreErrorWire =>
  typeof error === "object" &&
  error !== null &&
  typeof (error as CoreErrorWire).code === "string" &&
  typeof (error as CoreErrorWire).message === "string";

/** Anything the backend rejects with, normalised. Never throws. */
export const asCoreError = (error: unknown): CoreErrorWire =>
  isWire(error) ? error : { code: "unknown", message: describe(error) };

const describe = (error: unknown): string =>
  isWire(error)
    ? error.message
    : typeof error === "string"
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
  staging: { busy: false, error: null },
  gitProbe: null,
  commitWarning: null,

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
        staging: { busy: false, error: null },
        gitProbe: null,
      });
      await get().refresh();
      // A hint, not authoritative (design.md §4.4) — never blocks `refresh()`.
      try {
        const probe = await api.gitProbe(info.path, rememberedGitPath());
        set({ gitProbe: probe });
      } catch {
        // Leave gitProbe null; the commit control renders "unknown"/disabled
        // rather than crash the whole repo open on a probe failure.
      }
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

  refreshStatus: async () => {
    const { info, staging } = get();
    if (!info || staging.busy) return;
    try {
      const status = await api.workingStatus(info.path);
      set({ status });
    } catch (error) {
      set({ error: describe(error) });
    }
  },

  stagePaths: async (paths) => {
    const { info } = get();
    if (!info || paths.length === 0) return;
    set({ staging: { busy: true, error: null } });
    try {
      const outcome = await api.stagePaths(info.path, paths);
      set({ status: outcome.status, staging: { busy: false, error: null } });
    } catch (error) {
      set({ staging: { busy: false, error: asCoreError(error) } });
    }
  },

  unstagePaths: async (paths) => {
    const { info } = get();
    if (!info || paths.length === 0) return;
    set({ staging: { busy: true, error: null } });
    try {
      const outcome = await api.unstagePaths(info.path, paths);
      set({ status: outcome.status, staging: { busy: false, error: null } });
    } catch (error) {
      set({ staging: { busy: false, error: asCoreError(error) } });
    }
  },

  createCommit: async (message) => {
    const { info } = get();
    if (!info || message.trim().length === 0) return;
    set({ staging: { busy: true, error: null }, commitWarning: null });
    try {
      const outcome = await api.createCommit(info.path, message, rememberedGitPath());
      set({
        staging: { busy: false, error: null },
        commitWarning: outcome.warning,
      });
      // The graph really did change — the full refresh (not refreshStatus)
      // re-walks it and re-selects, unlike a stage/unstage (design.md §11).
      await get().refresh();
    } catch (error) {
      set({ staging: { busy: false, error: asCoreError(error) } });
    }
  },
}));
