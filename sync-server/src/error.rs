//! What the API answers when it refuses.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug)]
pub(crate) enum ApiError {
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
    /// No exchange rate to suggest.
    NoRate,
    /// A login link that was used already, expired, or never existed.
    LinkGone,
    /// A document that went with its account (`accounts::delete`).
    DocumentGone,
    /// An app older than the relay still answers (`versions`).
    UpdateRequired,
    /// Something the relay depends on didn't answer.
    Unavailable(String),
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
            ApiError::NoRate => (StatusCode::NOT_FOUND, "no rate").into_response(),
            ApiError::LinkGone => (StatusCode::GONE, "link used or expired").into_response(),
            ApiError::DocumentGone => {
                (StatusCode::GONE, "this account was deleted").into_response()
            }
            ApiError::UpdateRequired => (
                StatusCode::UPGRADE_REQUIRED,
                "this version of the app is too old for this relay",
            )
                .into_response(),
            ApiError::Unavailable(e) => {
                eprintln!("[relay] {e}");
                (StatusCode::SERVICE_UNAVAILABLE, "try later").into_response()
            }
            ApiError::Internal(e) => {
                eprintln!("[relay] internal error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests;
