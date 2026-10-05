//! A document's updates: `POST` adds one, `GET` hands back those after a sequence number.

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap};
use axum::Json;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::limits::{Client, HOUR};
use crate::{with_db, ApiError, Relay};

/// Largest single update accepted. A full history of a big group is well under this.
pub(crate) const MAX_UPDATE_BYTES: usize = 8 * 1024 * 1024;

const DEFAULT_PAGE: u32 = 200;

const MAX_PAGE: u32 = 1000;

/// A page of updates also ends at this size, so that answering doesn't hold a whole document
/// (up to `Limits::max_document_bytes`) in memory at once.
const MAX_PAGE_BYTES: usize = 4 * 1024 * 1024;

pub(crate) fn check_group_id(id: &str) -> Result<(), ApiError> {
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

pub(crate) fn stored_hash(db: &Connection, group_id: &str) -> rusqlite::Result<Option<Vec<u8>>> {
    db.query_row(
        "SELECT key_hash FROM groups WHERE id = ?1",
        [group_id],
        |r| r.get(0),
    )
    .optional()
}

/// Whether this document went with its account (`accounts::delete`). Its id is kept, so that
/// a device still logged in can't upload the document again: without it, the relay would
/// take that upload for a new document.
pub(crate) fn was_deleted(db: &Connection, id: &str) -> rusqlite::Result<bool> {
    db.query_row(
        "SELECT EXISTS (SELECT 1 FROM deleted_documents WHERE id = ?1)",
        [id],
        |r| r.get(0),
    )
}

#[derive(Serialize)]
pub(crate) struct PushResponse {
    seq: i64,
    relay_id: String,
}

pub(crate) async fn push(
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
                if was_deleted(&tx, &group_id)? {
                    return Err(ApiError::DocumentGone);
                }
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
pub(crate) struct PullQuery {
    #[serde(default)]
    after: i64,
    limit: Option<u32>,
}

#[derive(Serialize)]
pub(crate) struct PullResponse {
    updates: Vec<Update>,
    has_more: bool,
    relay_id: String,
}

#[derive(Serialize)]
pub(crate) struct Update {
    seq: i64,
    data: String,
}

pub(crate) async fn pull(
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
            None if was_deleted(db, &group_id)? => return Err(ApiError::DocumentGone),
            None => return Err(ApiError::NotFound),
            Some(stored) if stored != hash => return Err(ApiError::Unauthorized),
            Some(_) => {}
        }
        let mut stmt = db.prepare(
            "SELECT seq, data FROM updates WHERE group_id = ?1 AND seq > ?2 ORDER BY seq LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![group_id, query.after, limit + 1], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?))
        })?;
        let mut updates = Vec::new();
        let mut bytes = 0;
        let mut has_more = false;
        for row in rows {
            let (seq, data) = row?;
            // Always one update, however large: a client couldn't get past an empty page.
            let full = updates.len() == limit as usize
                || (!updates.is_empty() && bytes + data.len() > MAX_PAGE_BYTES);
            if full {
                has_more = true;
                break;
            }
            bytes += data.len();
            updates.push(Update {
                seq,
                data: STANDARD.encode(data),
            });
        }
        Ok(Json(PullResponse {
            updates,
            has_more,
            relay_id: relay.relay_id.clone(),
        }))
    })
    .await
}

#[cfg(test)]
mod tests;
