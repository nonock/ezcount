// Playwright's global setup: bundles the Tauri mock (`mock/`) into the one script the tests
// add to each page. Playwright can only add a file or a function that imports nothing.

import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "vite";
import { MOCK_BUNDLE } from "./tauri-mock";

export default async function buildMock() {
  await build({
    // Not the app's configuration: the mock is plain TypeScript.
    configFile: false,
    logLevel: "warn",
    publicDir: false,
    build: {
      lib: {
        entry: fileURLToPath(new URL("./mock/entry.ts", import.meta.url)),
        formats: ["iife"],
        name: "ezcountTauriMock",
        fileName: () => "tauri-mock.js",
      },
      outDir: dirname(MOCK_BUNDLE),
      emptyOutDir: false,
      minify: false,
    },
  });
}
