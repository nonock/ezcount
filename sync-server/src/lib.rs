//! Relay server for ezcount group sync.
//!
//! Stores updates per group, in arrival order, and hands them back by sequence number.
//! Updates are end-to-end encrypted by the devices, so the relay cannot read them; merging
//! happens on the devices.
//!
//! Access control: the first push to a group registers the SHA-256 hash of its bearer token.
//! Every later request for that group must present the same token. The token is derived from
//! the group secret on the devices and reveals nothing about the encryption key. Group IDs
//! are random UUIDs, so they cannot be guessed and claimed before their creator.
//!
//! - `POST /v1/groups/{id}/updates`: body is one update; returns `{ "seq": n, "relay_id" }`
//! - `GET  /v1/groups/{id}/updates?after=n`: returns
//!   `{ "updates": [{ "seq", "data" }], "has_more", "relay_id" }` with `data` in standard base64
//!
//! `relay_id` is random and fixed for the lifetime of the database. When it changes (the
//! database was reset or replaced), sequence numbers start over, so devices re-upload
//! everything and read again from the start.
//!
//! Accounts let one person use several devices. Passwords never reach the relay: devices
//! derive a login token from the password (Argon2id) and upload their account key encrypted
//! with a second key derived from the same password. The relay stores the token's hash and
//! that encrypted blob, and hands the blob back on login. The account itself is an encrypted
//! document synced through the group endpoints above, registered at sign-up.
//!
//! - `POST /v1/accounts`: `{ username, login_token, wrapped_key, account_id, doc_token }`;
//!   409 if the username is taken
//! - `POST /v1/accounts/login`: `{ username, login_token }` returns
//!   `{ account_id, wrapped_key }`; 401 on a wrong username or password, 429 after
//!   repeated failures
//!
//! A recovery key works like a second password, for when the password is forgotten: the device
//! derives a recovery token from it and a key that encrypts the account key a second time. The
//! relay stores the token's hash and that second blob.
//!
//! - `POST /v1/accounts` also takes `recovery_token` and `recovery_wrapped_key`, and answers
//!   `{ "recovery": true }` (relays before recovery keys answer with an empty body)
//! - `POST /v1/accounts/recover`: `{ username, recovery_token }` returns
//!   `{ account_id, wrapped_key }` with the recovery blob; errors as for login
//! - `POST /v1/accounts/credentials`: proves the account with `login_token` or
//!   `recovery_token`, and replaces the password (`new_login_token`, `new_wrapped_key`), the
//!   recovery key (`new_recovery_token`, `new_recovery_wrapped_key`), or both. A recovery key
//!   works once: proving with it requires replacing it. 204, or errors as for login
//!
//! A logged-in device can log another one in without the password being typed there (see
//! `links`): it leaves the account's key here, encrypted with a code only the two devices see,
//! for the other one to fetch once, within two minutes.
//!
//! - `POST /v1/accounts/links`: `{ username, login_token, ticket, data }` returns
//!   `{ "expires_in": 120 }`; errors as for login
//! - `POST /v1/accounts/links/claim`: `{ ticket }` returns `{ data }` and forgets it; 410 once
//!   used or expired
//!
//! - `GET /v1/rates/{from}/{to}?date=YYYY-MM-DD`: the exchange rate between two currencies
//!   that day (the latest without a date, or when that day has none), as
//!   `{ "rate": "0.85856", "date": "2026-08-29" }`, from a public rate service
//!   ([`Settings::rates_url`]). 404 when there is none: no service configured, or a currency
//!   it doesn't know; 503 when the service can't be reached
//!
//! Invite links point at the relay: `GET /join` is a small page that opens the app with the
//! invite in the link's fragment, which never reaches the relay. With [`AndroidApp`]
//! configured, `GET /.well-known/assetlinks.json` lets that Android app open invite links
//! directly (Android App Links).
//!
//! With [`Settings::web_dir`] set, the relay also serves the web version of the app (`GET /`, its
//! files, under a strict Content-Security-Policy), on the same origin as the API so it needs
//! no CORS, and the join page offers to open invites there.
//!
//! [`Limits`] keep one client from filling the disk or locking other people out: per-document
//! and total sizes (413 and 507 past them), and per-client rates of uploads, new documents,
//! sign-ups and failed logins (429).

mod accounts;
mod error;
mod feedback;
mod limits;
mod links;
mod rates;
mod updates;
mod web;

pub(crate) use error::ApiError;
pub use limits::Limits;
pub use rates::DEFAULT_RATES_URL;

use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderName, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use limits::Counters;
use rusqlite::Connection;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

/// How a relay runs. `Default`: no Android app, client addresses taken from the connection,
/// default limits.
#[derive(Clone, Debug, Default)]
pub struct Settings {
    pub android_app: Option<AndroidApp>,
    /// Header holding the client's address, set by a reverse proxy in front of the relay
    /// (`Fly-Client-IP` on Fly.io, `X-Forwarded-For` behind Caddy). Only set it behind such a
    /// proxy: otherwise clients pick their own address and slip past the limits.
    pub client_ip_header: Option<HeaderName>,
    pub limits: Limits,
    /// The web version to serve (`bun run build:web` writes it to `sync-server/web`). Ignored
    /// without an `index.html` in it.
    pub web_dir: Option<PathBuf>,
    /// The rate service behind `/v1/rates` ([`DEFAULT_RATES_URL`] is one). Without it the
    /// relay suggests no exchange rates, and makes no requests of its own.
    pub rates_url: Option<String>,
    /// Lets whoever has it read what people sent from the app's feedback form
    /// (`GET /v1/feedback`). Without it the messages are kept, and not served.
    pub admin_token: Option<String>,
}

pub struct Relay {
    db: Mutex<Connection>,
    /// Rate-limit counters, in memory only; see `limits`.
    counters: Counters,
    /// Bytes stored for all documents, kept in step with the `groups.bytes` column.
    stored_bytes: AtomicU64,
    /// Identifies this database; see the module docs.
    relay_id: String,
    android_app: Option<AndroidApp>,
    pub(crate) client_ip_header: Option<HeaderName>,
    limits: Limits,
    web_dir: Option<PathBuf>,
    rates: rates::Rates,
    links: links::Links,
    admin_token: Option<String>,
}

impl Relay {
    pub fn open(path: &std::path::Path) -> rusqlite::Result<Arc<Self>> {
        Self::open_with(path, Settings::default())
    }

    pub fn open_with(path: &std::path::Path, settings: Settings) -> rusqlite::Result<Arc<Self>> {
        let db = Connection::open(path)?;
        db.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = FULL;
             CREATE TABLE IF NOT EXISTS groups (
                 id         TEXT PRIMARY KEY,
                 key_hash   BLOB NOT NULL,
                 created_at TEXT NOT NULL DEFAULT (datetime('now'))
             );
             CREATE TABLE IF NOT EXISTS updates (
                 seq        INTEGER PRIMARY KEY AUTOINCREMENT,
                 group_id   TEXT NOT NULL REFERENCES groups(id),
                 data       BLOB NOT NULL,
                 created_at TEXT NOT NULL DEFAULT (datetime('now'))
             );
             CREATE INDEX IF NOT EXISTS updates_by_group ON updates (group_id, seq);
             CREATE TABLE IF NOT EXISTS accounts (
                 username    TEXT PRIMARY KEY,
                 account_id  TEXT NOT NULL UNIQUE,
                 login_hash  BLOB NOT NULL,
                 wrapped_key BLOB NOT NULL,
                 created_at  TEXT NOT NULL DEFAULT (datetime('now'))
             );
             CREATE TABLE IF NOT EXISTS feedback (
                 id         INTEGER PRIMARY KEY AUTOINCREMENT,
                 message    TEXT NOT NULL,
                 contact    TEXT,
                 app        TEXT,
                 created_at TEXT NOT NULL DEFAULT (datetime('now'))
             );
             CREATE TABLE IF NOT EXISTS relay_meta (
                 key   TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );
             INSERT OR IGNORE INTO relay_meta (key, value)
                 VALUES ('relay_id', lower(hex(randomblob(16))));",
        )?;
        // Recovery keys came after accounts: add their columns to older databases too.
        let has_recovery: bool = db.query_row(
            "SELECT EXISTS (SELECT 1 FROM pragma_table_info('accounts') WHERE name = 'recovery_hash')",
            [],
            |r| r.get(0),
        )?;
        if !has_recovery {
            db.execute_batch(
                "ALTER TABLE accounts ADD COLUMN recovery_hash BLOB;
                 ALTER TABLE accounts ADD COLUMN recovery_wrapped_key BLOB;",
            )?;
        }
        // Stored size per document, for the size limits; counted once for older databases.
        let has_bytes: bool = db.query_row(
            "SELECT EXISTS (SELECT 1 FROM pragma_table_info('groups') WHERE name = 'bytes')",
            [],
            |r| r.get(0),
        )?;
        if !has_bytes {
            db.execute_batch(
                "BEGIN;
                 ALTER TABLE groups ADD COLUMN bytes INTEGER NOT NULL DEFAULT 0;
                 UPDATE groups SET bytes =
                     (SELECT COALESCE(SUM(length(data)), 0) FROM updates WHERE group_id = groups.id);
                 COMMIT;",
            )?;
        }
        let stored_bytes: i64 =
            db.query_row("SELECT COALESCE(SUM(bytes), 0) FROM groups", [], |r| {
                r.get(0)
            })?;
        let relay_id = db.query_row(
            "SELECT value FROM relay_meta WHERE key = 'relay_id'",
            [],
            |r| r.get(0),
        )?;
        Ok(Arc::new(Self {
            db: Mutex::new(db),
            counters: Counters::default(),
            stored_bytes: AtomicU64::new(stored_bytes.max(0) as u64),
            relay_id,
            android_app: settings.android_app,
            client_ip_header: settings.client_ip_header,
            limits: settings.limits,
            web_dir: settings
                .web_dir
                .filter(|dir| dir.join("index.html").is_file()),
            rates: rates::Rates::new(settings.rates_url),
            links: links::Links::default(),
            admin_token: settings.admin_token.filter(|token| !token.is_empty()),
        }))
    }
}

/// An Android app allowed to open this relay's invite links directly.
#[derive(Clone, Debug)]
pub struct AndroidApp {
    pub package: String,
    /// SHA-256 fingerprints of the app's signing certificates, as `AB:CD:…`.
    pub cert_sha256: Vec<String>,
}

pub fn router(relay: Arc<Relay>) -> Router {
    let asset_links = match &relay.android_app {
        Some(app) => {
            let links = serde_json::json!([{
                "relation": ["delegate_permission/common.handle_all_urls"],
                "target": {
                    "namespace": "android_app",
                    "package_name": app.package,
                    "sha256_cert_fingerprints": app.cert_sha256,
                },
            }]);
            get(move || async move { Json(links) })
        }
        None => get(|| async { StatusCode::NOT_FOUND }),
    };
    let has_web = relay.web_dir.is_some();
    let mut router = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/join", get(move || web::join_page(has_web)))
        .route("/.well-known/assetlinks.json", asset_links)
        .route(
            "/v1/groups/{id}/updates",
            get(updates::pull).post(updates::push),
        )
        .route("/v1/accounts", post(accounts::sign_up))
        .route("/v1/accounts/login", post(accounts::log_in))
        .route("/v1/accounts/recover", post(accounts::recover))
        .route(
            "/v1/accounts/credentials",
            post(accounts::update_credentials),
        )
        .route("/v1/accounts/links", post(links::create))
        .route("/v1/accounts/links/claim", post(links::claim))
        .route("/v1/handoffs", post(links::hand_over))
        .route("/v1/handoffs/claim", post(links::poll))
        .route("/v1/rates/{from}/{to}", get(rates::rate))
        .route("/v1/feedback", get(feedback::list).post(feedback::send))
        .route("/feedback", get(feedback::page));
    if let Some(dir) = &relay.web_dir {
        router = router.fallback_service(web::web_app(dir));
    }
    router
        .layer(DefaultBodyLimit::max(updates::MAX_UPDATE_BYTES))
        .with_state(relay)
}

pub async fn serve(listener: tokio::net::TcpListener, relay: Arc<Relay>) -> std::io::Result<()> {
    // The connection's address is the client's, unless a proxy header says otherwise.
    let app = router(relay).into_make_service_with_connect_info::<SocketAddr>();
    axum::serve(listener, app).await
}

/// Runs `work` with the database on a blocking thread. SQLite waits for the disk on every
/// commit and one connection serves every client, so doing this on the async workers would
/// let a burst of requests stall the whole relay.
async fn with_db<T: Send + 'static>(
    relay: &Arc<Relay>,
    work: impl FnOnce(&Relay, &mut Connection) -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    let relay = Arc::clone(relay);
    tokio::task::spawn_blocking(move || {
        let mut db = relay.db.lock().unwrap_or_else(|e| e.into_inner());
        work(&relay, &mut db)
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
}
