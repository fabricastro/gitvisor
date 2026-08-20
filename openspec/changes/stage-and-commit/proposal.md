# Proposal: stage-and-commit

**Date**: 2026-08-20 · **Input**: `explore.md` (including the orchestrator's measured hook experiment) and `measurements.md` (M1 signing, M2 stale index)

Gitvisor's first write capability: file-level stage, unstage, and commit. The commit step
**shells out to the user's `git` binary**; staging stays pure libgit2. That single decision is
what the rest of this proposal follows from.

---

## 1. Intent

Gitvisor is read-only. `product_scope` puts "stage, unstage, commit" in scope precisely because
those operations refuse rather than destroy when they go wrong — *if* implemented so that they do
not silently diverge from what `git` would have done in the same working tree.

The exploration found three ways they could diverge silently. **All three are now measured, not
reasoned.**

```
Hooks — same repo, same live pre-commit hook (exit 1), same moment:
  libgit2:      COMMITTED 6fd1c9e…  — the rejecting hook did NOT run
  git commit:   exit=1, stderr="HOOK RAN — rejecting"

Signing (M1) — commit.gpgsign=true, committed via git2::Repository::commit():
  gpgsig header present?  NO — silently unsigned

Index freshness (M2) — a Repository held open, as RepoRegistry holds it:
  entries before external add            = 1
  repo.index() after external `git add`  = 1   -> STALE
  after index.read(true)                 = 2
```

A `git2::Repository::commit()` implementation would produce commits the user's own
husky / lint-staged / commitlint tooling had already rejected, and would produce unsigned commits in
organisations that mandate signing — in both cases with no warning, the user finding out from a
rejected push. M2 is worse in kind: it is not a display bug but a **work-destruction** mechanism
(§7), and it is caused by the cache Gitvisor already ships.

Success looks like: a commit made in Gitvisor is indistinguishable from the same commit made in the
user's terminal, or it did not happen and the user was told why — and nothing the user staged in
their terminal is ever silently undone.

## 2. Product-scope check — explicit result

| Check | Result |
|---|---|
| `in_scope` "Write: stage, unstage, commit" | **In scope.** This change is exactly that line. |
| `out_of_scope` (rebase, cherry-pick, force-push, visual conflict resolution, `reset --hard`) | **Not touched, and not opened.** See below. |
| `out_of_scope_ux` ("say so plainly and point at the terminal") | **Applied** as the pattern for every refusal in D2. |

**What this change explicitly does not open the door to** — each is a separate product decision,
not a follow-up task: `commit --amend` (rewrites history), `commit -a`, `--no-verify` (the whole
point of D1 is that hooks run), discarding working-tree changes / `git checkout -- <path>` /
`restore`, `reset --hard`, branch or remote operations, and **hunk-level staging** (deferred by
D7, a granularity call, not a scope violation).

## 3. Scope

### In scope
- `git-core`: `stage(path)`, `unstage(path)`, `commit(message)` on `GitRepo`, plus refusal
  pre-flight checks and a `git`-binary probe.
- `error.rs`: structured refusal variants (D3).
- `src-tauri`: three thin commands + one probe command, following the existing seven.
- Frontend: the staging/commit panel (greenfield — no staging UI exists today) and a
  `refreshStatus()` store action cheaper than the full `refresh()`.
- E2E: dedicated write fixtures (D5), one native write spec, browser-mode specs for every UI state.
- `cargo test` coverage for unborn-branch, detached-HEAD, and the hook-runs proof (D6).

### Out of scope
- Hunk/line-level staging — **deferred fast-follow**, named here rather than silently dropped.
- Amend, discard, revert-file, branch, remote — see §2.
- Bulk "stage all" — decide in specs; not assumed.

## 4. Decisions — the six open questions from `explore.md` §5

### D1 — Commit shells out to `git`; staging does not *(§5 Q1)*

**Position: adopt the exploration's recommendation.** The hook evidence is measured with a positive
control, and no libgit2-side mitigation closes it: detecting hooks and running them ourselves means
reimplementing git's argv/env/temp-index contract per hook type (worse guarantee, more code), and
refusing on hook presence makes the feature useless for the exact audience that installs a git GUI.
M1 removes the last reason to hesitate: shelling out closes the hook gap and the signing gap with
one decision, and neither gap then needs any Gitvisor code at all.

| Aspect | Decision |
|---|---|
| Staging | **libgit2** — `Index::add_path` / index-only unstage. No hook fires on `git add`, so there is nothing to lose. |
| Commit | `git -C <workdir> commit` as a subprocess, message passed via argv/stdin (never a shell string), **never** `--no-verify`, **never** `-a`. |
| Locating `git` | Resolve `git` from `PATH` at invocation time, with an explicit override setting. Never cached across the app's lifetime — installing git while Gitvisor is open must start working. |
| `git` absent | Refuse with `GitUnavailable`. **Never** fall back to a libgit2 commit. Never silently downgrade safe → unsafe. |
| Exit mapping | `0` → success, then read the new HEAD **through libgit2** (do not parse stdout for the OID). Non-zero → `CommitFailed { exit_code, stderr }`, surfaced verbatim, because the hook's own output is the message the user needs. Do not classify hook failures by parsing text. |
| Hanging | Set `GIT_TERMINAL_PROMPT=0`, no editor invocation, and a bounded timeout that kills and reports. A `gpg` pinentry needing a TTY would otherwise block the UI forever (**unverified** — needs a design-phase check on macOS and Linux). |

### D2 — Refusals *(supports §5 Q2)*

Every one of these refuses **before** mutating anything, with a message naming the cause and, where
`out_of_scope_ux` applies, the terminal: conflicted paths present, bare repository, nothing staged,
missing `user.name`/`user.email`, `.git/index.lock` held, `git` unavailable. Detached-HEAD commit is
**allowed** (legal in git) with a plain statement that no branch will move.

### D3 — Structured errors, not string parsing *(§5 Q2)*

`CoreError` gains refusal variants (`GitUnavailable`, `CommitFailed`, `IdentityMissing`,
`ConflictsPresent`, `IndexLocked`, `BareRepository`, `NothingStaged`). The `Serialize` impl changes
from a bare string to `{ code, message, details? }`. `message` stays human-readable so the existing
seven commands' UX is unchanged; `describe()` in `src/features/repo/store.ts` — the single error
chokepoint — is updated to prefer `message` over `JSON.stringify`. The UI branches on `code`, never
on message text. No `SigningRequired` variant: M1 plus D1 makes signing git's job.

### D4 — Index freshness is a correctness requirement, structurally enforced *(§5 Q3)*

**M2 measured this, and it changes the severity.** A `Repository` held open — exactly what
`RepoRegistry` does, deliberately, to keep the object database warm — returned a **stale** index
after a terminal `git add`. `Repository::index()` did not observe it; only `index.read(true)` did.

The failure mode is not a stale display. A write command that takes that stale index, adds its own
path and calls `index.write()` **overwrites the external `git add`**: the user staged a file in
their terminal, and Gitvisor silently unstaged it. That is work destruction, and it is caused by
Gitvisor's own cache, not by anything the user did.

Therefore:

1. `index.read(true)` before **any** index mutation is a **correctness requirement**, not a good
   practice, and not something a reviewer may waive as a micro-optimisation concern.
2. It is **structurally enforced**: a private `with_fresh_index()` helper in `repo.rs` returns an
   already-refreshed `Index` and is the **only** way a write path obtains one. The correct path is
   the only convenient path; a future write method cannot forget the invariant because there is no
   other handle to forget it with.
3. The helper carries a doc comment citing M2, in the style of the existing sort-ordering comment,
   so it cannot be deleted later as "seems unnecessary". A `cargo test` reproduces M2 and asserts
   the external staging survives (D6).

**Still unverified** (`measurements.md`): whether a cached `Repository` observes external `HEAD`
movement. Refs are generally re-read per call, unlike the index — but M2 is precisely why
"generally" is not good enough here. Design must measure it before relying on it.

### D5 — Dedicated fixtures per write spec *(§5 Q4)*

Write specs get their own fixture directories; the shared read-only `history` fixture is **never**
written to. Correction to the exploration: `build-fixture.rs` parameterizes the *out-root*, but the
name is hardcoded (`let name = "history"`), so a small argument change is needed — not zero.
**Open for design**: `wdio.native.conf.ts` passes one fixture path via `appArgs` set once in
`onPrepare`; pointing different specs at different fixtures needs a per-session mechanism
(**unverified** — whether the Tauri service re-spawns the app per spec file).

### D6 — Explicit coverage for the branchy paths *(§5 Q5)*

`cargo test` in `git-core` (seconds, both platforms) covers: unborn-branch first commit,
detached-HEAD commit, every D2 refusal, **M2's stale-index sequence**, and **a regression test
replicating the orchestrator's hook experiment** — a repo with a rejecting `pre-commit` hook plus a
positive control. Together these mean a future revert to `Repository::commit()`, or a new write path
that bypasses `with_fresh_index()`, fails a test instead of shipping.

### D7 — `git` availability gates only the commit step *(§5 Q6)*

A probe command called on repo open disables the commit action with an explanation; stage and
unstage stay enabled, since they never needed `git`. The probe is a UI hint; the authoritative check
is at commit time.

### D8 — Test mode split (native costs minutes, browser costs milliseconds)

| Mode | Covers |
|---|---|
| `cargo test -p git-core` | All correctness: refusals, hooks, unborn/detached, index freshness. |
| Browser (ms, mocked `invoke`) | Every UI state — button enablement, each refusal message by `code`, status refresh after a write. |
| Native (minutes, macOS **and** Linux — WebKitGTK is proven, not assumed) | **One** spec: stage → commit → the new commit appears in the graph, against a dedicated fixture. |

Binaries must be built with `pnpm run e2e:build` (`onPrepare` refuses a plain `cargo build`).
Per finding H2, **no assertion may depend on rendered date text.**

### D9 — Ordering

Any new listing funnels through the existing sorted `status()`. No raw `Index` iteration reaches the
UI, so the `core.ignorecase` platform-ordering bug cannot be reintroduced.

## 5. Capabilities

### New
- `working-directory-writes`: staging, unstaging, committing, refusal semantics, the index-freshness
  invariant, and the `git`-binary dependency.

### Modified
- `e2e-verification-harness`: gains a fixture-isolation requirement for write specs.
  Note: `openspec/specs/` is empty — this capability currently lives only in
  `openspec/changes/visual-verification-harness/specs/`, so the spec phase must decide whether to
  write a delta there or promote it first.

## 6. Affected areas

| Area | Impact | Change |
|---|---|---|
| `crates/git-core/src/repo.rs` | Modified | `stage`/`unstage`/`commit`, `with_fresh_index`, git probe |
| `crates/git-core/src/error.rs` | Modified | Refusal variants; `Serialize` becomes `{code, message}` |
| `crates/git-core/src/model.rs` | Modified | Minimal commit-result type; reuse `WorkingStatus` |
| `src-tauri/src/commands.rs` | Modified | 4 new thin commands |
| `src-tauri/src/state.rs` | Unchanged | `with(&GitRepo)` already supports owned-`Index` mutation |
| `src/features/repo/{api,store}.ts` | Modified | New calls, `refreshStatus()`, `describe()` update |
| `src/features/…` (new component) | New | Staging + commit panel |
| `tools/git-fixtures`, `wdio.native.conf.ts`, `e2e/` | Modified | Named fixtures, write specs |

## 7. Risks

### What could still destroy work

Staging and committing are on the safe side of `product_scope` **only if implemented correctly**.
Three concrete mechanisms would put them on the wrong side:

1. **Stale-index overwrite — measured, not hypothetical (M2).** Gitvisor's `RepoRegistry` holds a
   `Repository` open; its index does not observe a terminal `git add`. A write command that mutates
   that stale index and writes it back silently unstages what the user staged. Small blast radius,
   real work lost, and it needs no user mistake to trigger. **Mitigation: D4's helper — the only
   handle to an index is an already-refreshed one.**
2. **Unstage reaching the working tree.** Implemented as `reset --hard`, `checkout_head(force)` or
   any worktree-touching call, "unstage" would wipe uncommitted edits. Unstage is **index-only** —
   restore the entry from the HEAD tree, or remove it when HEAD is unborn.
3. **Staging more than the user pointed at.** A pathspec interpreted as a glob, or `add_all`, stages
   files the user never selected — a commit that quietly contains someone else's work in progress.
   One literal path per call, no globbing, and reject any path escaping the workdir.

A fourth, non-destructive but trust-destroying: committing content the user's hooks would have
rejected, or unsigned where signing is mandated. That is what D1 exists for.

| Risk | Likelihood | Mitigation |
|---|---|---|
| Stale index overwrites an external `git add` (**measured, M2**) | **High without mitigation** — the cache makes it the default path | D4: refreshed-index helper as the only handle, plus a regression test reproducing M2 |
| Unstage implemented as a worktree reset | Low / catastrophic | Index-only unstage; forbidden-API list in design; a test asserts a dirty worktree survives unstage |
| Staging a broader pathspec than the user selected | Low | One literal path per call; no globbing, no `add_all`; reject paths that escape the workdir |
| `git` missing or hanging on pinentry | Low | D1: refuse loudly; bounded timeout |
| Write specs corrupting the shared read-only fixture | Medium | D5 |
| `.git/index.lock` "helpfully" removed by us | Low / severe | Explicitly forbidden — refuse and name the lock file |
| Structured error change breaks the seven existing commands' UX | Low | `message` field preserved; one chokepoint updated |
| Windows hook detection heuristic | — | **Unverified**, and now moot: D1 delegates hook execution to `git` itself |
| Hunk staging perceived as a missing table-stakes feature | Medium | Named as an explicit fast-follow, not dropped |

## 8. Rollback plan

Required by `rules.proposal`. This change can modify a user's repository, so rollback is stated in
terms of persisted state, not just code.

| Question | Answer |
|---|---|
| How to revert | `git revert` the change's PR slices. No schema, no migration, no persisted app state is introduced (nothing new in `localStorage`). |
| What persists after revert | **Anything the user already did stays done.** A commit is history; a staged file stays staged. Uninstalling a GUI does not un-commit. This is normal git behaviour, and every such state is reachable and undoable with ordinary `git` commands. |
| Reverted mid-flight | Safe by construction. Each command is one synchronous open → `read(true)` → mutate → `write()`; nothing is held across calls. A revert between two commands leaves a valid git state. |
| Subprocess killed mid-commit | `git commit` either moved the ref or did not. Residue is at most `.git/index.lock` or `COMMIT_EDITMSG`. Gitvisor **must not** auto-clean either; it reports and points at the terminal. |
| Partial rollback | The UI panel can be removed independently of the `git-core` methods; the backend is inert without a caller. |

## 9. Success criteria

- [ ] A repo with a rejecting `pre-commit` hook **cannot** be committed to from Gitvisor, and a
      `cargo test` proves it (with a positive control).
- [ ] A repo with `commit.gpgsign=true` produces a **signed** commit (M1's failing case), verified
      with `git log --show-signature`.
- [ ] Every D2 refusal renders a specific message via `code`, never parsed text.
- [ ] M2's exact sequence — external `git add`, then a Gitvisor stage of a different path — leaves
      **both** files staged. Covered by a regression test, not by manual checking.
- [ ] Unstage leaves the working tree byte-identical.
- [ ] Unborn-branch and detached-HEAD commits covered by their own tests.
- [ ] One native write spec green on macOS **and** Linux; no assertion on date text.
- [ ] `cargo clippy --workspace --all-targets && cargo fmt --all --check && pnpm build` clean;
      `crates/git-core` still free of Tauri/React imports.

## 10. Dependencies

- **New**: the user's `git` binary at commit time (`PATH` or override). First runtime dependency
  outside the process. Open-source; no proprietary tooling introduced.
- `visual-verification-harness` (done) — extended, not modified in place.

---

## Orchestrator decisions on the proposal question round (2026-08-20)

The proposal could not ask interactively. Three product calls, answered.

### Q1 — "stage all" / "unstage all": **in scope**, with one constraint

Excluding them is defensible on risk grounds and wrong on product grounds: a
twenty-file change staged one row at a time is a feature people abandon. The
code path is the same one applied to a set, so the risk profile does not change.

**The constraint:** "stage all" operates on **exactly the entries the UI is
currently listing**, not a blind `add_all` glob. What the user sees is what gets
staged. A glob can pull in a build artifact that is untracked and un-ignored,
which the user never saw and did not choose — that is a surprise write, and
surprise writes are what this product boundary exists to prevent.

### Q2 — `git` missing: **shown and disabled, with the reason**

Hiding the commit panel makes the app look broken and teaches the user nothing.
`config.yaml`'s `out_of_scope_ux` already sets the pattern: say so plainly and
point at the terminal. The message must name the actual cause — `git` was not
found on `PATH` — and mention the override, so it is fixable rather than
mysterious.

### Q3 — hook rejection: **show the hook's own stderr, verbatim and attributed**

Yes. The hook's message *is* the actionable content: `subject may not be empty`,
`eslint found 3 errors`, `tests failed`. A Gitvisor-authored summary can only
lose information the user needs.

Present it as clearly quoted output attributed to the hook that produced it, so
nobody mistakes tool output for something the app is saying in its own voice.
Raw is right here; unattributed raw is not.

### One risk this raises for design

The proposal flags, unverified, that a `gpg` pinentry needing a TTY could block
the commit subprocess indefinitely. `GIT_TERMINAL_PROMPT=0` plus a bounded
timeout is specified but **not measured**. A commit that hangs forever with no
feedback is worse UX than a refusal, so design must decide how the timeout
surfaces and must keep the "unverified" label until someone runs it.
