//! Accounts: a username, the hashes of its login and recovery tokens, and the account's
//! key wrapped with each, which only the client can open.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

use crate::limits::{Client, HOUR, LOGIN_WINDOW};
use crate::updates::{check_group_id, stored_hash};
use crate::{with_db, ApiError, Relay};

/// Same rule as the app: 3 to 32 of `a-z 0-9 . _ -`, starting with a letter or digit.
pub(crate) fn check_username(username: &str) -> Result<(), ApiError> {
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

pub(crate) fn token_hash(token: &str) -> Result<Vec<u8>, ApiError> {
    if token.len() < 16 {
        return Err(ApiError::BadRequest("invalid token"));
    }
    Ok(Sha256::digest(token.as_bytes()).to_vec())
}

#[derive(Deserialize)]
pub(crate) struct SignUpRequest {
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
pub(crate) struct SignUpResponse {
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

pub(crate) async fn sign_up(
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
pub(crate) struct LogInRequest {
    username: String,
    login_token: String,
}

#[derive(Serialize)]
pub(crate) struct LogInResponse {
    account_id: String,
    wrapped_key: String,
}

impl Relay {
    /// Refuses logins for a username that failed too often recently, from this client or
    /// from everywhere.
    pub(crate) fn check_throttle(&self, username: &str, client: &Client) -> Result<(), ApiError> {
        let (from_client, overall) = login_keys(username, client);
        let limits = &self.limits;
        if self.counters.count(&from_client, LOGIN_WINDOW) >= limits.login_failures_per_client
            || self.counters.count(&overall, LOGIN_WINDOW) >= limits.login_failures_per_username
        {
            return Err(ApiError::TooManyAttempts);
        }
        Ok(())
    }

    pub(crate) fn record_login(&self, username: &str, client: &Client, success: bool) {
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

pub(crate) async fn log_in(
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
pub(crate) struct RecoverRequest {
    username: String,
    recovery_token: String,
}

/// Like logging in, with the recovery key: hands back the account key it encrypts.
pub(crate) async fn recover(
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
pub(crate) struct CredentialsRequest {
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
pub(crate) async fn update_credentials(
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

#[cfg(test)]
mod tests;
