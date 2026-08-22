import { execSync } from "node:child_process";
import { readFileSync, rmSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

import type { Options } from "@wdio/types";

import { readFixture } from "./e2e/support/fixture";
import { sharedConfig } from "./wdio.shared.conf";

const APPLICATION = "./target/debug/gitvisor";

/**
 * A separate config from `wdio.native.conf.ts`, the same shape as the
 * existing native/browser split (design.md §9.3) — driven by U4 staying
 * unverified (whether `@wdio/tauri-service` re-spawns the app per spec
 * file). Its own `writes` fixture, its own `onPrepare`, its own
 * `appArgs` — zero unknowns, rather than betting on service internals a
 * second time (the same class of trap `visual-verification-harness` hit
 * with `tauri:options.args`).
 */
function clearRememberedRepoStorage(): void {
  if (process.platform !== "darwin") return;
  const dataDir = join(homedir(), "Library", "WebKit", "gitvisor", "WebsiteData");
  rmSync(dataDir, { recursive: true, force: true });
}

/** See `wdio.native.conf.ts`'s identical check — a plain `cargo build`
 * embeds `devUrl` and the resulting binary only works against a running
 * Vite dev server. */
function assertFrontendIsEmbedded(binary: string): void {
  const contents = readFileSync(binary);
  if (contents.includes("localhost:1420")) {
    throw new Error(
      `${binary} still points at the Vite dev server.\n` +
        "Build it with `pnpm run e2e:build` — a plain `cargo build` embeds devUrl " +
        "and the app will only work while a dev server happens to be running.",
    );
  }
}

export const config: Options.Testrunner = {
  ...sharedConfig,
  specs: ["./e2e/native/writes/**/*.spec.ts"],
  capabilities: [
    {
      browserName: "tauri",
      "tauri:options": {
        application: APPLICATION,
      },
      "wdio:tauriServiceOptions": {
        appArgs: [] as string[],
        env: { LANG: "en_US.UTF-8" },
      },
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
    } as any,
  ],
  services: [["@wdio/tauri-service", {}]],

  onPrepare: (_config, capabilities) => {
    assertFrontendIsEmbedded(APPLICATION);
    // Load-bearing here too: a stale `gitvisor:last-repo` from a `history`
    // run would point this suite at the wrong repository (design.md §9.3).
    clearRememberedRepoStorage();

    execSync("cargo run -p git-fixtures --bin build-fixture -- --name writes", {
      stdio: "inherit",
      env: { ...process.env, PATH: `${homedir()}/.cargo/bin:${process.env.PATH ?? ""}` },
    });

    const fixture = readFixture("writes");
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const caps = capabilities as any[];
    caps[0]["wdio:tauriServiceOptions"].appArgs = [fixture.path];
  },
};
