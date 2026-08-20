/**
 * The only place that talks to the Rust side. Everything else works with the
 * types in `@/shared/types`, so swapping the transport touches one file.
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  CommitDetail,
  Graph,
  RefEntry,
  RepoInfo,
  WorkingStatus,
  WriteOutcome,
} from "@/shared/types";

/** Repository passed on the command line, if any. */
export function startupPath(): Promise<string | null> {
  return invoke("startup_path");
}

export function openRepository(path: string): Promise<RepoInfo> {
  return invoke("open_repository", { path });
}

export function closeRepository(path: string): Promise<void> {
  return invoke("close_repository", { path });
}

export function listRefs(path: string): Promise<RefEntry[]> {
  return invoke("list_refs", { path });
}

export function commitGraph(path: string, limit?: number): Promise<Graph> {
  return invoke("commit_graph", { path, limit: limit ?? null });
}

export function commitDetail(path: string, id: string): Promise<CommitDetail> {
  return invoke("commit_detail", { path, id });
}

export function workingStatus(path: string): Promise<WorkingStatus> {
  return invoke("working_status", { path });
}

export function stagePaths(path: string, paths: string[]): Promise<WriteOutcome> {
  return invoke("stage_paths", { path, paths });
}

export function unstagePaths(path: string, paths: string[]): Promise<WriteOutcome> {
  return invoke("unstage_paths", { path, paths });
}
