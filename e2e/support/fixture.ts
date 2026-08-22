import { readFileSync } from "node:fs";
import { join } from "node:path";

/**
 * Mirrors `tools/git-fixtures/src/bin/build-fixture.rs`'s `Manifest` /
 * `CommitManifest` structs. The manifest is the single Rust↔TypeScript seam
 * (design.md §2.4) — no OID, branch name, or row count is hardcoded here.
 */
export interface FixtureCommit {
  alias: string;
  oid: string;
  shortId: string;
  summary: string;
  lane: number;
}

export interface FixtureFileChange {
  path: string;
  oldPath: string | null;
  status: string;
  insertions: number;
  deletions: number;
  isBinary: boolean;
}

export interface FixtureWorkingStatus {
  staged: FixtureFileChange[];
  unstaged: FixtureFileChange[];
  conflicted: string[];
}

export interface FixtureManifest {
  name: string;
  path: string;
  headOid: string;
  commitCount: number;
  laneCount: number;
  rowHeight: number;
  /** The commit alias real-graph row 0 belongs to (design.md §5's probe coordinate). */
  row0Alias: string;
  row0Lane: number;
  commits: FixtureCommit[];
  branches: string[];
  remotes: string[];
  tags: string[];
  /** The fixture's own `repo.status()` at build time (design.md §9.4,
   * tasks.md 6.3) — a write spec reads the path to stage from here instead
   * of hardcoding it. */
  initialStatus: FixtureWorkingStatus;
}

const FIXTURE_ROOT = "./target/e2e-fixtures";

/** Read the manifest `build-fixture` wrote. Throws if it hasn't run yet. */
export function readFixture(name = "history"): FixtureManifest {
  // Inside `.git/`, not the working tree — see `build-fixture.rs`'s comment
  // at its manifest-write site: writing it into the working tree would make
  // it its own untracked entry the moment anything re-reads status.
  const manifestPath = join(FIXTURE_ROOT, name, ".git", "fixture.json");
  let raw: string;
  try {
    raw = readFileSync(manifestPath, "utf-8");
  } catch (cause) {
    throw new Error(
      `Fixture manifest not found at ${manifestPath}. Run ` +
        `"cargo run -p git-fixtures --bin build-fixture" first, or let ` +
        `wdio.native.conf.ts's onPrepare do it.`,
      { cause },
    );
  }
  return JSON.parse(raw) as FixtureManifest;
}

/** Look up a commit by its spec alias (e.g. `"m1"`), not a hardcoded OID. */
export function fixtureCommit(manifest: FixtureManifest, alias: string): FixtureCommit {
  const commit = manifest.commits.find((entry) => entry.alias === alias);
  if (!commit) {
    throw new Error(`Fixture "${manifest.name}" has no commit aliased "${alias}"`);
  }
  return commit;
}
