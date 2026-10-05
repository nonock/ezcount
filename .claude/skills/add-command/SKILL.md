---
name: add-command
description: Checklist for adding or changing a Tauri command (Rust function called from the Svelte frontend) in ezcount, end to end - Rust, specta types, bindings, api.ts, the Playwright mock and tests. Use for any new backend feature the UI calls, or when a command's arguments or result change.
---

# Add or change a Tauri command

A command crosses seven places. Missing one usually still compiles but breaks the UI or the
Playwright tests, so go through all of them.

## 1. Rust logic

Put the real work in the core crate (`core/src/`), in the module it belongs to (`sync/` for
accounts and sync, `doc/` for group data, `storage.rs` for persistence), as a plain function
returning `Res<T>` (= `Result<T, String>`). The core crate must not depend on Tauri. Error
strings are shown to users as is: write them as full sentences ("Your current password is
wrong"), never debug output.

Test it there: unit tests in the module's `tests.rs` (`doc/tests/expenses.rs` for
`doc/expenses.rs`, `csv_file/tests.rs`…), or in `sync/end_to_end/` when it talks to the relay
(it starts a real relay in-process; see the `Device` helper and `start_relay` in
`sync/end_to_end.rs`). Run them with `cargo test --manifest-path core/Cargo.toml`.

## 2. The command (`core/src/api.rs`, then `src-tauri/src/commands/`)

Write what the command does as a function in `core/src/api.rs`, taking `&AppState` first, and add
its name to the `match` in `api::invoke` (`core/src/api/invoke.rs`: the web version calls
commands through it, with camelCase JSON arguments: `arg(&args, "someArg")`).
`api::tests::every_bound_command_is_dispatched` fails for a command in `bindings.ts` that
`invoke` doesn't know. Then the Tauri command, in the module of `src-tauri/src/commands/` it
belongs to (`groups`, `expenses`, `account`, or `native` for what only the app can do), is a
one-line wrapper:

```rust
/// One line on what it does; this becomes the TS doc comment.
#[tauri::command]
#[specta::specta]
pub(crate) async fn do_thing(state: State<'_, AppState>, some_arg: String) -> Result<Thing, String> {
    api::do_thing(&state, &some_arg).await
}
```

- `async fn` for anything doing I/O or taking more than a few milliseconds: sync commands run
  on the main thread and freeze the window.
- Register it in `create_specta_builder()`'s `collect_commands![…]` list (`src-tauri/src/lib.rs`),
  with its module (`commands::groups::do_thing`). Unregistered commands compile fine and fail
  at runtime.
- Arguments are camelCased on the TS side (`some_arg` → `someArg`).

## 3. Types (`core/src/models.rs`)

New structs need `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]` and
must be imported where the command is (`use ezcount_core::models::{…}`).

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

## 6. The Playwright mock (`e2e/fixtures/mock/`)

Playwright runs the UI without Tauri: the mock reimplements every command in memory. Add a
`do_thing(args) { … }` to the commands of its subject in `e2e/fixtures/mock/commands/`
(`groups`, `expenses`, `account`, or `device` for what only the app does; snake_case name,
camelCase `args`), with the same error messages as the Rust side. Unknown commands throw
`Unknown command`, which shows up as a `>>> BROWSER ERROR` in the test output.

- What the mock remembers (the account, its groups) is `state` in `mock/state.ts`; the rules
  mirrored from the core are in `mock/engine.ts`, `recurring.ts` and `checks.ts`.
- Playwright can only add one script to a page, so `e2e/fixtures/build-mock.ts` (the
  configuration's `globalSetup`) bundles `mock/` with Vite before the tests run; the tests
  add it with `page.addInitScript(installTauriMock)`. The mock runs in the page: it can't
  import from Node, and the tests can only import `mock/constants.ts` and the types from it.
- Test seeds (`window.__SEED_…__`, set by the tests' `seed()`) are assigned *after* the mock
  installs, so read them lazily, when a command first needs them, not at install time.
- Document new seeds in the comment on `installTauriMock` (`e2e/fixtures/tauri-mock.ts`).

## 7. Tests and checks

- Unit tests (Vitest, `bun run test:unit`) for what the interface works out itself: a
  `*.test.ts` next to the module or the component, with the fixtures of `src/test/`.
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
