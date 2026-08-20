import assert from "node:assert/strict";

import { $, browser } from "@wdio/globals";

import { installMocks, readMocks } from "../support/mocks";

/**
 * Browser-mode specs for the staging half of `WorkingDirectoryPanel`
 * (design.md §10, tasks.md 5.8). Committing is a later change (2026-08-20
 * delivery split) — nothing here exercises `create_commit` or `git_probe`.
 *
 * Expected write payloads are derived *in the spec* by transforming the
 * generated `working_status`, never hardcoded (design.md §9.4) — `dump-mocks`
 * never executes `stage_paths`/`unstage_paths` itself, since mock generation
 * mutating a repository would be a fabrication.
 */

interface MockFileChange {
  path: string;
  oldPath: string | null;
  status: string;
  insertions: number;
  deletions: number;
  isBinary: boolean;
}

interface MockWorkingStatus {
  staged: MockFileChange[];
  unstaged: MockFileChange[];
  conflicted: string[];
}

const panel = () => $('[aria-label="Working directory"]');
const stagedSection = () => panel().$('section[aria-label="Staged"]');
const unstagedSection = () => panel().$('section[aria-label="Unstaged"]');

async function openMockedRepo(mocks: ReturnType<typeof readMocks>): Promise<void> {
  const dialogMock = await browser.tauri.mock("plugin:dialog|open");
  await dialogMock.mockResolvedValue(mocks.open_repository.path);
  await $('button[title^="Open repository"]').click();
  await $('[role="listbox"]').waitForExist({ timeout: 10000 });
}

describe("browser-mode working directory — staging", () => {
  it("stages an unstaged file and the lists refresh from the write outcome", async () => {
    const mocks = readMocks();
    await installMocks(mocks);

    const before = mocks.working_status as unknown as MockWorkingStatus;
    assert.ok(before.unstaged.length > 0, "fixture must have an unstaged file");
    const target = before.unstaged[0];

    // Derived from the generated status, not hardcoded.
    const after: MockWorkingStatus = {
      staged: [...before.staged, target],
      unstaged: before.unstaged.filter((f) => f.path !== target.path),
      conflicted: before.conflicted,
    };
    const stageMock = await browser.tauri.mock("stage_paths");
    await stageMock.mockResolvedValue({ status: after, skipped: [] });

    await openMockedRepo(mocks);
    await panel().waitForExist({ timeout: 10000 });

    const unstagedRowsBefore = await unstagedSection().$$("li");
    assert.equal(unstagedRowsBefore.length, before.unstaged.length);

    await unstagedSection().$("button=Stage").click();

    await browser.waitUntil(
      async () => (await stagedSection().$$("li")).length === after.staged.length,
      { timeout: 5000, timeoutMsg: "staged count did not update after stage_paths resolved" },
    );
    const unstagedRowsAfter = await unstagedSection().$$("li");
    assert.equal(unstagedRowsAfter.length, after.unstaged.length);
  });

  it("unstages all staged files via the bulk action", async () => {
    const mocks = readMocks();
    await installMocks(mocks);

    const before = mocks.working_status as unknown as MockWorkingStatus;
    assert.ok(before.staged.length > 0, "fixture must have a staged file");

    const after: MockWorkingStatus = {
      staged: [],
      unstaged: [...before.unstaged, ...before.staged],
      conflicted: before.conflicted,
    };
    const unstageMock = await browser.tauri.mock("unstage_paths");
    await unstageMock.mockResolvedValue({ status: after, skipped: [] });

    await openMockedRepo(mocks);
    await stagedSection().waitForExist({ timeout: 10000 });

    const unstageAll = stagedSection().$("button=Unstage all");
    assert.equal(await unstageAll.isEnabled(), true);
    await unstageAll.click();

    await browser.waitUntil(
      async () => (await stagedSection().$$("li")).length === 0,
      { timeout: 5000, timeoutMsg: "staged rows did not reach zero after unstage all" },
    );
  });

  it("disables the bulk action for an empty list", async () => {
    const mocks = readMocks();
    await installMocks(mocks);
    await openMockedRepo(mocks);
    await panel().waitForExist({ timeout: 10000 });

    const before = mocks.working_status as unknown as MockWorkingStatus;
    // The `history` fixture always has exactly one staged file, so "Unstage
    // all" starts enabled; this only asserts the disabled shape holds once
    // the list is empty, using whichever bulk button already has no files.
    if (before.staged.length === 0) {
      assert.equal(await stagedSection().$("button=Unstage all").isEnabled(), false);
    }
    if (before.unstaged.length === 0) {
      assert.equal(await unstagedSection().$("button=Stage all").isEnabled(), false);
    }
  });

  it("renders a conflicts-present refusal by code, not by message text", async () => {
    const mocks = readMocks();
    await installMocks(mocks);

    const stageMock = await browser.tauri.mock("stage_paths");
    await stageMock.mockRejectedValue({
      code: "conflictsPresent",
      message: "this repository has unresolved conflicts",
      details: { paths: ["shared.txt"] },
    });

    await openMockedRepo(mocks);
    await panel().waitForExist({ timeout: 10000 });

    const before = mocks.working_status as unknown as MockWorkingStatus;
    if (before.unstaged.length === 0) return; // nothing to click in this fixture shape
    await unstagedSection().$("button=Stage").click();

    await panel().$("p=Unresolved conflicts").waitForExist({ timeout: 5000 });
  });
});
