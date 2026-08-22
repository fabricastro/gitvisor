import assert from "node:assert/strict";

import { $, browser } from "@wdio/globals";

import { artifactPath } from "../../support/artifacts";
import { readFixture } from "../../support/fixture";

/**
 * The one native write spec (spec.md, requirement
 * "e2e-verification-harness delta" — "Exactly one native spec MUST prove
 * the end-to-end path — stage, commit, and the new commit appearing in the
 * graph — against a dedicated fixture, on both macOS and Linux").
 *
 * Real binary, real WebKitGTK/WKWebView, the real system `git` as a
 * subprocess (design.md §2) — against the dedicated `writes` fixture, never
 * `history` (design.md §9.2: `history`'s determinism must never be touched
 * by a write).
 *
 * Per finding H2: never assert on rendered date text. This spec asserts on
 * the commit summary text and the row count, both derived from the fixture
 * manifest or from what this spec itself typed — never a timestamp.
 *
 * Interactions and reads are consolidated into single `browser.execute()`
 * calls wherever the action does not itself need real WebDriver input
 * semantics: `withGlobalTauri` is off (finding H1), so `@wdio/tauri-service`'s
 * `ensureActiveWindowFocus` pre-command check fails and retries on every
 * discrete WebDriver command in this harness, adding real per-command
 * latency. A synthetic DOM `.click()` fires the same bubbling event React's
 * delegated listeners handle, and a native input-value setter plus a
 * dispatched `input` event is the standard way to drive a React-controlled
 * field without going through WebDriver's own (slower) element interaction
 * protocol — so one `execute()` does the work many separate commands would.
 */
describe("native write path: stage a file, commit through the UI, see it in the graph", () => {
  const fixture = readFixture("writes");

  it("stages an unstaged file and commits it, and the new commit appears at the top of the graph", async function () {
    this.timeout(150000);

    await $('[role="listbox"]').waitForExist({ timeout: 20000 });

    assert.equal(fixture.initialStatus.staged.length, 0, "fixture must start with nothing staged");
    const target = fixture.initialStatus.unstaged.find((entry) => entry.status === "modified");
    assert.ok(target, "expected the `writes` fixture to have a modified unstaged file");

    // One round trip: read the initial panel state.
    const before = await browser.execute(() => {
      const panel = document.querySelector('[aria-label="Working directory"]');
      const staged = panel?.querySelector('section[aria-label="Staged"]');
      const unstaged = panel?.querySelector('section[aria-label="Unstaged"]');
      return {
        panelExists: Boolean(panel),
        stagedCount: staged ? staged.querySelectorAll("li").length : -1,
        unstagedCount: unstaged ? unstaged.querySelectorAll("li").length : -1,
      };
    });
    assert.ok(before.panelExists, "expected the working-directory panel to exist");
    assert.equal(before.stagedCount, 0);
    assert.equal(before.unstagedCount, fixture.initialStatus.unstaged.length);

    // One round trip: click "Stage" on the row whose text contains the
    // target path — a real DOM click, which React's delegated listener
    // handles identically to a user click.
    const staged = await browser.execute((path: string) => {
      const panel = document.querySelector('[aria-label="Working directory"]');
      const unstaged = panel?.querySelector('section[aria-label="Unstaged"]');
      const rows = Array.from(unstaged?.querySelectorAll("li") ?? []);
      const row = rows.find((el) => el.textContent?.includes(path));
      if (!row) return { found: false };
      const button = Array.from(row.querySelectorAll("button")).find(
        (b) => b.textContent === "Stage",
      );
      if (!button) return { found: true, clicked: false };
      (button as HTMLButtonElement).click();
      return { found: true, clicked: true };
    }, target.path);
    assert.ok(staged.found, `expected an unstaged row for ${target.path}`);
    assert.ok(staged.clicked, `expected a "Stage" button on the row for ${target.path}`);

    await browser.waitUntil(
      async () =>
        (await browser.execute(() => {
          const section = document
            .querySelector('[aria-label="Working directory"]')
            ?.querySelector('section[aria-label="Staged"]');
          return section ? section.querySelectorAll("li").length : -1;
        })) === 1,
      { timeout: 60000, interval: 1000, timeoutMsg: "staged list did not reach 1 entry after staging" },
    );

    // One round trip: type the commit message (via the native value setter +
    // a dispatched `input` event, so React's controlled state updates) and
    // click "Commit".
    const commitMessage = "native write spec: stage and commit through the UI";
    const submitted = await browser.execute((message: string) => {
      const panel = document.querySelector('[aria-label="Working directory"]');
      const textarea = panel?.querySelector("textarea");
      if (!textarea) return { hasTextarea: false, clicked: false };
      const setter = Object.getOwnPropertyDescriptor(
        window.HTMLTextAreaElement.prototype,
        "value",
      )?.set;
      setter?.call(textarea, message);
      textarea.dispatchEvent(new Event("input", { bubbles: true }));

      const buttons = Array.from(panel?.querySelectorAll("button") ?? []);
      const commitButton = buttons.find((b) => b.textContent === "Commit") as
        | HTMLButtonElement
        | undefined;
      if (!commitButton || commitButton.disabled) {
        return { hasTextarea: true, clicked: false, disabled: commitButton?.disabled ?? null };
      }
      commitButton.click();
      return { hasTextarea: true, clicked: true };
    }, commitMessage);
    assert.ok(submitted.hasTextarea, "expected a commit-message textarea");
    assert.ok(
      submitted.clicked,
      `expected the Commit button to be enabled and clicked, got: ${JSON.stringify(submitted)}`,
    );

    // The new commit appearing in the graph is the actual assertion this
    // requirement is about — not just that the write outcome resolved.
    await browser.waitUntil(
      async () =>
        (await browser.execute(
          () => document.querySelectorAll('[role="option"]').length,
        )) === fixture.commitCount + 1,
      {
        timeout: 90000,
        interval: 1000,
        timeoutMsg: `expected ${fixture.commitCount + 1} graph rows after the commit`,
      },
    );

    const after = await browser.execute(() => {
      const rows = document.querySelectorAll('[role="option"]');
      const topRow = rows[0];
      // `span.flex-1[title]` is specifically the summary span
      // (`CommitRow.tsx`) — a ref badge is also a `span[title]`, but not
      // `flex-1`, and would otherwise be matched first when present.
      const titleSpan = topRow?.querySelector("span.flex-1[title]");
      const panel = document.querySelector('[aria-label="Working directory"]');
      const staged = panel?.querySelector('section[aria-label="Staged"]');
      return {
        topTitle: titleSpan?.getAttribute("title") ?? null,
        stagedCount: staged ? staged.querySelectorAll("li").length : -1,
      };
    });
    assert.equal(
      after.topTitle,
      commitMessage,
      `expected the new commit's summary at the top of the graph, got: ${after.topTitle}`,
    );
    assert.equal(after.stagedCount, 0, "expected the staged file to be gone after a real commit");

    await browser.saveScreenshot(artifactPath("native-write-stage-commit.png"));
  });
});
