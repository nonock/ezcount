// In-memory Tauri IPC simulator for Playwright E2E tests.
// Validates that all invocation signatures match the Rust tauri::command definitions.
//
// The mock itself is in `mock/` (state, the engine's rules, and the commands by subject).
// Playwright can only add a script to a page, so `build-mock.ts` bundles it into one file
// before the tests run (the configuration's `globalSetup`).

import { fileURLToPath } from "node:url";

export { MOCK_PASSWORD, MOCK_SERVER } from "./mock/constants";
export type {
  MockExpense,
  MockExpenseHistoryEntry,
  MockExpenseSplit,
  MockGroup,
  MockOriginalAmount,
  MockRecurringExpense,
} from "./mock/types";

/** Where `build-mock.ts` writes the script. */
export const MOCK_BUNDLE = fileURLToPath(
  new URL("../../node_modules/.cache/ezcount/tauri-mock.js", import.meta.url)
);

/**
 * The mock, for `page.addInitScript(installTauriMock)`.
 *
 * Seeds (set on `window` before the app loads, by a second `addInitScript`; the mock reads
 * them when a command first needs them):
 * - `__SEED_GROUPS__`: groups on the device; the user is their first participant
 *   unless `__SEED_IDENTITIES__` (group id -> participant id) says otherwise. A group's
 *   `recurring` expenses need only their `start`: the ones due are added when it is read
 * - `__LOGGED_OUT__`: start on the login screen
 * - `__REMOTE_GROUPS__`: groups that can be joined with an invite code
 * - `__UNSYNCED__`: log out fails unless forced
 * - `__OPENED_WITH__`: the link the app was opened with (deep link)
 * - `__RECOVERY_KEY__`: the account's recovery key; new ones are `MOCK-KEY<n>-AAAA-…`
 * - `__OLD_RELAY__`: sign-up gets no recovery key, like on a relay from before them
 * - `__NATIVE__`: `{ share, scan, save }` features, none by default; shared texts land in
 *   `window.__shared`, saved files in `window.__saved` (with `text`, or `bytes` for a PDF)
 * - `__SCANNED__`: what the camera "scans"
 * - messages sent from the feedback form land in `window.__feedback`; with `__OLD_RELAY__`
 *   the relay doesn't take them
 * - `__LINK_SECONDS__`: how long a login link works, 120 by default
 * - `__PHONE_SCANS__`: a phone scans the code this device shows (to log in, or to join the
 *   first of `__REMOTE_GROUPS__`) once it has been asked for this many times; never when
 *   left out. What this device sent to a code it scanned lands in `window.__sent`
 * - `__RATES__`: exchange rates the relay suggests, as `{ "USD/EUR": "0.9234" }`
 * - `__ARCHIVED__`: ids of the groups the user archived
 * - `__PROFILE__`: the account's `{ display_name, avatar, iban }`
 * - `__STORAGE_WARNINGS__`
 * - `__TAKEN_USERNAMES__`: usernames signing up refuses
 */
export const installTauriMock = { path: MOCK_BUNDLE };
