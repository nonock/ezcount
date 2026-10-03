//! Limits that keep one client from filling the relay's disk or locking other people out.
//!
//! Counted per client network in memory, over fixed windows: they reset when the relay
//! restarts, which only ever loosens them for a while.

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use axum::http::HeaderName;
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::Relay;

const MIB: u64 = 1024 * 1024;
pub(crate) const HOUR: Duration = Duration::from_secs(60 * 60);
pub(crate) const LOGIN_WINDOW: Duration = Duration::from_secs(15 * 60);

/// Sizes and rates. `Default` suits a small relay for friends and family.
#[derive(Clone, Debug)]
pub struct Limits {
    /// Stored bytes per document (group or account). A busy group stays far below.
    pub max_document_bytes: u64,
    /// Stored bytes for all documents: keep it under the disk's size.
    pub max_total_bytes: u64,
    /// Bytes one client network may upload per hour.
    pub upload_bytes_per_hour: u64,
    /// Documents one client network may create per hour (new groups; sign-ups count apart).
    pub new_documents_per_hour: u64,
    /// Accounts one client network may create per hour.
    pub sign_ups_per_hour: u64,
    /// Failed logins (password or recovery key) per username from one client network, per
    /// `LOGIN_WINDOW`. Past it, that network can't try that username for a while.
    pub login_failures_per_client: u64,
    /// Failed logins per username from all networks together, per `LOGIN_WINDOW`: bounds
    /// guessing spread over many addresses. Locking someone out takes that many addresses.
    pub login_failures_per_username: u64,
    /// Exchange rates one client network may have the relay look up per hour. Rates the relay
    /// already knows don't count.
    pub rate_lookups_per_hour: u64,
    /// The same for all networks together: the most the relay asks the rate service per hour.
    pub rate_lookups_per_hour_total: u64,
    /// How long a login link can be used: the time to pick up the phone and scan.
    pub link_lifetime: Duration,
    /// Login links one client network may try to claim per hour.
    pub link_claims_per_hour: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_document_bytes: 50 * MIB,
            max_total_bytes: 1024 * MIB,
            upload_bytes_per_hour: 50 * MIB,
            new_documents_per_hour: 30,
            sign_ups_per_hour: 10,
            login_failures_per_client: 5,
            login_failures_per_username: 50,
            rate_lookups_per_hour: 120,
            rate_lookups_per_hour_total: 600,
            link_lifetime: Duration::from_secs(120),
            link_claims_per_hour: 60,
        }
    }
}

/// Counts per key over a fixed window.
#[derive(Default)]
pub(crate) struct Counters(Mutex<HashMap<String, (u64, Instant)>>);

impl Counters {
    fn entries(&self) -> std::sync::MutexGuard<'_, HashMap<String, (u64, Instant)>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The count for `key` in its current window.
    pub(crate) fn count(&self, key: &str, window: Duration) -> u64 {
        match self.entries().get(key) {
            Some(&(count, since)) if since.elapsed() < window => count,
            _ => 0,
        }
    }

    /// Adds `amount` to `key` if the result stays within `limit`, and says whether it did.
    pub(crate) fn try_add(&self, key: &str, amount: u64, limit: u64, window: Duration) -> bool {
        let mut entries = self.entries();
        // Keep the map small when someone sprays keys (usernames, addresses).
        if entries.len() > 10_000 {
            entries.retain(|_, (_, since)| since.elapsed() < window.max(HOUR));
        }
        let entry = entries
            .entry(key.to_string())
            .or_insert((0, Instant::now()));
        if entry.1.elapsed() >= window {
            *entry = (0, Instant::now());
        }
        if entry.0.saturating_add(amount) > limit {
            return false;
        }
        entry.0 += amount;
        true
    }

    pub(crate) fn clear(&self, key: &str) {
        self.entries().remove(key);
    }
}

/// The client's network, as a rate-limit key: its IPv4 address, or its IPv6 /64, since one
/// household or server gets a whole /64.
pub(crate) struct Client(pub String);

impl FromRequestParts<Arc<Relay>> for Client {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, relay: &Arc<Relay>) -> Result<Self, Infallible> {
        let ip = client_ip(parts, relay.client_ip_header.as_ref());
        Ok(Client(match ip {
            Some(IpAddr::V6(ip)) => {
                let s = ip.segments();
                format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
            }
            Some(ip) => ip.to_string(),
            None => "unknown".to_string(),
        }))
    }
}

fn client_ip(parts: &Parts, header: Option<&HeaderName>) -> Option<IpAddr> {
    match header {
        // Set by a reverse proxy in front of the relay. The last entry is the one the nearest
        // proxy added; anything before it came from the client and proves nothing.
        Some(name) => parts
            .headers
            .get(name)?
            .to_str()
            .ok()?
            .rsplit(',')
            .next()?
            .trim()
            .parse()
            .ok(),
        None => parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|info| info.0.ip()),
    }
}
