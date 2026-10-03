// The web version's backend: the Rust core (web/, compiled to WebAssembly) in a dedicated Web
// Worker. Its storage, SQLite in the origin's private file system, only works in one, and
// password hashing then doesn't freeze the page. install.ts talks to it.
import init, { invoke, start } from "ezcount-web";
import wasmUrl from "ezcount-web/ezcount_web_bg.wasm?url";
import type { CommandMessage, WorkerMessage } from "./protocol";

// The worker's global scope, typed by hand: the project compiles against the DOM library.
const scope = self as unknown as {
  postMessage(message: WorkerMessage): void;
  onmessage: ((event: MessageEvent<CommandMessage>) => void) | null;
};

const ready = (async () => {
  await init({ module_or_path: wasmUrl });
  await start((event, payload) => {
    scope.postMessage({ type: "event", event, payload: JSON.parse(payload) });
  });
})();

scope.onmessage = async ({ data }) => {
  try {
    await ready;
    const result = await invoke(data.command, JSON.stringify(data.args ?? {}));
    scope.postMessage({ type: "result", id: data.id, ok: true, value: JSON.parse(result) });
  } catch (err) {
    // Command errors are the messages to show, as Tauri's are.
    const error = err instanceof Error ? err.message : String(err);
    scope.postMessage({ type: "result", id: data.id, ok: false, error });
  }
};
