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
    default:
      return "Could not complete that action";
  }
}

function bodyFor(error: CoreErrorWire): string {
  switch (error.code) {
    case "conflictsPresent":
      return "This repository has unresolved conflicts. Resolve them in a terminal before staging or unstaging.";
    case "bareRepository":
      return "This repository has no working directory, so files cannot be staged or unstaged.";
    case "pathOutsideRepo":
      return "That path is outside the repository and was refused.";
    default:
      return error.message;
  }
}
