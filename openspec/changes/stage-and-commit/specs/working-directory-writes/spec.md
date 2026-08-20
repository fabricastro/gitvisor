# Working Directory Writes Specification

## Purpose

Gitvisor's first write capability: file-level stage, unstage, and commit. A
commit made in Gitvisor MUST be indistinguishable from the same commit made in
the user's terminal, or it MUST NOT happen and the user MUST be told why.
Nothing the user staged outside Gitvisor may ever be silently undone.

## Requirements

### Requirement: Stage a Single Working-Tree Path

The system MUST stage exactly one literal path per call, via the libgit2
index, after refreshing the index from disk. The system MUST NOT interpret
the path as a glob and MUST NOT use an `add_all`-style pathspec.

#### Scenario: A modified file is staged
- GIVEN an unstaged modification to `src/main.rs`
- WHEN the user stages `src/main.rs`
- THEN the file appears in the staged list and nowhere else was touched

### Requirement: Unstage a Single Working-Tree Path

The system MUST unstage exactly one literal path via an index-only operation
(restoring the entry from the `HEAD` tree, or removing it when `HEAD` is
unborn). The system MUST NOT modify the working tree.

#### Scenario: A staged file is unstaged without touching its contents
- GIVEN a file staged with local edits
- WHEN the user unstages it
- THEN the file moves to the unstaged list and its on-disk content is
  byte-identical to before the unstage

### Requirement: Bulk Stage and Unstage Operate Only on Listed Entries

"Stage all" and "unstage all" MUST operate on exactly the set of paths the UI
is currently listing at the moment of the action, never a blind glob or
`add_all` pathspec.

#### Scenario: Stage all stages only what was shown
- GIVEN the UI lists three unstaged paths and an untracked, un-ignored build
  artifact exists on disk that is not in that list
- WHEN the user chooses "stage all"
- THEN exactly the three listed paths are staged and the build artifact
  remains untracked

### Requirement: External Staging Is Never Destroyed

Before mutating the index for any write operation, the system MUST reload the
index from disk so that changes made outside Gitvisor are observed and
preserved. A write MUST NOT overwrite index state it did not itself read.

#### Scenario: A terminal `git add` survives a Gitvisor stage
- GIVEN a file staged in a terminal while Gitvisor is open on the same
  repository
- WHEN the user stages a different file through Gitvisor
- THEN both files are staged afterward

### Requirement: Commit Runs Through the User's `git` Binary

Commit MUST invoke the system `git` binary as a subprocess (`git -C <workdir>
commit`), passing the message via argv or stdin, never as an interpolated
shell string. The system MUST NOT pass `--no-verify` and MUST NOT pass `-a`.
On success the new `HEAD` MUST be read back through libgit2, not parsed from
subprocess output.

#### Scenario: A successful commit is read back via libgit2
- GIVEN staged changes and a clean commit
- WHEN the user commits
- THEN `git` exits `0` and the resulting `HEAD` OID, read through libgit2,
  matches the new commit

### Requirement: Commit Hooks Run and a Rejection Blocks the Commit

A commit attempted on a repository whose `pre-commit` (or other rejecting)
hook exits non-zero MUST NOT produce a commit. The hook's own stderr MUST
reach the user verbatim, presented as output attributed to the hook, not
rewritten or summarized by Gitvisor.

#### Scenario: A rejecting pre-commit hook blocks the commit
- GIVEN a repository with a live `pre-commit` hook that exits 1 and prints
  `HOOK RAN — rejecting`
- WHEN the user attempts to commit staged changes
- THEN no new commit is created, `HEAD` is unchanged, and the UI shows
  `HOOK RAN — rejecting` attributed to the hook

### Requirement: Commit Honours Signing Configuration

A repository configured to require commit signing (e.g. `commit.gpgsign =
true`) MUST produce a signed commit when Gitvisor commits, with no separate
signing logic in Gitvisor.

#### Scenario: A commit is signed when signing is required
- GIVEN a repository with `commit.gpgsign = true` and a working signing setup
- WHEN the user commits staged changes
- THEN the resulting commit carries a `gpgsig` header, verifiable via
  `git log --show-signature`

### Requirement: Commit and Staging Refusals Use Distinct, Machine-Readable Codes

Each of the following MUST be refused before any mutation occurs, each with
its own distinct code, never collapsed into one generic failure: nothing
staged, conflicted paths present, bare repository, missing author identity
(`user.name`/`user.email`), a locked index (`.git/index.lock` present), and
`git` unavailable for the commit step. The UI MUST branch on the refusal
`code`, never on message text.

#### Scenario: Nothing staged
- GIVEN no staged changes
- WHEN the user attempts to commit
- THEN the commit is refused with a distinct "nothing staged" code and no
  subprocess is invoked

#### Scenario: Conflicted paths present
- GIVEN one or more conflicted paths in the index
- WHEN the user attempts to stage, unstage, or commit
- THEN the operation is refused with a distinct "conflicts present" code,
  naming the terminal as where conflicts must be resolved

#### Scenario: Bare repository
- GIVEN a repository opened with no working directory
- WHEN the user attempts to stage, unstage, or commit
- THEN the operation is refused with a distinct "bare repository" code

#### Scenario: Missing author identity
- GIVEN neither `user.name` nor `user.email` is configured
- WHEN the user attempts to commit
- THEN the commit is refused with a distinct "identity missing" code before
  the `git` subprocess is invoked

#### Scenario: Locked index
- GIVEN `.git/index.lock` exists
- WHEN the user attempts to stage, unstage, or commit
- THEN the operation is refused with a distinct "index locked" code naming
  the lock file, and the lock file is never removed automatically

### Requirement: `git` Availability Gates Only the Commit Step

If the `git` binary cannot be resolved, the system MUST refuse commit with a
distinct "git unavailable" code and MUST NOT fall back to a libgit2 commit.
Stage and unstage MUST remain available, shown and enabled, since they never
depend on `git`. The commit control MUST remain visible, shown disabled, with
a message naming the cause and mentioning the override setting.

#### Scenario: `git` is not on `PATH`
- GIVEN `git` cannot be resolved on `PATH` and no override is configured
- WHEN the user opens the write panel
- THEN stage and unstage remain enabled, and the commit control is visible
  but disabled with a message stating `git` was not found and that an
  override can be configured

#### Scenario: Staging still works without `git`
- GIVEN `git` is unavailable
- WHEN the user stages a file
- THEN the file is staged successfully via libgit2

### Requirement: Commit Succeeds on an Unborn Branch

The first commit on a branch with no prior history MUST succeed.

#### Scenario: First commit in a new repository
- GIVEN a repository with staged changes and no existing commits
- WHEN the user commits
- THEN the commit succeeds and becomes the branch's first commit

### Requirement: Commit Succeeds on a Detached HEAD

Committing while `HEAD` is detached MUST succeed, since it is legal in git.
The UI MUST state plainly that no branch will move.

#### Scenario: A commit is made in detached HEAD state
- GIVEN `HEAD` is detached and changes are staged
- WHEN the user commits
- THEN the commit succeeds, `HEAD` moves to the new commit, no branch ref
  moves, and the UI states that no branch moved

### Requirement: Listings Are Deterministically Ordered

Any list of paths this change surfaces (staged, unstaged, conflicted) MUST
come through the existing sorted `status()` path, never a raw, unsorted index
iteration, so ordering does not depend on the filesystem's case sensitivity.

#### Scenario: Staged list order is stable across platforms
- GIVEN the same repository and the same staged paths
- WHEN the staged list is read on a case-sensitive and a case-insensitive
  filesystem
- THEN the reported order is identical on both
