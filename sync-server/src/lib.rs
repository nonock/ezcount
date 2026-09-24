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

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Largest single update accepted. A full history of a big group is well under this.
const MAX_UPDATE_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_PAGE: u32 = 200;
const MAX_PAGE: u32 = 1000;
/// Failed logins allowed per username within `LOGIN_WINDOW` before further attempts are refused.
const MAX_LOGIN_FAILURES: u32 = 5;
const LOGIN_WINDOW: Duration = Duration::from_secs(15 * 60);

pub struct Relay {
    db: Mutex<Connection>,
    /// Failed login count and time of the first failure, per username. In memory only.
    login_failures: Mutex<HashMap<String, (u32, Instant)>>,
    /// Identifies this database; see the module docs.
    relay_id: String,
}

impl Relay {
    pub fn open(path: &std::path::Path) -> rusqlite::Result<Arc<Self>> {
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
        let relay_id = db.query_row(
            "SELECT value FROM relay_meta WHERE key = 'relay_id'",
            [],
            |r| r.get(0),
        )?;
        Ok(Arc::new(Self {
            db: Mutex::new(db),
            login_failures: Mutex::new(HashMap::new()),
            relay_id,
        }))
    }
}

pub fn router(relay: Arc<Relay>) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/v1/groups/{id}/updates", get(pull).post(push))
        .route("/v1/accounts", post(sign_up))
        .route("/v1/accounts/login", post(log_in))
        .layer(DefaultBodyLimit::max(MAX_UPDATE_BYTES))
        .with_state(relay)
}

pub async fn serve(listener: tokio::net::TcpListener, relay: Arc<Relay>) -> std::io::Result<()> {
    axum::serve(listener, router(relay)).await
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
            ApiError::Internal(e) => {
                eprintln!("[relay] internal error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
            }
        }
    }
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
    Path(group_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<PushResponse>, ApiError> {
    check_group_id(&group_id)?;
    let hash = key_hash(&headers)?;
    if body.is_empty() {
        return Err(ApiError::BadRequest("empty update"));
    }

    let mut db = relay.db.lock().unwrap_or_else(|e| e.into_inner());
    let tx = db.transaction()?;
    match stored_hash(&tx, &group_id)? {
        None => {
            tx.execute(
                "INSERT INTO groups (id, key_hash) VALUES (?1, ?2)",
                params![group_id, hash],
            )?;
        }
        Some(stored) if stored != hash => return Err(ApiError::Unauthorized),
        Some(_) => {}
    }
    tx.execute(
        "INSERT INTO updates (group_id, data) VALUES (?1, ?2)",
        params![group_id, body.as_ref()],
    )?;
    let seq = tx.last_insert_rowid();
    tx.commit()?;
    Ok(Json(PushResponse {
        seq,
        relay_id: relay.relay_id.clone(),
    }))
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

    let db = relay.db.lock().unwrap_or_else(|e| e.into_inner());
    match stored_hash(&db, &group_id)? {
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
}

async fn sign_up(
    State(relay): State<Arc<Relay>>,
    Json(req): Json<SignUpRequest>,
) -> Result<StatusCode, ApiError> {
    check_username(&req.username)?;
    check_group_id(&req.account_id)?;
    let login_hash = token_hash(&req.login_token)?;
    let doc_hash = token_hash(&req.doc_token)?;
    let wrapped_key = STANDARD
        .decode(&req.wrapped_key)
        .ok()
        .filter(|k| !k.is_empty() && k.len() <= 1024)
        .ok_or(ApiError::BadRequest("invalid wrapped key"))?;

    let mut db = relay.db.lock().unwrap_or_else(|e| e.into_inner());
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
        "INSERT INTO accounts (username, account_id, login_hash, wrapped_key)
         VALUES (?1, ?2, ?3, ?4)",
        params![req.username, req.account_id, login_hash, wrapped_key],
    )?;
    tx.execute(
        "INSERT INTO groups (id, key_hash) VALUES (?1, ?2)",
        params![req.account_id, doc_hash],
    )?;
    tx.commit()?;
    Ok(StatusCode::CREATED)
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
    /// Refuses logins for a username that failed too often recently.
    fn check_throttle(&self, username: &str) -> Result<(), ApiError> {
        let failures = self
            .login_failures
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match failures.get(username) {
            Some(&(count, since))
                if count >= MAX_LOGIN_FAILURES && since.elapsed() < LOGIN_WINDOW =>
            {
                Err(ApiError::TooManyAttempts)
            }
            _ => Ok(()),
        }
    }

    fn record_login(&self, username: &str, success: bool) {
        let mut failures = self
            .login_failures
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if success {
            failures.remove(username);
            return;
        }
        // Keep the map small when someone sprays random usernames.
        if failures.len() > 10_000 {
            failures.retain(|_, (_, since)| since.elapsed() < LOGIN_WINDOW);
        }
        let entry = failures
            .entry(username.to_string())
            .or_insert((0, Instant::now()));
        if entry.1.elapsed() >= LOGIN_WINDOW {
            *entry = (0, Instant::now());
        }
        entry.0 += 1;
    }
}

async fn log_in(
    State(relay): State<Arc<Relay>>,
    Json(req): Json<LogInRequest>,
) -> Result<Json<LogInResponse>, ApiError> {
    check_username(&req.username)?;
    relay.check_throttle(&req.username)?;
    let login_hash = token_hash(&req.login_token)?;

    let found = {
        let db = relay.db.lock().unwrap_or_else(|e| e.into_inner());
        db.query_row(
            "SELECT account_id, login_hash, wrapped_key FROM accounts WHERE username = ?1",
            [&req.username],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Vec<u8>>(1)?,
                    r.get::<_, Vec<u8>>(2)?,
                ))
            },
        )
        .optional()?
    };
    match found {
        Some((account_id, stored, wrapped_key)) if stored == login_hash => {
            relay.record_login(&req.username, true);
            Ok(Json(LogInResponse {
                account_id,
                wrapped_key: STANDARD.encode(wrapped_key),
            }))
        }
        // Unknown username and wrong password look the same from outside.
        _ => {
            relay.record_login(&req.username, false);
            Err(ApiError::BadLogin)
        }
    }
}
