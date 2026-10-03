// Messages between the page (install.ts) and the Web Worker running the Rust core (worker.ts).

export interface CommandMessage {
  id: number;
  command: string;
  args: unknown;
}

export type WorkerMessage =
  | { type: "result"; id: number; ok: true; value: unknown }
  | { type: "result"; id: number; ok: false; error: string }
  /** A backend event, as the Tauri app emits them (`sync-updated`, `account-updated`). */
  | { type: "event"; event: string; payload: unknown };
