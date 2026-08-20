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
| M-series | Reference | Measured facts underpinning `stage-and-commit`: libgit2 runs no hooks; ignores `commit.gpgsign` (M1); returns a stale index after an external `git add`, and writing it back destroys that work (M2); a SIGTERM'd signing commit is a safe no-op (M3); path escapes are refused but only as `GenericError` (M4); `Repository::signature()` reads config only, so a pre-flight built on it falsely refuses environment identities (M5). Full evidence in `openspec/changes/stage-and-commit/measurements.md`. |

## Why M5 has its own note

`explore.md` §3.3 asserted the opposite of M5 and marked it **"Verified"**
without measuring it. The word is a contract with the next reader: it says the
checking is already done. Applied to reasoning, however sound, it removes the
doubt that would have caught the error.

**"Verified" means someone ran it and can show the output.** Everything else is
"reasoned", and reasoned things belong in an unverified register with the cost
of finding out written next to them.
