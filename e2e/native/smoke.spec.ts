import assert from "node:assert/strict";

import { $, $$, browser } from "@wdio/globals";

import { readFixture } from "../support/fixture";
import { artifactPath } from "../support/artifacts";

/**
 * Spec A — proof the harness is alive, not proof the product is correct.
 * MUST stay green: a red suite must never be confused with a broken harness
 * (spec.md, Requirement "Native Smoke Spec Proves the Harness Is Alive").
 */
describe("native smoke", () => {
  const fixture = readFixture();

  it("boots the real binary against the fixture: window title, header repo name, sidebar branches and tags", async () => {
    // The listbox only exists once `open_repository` resolved and the main
    // view replaced the WelcomeScreen — i.e. startup_path really carried
    // the fixture path through argv (design.md's U3 resolution).
    await $('[role="listbox"]').waitForExist({ timeout: 15000 });

    const title = await browser.getTitle();
    assert.equal(title, "Gitvisor", `window title: expected "Gitvisor", got "${title}"`);

    const headerName = await $("header span.truncate.font-semibold").getText();
    assert.equal(
      headerName,
      fixture.name,
      `header repo name: expected "${fixture.name}", got "${headerName}"`,
    );

    // "Branches" is the one sidebar section open by default (Sidebar.tsx).
    const branchLabels = await $$(
      'nav section:first-of-type button span.flex-1.truncate',
    ).map((element) => element.getText());
    for (const branch of fixture.branches) {
      assert.ok(
        branchLabels.includes(branch),
        `sidebar branches: expected "${branch}" among ${JSON.stringify(branchLabels)}`,
      );
    }

    // "Tags" starts collapsed (no `defaultOpen` in Sidebar.tsx) — expand it.
    const sectionButtons = await $$("nav section button");
    let tagsButton: WebdriverIO.Element | undefined;
    for (const button of sectionButtons) {
      if ((await button.getText()).startsWith("Tags")) {
        tagsButton = button;
        break;
      }
    }
    assert.ok(tagsButton, 'sidebar: expected a "Tags" section');
    await tagsButton!.click();

    const tagLabels = await $$(
      'nav section button span.flex-1.truncate',
    ).map((element) => element.getText());
    for (const tag of fixture.tags) {
      assert.ok(
        tagLabels.includes(tag),
        `sidebar tags: expected "${tag}" among ${JSON.stringify(tagLabels)}`,
      );
    }

    await browser.saveScreenshot(artifactPath("smoke.png"));
  });
});
