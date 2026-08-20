# Delta for E2E Verification Harness

Note for archive: `openspec/specs/` has no promoted `e2e-verification-harness`
spec yet — it currently lives only under
`openspec/changes/visual-verification-harness/specs/e2e-verification-harness/spec.md`.
This delta targets that same capability name; the archive step MUST reconcile
against whichever of the two (promoted main spec, or the still-active harness
change's spec) is current at archive time.

## ADDED Requirements

### Requirement: Write Specs Use Dedicated, Isolated Fixtures

Any E2E spec that stages, unstages, or commits MUST run against its own
dedicated fixture, built for that spec alone. The shared read-only `history`
fixture MUST NOT be written to by any spec.

#### Scenario: A write spec does not disturb the shared fixture
- GIVEN a write spec runs against its own dedicated fixture
- WHEN a read-only spec runs afterward against the shared `history` fixture
  in the same suite invocation
- THEN the shared fixture shows no staged, unstaged, or committed changes
  introduced by the write spec

### Requirement: Write-Path Test Coverage Is Split by Speed and Fidelity

`cargo test -p git-core` MUST prove correctness: refusal codes, unborn-branch
and detached-HEAD commits, the hook-rejection regression, and the index-
freshness regression. Browser-mode specs (mocked `invoke`) MUST prove UI
state: button enablement, each refusal message rendered by `code`, and status
refresh after a write. Exactly one native spec MUST prove the end-to-end path
— stage, commit, and the new commit appearing in the graph — against a
dedicated fixture, on both macOS and Linux.

#### Scenario: cargo test proves a refusal is unreachable through a bypass
- GIVEN a write path that skips the index-refresh helper
- WHEN `cargo test -p git-core` runs
- THEN the index-freshness regression test fails

#### Scenario: Browser mode proves a disabled commit button and its reason
- GIVEN a mocked `invoke` reporting `git` unavailable
- WHEN the write panel renders
- THEN the commit control is disabled and shows the "git unavailable" message
  by `code`

#### Scenario: The single native spec proves a real commit reaches the graph
- GIVEN the real binary launched against a dedicated write fixture
- WHEN the spec stages a file and commits through the UI
- THEN the new commit appears in the rendered graph

### Requirement: Write Specs Never Assert on Rendered Date Text

Per finding H2, no E2E scenario introduced by this change may assert on
rendered relative or absolute date text, since that text is computed against
wall-clock time and decays independently of the code under test.

#### Scenario: A write spec asserts on commit identity, not on date text
- GIVEN the single native write spec verifies the new commit appears
- WHEN assertions are written
- THEN they check commit message, author, and graph position, and make no
  assertion on any rendered date string
