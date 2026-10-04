# Running the sync relay

The relay (`sync-server/`) stores the encrypted updates of each group and account, in order, and serves the web version. It can't read any of it (see [security.md](security.md)).

```sh
cargo run --release --manifest-path sync-server/Cargo.toml
```

| Variable                      | Default                | Meaning                                                                    |
| ----------------------------- | ---------------------- | -------------------------------------------------------------------------- |
| `EZCOUNT_SYNC_ADDR`           | `0.0.0.0:8787`         | Listen address                                                             |
| `EZCOUNT_SYNC_DB`             | `ezcount-sync.sqlite3` | SQLite file for data                                                       |
| `EZCOUNT_ANDROID_PACKAGE`     | (none)                 | With the next one, the Android app that may open invite links directly    |
| `EZCOUNT_ANDROID_CERT_SHA256` | (none)                 | That app's signing certificate fingerprints (`AB:CD:…`), comma-separated |
| `EZCOUNT_CLIENT_IP_HEADER`    | (none)                 | Header with the client's address, set by a reverse proxy in front (`Fly-Client-IP`, `X-Forwarded-For`). Only behind one: clients could fake it otherwise |
| `EZCOUNT_MAX_STORAGE_MB`      | `1024`                 | Data stored in all; keep it under the disk's size                          |
| `EZCOUNT_MAX_DOCUMENT_MB`     | `50`                   | Data stored per group or account                                           |
| `EZCOUNT_WEB_DIR`             | (none)                 | The web version to serve at `/` (`bun run build:web` writes `sync-server/web`) |
| `EZCOUNT_ADMIN_TOKEN`         | none                   | Lets you read what people sent from "Suggest a feature": open `https://<relay>/feedback` and enter it (or `curl -H "Authorization: Bearer <token>" https://<relay>/v1/feedback`) |
| `EZCOUNT_RATES_URL`           | Frankfurter            | Where the relay gets the exchange rates the app suggests; `off` for none     |

To keep one client from filling the disk or locking others out, the relay also limits each client network (an IPv4 address, or an IPv6 /64) per hour: 50 MB of uploads, 30 new groups and 10 sign-ups. Failed logins (passwords or recovery keys) are limited per username: 5 per 15 minutes from one network, 50 from all networks together. Past a limit the relay answers 413 (group too big), 507 (relay full) or 429, and the app explains it. The counters are in memory and reset when the relay restarts.

Besides the sync API, the relay serves `/join`, the page invite links open, with both Android variables set, `/.well-known/assetlinks.json` for Android App Links, and with `EZCOUNT_WEB_DIR`, the web version at `/`.

The relay itself speaks plain HTTP, so put it behind a TLS reverse proxy (Fly.io, Caddy): the app requires `https://` except for a relay on the same device or a private network.

## With Docker

Data lives in the `ezcount-relay` volume; `GET /health` answers `ok`. Run `bun run build:web` first to include the web version:

```sh
docker build -t ezcount-relay sync-server
docker run -d --name ezcount-relay --restart unless-stopped -p 8787:8787 -v ezcount-relay:/data ezcount-relay
```

## On Fly.io

[Fly.io](https://fly.io) runs the relay's Docker image on a small machine in Paris with a persistent volume for its database, and gives it an HTTPS address, so there is no server to maintain. It costs a few dollars a month. `sync-server/fly.toml` holds the setup: one machine that stops when idle and starts on the next request, and daily volume snapshots kept 14 days.

One-time setup:

1. Create an account on fly.io (it needs a card) and install [flyctl](https://fly.io/docs/flyctl/install/).
2. Create the app and deploy it once by hand. The app name is global on Fly.io: if `ezcount-relay` is taken, pick another and put it in `sync-server/fly.toml`.

   ```sh
   fly auth login
   fly apps create ezcount-relay
   bun run build:web            # optional: the web version, served at /
   cd sync-server
   fly deploy --ha=false        # creates the volume on first deploy
   curl https://ezcount-relay.fly.dev/health   # → ok
   ```

   Always keep `--ha=false`: each machine gets its own volume, so a second machine would hold a second, separate database.
3. Let releases deploy it: run `fly tokens create deploy -a ezcount-relay`, then in GitHub go to Settings → Environments, create `production`, and add the token as the secret `FLY_API_TOKEN`. Optionally add yourself as a required reviewer there, so each deploy waits for your approval.
4. If you picked another app name, update the default server in `src/components/auth/AuthScreen.svelte` and the App Links host in `src-tauri/tauri.conf.json`.

Accounts and groups don't move to a new relay address: each group remembers the URL of the relay it syncs through. Create your account on the new relay and your groups there; moving existing ones would need a "relay moved" feature in the app. Pick the final address before real use.

Useful commands: `fly logs`, `fly status`, `fly volumes snapshots list <volume id>` (restore one with `fly volumes create relay_data --snapshot-id <id>`).

## On your own server, with HTTPS

`deploy/` runs the relay behind [Caddy](https://caddyserver.com), which gets and renews a Let's Encrypt certificate automatically. You need a server with ports 80 and 443 open and a domain whose DNS record points at it.

```sh
cd deploy
cp .env.example .env        # set EZCOUNT_DOMAIN=ezcount.example.com
docker compose up -d --build
curl https://ezcount.example.com/health   # → ok
```

Then use `https://ezcount.example.com` as the server in the app (**Change** on the login screen). The relay's data is in the `ezcount_relay-data` volume; back it up to keep your accounts and groups.

## If the relay loses its data

If the relay loses its data, or you move to a new one at the same address, devices notice (each relay database has a random ID) and upload their groups and account lists again, so nothing is lost while one device still has them. Logins are not restored: the relay only knows your password's hash, so after such a loss, devices that are still logged in keep working but new ones can't log in. Back up the database.
