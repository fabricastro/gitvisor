#!/usr/bin/env bash
# G2 — the release-artifact scan (design.md §1.3, requirement "Release Safety
# Verification"). Inspects every Mach-O file under a given path for
# tauri-plugin-wdio-webdriver's IPC identifier — never just the main binary,
# since a bundled helper or framework could carry the string too.
#
# Primary probe: embedded strings (`rg -a --binary`) for the literal IPC
# identifier "wdio-webdriver" (Tauri routes plugin IPC as
# `plugin:<name>|<command>`, and `<name>` is a `&'static str` compiled into
# `__TEXT,__cstring`). String literals survive `strip`, which removes the
# symbol table, not the text section.
#
# Corroborating probe: the exported Rust symbol `tauri_plugin_wdio_webdriver`
# via `nm -aU`. Recorded, but NEVER treated as evidence of absence on its
# own: a stripped binary yields no symbols regardless of whether the plugin
# is linked in, so an empty symbol table is uninformative, not clean.
#
# Usage:
#   release-scan.sh <path>
#     Scans a single artifact. Prints "present" or "absent" to stdout,
#     everything else to stderr. Exits 0 when absent, 1 when present — for
#     direct use as a simple pass/fail gate.
#
#   release-scan.sh --positive-control <release-path> <e2e-path>
#     The mandatory two-artifact check (design.md §1.3's 4-outcome table).
#     A scan whose pattern went stale must not read as a silent, permanent
#     pass — this is what proves the scan can still fail.
#
#       release=absent,  e2e=present -> PASS  (exit 0)
#       release=present, e2e=present -> FAIL  (exit 1) — the plugin shipped
#       release=absent,  e2e=absent  -> FAIL  (exit 1) — the scan is broken
#       release=present, e2e=absent  -> FAIL  (exit 1) — inverted/nonsensical
set -euo pipefail

PLUGIN_IDENTIFIER="wdio-webdriver"
PLUGIN_SYMBOL="tauri_plugin_wdio_webdriver"

# Every Mach-O file under $1 (or $1 itself, if it is already a file).
find_macho_files() {
  local root="$1"
  if [[ -f "$root" ]]; then
    echo "$root"
    return 0
  fi
  find "$root" -type f 2>/dev/null | while IFS= read -r file; do
    if file "$file" 2>/dev/null | grep -q "Mach-O"; then
      echo "$file"
    fi
  done
}

# Scans $1 and prints "present" or "absent" on stdout. All logging goes to
# stderr, so callers can safely capture stdout with `$(...)`.
scan() {
  local root="$1"
  local files
  files="$(find_macho_files "$root")"

  if [[ -z "$files" ]]; then
    echo "release-scan.sh: no Mach-O files found under $root" >&2
    echo "absent"
    return 0
  fi

  local string_match="absent"
  local symbol_match="absent"
  while IFS= read -r file; do
    if rg -a --binary -q -- "$PLUGIN_IDENTIFIER" "$file" 2>/dev/null; then
      string_match="present"
      echo "release-scan.sh: string probe — \"$PLUGIN_IDENTIFIER\" found in $file" >&2
    fi
    # Captured into a variable rather than piped straight into `grep -q`:
    # `grep -q` exits as soon as it finds a match, which SIGPIPEs `nm` on a
    # large symbol table. Under `pipefail` that turns a real match into a
    # false "not found" (nm's SIGPIPE exit code becomes the pipeline's
    # status, not grep's). Measured — this is not a hypothetical.
    local symbol_table
    symbol_table="$(nm -aU "$file" 2>/dev/null || true)"
    if grep -q "$PLUGIN_SYMBOL" <<<"$symbol_table"; then
      symbol_match="present"
      echo "release-scan.sh: symbol probe — \"$PLUGIN_SYMBOL\" found in $file (corroborating only)" >&2
    fi
  done <<<"$files"

  if [[ "$symbol_match" == "absent" ]]; then
    echo "release-scan.sh: symbol probe found nothing under $root — uninformative on a stripped binary, never treated as evidence of absence" >&2
  fi

  echo "$string_match"
}

main() {
  if [[ "${1:-}" == "--positive-control" ]]; then
    local release_path="${2:?usage: release-scan.sh --positive-control <release-path> <e2e-path>}"
    local e2e_path="${3:?usage: release-scan.sh --positive-control <release-path> <e2e-path>}"

    local release_result e2e_result
    release_result="$(scan "$release_path")"
    e2e_result="$(scan "$e2e_path")"

    echo "release-scan.sh: release artifact ($release_path) = $release_result"
    echo "release-scan.sh: e2e artifact ($e2e_path) = $e2e_result"

    if [[ "$release_result" == "absent" && "$e2e_result" == "present" ]]; then
      echo "release-scan.sh: PASS — absent from the release artifact, present in the known-positive e2e artifact"
      exit 0
    elif [[ "$release_result" == "present" && "$e2e_result" == "present" ]]; then
      echo "release-scan.sh: FAIL — the plugin shipped in the release artifact"
      exit 1
    elif [[ "$release_result" == "absent" && "$e2e_result" == "absent" ]]; then
      echo "release-scan.sh: FAIL — scan produced no match on a known-positive artifact. The scan itself is broken (stale pattern, wrong path, or similar); fix it before trusting any prior pass."
      exit 1
    else
      echo "release-scan.sh: FAIL — inverted result (release=present, e2e=absent). Nonsensical; treat the scan as broken."
      exit 1
    fi
  fi

  local path="${1:?usage: release-scan.sh <path> | release-scan.sh --positive-control <release-path> <e2e-path>}"
  local result
  result="$(scan "$path")"
  echo "release-scan.sh: $result"
  [[ "$result" == "absent" ]]
}

main "$@"
