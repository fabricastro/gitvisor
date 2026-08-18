# Backlog

Ideas captured but **not scheduled**. Nothing here enters the build queue until
it has its own proposal and, where noted, an amendment to `product_scope` in
`config.yaml`.

Recording an idea is not approving it. This file exists so good ideas survive
without silently becoming work.

**Reviewed 2026-08-18.** All four items below were proposed, triaged and
**deferred**. `product_scope` in `config.yaml` was deliberately left unchanged.
The reason given: the commit graph does not render at all today (defect F1,
measured), and anything built on top of that is decoration over something
broken. The backlog is revisited once F1 is fixed and the graph works.

---

## B1 — MCP server (`gitvisor-mcp`)

Expose the repository through the Model Context Protocol so AI coding agents
(Claude Code, Codex, OpenCode, …) can query real history instead of shelling out
to `git log` and parsing text.

**Scope check:** compatible with `product_scope` as read-only. No amendment needed.

**Why this is the strongest idea in the backlog:**

- Architecturally close to free. `crates/git-core` is already transport-agnostic;
  `src-tauri` is one thin consumer. An MCP server is a second one, exactly the
  pattern the crate was built for. No new domain logic.
- Read-only means it cannot destroy a repository.
- Genuinely differentiated. Desktop git clients do not expose an agent surface.
- Dogfooding with teeth: this session was spent working blind against a
  repository. This is the tool that fixes that class of problem for everyone.

**Open questions:** separate binary sharing `git-core`, or a mode of the app?
Which operations — graph, blame, branch comparison, search? Read-only forever, or
does write access get considered later under its own decision?

**Rough size:** small-to-medium. Mostly protocol plumbing over existing methods.

---

## B2 — AI-assisted commit messages

Generate a proposed commit message from the staged diff. Human edits and
approves before anything is written.

**Scope check:** compatible. `commit` is already in scope.

**Notes:** introduces the project's first credential surface (a provider API
key), which needs OS-keychain storage and a "bring your own key" story. The
generated message MUST be a draft in an editable field, never committed
automatically.

**Rough size:** small, plus the credential-storage question it drags in.

---

## B3 — AI-assisted conflict resolution — **BLOCKED**

**Scope check: DIRECTLY VIOLATES `product_scope`.** Visual merge-conflict
resolution is on the `out_of_scope` list, decided 2026-08-18. Building this
requires an explicit, deliberate amendment — not a mention in passing.

**Why the recommendation is against it, beyond the scope rule:**

Conflict resolution was deferred because a defect there destroys a user's work.
Adding AI does not reduce that risk, it changes its shape for the worse: the
output becomes non-deterministic and *plausible*. A wrong merge that looks right
is the worst failure mode version control has, because git's entire value
proposition is being deterministic and auditable.

**If it is ever built, the safe shape is:** the AI *proposes* a resolution and
never applies it; the user sees a normal diff and accepts it hunk by hunk
through the same path as a manual resolution. Suggestion, never authority.

---

## B4 — Forge and issue-tracker links (GitHub, Jira, Linear)

Two very different products live inside this idea, and they should not be built
in the same order they were requested.

### B4a — Link-only, no authentication. **Recommended first.**

Parse the ticket key out of the branch name (`ABC-123-add-widget`) and linkify
it via a configurable URL template. Detect the `origin` remote and linkify
commits, branches and tags to the forge's web UI.

**Scope check:** compatible. Read-only, no credentials.

One small feature covers Jira, Linear, GitHub Issues and anything else with a
URL pattern. No API, no OAuth, no rate limits, no per-provider maintenance —
and it delivers most of the daily value: *"which ticket is this branch, and take
me there."*

### B4b — Authenticated API integration

Pull requests, issue state, CI status, assignees. Requires OAuth device flow,
keychain token storage, scopes, rate-limit handling, and ongoing maintenance per
provider.

**Scope check:** needs an amendment, and a product identity decision. Showing and
managing pull requests makes this a *GitHub client*, not a git client. That may
be the right ambition — but it should be chosen, not drifted into.

`product_scope`'s rationale already names credential handling as one of the two
places open-source git GUIs stall. B4a buys most of the value while that
decision stays open.

---

## Ordering note

None of the above is scheduled while defect F1 stands: the commit graph does not
render at all (`openspec/changes/visual-verification-harness/findings.md`,
measured, not inferred). A backlog is how good ideas wait without becoming
pressure.
