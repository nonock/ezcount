// The web version's stand-in for Tauri. The app reaches its backend through Tauri's IPC
// (`bindings.ts` and the @tauri-apps APIs all call `window.__TAURI_INTERNALS__`), so the web
// build provides one: app commands go to the Rust core in a Web Worker (worker.ts), with the
// same JSON in and out as Tauri's, and the native ones get browser equivalents.
import { t } from "@/lib/i18n/index.svelte";
import type { CommandMessage, WorkerMessage } from "./protocol";

/** Holds the database: OPFS lets one worker open it at a time, so one tab runs ezcount. */
const LOCK = "ezcount-database";
/** Invite links opened in another tab come to the one running ezcount. */
const INVITES = "ezcount-invites";
/** The deep-link plugin's event for links opened while the app runs. */
const OPENED_LINK_EVENT = "deep-link://new-url";

type Callback = (event: unknown) => void;

/**
 * Sets up the backend. False when ezcount is already open in another tab: this tab then
 * hands over the invite it was opened with, if any, and shows a note instead of the app.
 */
export async function installWebBackend(): Promise<boolean> {
  const invite = takeInvite();
  if (!(await holdLock())) {
    if (invite) new BroadcastChannel(INVITES).postMessage(invite);
    showElsewhere(invite !== null);
    return false;
  }

  const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
  let nextId = 1;
  const pending = new Map<number, { resolve: (value: unknown) => void; reject: Callback }>();
  const callbacks = new Map<number, Callback>();
  const listeners = new Map<string, Set<number>>();

  const emit = (event: string, payload: unknown) => {
    for (const id of listeners.get(event) ?? []) callbacks.get(id)?.({ event, id, payload });
  };

  worker.onmessage = ({ data }: MessageEvent<WorkerMessage>) => {
    if (data.type === "event") {
      emit(data.event, data.payload);
      return;
    }
    const call = pending.get(data.id);
    pending.delete(data.id);
    if (data.ok) call?.resolve(data.value);
    else call?.reject(data.error);
  };
  new BroadcastChannel(INVITES).onmessage = ({ data }) => {
    if (typeof data === "string") emit(OPENED_LINK_EVENT, [data]);
  };

  const backend = (command: string, args: unknown) =>
    new Promise<unknown>((resolve, reject) => {
      const id = nextId++;
      pending.set(id, { resolve, reject });
      worker.postMessage({ id, command, args } satisfies CommandMessage);
    });

  const w = window as unknown as Record<string, unknown>;
  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (event: string, id: number) => listeners.get(event)?.delete(id),
  };
  w.__TAURI_INTERNALS__ = {
    transformCallback: (callback: Callback = () => {}, once = false) => {
      const id = nextId++;
      callbacks.set(id, (event) => {
        if (once) callbacks.delete(id);
        callback(event);
      });
      return id;
    },
    unregisterCallback: (id: number) => callbacks.delete(id),
    invoke: async (command: string, args: Record<string, unknown> = {}) => {
      switch (command) {
        case "plugin:event|listen": {
          const event = args.event as string;
          const ids = listeners.get(event) ?? new Set<number>();
          ids.add(args.handler as number);
          listeners.set(event, ids);
          return args.handler;
        }
        case "plugin:event|unlisten":
          return null;
        case "plugin:deep-link|get_current":
          return invite ? [invite] : null;
        case "native_features":
          // No camera scanning: the browser's would need its own QR decoder.
          return { share: typeof navigator.share === "function", scan: false, save: false };
        case "set_bars_color":
          return null;
        case "share_text":
          try {
            await navigator.share({ text: args.text as string, title: args.title as string });
          } catch (err) {
            // Closing the share sheet isn't an error.
            if (!(err instanceof DOMException && err.name === "AbortError")) throw String(err);
          }
          return null;
        default:
          // The other plugins (the Back button, scanning) are native only.
          if (command.startsWith("plugin:")) return null;
          return backend(command, args);
      }
    },
  };
  return true;
}

/**
 * The invite this page was opened with, from its `#v=…&g=…&k=…` fragment (the relay's join
 * page links here with it), as the invite link the app expects. The fragment is removed so
 * the key doesn't stay in the address bar or the history.
 */
function takeInvite(): string | null {
  const fragment = new URLSearchParams(location.hash.slice(1));
  if (!fragment.get("g") || !fragment.get("k")) return null;
  const invite = `${location.origin}/join${location.hash}`;
  history.replaceState(history.state, "", location.pathname + location.search);
  return invite;
}

/** Takes the database lock for this page's lifetime; false if another tab has it. */
function holdLock(): Promise<boolean> {
  if (!navigator.locks) return Promise.resolve(true);
  return new Promise((resolve) => {
    navigator.locks.request(LOCK, { ifAvailable: true }, (lock) => {
      resolve(lock !== null);
      // Held until the page closes.
      return lock ? new Promise<void>(() => {}) : undefined;
    });
  });
}

/** Replaces the splash with a note that ezcount runs in another tab. */
function showElsewhere(sentInvite: boolean) {
  const note = document.createElement("p");
  note.className = "ez-splash-note";
  note.textContent = sentInvite ? t("app.otherTabInvite") : t("app.otherTab");
  document.querySelector(".ez-splash")?.append(note);
  document.querySelector(".ez-splash-part")?.classList.remove("ez-splash-part");
}
