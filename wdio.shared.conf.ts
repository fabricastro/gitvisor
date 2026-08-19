import type { Options } from "@wdio/types";

import { artifactPath } from "./e2e/support/artifacts";

// Pinned for the whole run — both the wdio/mocha process and (via each
// native capability's `wdio:tauriServiceOptions.env`) the spawned app. This
// is exactly why F2's locale-dependent class of defect stays out of reach
// for this harness (proposal §5.6).
process.env.LANG = "en_US.UTF-8";

/** Options shared by every wdio config (native, and later browser). */
export const sharedConfig: Partial<Options.Testrunner> = {
  runner: "local",
  tsConfigPath: "./tsconfig.wdio.json",
  maxInstances: 1,
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: { ui: "bdd", timeout: 180000 },
  logLevel: "info",

  /**
   * Screenshot every failing test, before anything else runs.
   *
   * Specs capture their own screenshots at the point of interest, but those
   * only run when the spec gets that far. The Linux probe failed waiting for
   * an element on line 20 while its capture sat on line 65, so the run that
   * most needed a picture produced none — and the "upload artifacts always"
   * step had nothing to upload. A failure screenshot has to be a hook, not a
   * statement someone remembered to put in the right place.
   */
  afterTest: async function (test, _context, { passed }) {
    if (passed) return;
    const slug = `${test.parent} ${test.title}`
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "")
      .slice(0, 80);
    try {
      await browser.saveScreenshot(artifactPath(`failure-${slug}.png`));
    } catch (error) {
      // A session that died before the screenshot is itself the diagnosis;
      // never let the capture attempt replace the real failure.
      console.warn(`could not capture a failure screenshot: ${String(error)}`);
    }
  },
};
