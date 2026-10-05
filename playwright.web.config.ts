import { readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { defineConfig, devices } from "@playwright/test";

// The web version end to end, as deployed: the relay serving it (with its Content-Security-
// Policy) and its API, on a fresh database each run, and the real Rust core in the browser
// (WebAssembly, in a Web Worker). Build it first: `bun run build:web`, then `bun run test:web`.

// The relay answers this version of the app and none before. Every test then also checks that
// the web version says which it is: a build that didn't would get no answer at all.
const { version } = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8"));

export default defineConfig({
  testDir: "./e2e-web",
  fullyParallel: false,
  workers: 1,
  forbidOnly: !!process.env.CI,
  retries: 0,
  // Password hashing (Argon2id) runs in WebAssembly, and sync waits for the relay.
  timeout: 90_000,
  expect: { timeout: 20_000 },
  use: {
    baseURL: "http://127.0.0.1:8787",
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "cargo run --quiet --manifest-path sync-server/Cargo.toml",
    env: {
      EZCOUNT_SYNC_ADDR: "127.0.0.1:8787",
      EZCOUNT_SYNC_DB: join(tmpdir(), `ezcount-web-e2e-${Date.now()}.sqlite3`),
      EZCOUNT_WEB_DIR: "sync-server/web",
      EZCOUNT_MIN_APP_VERSION: version,
    },
    url: "http://127.0.0.1:8787/health",
    reuseExistingServer: false,
    timeout: 300_000,
  },
});
