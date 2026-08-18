import { mkdirSync } from "node:fs";
import { join } from "node:path";

/** Where screenshots and other run artifacts land. Already covered by the `target/` `.gitignore` entry. */
export const ARTIFACT_DIR = "./target/e2e-artifacts";

/** Resolve (and ensure) a path under {@link ARTIFACT_DIR} for a named artifact. */
export function artifactPath(name: string): string {
  mkdirSync(ARTIFACT_DIR, { recursive: true });
  return join(ARTIFACT_DIR, name);
}
