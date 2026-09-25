# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

ezcount splits group expenses (Tricount-style) on Windows, Linux and Android: Tauri 2 app with a Rust core (`src-tauri/`), a React 19 + Tailwind v4 + shadcn/ui frontend (`src/`), and a separate sync relay crate (`sync-server/`). Package manager and script runner is **Bun**. README.md covers deployment, the Android toolchain and the security model in detail.

## Commands

```sh
bun install                 # also installs lefthook git hooks
bun run tauri dev           # desktop app (starts Vite on :1420)
bun run relay               # local sync relay on :8787
bun run verify              # everything CI runs
bun run test:rust           # cargo test for src-tauri and sync-server
bun run test:e2e            # Playwright against Vite with a mocked Tauri backend
bun run lint:rust           # clippy -D warnings, both crates
bun run check               # biome check --write (format + lint)
bun run typecheck
bun run bindings            # regenerate src/bindings.ts from Rust commands
```

Single tests:

```sh
cargo test --manifest-path src-tauri/Cargo.toml <name_filter>
cargo test --manifest-path sync-server/Cargo.toml <name_filter>
bunx playwright test e2e/group-journey.spec.ts -g "<test title>"
```

- `tauri::generate_context!` embeds `dist/`, so on a fresh checkout run `bun run build` before `cargo` commands on `src-tauri` (CI does this).
- Two independent Cargo projects (no workspace): always pass `--manifest-path`. `src-tauri` has `sync-server` as a dev-dependency, so relay changes can break app tests.

## Conventions enforced by hooks and CI

- **Commit subjects must be Conventional Commits** (`feat(ui): …`, `fix: …`; checked by `scripts/check-commit-msg.ts`).
- **`src/bindings.ts` is generated** by tauri-specta (`src-tauri/src/bin/export_bindings.rs`). Never edit it by hand; regenerate after changing any `#[tauri::command]` or a type in `models.rs`. The pre-commit hook does this automatically and CI fails if it is stale.
- **Tauri Rust crates and npm packages must share major.minor** (`tauri = "2.11"` / `~2.11`). `bun run check:tauri` compares the lock files; see README "Upgrading Tauri" to bump.
- Biome (2-space, 100 cols) ignores `src/bindings.ts` and `src/components/ui/**` (shadcn-generated; add components with `bunx shadcn@latest add <name>`).
- pre-push runs Rust tests and Playwright. Skip hooks once with `LEFTHOOK=0`.

## Architecture

**Data model: every group is a Loro CRDT document** (`src-tauri/src/doc.rs`, schema in its module doc). The `LoroDoc` is the source of truth; `models::Group` is a read-only projection produced by `doc::read_group`. Every field is its own LWW register; `splits` is stored as one plain value so an allocation is replaced atomically. Plain values are built by hand, not via serde. Money is `i64` cents; splits are integer `shares`. Members are soft-deleted (`removed`) and keep their history and balances.

**Command flow:** React component → `src/services/api.ts` (unwraps specta `Result` into thrown errors) → `commands.*` in `bindings.ts` → `#[tauri::command]` in `src-tauri/src/lib.rs` → `AppState::mutate`, which applies a closure to the group's `LoroDoc` via `Store::update`, persists, and wakes background sync if the group is shared. Commands return the updated `Group`. `engine.rs` computes balances and settlement transfers from a `Group`.

**Storage** (`storage.rs`): SQLite holding one Loro snapshot per group plus sync metadata and the session. Every write commits to SQLite before in-memory state changes. Unreadable data is surfaced as warnings (`get_storage_warnings`) and left on disk, never dropped. A legacy `ezcount_data.json` is migrated once.

**Accounts** (`account.rs`): the account is another encrypted Loro doc (group list with invite secrets, plus per-group "which participant am I" identities), stored and synced exactly like a group under the account id but never listed as a group.

**Sync** (`sync.rs`, `sync-server/`): the relay is dumb. It stores opaque, ordered, encrypted updates per document and serves them by sequence number. Clients push ops the server lacks and pull after their last imported seq; Loro merges. `spawn_background_sync` pushes after each edit (via `sync_wakeup`) and polls every 20 s; `reconcile` adds/removes local groups to match the account doc. Rust emits `sync-updated` / `account-updated` events that `App.tsx` listens for to refresh. If a relay's random DB id changes, clients re-upload everything.

**Crypto** (`crypto.rs`): invite code = server URL + group id + secret. HKDF-SHA256 derives an auth token (only thing the relay sees, stored hashed) and an XChaCha20-Poly1305 key bound to the group id. Passwords go through Argon2id into a login token and a key wrapping the random account key.

**Tests:**
- Rust unit tests live in `#[cfg(test)]` modules; `sync.rs` tests spin up a real `ezcount_sync_server` relay in-process for end-to-end sync.
- Playwright tests don't run Tauri. `e2e/fixtures/tauri-mock.ts` installs an in-memory fake `__TAURI_INTERNALS__` that reimplements the commands (including balance/settlement logic mirroring `engine.rs`) and is seeded through `window.__SEED_GROUPS__` etc. **When changing a command's signature or behaviour, update the mock too.**
