//! Which versions of the app the relay still answers.
//!
//! The app says its version with every request (`ezcount-version: 0.5.0`). A relay with a
//! minimum ([`crate::Settings::min_app_version`]) answers 426 to an older app, which then tells
//! its user to update it, instead of failing in ways nobody can read. An app from before
//! versions were sent says nothing: it is older than any minimum.
//!
//! This is for the day a change can't be made the usual way, by only adding to the API (a
//! flaw in what old apps send, say). Raise the minimum only once the newer app is out
//! everywhere people get it: until they update, they can't sync.

use axum::extract::{Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::str::FromStr;
use std::sync::Arc;

use crate::{ApiError, Relay};

/// The header the app's version comes in.
pub(crate) const VERSION_HEADER: &str = "ezcount-version";

/// A version of the app, as releases number them: `major.minor.patch`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AppVersion(u32, u32, u32);

impl FromStr for AppVersion {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        // Digits only: `u32::from_str` would take a leading `+`.
        let number = |n: &str| {
            n.bytes()
                .all(|b| b.is_ascii_digit())
                .then(|| n.parse::<u32>().ok())
                .flatten()
        };
        let numbers: Vec<Option<u32>> = text.trim().split('.').map(number).collect();
        match numbers[..] {
            [Some(major), Some(minor), Some(patch)] => Ok(Self(major, minor, patch)),
            _ => Err(format!(
                "{text:?} is not a version like 1.2.3 (major.minor.patch)"
            )),
        }
    }
}

impl std::fmt::Display for AppVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// Whether a request is refused for coming from an app older than `oldest`. Only the API the
/// app uses is: the pages are for browsers, and so is reading the feedback
/// (`GET /v1/feedback`, from the relay's own page).
fn too_old(oldest: Option<AppVersion>, method: &Method, path: &str, sent: Option<&str>) -> bool {
    let Some(oldest) = oldest else {
        return false;
    };
    let from_app = path.starts_with("/v1/") && !(path == "/v1/feedback" && method == Method::GET);
    // A version that can't be read is no better than none.
    from_app
        && sent
            .and_then(|v| v.parse::<AppVersion>().ok())
            .is_none_or(|v| v < oldest)
}

/// Answers 426 to an app the relay no longer serves, before anything else looks at its
/// request.
pub(crate) async fn require(
    State(relay): State<Arc<Relay>>,
    request: Request,
    next: Next,
) -> Response {
    let sent = request
        .headers()
        .get(VERSION_HEADER)
        .and_then(|value| value.to_str().ok());
    if too_old(
        relay.min_app_version,
        request.method(),
        request.uri().path(),
        sent,
    ) {
        return ApiError::UpdateRequired.into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests;
