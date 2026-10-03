# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

ezcount splits group expenses (Tricount-style) on Windows, Linux, Android and the web: a Rust core crate without UI (`core/`) wrapped in a Tauri 2 app (`src-tauri/`) and, for the browser, a WebAssembly crate (`web/`), a Svelte 5 + Tailwind v4 + shadcn-svelte frontend (`src/`), and a separate sync relay crate (`sync-server/`). Package manager and script runner is **Bun**. `docs/` has the detail: `development.md` (toolchains, signing key, releases), `relay.md` (running and hosting the relay) and `security.md` (invites, encryption, accounts). `CHANGELOG.md` lists what changed for users: add a line under "Unreleased" with each user-visible change.

Project skills (`.claude/skills/`): `add-command` (a Tauri command end to end, with its traps), `phone-test` (build, install and drive the app on the USB-connected Android phone), `deploy-relay` (manual Fly.io deploy and checks).

## Commands

```sh
bun install                 # also installs lefthook git hooks
bun run tauri dev           # desktop app (starts Vite on :1420)
bun run relay               # local sync relay on :8787
bun run verify              # everything CI runs
bun run test:rust           # cargo test for core and sync-server
bun run test:e2e            # Playwright against Vite with a mocked Tauri backend
bun run dev:web             # web version on :1420 (needs `bun run relay`)
bun run build:web           # web version into sync-server/web (WebAssembly + Vite --mode web)
bun run test:web            # Playwright against the built web version served by a real relay
bun run lint:wasm           # clippy for wasm32 on core and web (the cfg(wasm) code)
bun run lint:rust           # clippy -D warnings, all three crates
bun run check               # biome check --write (format + lint)
bun run typecheck
bun run bindings            # regenerate src/bindings.ts from Rust commands
bun run release 0.2.0       # bump versions, commit and tag (see Releases below)
```

Single tests:

```sh
cargo test --manifest-path core/Cargo.toml <name_filter>
cargo test --manifest-path sync-server/Cargo.toml <name_filter>
bunx playwright test e2e/group-journey.spec.ts -g "<test title>"
```

- `tauri::generate_context!` embeds `dist/`, so on a fresh checkout run `bun run build` before `cargo` commands on `src-tauri` (CI does this).
- Four independent Cargo projects (no workspace), so always pass `--manifest-path`: `core` (all the logic and its tests, no Tauri), `src-tauri` (the commands, background sync loop and native bits; depends on `core` by path), `web` (the browser build, wasm32 only: natively it is an empty crate) and `sync-server`. `core` has `sync-server` as a dev-dependency, so relay changes can break core tests. Keep Tauri out of `core`: `web` builds it for `wasm32-unknown-unknown` too, where native-only code (blocking threads, reqwest's client options) sits behind `#[cfg(target_family = "wasm")]` (check with `bun run lint:wasm`). Building for wasm compiles SQLite's C with clang (`scripts/build-wasm.ts` finds it, or the NDK's through `NDK_HOME`) and needs the wasm-bindgen CLI at `web/Cargo.lock`'s version.

## Conventions enforced by hooks and CI

- **Commit subjects must be Conventional Commits** (`feat(ui): …`, `fix: …`; checked by `scripts/check-commit-msg.ts`).
- **`src/bindings.ts` is generated** by tauri-specta (`src-tauri/src/bin/export_bindings.rs`). Never edit it by hand; regenerate after changing any `#[tauri::command]` or a type in `core/src/models.rs`. The pre-commit hook does this automatically and CI fails if it is stale.
- **Tauri Rust crates and npm packages must share major.minor** (`tauri = "2.11"` / `~2.11`). `bun run check:tauri` compares the lock files; see docs/development.md "Upgrading Tauri" to bump.
- Biome (2-space, 100 cols) lints everything and formats TS/CSS/JSON; Prettier (`prettier-plugin-svelte`) formats `.svelte` files, because Biome's Svelte formatter breaks templates (it once rewrote `{@const x = …}` into invalid code). Both skip `src/bindings.ts` and `src/components/ui/**` (shadcn-svelte-generated; add components with `bunx shadcn-svelte@latest add <name>`, with `--overwrite` when it asks about existing files). `bunfig.toml` refuses packages published less than 7 days ago, so pin to an older version when the CLI writes a newer one into package.json.
- pre-push runs Rust tests and Playwright. Skip hooks once with `LEFTHOOK=0`.

## Releases and relay hosting

- `bun run release <x.y.z>` (`scripts/release.ts`, must be on a clean `main`) sets the version in `package.json`, `tauri.conf.json` and both `Cargo.toml`s, turns the changelog's "Unreleased" into that version's section (and refuses when it's empty), commits `chore(release): vX.Y.Z` and tags it; `git push --follow-tags` publishes. No pre-release suffixes (Android derives its versionCode from the version).
- The tag runs `.github/workflows/release.yml`: version check (`release.ts --check`), then `packages.yml` (called as a reusable workflow, with LTO on for tags) and the Fly.io relay deploy in parallel, then the GitHub release, with that changelog section as its notes. `packages.yml` skips `chore(release)` pushes to `main`, since the tag builds them.
- Android APKs: CI signs with the private release key from GitHub secrets (`ANDROID_RELEASE_KEYSTORE`, `…_PASSWORD`) and fails without it; local builds use the public test key `gen/android/app/debug.keystore` unless `EZCOUNT_RELEASE_KEYSTORE`/`…_PASSWORD` are set. Test-key APKs must never go to other people. See docs/development.md "Signing key".
- The relay runs on Fly.io (`sync-server/fly.toml`, app `ezcount-relay`, Paris) from the Dockerfile's root `fly` stage (Fly volumes are root-owned; the default image stays non-root). Always deploy with `--ha=false`: the data is one SQLite file on one machine's volume.
- **The relay API must stay backward compatible**: phones update at different times, so only add endpoints/fields, never change or remove them.
- Groups remember their relay URL, so changing the relay address strands existing accounts and groups.

## Architecture

**Data model: every group is a Loro CRDT document** (`core/src/doc.rs`, schema in its module doc). The `LoroDoc` is the source of truth; `models::Group` is a read-only projection produced by `doc::read_group`. Every field is its own LWW register; `splits` is stored as one plain value so an allocation is replaced atomically. Plain values are built by hand, not via serde. Money is `i64` cents; splits are integer `shares`. Members are soft-deleted (`removed`) and keep their history and balances.

**Command flow:** Svelte component (or an action in `src/lib/actions.ts`) → `src/services/api.ts` (unwraps specta `Result` into thrown errors) → `commands.*` in `bindings.ts` → `#[tauri::command]` in `src-tauri/src/lib.rs`, a one-line wrapper around the same-named function in `core/src/api.rs` → `AppState::mutate` (`core/src/lib.rs`), which applies a closure to the group's `LoroDoc` via `Store::update`, persists, and wakes background sync if the group is shared. Commands return the updated `Group`. `engine.rs` computes balances and settlement transfers from a `Group`.

**Frontend** (`src/`): `App.svelte` wires startup, backend events and the layout. App state lives in rune modules under `src/lib/state/` (`session`, `groups` for the list and the open group, `navigation` for the open group as a history entry so Back leaves it, `dialogs`, `confirm`), and actions that ask for confirmation or cross screens are in `src/lib/actions.ts`; components read and write that state directly rather than through props. bits-ui differs from Radix in ways that matter: it keeps inactive tab panels in the DOM (so `GroupPage` renders only the open tab), it doesn't hide the page behind a modal (so `App` sets `aria-hidden` on `#root` while one is open; dialogs and the toaster live outside it), and select triggers are buttons, not comboboxes. The look follows daisyUI 5's default themes without the daisyUI package: colors and its three radii (`--radius-field`/`-box`/`-selector`) are tokens in `styles.css`, and the button, field, checkbox, dialog, card, item and menu components in `src/components/ui/` are edited to match (raised semibold buttons, with `outline` a flat bordered one for secondary actions, 2.5rem controls, no dialog footer band). Text has two weights (`font-medium` is mapped to 600), surfaces have a hairline or a shadow but not both, and soft fills are their own tokens (`--positive-soft`, `--tone-fill`) rather than a color at low opacity. Re-adding one of those with `--overwrite` drops that styling. Members get a color by their position in the group (`memberTone` in `src/lib/tones.ts`, `.tone-N` in `styles.css`), used on their avatars and name badges.

**Languages** (`src/lib/i18n/`): no text is written in components. `t("key", …args)` returns it in the app's language (the device's, or the one picked in the menu and kept in `localStorage`); `en.ts` holds the English and defines the keys, `fr.ts` must have them all (the type checks it), and a message is a function when it takes values or a count. The core's own text (errors, edit summaries, password advice) stays English in Rust and is translated by pattern in `backend.ts`, through `errorMessage` / `backendText`: add a pattern there when adding an error users can meet. Amounts and dates go through `src/utils/formatters.ts` (`Intl`, in the app's language); `Amount.svelte` shows the cents smaller. Playwright runs in English, except `e2e/language.spec.ts`.

**Web version** (`web/`, `src/web/`): the same Svelte app, built with `vite --mode web` (only that mode imports `src/web/`). `src/web/install.ts` installs a stand-in `window.__TAURI_INTERNALS__` (like the Playwright mock): commands go to a Web Worker (`worker.ts`) running the `web` crate, whose `invoke` calls `core::api::invoke` (command name + camelCase JSON args, as Tauri sends them; `api::tests` checks every command in `bindings.ts` is routed); `native_features`/`share_text` and plugin calls are answered in JS. The database is SQLite in OPFS (sqlite-wasm-vfs's sahpool, dedicated workers only), so one tab runs at a time (a Web Lock); a second tab forwards its invite over a BroadcastChannel. Invites arrive as `/#v=2&g=…&k=…` (the relay's join page links there) and are removed from the URL once read. The relay serves the build from `EZCOUNT_WEB_DIR` under a strict CSP, which is why the theme script is `public/theme.js` rather than inline. The web version's default relay is the page's origin (`bun run dev:web` proxies `/v1` to `:8787`).

**Storage** (`storage.rs`): SQLite holding one Loro snapshot per group plus sync metadata and the session. Every write commits to SQLite before in-memory state changes. Unreadable data is surfaced as warnings (`get_storage_warnings`) and left on disk, never dropped. A legacy `ezcount_data.json` is migrated once.

**Splits and currencies:** a split is `shares` (parts) or `fixed_cents` (a set amount); the parts divide what the set amounts leave (`engine::owed`, mirrored in `src/lib/split.ts` and the Playwright mock). An expense paid in another currency keeps `amount_cents` in the group's currency and adds `original` (currency, amount, rate as typed); its fixed amounts are in that currency. Older app versions only read `shares`, so `doc::splits_value` stores there shares that give the same amounts, with the real ones in `parts`: keep that when changing how splits are stored.

**Exchange rate suggestions:** picking another currency in the expense form asks the account's relay for that day's rate (`suggest_exchange_rate` → `GET /v1/rates/{from}/{to}?date=`, `sync-server/src/rates.rs`), which asks Frankfurter (`EZCOUNT_RATES_URL`, `off` for none) and caches answers. It only fills the form: a rate the user typed, or one saved with an expense, is never replaced, and no saved expense is re-priced. Without an answer (old relay, offline) the form falls back to the rate last used in the group.

**CSV** (`csv_file.rs`): `export_group_csv` / `import_group_csv` turn a group into a file and back (a line per expense, a column per person with the amount owed, and a `Split` column saying how: parts, set amounts). Without a usable `Split`, importing works the split out from the amounts (`split_from_amounts`), accepting a cent of difference for files from other apps. `scripts/tricount-to-csv.ts` writes that format from a Tricount share link.

**Accounts** (`account.rs`): the account is another encrypted Loro doc (group list with invite secrets, plus per-group "which participant am I" identities), stored and synced exactly like a group under the account id but never listed as a group.

**Login links** (`sync::create_login_link` / `log_in_with_link`, `sync-server/src/links.rs`): "Connect a device" in the account menu asks for the password, then shows a QR code that the login screen of a phone scans (`ezcount://login?server=…&code=…`). The account's key waits on the relay encrypted with that code (`crypto::LinkKeys`: HKDF gives the ticket the relay stores it under and the key, so the relay can't read it), in memory, to be fetched once within `Limits::link_lifetime` (2 minutes). Only devices that can scan offer it (`nativeFeatures.scan`), so not the web version.

**Sync** (`sync.rs`, `sync-server/`): the relay is dumb. It stores opaque, ordered, encrypted updates per document and serves them by sequence number. Clients push ops the server lacks and pull after their last imported seq; Loro merges. The app's loop (`src-tauri/src/background.rs`) runs `sync::sync_all` after each edit (via `sync_wakeup`) and every 20 s; `reconcile` adds/removes local groups to match the account doc. it emits `sync-updated` / `account-updated` events that `App.svelte` listens for to refresh. If a relay's random DB id changes, clients re-upload everything.

**Crypto** (`crypto.rs`): invite code = server URL + group id + secret. HKDF-SHA256 derives an auth token (only thing the relay sees, stored hashed) and an XChaCha20-Poly1305 key bound to the group id. Passwords go through Argon2id into a login token and a key wrapping the random account key; recovery keys (HKDF, no stretching) give a second token and a second wrapped copy (`CredentialKeys`). Each recovery key works once: the relay's `/v1/accounts/credentials` requires replacing it when it's the proof.

**Tests:**
- Rust tests live in `core`, in `#[cfg(test)]` modules; `sync.rs` tests spin up a real `ezcount_sync_server` relay in-process for end-to-end sync.
- `e2e-web/` (`bun run test:web`, `playwright.web.config.ts`) runs the built web version served by a real relay on a fresh database: each browser context is a device, and console errors (CSP violations included) fail the test.
- Playwright tests don't run Tauri. `e2e/fixtures/tauri-mock.ts` installs an in-memory fake `__TAURI_INTERNALS__` that reimplements the commands (including balance/settlement logic mirroring `engine.rs`) and is seeded through `window.__SEED_GROUPS__` etc. **When changing a command's signature or behaviour, update the mock too.**
