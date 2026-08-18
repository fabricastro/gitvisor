#!/usr/bin/env python3
"""Parse every workflow/config file this project ships and fail loudly on
syntax errors.

Why this exists: `openspec/config.yaml` shipped invalid YAML earlier in this
project's history and a human reading the file did not catch it (recorded in
this change's tasks.md). A plain parse — no schema, no linting opinions,
just "does this file parse" — would have caught that immediately. This is
that check, run in CI on every push/PR (Phase 6, task 6.2).

Deliberately dependency-light: only `PyYAML` (a `pip install pyyaml` away,
already a step in ci.yml) beyond the standard library. `.github/workflows/*`
also gets `actionlint`'s schema-aware check as a separate CI step — this
script does not attempt to replace that, only to catch syntax errors in
every YAML/JSON file, workflow or not.
"""

from __future__ import annotations

import glob
import json
import sys

try:
    import yaml
except ImportError:  # pragma: no cover - CI installs this before running
    print("validate-config.py: PyYAML is required (pip install pyyaml)", file=sys.stderr)
    sys.exit(2)

GLOBS = [
    ".github/**/*.yml",
    ".github/**/*.yaml",
    "openspec/config.yaml",
    "package.json",
    "tsconfig*.json",
]


def collect_files() -> list[str]:
    files: set[str] = set()
    for pattern in GLOBS:
        files.update(glob.glob(pattern, recursive=True))
    return sorted(files)


def validate(path: str) -> str | None:
    """Returns an error message, or None if the file parses cleanly."""
    try:
        with open(path, "r", encoding="utf-8") as handle:
            content = handle.read()
    except OSError as error:
        return f"could not read file: {error}"

    if path.endswith((".yml", ".yaml")):
        try:
            # safe_load_all: workflow files are always a single document, but
            # this does not silently ignore a stray second `---` document the
            # way safe_load's "last document wins" behaviour would.
            list(yaml.safe_load_all(content))
        except yaml.YAMLError as error:
            return f"invalid YAML: {error}"
    elif path.endswith(".json"):
        try:
            json.loads(content)
        except json.JSONDecodeError as error:
            return f"invalid JSON: {error}"
    else:
        return f"unrecognised extension (expected .yml/.yaml/.json): {path}"

    return None


def main() -> int:
    files = collect_files()
    if not files:
        print("validate-config.py: no files matched the configured globs — check GLOBS", file=sys.stderr)
        return 1

    failed = False
    for path in files:
        error = validate(path)
        if error is None:
            print(f"validate-config.py: OK   {path}")
        else:
            print(f"validate-config.py: FAIL {path} — {error}", file=sys.stderr)
            failed = True

    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
