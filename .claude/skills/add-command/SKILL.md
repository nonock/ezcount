---
name: add-command
description: Checklist for adding or changing a Tauri command (Rust function called from the React frontend) in ezcount, end to end - Rust, specta types, bindings, api.ts, the Playwright mock and tests. Use for any new backend feature the UI calls, or when a command's arguments or result change.
---

# Add or change a Tauri command

A command crosses seven places. Missing one usually still compiles but breaks the UI or the
Playwright tests, so go through all of them.

## 1. Rust logic

Put the real work in the module it belongs to (`sync.rs` for accounts and sync, `doc.rs` for
group data, `storage.rs` for persistence), as a plain function returning `Res<T>` (=
`Result<T, String>`). Error strings are shown to users as is: write them as full sentences
("Your current password is wrong"), never debug output.

Test it there: unit tests in the module's `#[cfg(test)] mod tests`, or in `sync.rs`'s
`end_to_end` module when it talks to the relay (it starts a real relay in-process; see the
`Device` helper and `start_relay`).

## 2. The command (`src-tauri/src/lib.rs`)

```rust
/// One line on what it does; this becomes the TS doc comment.
#[tauri::command]
#[specta::specta]
async fn do_thing(state: State<'_, AppState>, some_arg: String) -> Result<Thing, String> {
    sync::do_thing(&state, &some_arg).await
}
```

- `async fn` for anything doing I/O or taking more than a few milliseconds: sync commands run
  on the main thread and freeze the window.
- Register it in `create_specta_builder()`'s `collect_commands![…]` list. Unregistered commands
  compile fine and fail at runtime.
- Arguments are camelCased on the TS side (`some_arg` → `someArg`).

## 3. Types (`src-tauri/src/models.rs`)

New structs need `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]` and
must be imported in `lib.rs`'s `use crate::models::{…}`.

**Comment fields with `//`, not `///`.** specta copies `///` field docs into `bindings.ts`
with trailing spaces, and the pre-commit whitespace check (`git diff --check`) then rejects
the commit. `///` on the struct itself is fine.

## 4. Bindings

```sh
bun run bindings   # regenerates src/bindings.ts; never edit it by hand
```

The pre-commit hook also regenerates it, and CI fails if it is stale.

## 5. Frontend wrapper (`src/services/api.ts`, `src/types/index.ts`)

Add a method to `api` that unwraps the specta `Result` into a thrown error, the way the others
do:

```ts
async doThing(someArg: string): Promise<Thing> {
  return unwrap(await commands.doThing(someArg));
},
```

Commands that can't fail return the value directly (`return commands.nativeFeatures();`).
Re-export new types from `src/types/index.ts` and import them from `@/types` in components.

## 6. The Playwright mock (`e2e/fixtures/tauri-mock.ts`)

Playwright runs the UI without Tauri: `tauri-mock.ts` reimplements every command in memory.
Add a `case "do_thing":` to the `invoke` switch (snake_case name, camelCase `args`), with the
same error messages as the Rust side. Unknown commands throw `Unknown command`, which shows up
as a `>>> BROWSER ERROR` in the test output.

- The mock is a function serialized into the page: it can't use module-level constants or
  imports. Declare what it needs inside `installTauriMock`.
- Test seeds (`window.__SEED_…__`, set by the tests' `seed()`) are assigned *after* the mock
  installs, so read them lazily, when a command first needs them, not at install time.
- Document new seeds in the comment at the top of `installTauriMock`.

## 7. Tests and checks

- A Playwright test in `e2e/*.spec.ts` for the user-visible flow. Find elements by role and
  label (`getByRole("button", { name: "…" })`, `getByLabel("…", { exact: true })`); `getByLabel`
  matches substrings and `aria-label`s unless `exact`.
- Run `bun run verify` (everything CI runs).

If `bun run tauri dev` is running, Playwright reuses its Vite server, whose file watcher can
miss edits written by scripts. If tests see old code (`api.x is not a function`), `touch` the
edited files, or restart `tauri dev`.

## If the command talks to the relay

The relay (`sync-server/`) serves phones that update at different times, so its API only
grows: add endpoints and optional fields (`#[serde(default)]`), never rename, remove or
change the meaning of existing ones. Schema changes go in `Relay::open` as additive,
idempotent migrations (check `pragma_table_info` before `ALTER TABLE … ADD COLUMN`). Have the
app handle an old relay: a 404 on a new endpoint should become a clear message ("Update the
ezcount relay"). Deploy with the `deploy-relay` skill.
