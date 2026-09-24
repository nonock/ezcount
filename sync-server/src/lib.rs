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
//! - `POST /v1/groups/{id}/updates`: body is one update; returns `{ "seq": n }`
//! - `GET  /v1/groups/{id}/updates?after=n`: returns `{ "updates": [{ "seq", "data" }], "has_more" }`
//!   with `data` in standard base64

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};

/// Largest single update accepted. A full history of a big group is well under this.
const MAX_UPDATE_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_PAGE: u32 = 200;
const MAX_PAGE: u32 = 1000;

pub struct Relay {
    db: Mutex<Connection>,
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
             CREATE INDEX IF NOT EXISTS updates_by_group ON updates (group_id, seq);",
        )?;
        Ok(Arc::new(Self { db: Mutex::new(db) }))
    }
}

pub fn router(relay: Arc<Relay>) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/v1/groups/{id}/updates", get(pull).post(push))
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
    Ok(Json(PushResponse { seq }))
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
    Ok(Json(PullResponse { updates, has_more }))
}
