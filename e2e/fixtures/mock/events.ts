// A minimal event plugin: `listen` registers a callback that tests fire with
// `window.__emitMockEvent(name, payload)`.

import { w } from "./state";
import type { Commands } from "./types";

let nextCallbackId = 1;
const callbacks = new Map<number, (event: any) => void>();
const listeners = new Map<string, number[]>();

/** What `window.__TAURI_INTERNALS__.transformCallback` does: keeps a callback under a number. */
export function transformCallback(callback: (event: any) => void) {
  const id = nextCallbackId++;
  callbacks.set(id, callback);
  return id;
}

export function installEvents() {
  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (event: string, id: number) => {
      listeners.set(
        event,
        (listeners.get(event) || []).filter((x) => x !== id)
      );
    },
  };
  w.__emitMockEvent = (event: string, payload: unknown) => {
    for (const id of listeners.get(event) || []) {
      callbacks.get(id)?.({ event, id, payload });
    }
  };
}

export const eventCommands: Commands = {
  "plugin:event|listen"(args) {
    listeners.set(args.event, [...(listeners.get(args.event) || []), args.handler]);
    return args.handler;
  },

  "plugin:event|unlisten": () => null,
};
