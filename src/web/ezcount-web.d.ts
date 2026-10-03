// The web crate's wasm-bindgen glue: web/pkg, built by `bun run build:wasm` and aliased as
// `ezcount-web` in vite.config.ts. See web/src/lib.rs.
declare module "ezcount-web" {
  export default function init(options?: { module_or_path?: string | URL }): Promise<unknown>;
  /** Opens the database and starts background sync; `onEvent` gets the backend's events. */
  export function start(onEvent: (event: string, payload: string) => void): Promise<void>;
  /** Runs a command with JSON arguments; resolves with its JSON result, rejects with a message. */
  export function invoke(command: string, args: string): Promise<string>;
}
