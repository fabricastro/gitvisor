/**
 * Mirror of the `git-core` model. Kept in sync by hand; every field here has a
 * counterpart in `crates/git-core/src/model.rs`.
 */

export type RefKind = "localBranch" | "remoteBranch" | "tag";

export type ChangeStatus =
  | "added"
  | "modified"
  | "deleted"
  | "renamed"
  | "copied"
  | "typeChange"
  | "untracked"
  | "conflicted";

export interface Signature {
  name: string;
  email: string;
  /** Seconds since the Unix epoch. */
  timestamp: number;
  offsetMinutes: number;
}

export interface RefBadge {
  name: string;
  kind: RefKind;
  isHead: boolean;
}

export interface Commit {
  id: string;
  shortId: string;
  summary: string;
  body: string;
  author: Signature;
  committer: Signature;
  parents: string[];
  refs: RefBadge[];
}

export interface Edge {
  fromLane: number;
  toLane: number;
  /** Row index of the parent, or null when it lies outside the walked window. */
  toRow: number | null;
  color: number;
}

export interface GraphRow {
  commit: Commit;
  lane: number;
  color: number;
  edges: Edge[];
}

export interface Graph {
  rows: GraphRow[];
  laneCount: number;
  truncated: boolean;
}

export interface RefEntry {
  name: string;
  fullName: string;
  kind: RefKind;
  target: string;
  isHead: boolean;
  upstream: string | null;
  ahead: number;
  behind: number;
}

export interface HeadInfo {
  name: string;
  oid: string | null;
  detached: boolean;
}

export interface RepoInfo {
  path: string;
  name: string;
  head: HeadInfo | null;
  isEmpty: boolean;
  remotes: string[];
}

export interface FileChange {
  path: string;
  oldPath: string | null;
  status: ChangeStatus;
  insertions: number;
  deletions: number;
  isBinary: boolean;
}

export interface CommitDetail {
  commit: Commit;
  files: FileChange[];
  insertions: number;
  deletions: number;
}

export interface WorkingStatus {
  staged: FileChange[];
  unstaged: FileChange[];
  conflicted: string[];
}

export type SkipReason = "vanished";

export interface SkippedPath {
  path: string;
  reason: SkipReason;
}

export interface WriteOutcome {
  status: WorkingStatus;
  skipped: SkippedPath[];
}
