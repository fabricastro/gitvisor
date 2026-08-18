/**
 * Minimal ambient type for `browser.tauri` (browser-mode IPC mocking API).
 *
 * `@wdio/tauri-service` ships no global `WebdriverIO.Browser` augmentation in
 * its published `dist/` (checked: no `declare global` anywhere in the
 * package) — its own docs use `browser.tauri.mock(...)` unchecked. `tsc` on
 * the wdio configs is a known pre-existing gap (`TS2688`, recorded in this
 * change's `apply-progress.md`): wdio runs specs through `tsx`, which does
 * not type-check, so this file is for editor/reader clarity, never load-bearing.
 *
 * Only the surface this harness actually calls is declared — see
 * `node_modules/@wdio/tauri-service/docs/api-reference.md` for the rest.
 */
export {};

interface TauriMockInstance {
  readonly mock: {
    calls: unknown[][];
    results: unknown[];
  };
  mockResolvedValue(value: unknown): Promise<TauriMockInstance>;
  mockResolvedValueOnce(value: unknown): Promise<TauriMockInstance>;
  mockReturnValue(value: unknown): Promise<TauriMockInstance>;
  mockImplementation(
    fn: (args: Record<string, unknown>) => unknown,
  ): Promise<TauriMockInstance>;
  mockReset(): Promise<TauriMockInstance>;
  mockClear(): Promise<TauriMockInstance>;
  mockRestore(): Promise<TauriMockInstance>;
  update(): Promise<void>;
}

declare global {
  namespace WebdriverIO {
    interface Browser {
      tauri: {
        mock(command: string): Promise<TauriMockInstance>;
        emitEvent(
          event: string,
          payload?: unknown,
          target?: string,
        ): Promise<void>;
      };
    }
  }
}
