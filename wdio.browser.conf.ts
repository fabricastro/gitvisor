import type { Options } from "@wdio/types";

import { sharedConfig } from "./wdio.shared.conf";

/** Tauri's default dev-server port; `vite.config.ts` pins it with `strictPort`. */
const DEV_SERVER_URL = "http://localhost:1420";

/**
 * Browser mode (design.md §4.2): the real frontend, in Chrome, against
 * generated `invoke()` mocks — no Tauri binary, no WebKit, no Rust compile
 * on the CI machine that runs this. Fast iteration loop, not the correctness
 * authority; `wdio.native.conf.ts` owns that.
 */
export const config: Options.Testrunner = {
  ...sharedConfig,
  specs: ["./e2e/browser/**/*.spec.ts"],
  capabilities: [{ browserName: "tauri" }],
  services: [
    [
      "@wdio/tauri-service",
      {
        mode: "browser",
        devServerUrl: DEV_SERVER_URL,
        // Spawns `pnpm dev` in onPrepare, waits for readiness, tears it down
        // on completion — so this needs no manually-running dev server, and
        // CI needs no Rust/webkit to run the browser-mode job at all.
        devServer: "pnpm dev",
      },
    ],
  ],
};
