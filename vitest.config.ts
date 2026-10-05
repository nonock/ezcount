import { svelteTesting } from "@testing-library/svelte/vite";
import { defineConfig, mergeConfig } from "vitest/config";
import viteConfig from "./vite.config.ts";

// Unit tests of the interface's functions and components (`src/**/*.test.ts`, next to what
// they test), in happy-dom with the app's own Vite configuration. Playwright (`e2e/`) drives the
// whole app.
export default defineConfig((env) =>
  mergeConfig(viteConfig(env), {
    plugins: [svelteTesting()],
    test: {
      environment: "happy-dom",
      include: ["src/**/*.test.ts"],
      restoreMocks: true,
    },
  })
);
