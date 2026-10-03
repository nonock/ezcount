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

mod limits;

pub use limits::Limits;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use limits::{Client, Counters, HOUR, LOGIN_WINDOW};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

/// Largest single update accepted. A full history of a big group is well under this.
const MAX_UPDATE_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_PAGE: u32 = 200;
const MAX_PAGE: u32 = 1000;

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
        .route("/join", get(move || join_page(has_web)))
        .route("/.well-known/assetlinks.json", asset_links)
        .route("/v1/groups/{id}/updates", get(pull).post(push))
        .route("/v1/accounts", post(sign_up))
        .route("/v1/accounts/login", post(log_in))
        .route("/v1/accounts/recover", post(recover))
        .route("/v1/accounts/credentials", post(update_credentials));
    if let Some(dir) = &relay.web_dir {
        router = router.fallback_service(web_app(dir));
    }
    router
        .layer(DefaultBodyLimit::max(MAX_UPDATE_BYTES))
        .with_state(relay)
}

/// What the web version may load and talk to: only this origin, plus running its WebAssembly.
const WEB_CSP: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval';      style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self';      connect-src 'self'; worker-src 'self'; object-src 'none'; base-uri 'none';      form-action 'none'; frame-ancestors 'none'";

/// The web version's files. Vite names everything in `assets/` by its content, so those are
/// cached for good; the rest (`index.html`, `theme.js`) is checked on every load.
fn web_app(dir: &FsPath) -> Router {
    let cache = |value: &'static str| {
        SetResponseHeaderLayer::overriding(header::CACHE_CONTROL, HeaderValue::from_static(value))
    };
    let assets = Router::new()
        .fallback_service(ServeDir::new(dir.join("assets")))
        .layer(cache("public, max-age=31536000, immutable"));
    let rest = Router::new()
        .fallback_service(ServeDir::new(dir))
        .layer(cache("no-cache"));
    Router::new()
        .nest_service("/assets", assets)
        .fallback_service(rest)
        .layer(SetResponseHeaderLayer::overriding(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(WEB_CSP),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
}

pub async fn serve(listener: tokio::net::TcpListener, relay: Arc<Relay>) -> std::io::Result<()> {
    // The connection's address is the client's, unless a proxy header says otherwise.
    let app = router(relay).into_make_service_with_connect_info::<SocketAddr>();
    axum::serve(listener, app).await
}

async fn join_page(has_web: bool) -> impl IntoResponse {
    let page = include_str!("join.html");
    // Tells the page to offer the web version too.
    let page = if has_web {
        page.replacen("<body>", "<body data-web>", 1)
    } else {
        page.to_string()
    };
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; \
                 base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
            ),
            (header::REFERRER_POLICY, "no-referrer"),
        ],
        page,
    )
}

// ---------------------------------------------------------------------------

#[derive(Debug)]
enum ApiError {
    BadRequest(&'static str),
    Unauthorized,
    NotFound,
    Conflict(&'static str),
    BadLogin,
    TooManyAttempts,
    /// A per-client rate limit.
    TooManyRequests,
    /// The document reached `Limits::max_document_bytes`.
    TooLarge,
    /// All documents together reached `Limits::max_total_bytes`.
    StorageFull,
    Internal(String),
}

impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
            ApiError::Unauthorized => {
                (StatusCode::UNAUTHORIZED, "invalid group key").into_response()
            }
            ApiError::NotFound => (StatusCode::NOT_FOUND, "unknown group").into_response(),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, msg).into_response(),
            ApiError::BadLogin => {
                (StatusCode::UNAUTHORIZED, "wrong username or password").into_response()
            }
            ApiError::TooManyAttempts => (
                StatusCode::TOO_MANY_REQUESTS,
                "too many failed logins, try later",
            )
                .into_response(),
            ApiError::TooManyRequests => (
                StatusCode::TOO_MANY_REQUESTS,
                "too many requests from your network, try later",
            )
                .into_response(),
            ApiError::TooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "this document reached the size limit",
            )
                .into_response(),
            ApiError::StorageFull => {
                eprintln!("[relay] storage limit reached: refusing uploads");
                (StatusCode::INSUFFICIENT_STORAGE, "the relay is full").into_response()
            }
            ApiError::Internal(e) => {
                eprintln!("[relay] internal error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
            }
        }
    }
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

fn check_group_id(id: &str) -> Result<(), ApiError> {
    let valid = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if valid {
        Ok(())
    } else {
        Err(ApiError::BadRequest("invalid group id"))
    }
}

fn key_hash(headers: &HeaderMap) -> Result<Vec<u8>, ApiError> {
    let key = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|k| k.len() >= 16)
        .ok_or(ApiError::Unauthorized)?;
    Ok(Sha256::digest(key.as_bytes()).to_vec())
}

fn stored_hash(db: &Connection, group_id: &str) -> rusqlite::Result<Option<Vec<u8>>> {
    db.query_row(
        "SELECT key_hash FROM groups WHERE id = ?1",
        [group_id],
        |r| r.get(0),
    )
    .optional()
}

#[derive(Serialize)]
struct PushResponse {
    seq: i64,
    relay_id: String,
}

async fn push(
    State(relay): State<Arc<Relay>>,
    client: Client,
    Path(group_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<PushResponse>, ApiError> {
    check_group_id(&group_id)?;
    let hash = key_hash(&headers)?;
    if body.is_empty() {
        return Err(ApiError::BadRequest("empty update"));
    }
    let size = body.len() as u64;
    let limits = &relay.limits;
    let uploads = format!("upload {}", client.0);
    if !relay
        .counters
        .try_add(&uploads, size, limits.upload_bytes_per_hour, HOUR)
    {
        return Err(ApiError::TooManyRequests);
    }

    let creations = format!("create {}", client.0);
    with_db(&relay, move |relay, db| {
        let limits = &relay.limits;
        let tx = db.transaction()?;
        let existing: Option<(Vec<u8>, i64)> = tx
            .query_row(
                "SELECT key_hash, bytes FROM groups WHERE id = ?1",
                [&group_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let stored = match existing {
            // The first push registers the document.
            None => {
                if !relay
                    .counters
                    .try_add(&creations, 1, limits.new_documents_per_hour, HOUR)
                {
                    return Err(ApiError::TooManyRequests);
                }
                tx.execute(
                    "INSERT INTO groups (id, key_hash) VALUES (?1, ?2)",
                    params![group_id, hash],
                )?;
                0
            }
            Some((stored, _)) if stored != hash => return Err(ApiError::Unauthorized),
            Some((_, bytes)) => bytes.max(0) as u64,
        };
        if stored + size > limits.max_document_bytes {
            return Err(ApiError::TooLarge);
        }
        if relay.stored_bytes.load(Ordering::Relaxed) + size > limits.max_total_bytes {
            return Err(ApiError::StorageFull);
        }
        tx.execute(
            "INSERT INTO updates (group_id, data) VALUES (?1, ?2)",
            params![group_id, body.as_ref()],
        )?;
        let seq = tx.last_insert_rowid();
        tx.execute(
            "UPDATE groups SET bytes = bytes + ?2 WHERE id = ?1",
            params![group_id, size as i64],
        )?;
        tx.commit()?;
        // Still under the database lock, so the check above and this stay in step.
        relay.stored_bytes.fetch_add(size, Ordering::Relaxed);
        Ok(Json(PushResponse {
            seq,
            relay_id: relay.relay_id.clone(),
        }))
    })
    .await
}

#[derive(Deserialize)]
struct PullQuery {
    #[serde(default)]
    after: i64,
    limit: Option<u32>,
}

#[derive(Serialize)]
struct PullResponse {
    updates: Vec<Update>,
    has_more: bool,
    relay_id: String,
}

#[derive(Serialize)]
struct Update {
    seq: i64,
    data: String,
}

async fn pull(
    State(relay): State<Arc<Relay>>,
    Path(group_id): Path<String>,
    Query(query): Query<PullQuery>,
    headers: HeaderMap,
) -> Result<Json<PullResponse>, ApiError> {
    check_group_id(&group_id)?;
    let hash = key_hash(&headers)?;
    let limit = query.limit.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE);

    with_db(&relay, move |relay, db| {
        match stored_hash(db, &group_id)? {
            None => return Err(ApiError::NotFound),
            Some(stored) if stored != hash => return Err(ApiError::Unauthorized),
            Some(_) => {}
        }
        let mut stmt = db.prepare(
            "SELECT seq, data FROM updates WHERE group_id = ?1 AND seq > ?2 ORDER BY seq LIMIT ?3",
        )?;
        let mut updates = stmt
            .query_map(params![group_id, query.after, limit + 1], |r| {
                Ok(Update {
                    seq: r.get(0)?,
                    data: STANDARD.encode(r.get::<_, Vec<u8>>(1)?),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let has_more = updates.len() > limit as usize;
        updates.truncate(limit as usize);
        Ok(Json(PullResponse {
            updates,
            has_more,
            relay_id: relay.relay_id.clone(),
        }))
    })
    .await
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

/// Same rule as the app: 3 to 32 of `a-z 0-9 . _ -`, starting with a letter or digit.
fn check_username(username: &str) -> Result<(), ApiError> {
    let bytes = username.as_bytes();
    let valid = (3..=32).contains(&bytes.len())
        && bytes[0].is_ascii_alphanumeric()
        && bytes.iter().all(|&b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        });
    if valid {
        Ok(())
    } else {
        Err(ApiError::BadRequest("invalid username"))
    }
}

fn token_hash(token: &str) -> Result<Vec<u8>, ApiError> {
    if token.len() < 16 {
        return Err(ApiError::BadRequest("invalid token"));
    }
    Ok(Sha256::digest(token.as_bytes()).to_vec())
}

#[derive(Deserialize)]
struct SignUpRequest {
    username: String,
    login_token: String,
    /// Account key encrypted on the device, standard base64.
    wrapped_key: String,
    account_id: String,
    /// Bearer token for the account document, registered like a group's.
    doc_token: String,
    /// Absent from apps before recovery keys.
    #[serde(default)]
    recovery_token: Option<String>,
    #[serde(default)]
    recovery_wrapped_key: Option<String>,
}

#[derive(Serialize)]
struct SignUpResponse {
    /// The recovery key was stored.
    recovery: bool,
}

/// An account key encrypted on the device, sent in standard base64.
fn wrapped_key(encoded: &str) -> Result<Vec<u8>, ApiError> {
    STANDARD
        .decode(encoded)
        .ok()
        .filter(|k| !k.is_empty() && k.len() <= 1024)
        .ok_or(ApiError::BadRequest("invalid wrapped key"))
}

/// A token's hash and the account key it unlocks, encrypted.
type Credential = (Vec<u8>, Vec<u8>);

/// A token and its wrapped key, both given or neither.
fn credential(
    token: Option<&String>,
    wrapped: Option<&String>,
) -> Result<Option<Credential>, ApiError> {
    match (token, wrapped) {
        (Some(token), Some(wrapped)) => Ok(Some((token_hash(token)?, wrapped_key(wrapped)?))),
        (None, None) => Ok(None),
        _ => Err(ApiError::BadRequest(
            "a token needs its wrapped key, and back",
        )),
    }
}

async fn sign_up(
    State(relay): State<Arc<Relay>>,
    client: Client,
    Json(req): Json<SignUpRequest>,
) -> Result<(StatusCode, Json<SignUpResponse>), ApiError> {
    check_username(&req.username)?;
    check_group_id(&req.account_id)?;
    let login_hash = token_hash(&req.login_token)?;
    let doc_hash = token_hash(&req.doc_token)?;
    let wrapped_key = wrapped_key(&req.wrapped_key)?;
    let recovery = credential(
        req.recovery_token.as_ref(),
        req.recovery_wrapped_key.as_ref(),
    )?;
    let (recovery_hash, recovery_wrapped_key) = recovery.clone().unzip();
    let sign_ups = format!("sign-up {}", client.0);
    if !relay
        .counters
        .try_add(&sign_ups, 1, relay.limits.sign_ups_per_hour, HOUR)
    {
        return Err(ApiError::TooManyRequests);
    }

    let recovery_stored = recovery.is_some();
    with_db(&relay, move |_, db| {
        let tx = db.transaction()?;
        let taken: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM accounts WHERE username = ?1)",
            [&req.username],
            |r| r.get(0),
        )?;
        if taken {
            return Err(ApiError::Conflict("username taken"));
        }
        if stored_hash(&tx, &req.account_id)?.is_some() {
            return Err(ApiError::Conflict("account id taken"));
        }
        tx.execute(
            "INSERT INTO accounts
                 (username, account_id, login_hash, wrapped_key, recovery_hash, recovery_wrapped_key)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                req.username,
                req.account_id,
                login_hash,
                wrapped_key,
                recovery_hash,
                recovery_wrapped_key
            ],
        )?;
        tx.execute(
            "INSERT INTO groups (id, key_hash) VALUES (?1, ?2)",
            params![req.account_id, doc_hash],
        )?;
        tx.commit()?;
        Ok(())
    })
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(SignUpResponse {
            recovery: recovery_stored,
        }),
    ))
}

#[derive(Deserialize)]
struct LogInRequest {
    username: String,
    login_token: String,
}

#[derive(Serialize)]
struct LogInResponse {
    account_id: String,
    wrapped_key: String,
}

impl Relay {
    /// Refuses logins for a username that failed too often recently, from this client or
    /// from everywhere.
    fn check_throttle(&self, username: &str, client: &Client) -> Result<(), ApiError> {
        let (from_client, overall) = login_keys(username, client);
        let limits = &self.limits;
        if self.counters.count(&from_client, LOGIN_WINDOW) >= limits.login_failures_per_client
            || self.counters.count(&overall, LOGIN_WINDOW) >= limits.login_failures_per_username
        {
            return Err(ApiError::TooManyAttempts);
        }
        Ok(())
    }

    fn record_login(&self, username: &str, client: &Client, success: bool) {
        let (from_client, overall) = login_keys(username, client);
        if success {
            // The username-wide count still expires by itself: failures elsewhere stand.
            self.counters.clear(&from_client);
        } else {
            self.counters
                .try_add(&from_client, 1, u64::MAX, LOGIN_WINDOW);
            self.counters.try_add(&overall, 1, u64::MAX, LOGIN_WINDOW);
        }
    }
}

fn login_keys(username: &str, client: &Client) -> (String, String) {
    (
        format!("login {username} {}", client.0),
        format!("login {username}"),
    )
}

async fn log_in(
    State(relay): State<Arc<Relay>>,
    client: Client,
    Json(req): Json<LogInRequest>,
) -> Result<Json<LogInResponse>, ApiError> {
    check_username(&req.username)?;
    relay.check_throttle(&req.username, &client)?;
    let login_hash = token_hash(&req.login_token)?;

    let username = req.username.clone();
    let found = with_db(&relay, move |_, db| {
        Ok(db
            .query_row(
                "SELECT account_id, login_hash, wrapped_key FROM accounts WHERE username = ?1",
                [&username],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Vec<u8>>(1)?,
                        r.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .optional()?)
    })
    .await?;
    match found {
        Some((account_id, stored, wrapped_key)) if stored == login_hash => {
            relay.record_login(&req.username, &client, true);
            Ok(Json(LogInResponse {
                account_id,
                wrapped_key: STANDARD.encode(wrapped_key),
            }))
        }
        // Unknown username and wrong password look the same from outside.
        _ => {
            relay.record_login(&req.username, &client, false);
            Err(ApiError::BadLogin)
        }
    }
}

#[derive(Deserialize)]
struct RecoverRequest {
    username: String,
    recovery_token: String,
}

/// Like logging in, with the recovery key: hands back the account key it encrypts.
async fn recover(
    State(relay): State<Arc<Relay>>,
    client: Client,
    Json(req): Json<RecoverRequest>,
) -> Result<Json<LogInResponse>, ApiError> {
    check_username(&req.username)?;
    relay.check_throttle(&req.username, &client)?;
    let recovery_hash = token_hash(&req.recovery_token)?;

    let username = req.username.clone();
    let found = with_db(&relay, move |_, db| {
        Ok(db
            .query_row(
                "SELECT account_id, recovery_hash, recovery_wrapped_key FROM accounts
                 WHERE username = ?1",
                [&username],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<Vec<u8>>>(1)?,
                        r.get::<_, Option<Vec<u8>>>(2)?,
                    ))
                },
            )
            .optional()?)
    })
    .await?;
    match found {
        Some((account_id, Some(stored), Some(wrapped_key))) if stored == recovery_hash => {
            relay.record_login(&req.username, &client, true);
            Ok(Json(LogInResponse {
                account_id,
                wrapped_key: STANDARD.encode(wrapped_key),
            }))
        }
        // Including accounts without a recovery key: same answer as a wrong key.
        _ => {
            relay.record_login(&req.username, &client, false);
            Err(ApiError::BadLogin)
        }
    }
}

#[derive(Deserialize)]
struct CredentialsRequest {
    username: String,
    /// Proof of the account: one of the two.
    #[serde(default)]
    login_token: Option<String>,
    #[serde(default)]
    recovery_token: Option<String>,
    /// A new password: its login token and the account key it encrypts.
    #[serde(default)]
    new_login_token: Option<String>,
    #[serde(default)]
    new_wrapped_key: Option<String>,
    /// A new recovery key, likewise.
    #[serde(default)]
    new_recovery_token: Option<String>,
    #[serde(default)]
    new_recovery_wrapped_key: Option<String>,
}

/// Changes the password, the recovery key, or both. See the module docs.
async fn update_credentials(
    State(relay): State<Arc<Relay>>,
    client: Client,
    Json(req): Json<CredentialsRequest>,
) -> Result<StatusCode, ApiError> {
    check_username(&req.username)?;
    let new_login = credential(req.new_login_token.as_ref(), req.new_wrapped_key.as_ref())?;
    let new_recovery = credential(
        req.new_recovery_token.as_ref(),
        req.new_recovery_wrapped_key.as_ref(),
    )?;
    let (proof_column, proof) = match (&req.login_token, &req.recovery_token) {
        (Some(token), None) => ("login_hash", token_hash(token)?),
        (None, Some(_)) if new_recovery.is_none() => {
            return Err(ApiError::BadRequest(
                "a recovery key works once: replace it",
            ))
        }
        (None, Some(token)) => ("recovery_hash", token_hash(token)?),
        _ => return Err(ApiError::BadRequest("prove the account with one token")),
    };
    if new_login.is_none() && new_recovery.is_none() {
        return Err(ApiError::BadRequest("nothing to change"));
    }
    relay.check_throttle(&req.username, &client)?;

    let username = req.username.clone();
    let proven = with_db(&relay, move |_, db| {
        let tx = db.transaction()?;
        // `proof_column` is one of two literals above, never user input.
        let stored: Option<Vec<u8>> = tx
            .query_row(
                &format!("SELECT {proof_column} FROM accounts WHERE username = ?1"),
                [&username],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        let proven = stored == Some(proof);
        if proven {
            if let Some((hash, wrapped)) = new_login {
                tx.execute(
                    "UPDATE accounts SET login_hash = ?2, wrapped_key = ?3 WHERE username = ?1",
                    params![username, hash, wrapped],
                )?;
            }
            if let Some((hash, wrapped)) = new_recovery {
                tx.execute(
                    "UPDATE accounts SET recovery_hash = ?2, recovery_wrapped_key = ?3
                     WHERE username = ?1",
                    params![username, hash, wrapped],
                )?;
            }
            tx.commit()?;
        }
        Ok(proven)
    })
    .await?;
    relay.record_login(&req.username, &client, proven);
    if proven {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::BadLogin)
    }
}
