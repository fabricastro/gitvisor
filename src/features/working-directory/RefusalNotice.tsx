import type { CoreErrorWire } from "@/features/repo/store";

/**
 * Switches on `code`, never on `message` text (spec.md, Requirement "Commit
 * and Staging Refusals Use Distinct, Machine-Readable Codes").
 */
export function RefusalNotice({ error }: { error: CoreErrorWire }) {
  return (
    <div className="mx-2 mb-2 rounded border border-removed/30 bg-removed/10 px-2.5 py-2 text-removed">
      <p className="font-medium">{titleFor(error.code)}</p>
      <p className="mt-0.5 text-[12.5px] text-removed/90">{bodyFor(error)}</p>
      {stderrFor(error) && <ProcessOutput stderr={stderrFor(error)!} />}
    </div>
  );
}

/** Tool output attributed to its producer — never mistaken for the app
 * speaking in its own voice (design.md §2.5). */
function ProcessOutput({ stderr }: { stderr: string }) {
  return (
    <div className="mt-1.5 rounded border border-removed/20 bg-black/20 px-2 py-1.5">
      <p className="text-[10.5px] font-medium uppercase tracking-wide text-removed/70">
        Output from git and your hooks
      </p>
      <pre className="mt-1 max-h-40 overflow-auto whitespace-pre-wrap break-words text-[11.5px] text-removed/90">
        {stderr}
      </pre>
    </div>
  );
}

function titleFor(code: string): string {
  switch (code) {
    case "conflictsPresent":
      return "Unresolved conflicts";
    case "bareRepository":
      return "Bare repository";
    case "pathOutsideRepo":
      return "Path outside the repository";
    case "nothingStaged":
      return "Nothing staged";
    case "identityMissing":
      return "No author identity configured";
    case "indexLocked":
      return "Index locked";
    case "gitUnavailable":
      return "git not found";
    case "commitFailed":
      return "Commit failed";
    case "commitTimedOut":
      return "Commit timed out";
    default:
      return "Could not complete that action";
  }
}

function bodyFor(error: CoreErrorWire): string {
  switch (error.code) {
    case "conflictsPresent":
      return "This repository has unresolved conflicts. Resolve them in a terminal before staging, unstaging, or committing.";
    case "bareRepository":
      return "This repository has no working directory, so files cannot be staged, unstaged, or committed.";
    case "pathOutsideRepo":
      return "That path is outside the repository and was refused.";
    case "nothingStaged":
      return "Stage at least one change before committing.";
    case "identityMissing":
      return "Set user.name and user.email (locally or globally) before committing.";
    case "indexLocked": {
      const lockPath = detailString(error, "lockPath");
      return lockPath
        ? `${lockPath} exists — another git process may be running. It was not removed automatically.`
        : "The index is locked by another git process. It was not removed automatically.";
    }
    case "gitUnavailable": {
      const lookedFor = detailString(error, "lookedFor");
      return `git was not found${lookedFor ? ` (looked for ${lookedFor})` : ""}. Staging still works. Install git, or set the path to it in Settings.`;
    }
    case "commitFailed":
      return "git refused the commit. See the output below.";
    case "commitTimedOut":
      return "git did not finish in time and was stopped. No commit was created; your staged files are unchanged.";
    default:
      return error.message;
  }
}

function stderrFor(error: CoreErrorWire): string | null {
  if (error.code !== "commitFailed" && error.code !== "commitTimedOut") return null;
  const stderr = detailString(error, "stderr");
  return stderr && stderr.length > 0 ? stderr : null;
}

function detailString(error: CoreErrorWire, key: string): string | null {
  if (typeof error.details !== "object" || error.details === null) return null;
  const value = (error.details as Record<string, unknown>)[key];
  return typeof value === "string" ? value : null;
}
