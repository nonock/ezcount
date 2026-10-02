---
name: deploy-relay
description: Deploy the ezcount sync relay (sync-server/) to Fly.io by hand and check it works - pre-deploy checks, backward compatibility, migrations, flyctl, health and endpoint probes, logs. Use after changing sync-server/ when the phone or app needs the new relay before the next release, or when the user asks to deploy the relay.
---

# Deploy the relay to Fly.io

The production relay is the Fly.io app `ezcount-relay` (`https://ezcount-relay.fly.dev`,
Paris), configured in `sync-server/fly.toml`. Tagged releases deploy it automatically
(`bun run release <x.y.z>`, see CLAUDE.md), once the `FLY_API_TOKEN` secret is set in the
GitHub `production` environment. Deploy by hand only when the app needs relay changes before
the next release.

Deploying changes a live service holding everyone's accounts: confirm with the user first
unless they asked for it.

## 1. Before deploying

```sh
cargo clippy --manifest-path sync-server/Cargo.toml --all-targets -- -D warnings
bun run test:rust          # the app's end-to-end tests run against the relay too
```

Check the change is backward compatible: apps already on phones must keep working. Only add
endpoints and optional request fields; never rename, remove or change existing ones.

Migrations run at startup in `Relay::open`, against the production database. They must be
additive and idempotent: check before altering (`pragma_table_info`), `ADD COLUMN` rather
than rebuilding tables, never drop data. A migration that fails keeps the relay from starting.

## 2. Deploy

`flyctl` is installed at `%USERPROFILE%\.fly\bin\flyctl.exe` (in Git Bash:
`"$USERPROFILE/.fly/bin/flyctl.exe"`), and may not be on `PATH`.

```sh
cd sync-server
"$USERPROFILE/.fly/bin/flyctl.exe" auth whoami
"$USERPROFILE/.fly/bin/flyctl.exe" deploy --remote-only --ha=false --yes
```

- **Always `--ha=false`.** The data is one SQLite file on one machine's volume; a second
  machine would get its own, empty volume and split the data.
- `--remote-only` builds on Fly's builders; no local Docker needed. `fly.toml` builds the
  Dockerfile's `fly` stage, which runs as root because Fly volumes are root-owned.
- If `auth whoami` fails, the user must run `fly auth login` in their own terminal: it needs
  an interactive one. Don't try to work around it.

## 3. Check it

```sh
curl -sS https://ezcount-relay.fly.dev/health                  # → ok
# Each new endpoint must exist: a 4xx from the handler, not a 404.
curl -sS -o /dev/null -w "%{http_code}\n" -H 'content-type: application/json' \
  -d '{"username":"nobody-here"}' https://ezcount-relay.fly.dev/v1/<new-endpoint>
"$USERPROFILE/.fly/bin/flyctl.exe" status
"$USERPROFILE/.fly/bin/flyctl.exe" logs --no-tail | tail -20
```

The machine stops when idle and starts on the next request, so a failed health check logged
right at boot is normal; what matters is that `/health` answers now. Look for migration errors
or panics after the latest "listening" line.

## Also good to know

- Daily volume snapshots are kept 14 days: `flyctl volumes list`, then
  `flyctl volumes snapshots list <volume id>`. Restoring means a new volume from a snapshot;
  confirm with the user before touching volumes.
- `fly.toml`'s `[env]` holds the Android App Links settings (package and signing-key
  fingerprint). Update the fingerprint when the app gets a real signing key.
- The address is baked into the app (`DEFAULT_SERVER` in `src/components/auth/AuthScreen.svelte`,
  the App Links host in `src-tauri/tauri.conf.json`), and every group remembers its relay URL.
  Changing the app name or domain strands existing accounts and groups.
