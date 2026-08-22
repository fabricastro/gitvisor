# Standing findings

Defects and constraints discovered by one change that outlive it. A finding
recorded only inside a change folder disappears when that change is archived,
which is how known problems get rediscovered the expensive way.

Closed items stay listed, with what closed them.

| ID | Status | Summary |
|---|---|---|
| F1 | **Closed** by `fix-graph-viewport` | The commit graph rendered zero pixels and listed only `OVERSCAN + 1` rows: a `ResizeObserver` installed in a `useLayoutEffect` with `[]` deps ran while the component was still showing its placeholder, so it observed nothing and never retried. |
| F2 | **Closed** by `fix-graph-viewport` | The relative-time column could wrap inside a fixed-height row, colliding neighbouring rows. Now structurally unable to wrap. |
| H1 | **Decided** — `stage-and-commit` design D8 | `withGlobalTauri` is off. The WebdriverIO service's window-state and log-forwarding features need it; nothing currently specified does. Enabling it would have to be scoped as tightly as the WebDriver plugin itself. |
| H2 | **OPEN** | Fixture determinism covers commit OIDs, **not rendered output**. `tools/git-fixtures` pins `EPOCH = 1_700_000_000`, so OIDs are byte-identical everywhere — but the UI renders *relative* dates, so the same fixture shows "2 years ago" today and "3 years ago" next year. **Never assert on rendered date text, and pixel baselines cannot work.** Options if it ever needs closing: inject a fixed clock for E2E, render absolute dates behind a test flag, or keep relative time and never assert on it (what happens today, by accident rather than decision). |
| F3 | **Closed** 2026-08-22 | The changes panel squeezes the commit-message column until it is unreadable. At 1440 logical px — an ordinary laptop — messages truncate to `merge refactor/par…` and the SHA column clips at the panel edge. Every test passes: the five browser specs are green, the graph renders correctly, the staging behaviour is right. The app is simply worse to use. Found by looking at a screenshot, which is the only thing that can find it. Fixed by moving the panel into the sidebar instead of giving it a fourth column: the sidebar already carried the working-directory summary, so the file list belongs there. Sidebar widened 240→288px and the standalone 288px column dropped, returning ~240px to the history pane. The sidebar also stopped being a `<nav>` wrapping non-navigation content. |
| F4 | **OPEN — serious** | Under CPU contention Gitvisor reports **2 of 3** working-directory changes: `tracked-a.txt`'s modification vanishes from the unstaged list while `git status` on the same repository reports all three. Measured: without load the native write spec passes 5/5; under load on 10 cores it fails 2/3, and a 15-second `waitUntil` never sees the third entry appear — so the count is wrong, not merely late. A git client that silently omits a modified file is a client you cannot trust to show you what you are about to commit. Cause unconfirmed; the leading hypothesis is git's "racily clean" condition (a file whose mtime is indistinguishable from the index's) being resolved differently by libgit2 than by `git`, but that is reasoning and has not been measured. |
| M-series | Reference | Measured facts underpinning `stage-and-commit`: libgit2 runs no hooks; ignores `commit.gpgsign` (M1); returns a stale index after an external `git add`, and writing it back destroys that work (M2); a SIGTERM'd signing commit is a safe no-op (M3); path escapes are refused but only as `GenericError` (M4); `Repository::signature()` reads config only, so a pre-flight built on it falsely refuses environment identities (M5). Full evidence in `openspec/changes/stage-and-commit/measurements.md`. |

## Why M5 has its own note

`explore.md` §3.3 asserted the opposite of M5 and marked it **"Verified"**
without measuring it. The word is a contract with the next reader: it says the
checking is already done. Applied to reasoning, however sound, it removes the
doubt that would have caught the error.

**"Verified" means someone ran it and can show the output.** Everything else is
"reasoned", and reasoned things belong in an unverified register with the cost
of finding out written next to them.

## A note on F3's category

F3 was not a missing test. There is no assertion for *"this column is too narrow
to read"* — the behaviour was correct, the five browser specs were green, the
graph rendered perfectly, and the app was worse to use than an hour earlier.

It is the exact mirror of F1: that one was broken behaviour with every gate
green; this one was correct behaviour with the experience broken. Both were
invisible to the same battery of checks and obvious in a single screenshot.

Some defects do not live in the space that tests cover. The harness exists to
put a picture in front of someone, and that is a different job from asserting.

## On calling F4 "flaky"

The native write spec failed 1-in-4, then passed 5-in-5, then failed 2-in-3 under
load. The obvious reading was a flaky test, and the obvious response would have
been a longer timeout or a retry.

It was not flaky. It was **correctly detecting an intermittent product defect**,
and every mitigation available for flakiness — retries, longer waits, quarantine —
would have hidden a real bug from a real user.

The measurement that separated the two was cheap: replace the assertion with a
15-second wait. A slow-but-correct app passes; an app that is simply wrong keeps
failing. It kept failing.

Before treating a test as flaky, establish that the behaviour it checks is
actually correct. "Intermittent" describes the observation, not the cause.
