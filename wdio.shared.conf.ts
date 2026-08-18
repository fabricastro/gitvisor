import type { Options } from "@wdio/types";

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
};
