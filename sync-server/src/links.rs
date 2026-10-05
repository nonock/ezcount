//! Login links: a logged-in device hands its account to another one, through a QR code.
//!
//! The device encrypts the account's key with a random code and leaves the result here under
//! a ticket derived from that code. The code goes to the other device in the QR code, never
//! here, so the relay can't read what it keeps. A link works once and for a short time:
//! a photo of the QR code is worth nothing afterwards. Links are kept in memory only.
//!
//! It also works the other way round, for a device that can't scan (a computer): that one
//! makes the code and shows it, a phone scans it and leaves what the computer is to get under
//! its ticket (`create` for an account, which asks for the password; `hand_over` for a
//! group's invite, which anyone holding the code may leave), and the computer asks for it
//! until it is there (`poll`).

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::accounts::{check_username, token_hash};
use crate::limits::{Client, HOUR};
use crate::{with_db, ApiError, Relay};

/// Links waiting at once. Each needs a password, so this is only reached under abuse.
const MAX_WAITING: usize = 1000;
/// An account's name, id and key, encrypted: a few hundred bytes.
const MAX_DATA_BYTES: usize = 2048;

/// Encrypted accounts waiting to be claimed, by ticket.
#[derive(Default)]
pub(crate) struct Links(Mutex<HashMap<String, (Vec<u8>, Instant)>>);

impl Links {
    fn waiting(&self) -> std::sync::MutexGuard<'_, HashMap<String, (Vec<u8>, Instant)>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Keeps `data` under `ticket`. False when too many links are waiting, or the ticket is
    /// already taken.
    fn put(&self, ticket: String, data: Vec<u8>, lifetime: Duration) -> bool {
        let mut waiting = self.waiting();
        waiting.retain(|_, (_, expires)| *expires > Instant::now());
        if waiting.len() >= MAX_WAITING || waiting.contains_key(&ticket) {
            return false;
        }
        waiting.insert(ticket, (data, Instant::now() + lifetime));
        true
    }

    /// Hands out what `ticket` holds, once.
    fn take(&self, ticket: &str) -> Option<Vec<u8>> {
        let (data, expires) = self.waiting().remove(ticket)?;
        (expires > Instant::now()).then_some(data)
    }
}

/// Tickets are 32 random bytes in base64url.
fn check_ticket(ticket: &str) -> Result<(), ApiError> {
    let valid = (32..=64).contains(&ticket.len())
        && ticket
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if valid {
        Ok(())
    } else {
        Err(ApiError::BadRequest("invalid ticket"))
    }
}

#[derive(Deserialize)]
pub(crate) struct CreateRequest {
    /// Proof of the account, as for logging in: making a link asks for the password.
    username: String,
    login_token: String,
    ticket: String,
    /// The account, encrypted on the device, in standard base64.
    data: String,
}

#[derive(Serialize)]
pub(crate) struct CreateResponse {
    /// Seconds the link can be used for.
    expires_in: u64,
}

pub(crate) async fn create(
    State(relay): State<Arc<Relay>>,
    client: Client,
    Json(req): Json<CreateRequest>,
) -> Result<(StatusCode, Json<CreateResponse>), ApiError> {
    check_username(&req.username)?;
    check_ticket(&req.ticket)?;
    let data = STANDARD
        .decode(&req.data)
        .ok()
        .filter(|d| !d.is_empty() && d.len() <= MAX_DATA_BYTES)
        .ok_or(ApiError::BadRequest("invalid data"))?;
    relay.check_throttle(&req.username, &client)?;
    let login_hash = token_hash(&req.login_token)?;

    let username = req.username.clone();
    let stored: Option<Vec<u8>> = with_db(&relay, move |_, db| {
        Ok(db
            .query_row(
                "SELECT login_hash FROM accounts WHERE username = ?1",
                [&username],
                |r| r.get(0),
            )
            .optional()?)
    })
    .await?;
    let proven = stored == Some(login_hash);
    relay.record_login(&req.username, &client, proven);
    if !proven {
        return Err(ApiError::BadLogin);
    }
    let lifetime = relay.limits.link_lifetime;
    if !relay.links.put(req.ticket, data, lifetime) {
        return Err(ApiError::TooManyRequests);
    }
    Ok((
        StatusCode::CREATED,
        Json(CreateResponse {
            expires_in: lifetime.as_secs(),
        }),
    ))
}

#[derive(Deserialize)]
pub(crate) struct HandOverRequest {
    ticket: String,
    /// What the other device is to get, encrypted with the code it showed, in standard base64.
    data: String,
}

/// `POST /v1/handoffs`: leaves something for the device that showed a code. No account is
/// asked for: the ticket comes from that code, which only who saw it has.
pub(crate) async fn hand_over(
    State(relay): State<Arc<Relay>>,
    client: Client,
    Json(req): Json<HandOverRequest>,
) -> Result<StatusCode, ApiError> {
    check_ticket(&req.ticket)?;
    let data = STANDARD
        .decode(&req.data)
        .ok()
        .filter(|d| !d.is_empty() && d.len() <= MAX_DATA_BYTES)
        .ok_or(ApiError::BadRequest("invalid data"))?;
    let limits = &relay.limits;
    let key = format!("handoff {}", client.0);
    if !relay
        .counters
        .try_add(&key, 1, limits.handoffs_per_hour, HOUR)
        || !relay.links.put(req.ticket, data, limits.link_lifetime)
    {
        return Err(ApiError::TooManyRequests);
    }
    Ok(StatusCode::CREATED)
}

/// `POST /v1/handoffs/claim`: what waits under a ticket, once; 204 while nothing does, for
/// the device that showed a code and asks until a phone has scanned it.
pub(crate) async fn poll(
    State(relay): State<Arc<Relay>>,
    client: Client,
    Json(req): Json<ClaimRequest>,
) -> Result<Response, ApiError> {
    check_ticket(&req.ticket)?;
    let polls = format!("handoff-poll {}", client.0);
    if !relay
        .counters
        .try_add(&polls, 1, relay.limits.handoff_polls_per_hour, HOUR)
    {
        return Err(ApiError::TooManyRequests);
    }
    Ok(match relay.links.take(&req.ticket) {
        Some(data) => Json(ClaimResponse {
            data: STANDARD.encode(data),
        })
        .into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    })
}

#[derive(Deserialize)]
pub(crate) struct ClaimRequest {
    ticket: String,
}

#[derive(Serialize)]
pub(crate) struct ClaimResponse {
    data: String,
}

pub(crate) async fn claim(
    State(relay): State<Arc<Relay>>,
    client: Client,
    Json(req): Json<ClaimRequest>,
) -> Result<Json<ClaimResponse>, ApiError> {
    check_ticket(&req.ticket)?;
    // Tickets can't be guessed; this only keeps a client from hammering the endpoint.
    let claims = format!("link-claim {}", client.0);
    if !relay
        .counters
        .try_add(&claims, 1, relay.limits.link_claims_per_hour, HOUR)
    {
        return Err(ApiError::TooManyRequests);
    }
    let data = relay.links.take(&req.ticket).ok_or(ApiError::LinkGone)?;
    Ok(Json(ClaimResponse {
        data: STANDARD.encode(data),
    }))
}

#[cfg(test)]
mod tests;
