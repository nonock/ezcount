//! What people write to whoever runs the relay from the app's "Suggest a feature" form: an
//! idea or a problem, and how to answer them if they want an answer.
//!
//! Messages are kept in the database. Reading them takes the admin token
//! (`Settings::admin_token`): the page at `/feedback` asks for it and lists them. A relay
//! without a token keeps them without serving them.

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

use crate::limits::{Client, HOUR};
use crate::{with_db, ApiError, Relay};

/// Longest message, in characters.
const MAX_MESSAGE_CHARS: usize = 2000;
/// Longest contact, or description of the app it comes from.
const MAX_NOTE_CHARS: usize = 200;
/// Most messages kept: past it, the oldest go.
const MAX_KEPT: i64 = 5000;

#[derive(Deserialize)]
pub(crate) struct Sent {
    message: String,
    /// How to answer, if the person wants an answer.
    #[serde(default)]
    contact: Option<String>,
    /// The app's version and where it runs.
    #[serde(default)]
    app: Option<String>,
}

#[derive(Serialize)]
struct Kept {
    id: i64,
    message: String,
    contact: Option<String>,
    app: Option<String>,
    created_at: String,
}

/// Trimmed, and nothing when empty.
fn note(text: Option<String>) -> Result<Option<String>, ApiError> {
    let text = text.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    if text
        .as_ref()
        .is_some_and(|t| t.chars().count() > MAX_NOTE_CHARS)
    {
        return Err(ApiError::BadRequest("the contact is too long"));
    }
    Ok(text)
}

/// `POST /v1/feedback`
pub(crate) async fn send(
    State(relay): State<Arc<Relay>>,
    Client(client): Client,
    Json(sent): Json<Sent>,
) -> Result<StatusCode, ApiError> {
    let message = sent.message.trim();
    if message.is_empty() {
        return Err(ApiError::BadRequest("the message is empty"));
    }
    if message.chars().count() > MAX_MESSAGE_CHARS {
        return Err(ApiError::BadRequest("the message is too long"));
    }
    let (contact, app) = (note(sent.contact)?, note(sent.app)?);
    let limits = &relay.limits;
    if !relay.counters.try_add(
        &format!("feedback:{client}"),
        1,
        limits.feedback_per_hour,
        HOUR,
    ) || !relay
        .counters
        .try_add("feedback:all", 1, limits.feedback_per_hour_total, HOUR)
    {
        return Err(ApiError::TooManyRequests);
    }
    let message = message.to_string();
    with_db(&relay, move |_, db| {
        db.execute(
            "INSERT INTO feedback (message, contact, app) VALUES (?1, ?2, ?3)",
            params![message, contact, app],
        )?;
        db.execute(
            "DELETE FROM feedback WHERE id <= (SELECT MAX(id) FROM feedback) - ?1",
            params![MAX_KEPT],
        )?;
        Ok(StatusCode::NO_CONTENT)
    })
    .await
}

/// `GET /feedback`: the page that asks for the admin token and lists the messages. It holds
/// nothing by itself.
pub(crate) async fn page(State(relay): State<Arc<Relay>>) -> Response {
    if relay.admin_token.is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline';                  connect-src 'self'; base-uri 'none'; form-action 'none';                  frame-ancestors 'none'",
            ),
            (header::REFERRER_POLICY, "no-referrer"),
        ],
        include_str!("feedback.html"),
    )
        .into_response()
}

/// `GET /v1/feedback`, with `Authorization: Bearer <admin token>`: the messages, the latest
/// first.
pub(crate) async fn list(State(relay): State<Arc<Relay>>, headers: HeaderMap) -> Response {
    // A relay without a token has nothing to show at this address.
    let Some(token) = &relay.admin_token else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let given = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    // Hashes are compared, so that how long it takes says nothing about the token.
    if Sha256::digest(given.as_bytes()) != Sha256::digest(token.as_bytes()) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match with_db(&relay, |_, db| Ok(kept(db)?)).await {
        Ok(messages) => Json(messages).into_response(),
        Err(e) => e.into_response(),
    }
}

fn kept(db: &Connection) -> rusqlite::Result<Vec<Kept>> {
    let mut query = db.prepare(
        "SELECT id, message, contact, app, created_at FROM feedback ORDER BY id DESC LIMIT 1000",
    )?;
    let rows = query.query_map([], |row| {
        Ok(Kept {
            id: row.get(0)?,
            message: row.get(1)?,
            contact: row.get(2)?,
            app: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;
    rows.collect()
}

#[cfg(test)]
mod tests;
