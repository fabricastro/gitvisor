import { execSync } from "node:child_process";
import { rmSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

import type { Options } from "@wdio/types";

import { readFixture } from "./e2e/support/fixture";
import { sharedConfig } from "./wdio.shared.conf";

const APPLICATION = "./target/debug/gitvisor";

/**
 * `rememberedRepo()` (design.md D5's documented fallback, `src/features/repo/store.ts`)
 * reads a `gitvisor:last-repo` key from WebKit's persisted `localStorage`,
 * independent of argv. That storage survives across process launches and
 * wdio runs — it is NOT reset by the harness or by `@wdio/tauri-service`.
 *
 * Measured during Phase 1's U3 spike (see design.md's "U3 resolution"
 * section): a stale entry from an earlier manual `pnpm app` session made a
 * spec look like it received the fixture path via argv when it had not.
 * Clearing this before every suite run is load-bearing, not cosmetic.
 */
function clearRememberedRepoStorage(): void {
  if (process.platform !== "darwin") return;
  const dataDir = join(homedir(), "Library", "WebKit", "gitvisor", "WebsiteData");
  rmSync(dataDir, { recursive: true, force: true });
}

export const config: Options.Testrunner = {
  ...sharedConfig,
  specs: ["./e2e/native/**/*.spec.ts"],
  capabilities: [
    {
      browserName: "tauri",
      "tauri:options": {
        application: APPLICATION,
      },
      // NOT `tauri:options.args` — measured (Phase 1, U3) to be validated
      // and logged by @wdio/tauri-service 1.3.0 but never forwarded to the
      // embedded provider's spawn call. `wdio:tauriServiceOptions.appArgs`
      // is the field that actually reaches `spawnTauriApp`. `appArgs` is
      // populated in `onPrepare`, once the fixture path is known.
      "wdio:tauriServiceOptions": {
        appArgs: [] as string[],
        env: { LANG: "en_US.UTF-8" },
      },
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
    } as any,
  ],
  services: [["@wdio/tauri-service", {}]],

  onPrepare: (_config, capabilities) => {
    clearRememberedRepoStorage();

    execSync("cargo run -p git-fixtures --bin build-fixture", {
      stdio: "inherit",
      env: { ...process.env, PATH: `${homedir()}/.cargo/bin:${process.env.PATH ?? ""}` },
    });

    const fixture = readFixture();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const caps = capabilities as any[];
    caps[0]["wdio:tauriServiceOptions"].appArgs = [fixture.path];
  },
};
