# Measurements

Claims the exploration labelled *unverified*, resolved by experiment on
2026-08-20 against `git2` 0.20 with vendored libgit2, on macOS. Each was run in
a throwaway repository built for the purpose.

## M1 — libgit2 ignores `commit.gpgsign`

```
SIGNING: commit.gpgsign=true -> libgit2 commit contains gpgsig header?
         NO — silently unsigned
```

A repository configured with `commit.gpgsign = true`, committed through
`git2::Repository::commit()`, produces a commit object with **no `gpgsig`
header**. No error, no warning. On a repository or organisation that requires
signed commits, Gitvisor would produce commits that fail verification, and the
user would find out from a rejected push or a red badge — not from the tool that
made them.

Confirms `explore.md` §3.2, which reasoned this from the libgit2 API surface
without measuring it.

## M2 — a cached `Repository` returns a stale index after an external `git add`

```
FRESHNESS: entries before external add            = 1
FRESHNESS: repo.index() after external `git add`  = 1 entries -> STALE
FRESHNESS: after index.read(true)                 = 2 entries
```

A `Repository` held open — exactly what `RepoRegistry` does — was asked for its
index after a `git add` run externally in a terminal. It returned the **old**
index. `Repository::index()` did not pick up the change; only an explicit
`index.read(true)` did.

**This is the severe one.** The failure is not a stale display. If a write
command takes that stale index, adds its own path, and calls `index.write()`,
the external `git add` is **overwritten and lost**. The user staged something in
their terminal, Gitvisor silently unstaged it.

That is work destruction — small in blast radius, but real, and precisely the
category `product_scope` draws its boundary around. It is caused by the
`RepoRegistry` cache that exists to keep the object database warm; a fresh
`Repository::open()` per command would not have this shape.

Confirms `explore.md` §3.4 and raises its recommendation from a good practice to
a correctness requirement: **`index.read(true)` before any index mutation is
mandatory, and must be structurally enforced** — a shared helper that returns
an already-refreshed index, not an instruction every future write method has to
remember.

## Still unverified

- Windows hook detection (`explore.md` §3.1) — reasoned, not measured; no
  Windows machine has run this project.
- Whether `HEAD` movements from an external `git commit` are observed by a
  cached `Repository` (refs are generally re-read per call, unlike the index,
  but "generally" is not a measurement).
- The exact `git2::Error` code for a locked index (`explore.md` §3.8).

## M3 — a timed-out signing commit is safely a no-op

Measured 2026-08-20 on macOS, against `git` 2.x, with a stub `gpg.program`
standing in for a pinentry that needs a TTY.

| Scenario | `git commit` result | Commits created | Index |
|---|---|---|---|
| Signer exits non-zero | `exit=128`, stderr `gpg failed to sign the data: … No secret key` | **0** | untouched |
| Signer hangs | hangs indefinitely — `GIT_TERMINAL_PROMPT=0` does **not** prevent it | — | — |
| Hanging signer, `SIGTERM` after 5s | `exit=143` | **0** | untouched, still staged |

Three things follow, and the third is the one the design was worried about.

**`GIT_TERMINAL_PROMPT=0` is not a hang guard.** It suppresses git's own
credential prompting; it has no effect on a `gpg.program` that blocks. A bounded
timeout is therefore required, not merely prudent.

**A failing signer refuses cleanly.** No partial state, an actionable message.
That message should reach the user verbatim, like a hook's.

**A killed commit leaves nothing behind.** `proposal.md` §7 flags "a commit whose
outcome is unknown" as the worst possible state. In this measurement it does not
arise: SIGTERM during signing produced no commit and left staging intact, so the
timeout path is safely reportable as "did not commit" rather than "unknown".

**Scope of the claim — do not over-read it.** One platform, one hang point
(the signer blocks before any object is written). A hang at a different stage
— after the commit object exists but before the ref moves — was **not**
measured and could behave differently. The design should still read HEAD through
libgit2 after a timeout and report what it finds, rather than assuming this
result generalises to every hang.

## M4 — libgit2 refuses path escapes, but only as `GenericError`

Measured 2026-08-20. `Index::add_path` against a repository, with paths that try
to leave the working directory:

| Path | Result |
|---|---|
| `inside.txt` | accepted |
| `../outside.txt` | refused — "repo path `../outside.txt` should not start with `..`" (`GenericError`) |
| `/etc/hosts` | refused — "repo path `/etc/hosts` should be relative" (`GenericError`) |
| `sub/../inside.txt` | refused — but as `NotFound`, because `sub/` does not exist |

The index ended holding only `inside.txt`.

**The safety property is real**: libgit2 will not stage a file outside the
repository. This is a defence Gitvisor inherits rather than has to invent.

**But the error classification is not usable.** Both escape attempts come back
as `GenericError` with the distinction only in the human-readable message.
`spec.md` requires refusals to carry distinct, machine-readable codes and forbids
the UI branching on message text — which this cannot satisfy if the check is left
to libgit2.

**Consequence for design:** validate the path in `git-core` *before* calling
`add_path`, and emit a structured refusal. Not because libgit2's check is
insufficient — it is sufficient — but because a code the UI can branch on cannot
be recovered from a `GenericError` afterwards. Keep libgit2's check as the
backstop; it is defence in depth, and it is the one that would still hold if the
explicit validation were ever removed.

**A nuance worth not over-correcting:** the third row was refused for an
unrelated reason (`sub/` does not exist), not for containing `..`. A `..` that
stays inside the repository is harmless and normalises fine. Reject paths that
*escape*, not paths that merely contain `..` — a naive substring check would
refuse legitimate input.
