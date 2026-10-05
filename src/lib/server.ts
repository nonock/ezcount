// The relay accounts live on: the app's own, unless the user picked another on the login screen.

// A relay the user picked instead of the default; unset when they use the default.
const SERVER_KEY = "ezcount_sync_server";

/** The relay accounts live on unless the user picks another. */
export const DEFAULT_SERVER: string =
  import.meta.env.VITE_EZCOUNT_SERVER ||
  // The web version is served by its relay (proxied to it in development).
  (import.meta.env.MODE === "web"
    ? location.origin
    : import.meta.env.DEV
      ? "http://localhost:8787"
      : "https://ezcount-relay.fly.dev");

/** The relay last logged into from this device, the default one otherwise. */
export function rememberedServer(): string {
  try {
    return localStorage.getItem(SERVER_KEY) || DEFAULT_SERVER;
  } catch {
    return DEFAULT_SERVER;
  }
}

/** A relay's page about what it keeps of its users, in a language of the app. */
export function privacyUrl(server: string, language: string): string {
  return `${server.trim().replace(/\/+$/, "")}/privacy?lang=${language}`;
}

export function rememberServer(server: string) {
  try {
    if (server === DEFAULT_SERVER) localStorage.removeItem(SERVER_KEY);
    else localStorage.setItem(SERVER_KEY, server);
  } catch {
    // Remembering the server is only a convenience.
  }
}
