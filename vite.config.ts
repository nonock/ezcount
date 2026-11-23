import { fileURLToPath } from "node:url";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

/** In the web version's development server (`bun run dev:web`), the relay's API is reached on the
 * page's own origin, as when the relay serves it. */
const relayProxy = {
  "/v1": "http://localhost:8787",
  "/health": "http://localhost:8787",
};

// https://vite.dev/config/
export default defineConfig(({ mode }) => ({
  plugins: [tailwindcss(), svelte()],
  resolve: {
    alias: [
      { find: "@", replacement: fileURLToPath(new URL("./src", import.meta.url)) },
      // The web version's Rust core, built by `bun run build:wasm` (see src/web/).
      {
        find: /^ezcount-web$/,
        replacement: fileURLToPath(new URL("./web/pkg/ezcount_web.js", import.meta.url)),
      },
      {
        find: /^ezcount-web\//,
        replacement: fileURLToPath(new URL("./web/pkg/", import.meta.url)),
      },
    ],
  },
  worker: { format: "es" as const },
  // Loaded only when a PDF is exported: found late, the development server would bundle them
  // then and reload the page under the user (and the tests).
  optimizeDeps: { include: ["pdf-lib", "@pdf-lib/fontkit"] },
  // The relay serves the web version from sync-server/web (its Docker build copies it in).
  build: { outDir: mode === "web" ? "sync-server/web" : "dist", emptyOutDir: true },
  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`, and the other crates' build output: tens of
      //    thousands of files it would go through before answering its first page
      ignored: ["**/src-tauri/**", "**/target/**"],
    },
    proxy: mode === "web" ? relayProxy : undefined,
  },
}));
