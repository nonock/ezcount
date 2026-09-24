# ezcount

Split group expenses (Tricount-style) on Windows, Linux and Android. Built with Tauri 2, a Rust core and a React frontend.

## Architecture

- **Each group is a [Loro](https://loro.dev) CRDT document** (`src-tauri/src/doc.rs`). Devices edit offline, and their changes merge automatically. Every field is merged on its own, so concurrent edits to different fields of one expense both survive.
- **Local storage is SQLite** (`src-tauri/src/storage.rs`). It holds one Loro snapshot per group, and every write is atomic. Data that can't be read is reported in the app and left on disk, never discarded. A pre-SQLite `ezcount_data.json` is migrated once, then renamed to `ezcount_data.migrated.json`.
- **Sync goes through a relay** (`sync-server/`), which stores the encrypted updates of each group in order. Devices push local changes right after each edit and pull every 20 seconds, as well as when the app comes back to the foreground.
- **Members are soft-deleted.** A removed member's past expenses and open balance are kept, and they can still settle up.

## Development

```sh
bun install
bun run tauri dev          # desktop app
bun run tauri android dev  # Android (needs the Android SDK/NDK and Rust Android targets)
cargo test --manifest-path src-tauri/Cargo.toml   # includes an end-to-end sync test against a real relay
bun run test:e2e           # Playwright UI tests against a mocked backend
```

## Running the sync relay

```sh
cargo run --release --manifest-path sync-server/Cargo.toml
```

| Variable            | Default                | Meaning               |
| ------------------- | ---------------------- | --------------------- |
| `EZCOUNT_SYNC_ADDR` | `0.0.0.0:8787`         | Listen address        |
| `EZCOUNT_SYNC_DB`   | `ezcount-sync.sqlite3` | SQLite file for data  |

In the app, open a group and tap **Share**. Enter the relay URL and copy the invite code. Other members use **Join with Code** on the home screen.

- **Access:** anyone with a group's invite code can read and edit that group.
- **End-to-end encryption** (`src-tauri/src/crypto.rs`): the secret in the invite code never leaves the devices. Two keys are derived from it with HKDF-SHA256:
  - an auth token, the only thing sent to the relay (which stores its hash),
  - an encryption key, used with XChaCha20-Poly1305 to seal every update, with the group ID bound in.

  The relay operator can see group IDs, update sizes and timing, but not names, amounts or titles. Any change the relay makes to an update is detected.
- **Transport:** the relay speaks plain HTTP. Updates are already encrypted, but outside a trusted local network put it behind a TLS reverse proxy (Caddy, nginx) and share an `https://` URL anyway: TLS protects the auth token and the metadata.
- **Limitation:** removing a member doesn't revoke their access. A removed member who kept the invite code can still read and edit the group. Fixing that would need key rotation.
