# ezcount

Split group expenses (Tricount-style) on Windows, Linux, Android and in the browser. Groups work offline, sync between everyone's devices through a relay that can't read them, and the whole thing can be self-hosted.

- **Expenses and payments** split in parts or set amounts, paid in any currency, with who owes whom worked out in the fewest transfers.
- **Accounts**: log in with a username and password and find your groups on every device; a phone logs in by scanning a QR code shown by another device. Your name and picture follow you into your groups.
- **Invites** by link or QR code. Anyone with the link can read and edit the group.
- **End-to-end encrypted**: the relay stores only ciphertext.
- **CSV** import and export, and `bun scripts/tricount-to-csv.ts <share link>` to move a group over from Tricount.
- **English and French**, light and dark.

Packages (Android APK, Linux `.deb` and `.AppImage`) are on the [releases page](https://github.com/nonock/ezcount/releases); the web version is at <https://ezcount-relay.fly.dev>. What changed in each version is in [CHANGELOG.md](CHANGELOG.md).

## How it's built

A Rust core (`core/`) holds all the logic. Tauri 2 (`src-tauri/`) wraps it for desktop and Android, and the browser runs the same core as WebAssembly (`web/`). The interface is Svelte 5 (`src/`).

- **Each group is a [Loro](https://loro.dev) CRDT document.** Devices edit offline and their changes merge automatically, field by field.
- **Local storage is SQLite**, one snapshot per group. Data that can't be read is reported in the app and left on disk, never discarded.
- **Sync goes through a relay** (`sync-server/`), which stores each group's encrypted updates in order. Devices push after each edit and pull every 20 seconds.
- **The account is one more encrypted document**, listing your groups and which member you are in each.

[CLAUDE.md](CLAUDE.md) describes the architecture in more detail.

## Getting started

Needs [Bun](https://bun.sh) and Rust.

```sh
bun install
bun run relay              # local sync relay on :8787
bun run tauri dev          # desktop app
bun run verify             # everything CI runs
```

## Documentation

- [docs/development.md](docs/development.md): all commands, git hooks, test builds, the Android toolchain and signing key, releases, upgrading Tauri, building the web version.
- [docs/relay.md](docs/relay.md): running the relay, its settings and limits, hosting it on Fly.io or your own server.
- [docs/security.md](docs/security.md): how invites, encryption, accounts and recovery keys work, and their limits.
