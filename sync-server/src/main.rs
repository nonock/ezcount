//! Runs the ezcount sync relay.
//!
//! Configuration (environment variables):
//! - `EZCOUNT_SYNC_ADDR`: listen address, default `0.0.0.0:8787`
//! - `EZCOUNT_SYNC_DB`: SQLite file, default `ezcount-sync.sqlite3`
//! - `EZCOUNT_ANDROID_PACKAGE` and `EZCOUNT_ANDROID_CERT_SHA256` (comma-separated
//!   fingerprints): the Android app allowed to open this relay's invite links directly
//! - `EZCOUNT_CLIENT_IP_HEADER`: header holding the client's address, set by the reverse proxy
//!   in front of the relay (`Fly-Client-IP` on Fly.io, `X-Forwarded-For` behind Caddy). Only
//!   behind such a proxy: otherwise clients could pick their own address
//! - `EZCOUNT_MAX_STORAGE_MB` (default 1024) and `EZCOUNT_MAX_DOCUMENT_MB` (default 50):
//!   stored data in all, and per group or account. Keep the first under the disk's size
//! - `EZCOUNT_WEB_DIR`: the web version to serve at `/` (`bun run build:web` writes it to
//!   `sync-server/web`); none when unset or without an `index.html`
//! - `EZCOUNT_RATES_URL`: the service asked for exchange rates, Frankfurter by default
//!   (`<url>/USD/EUR?date=…` answering `{"date": …, "rate": …}`). Set it to `off` for a relay
//!   that makes no requests of its own: the app then suggests no rates
//! - `EZCOUNT_CONTACT`: how to reach whoever runs the relay (an e-mail address or a page's
//!   address), shown on its `/privacy` and `/delete-account` pages
//! - `EZCOUNT_MIN_APP_VERSION`: the oldest version of the app the relay answers (`0.5.0`).
//!   Older ones, and those from before the app said its version, are told to update. Every
//!   app is answered when unset

use ezcount_sync_server::{AndroidApp, Limits, Settings, DEFAULT_RATES_URL};
use std::path::PathBuf;

fn megabytes(name: &str, default: u64) -> Result<u64, String> {
    match std::env::var(name) {
        Ok(value) => value
            .trim()
            .parse::<u64>()
            .ok()
            .filter(|&mb| mb > 0)
            .map(|mb| mb * 1024 * 1024)
            .ok_or_else(|| format!("{name} must be a number of megabytes above 0, not {value:?}")),
        Err(_) => Ok(default),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::var("EZCOUNT_SYNC_ADDR").unwrap_or_else(|_| "0.0.0.0:8787".into());
    let db = PathBuf::from(
        std::env::var("EZCOUNT_SYNC_DB").unwrap_or_else(|_| "ezcount-sync.sqlite3".into()),
    );
    let defaults = Limits::default();
    let settings = Settings {
        android_app: match (
            std::env::var("EZCOUNT_ANDROID_PACKAGE"),
            std::env::var("EZCOUNT_ANDROID_CERT_SHA256"),
        ) {
            (Ok(package), Ok(certs)) => Some(AndroidApp {
                package,
                cert_sha256: certs.split(',').map(|c| c.trim().to_string()).collect(),
            }),
            _ => None,
        },
        client_ip_header: match std::env::var("EZCOUNT_CLIENT_IP_HEADER") {
            Ok(name) => Some(name.trim().parse().map_err(|_| {
                format!("EZCOUNT_CLIENT_IP_HEADER is not a valid header name: {name:?}")
            })?),
            Err(_) => None,
        },
        limits: Limits {
            max_total_bytes: megabytes("EZCOUNT_MAX_STORAGE_MB", defaults.max_total_bytes)?,
            max_document_bytes: megabytes("EZCOUNT_MAX_DOCUMENT_MB", defaults.max_document_bytes)?,
            ..defaults
        },
        web_dir: std::env::var_os("EZCOUNT_WEB_DIR").map(PathBuf::from),
        rates_url: match std::env::var("EZCOUNT_RATES_URL") {
            Ok(url) if matches!(url.trim(), "" | "off") => None,
            Ok(url) => Some(url.trim().to_string()),
            Err(_) => Some(DEFAULT_RATES_URL.to_string()),
        },
        admin_token: std::env::var("EZCOUNT_ADMIN_TOKEN").ok(),
        contact: std::env::var("EZCOUNT_CONTACT").ok(),
        min_app_version: match std::env::var("EZCOUNT_MIN_APP_VERSION") {
            Ok(version) if version.trim().is_empty() => None,
            Ok(version) => Some(
                version
                    .parse()
                    .map_err(|e| format!("EZCOUNT_MIN_APP_VERSION: {e}"))?,
            ),
            Err(_) => None,
        },
    };

    println!(
        "limits: {} MB stored, {} MB per document; client address from {}",
        settings.limits.max_total_bytes / 1024 / 1024,
        settings.limits.max_document_bytes / 1024 / 1024,
        settings
            .client_ip_header
            .as_ref()
            .map_or("the connection", |h| h.as_str()),
    );
    if let Some(dir) = &settings.web_dir {
        match dir.join("index.html").is_file() {
            true => println!("serving the web version from {}", dir.display()),
            false => println!("no web version in {} (no index.html)", dir.display()),
        }
    }
    match &settings.rates_url {
        Some(url) => println!("exchange rates from {url}"),
        None => println!("no exchange rates"),
    }
    if settings
        .admin_token
        .as_deref()
        .is_some_and(|t| !t.is_empty())
    {
        println!("feedback can be read at /feedback with the admin token");
    }
    if let Some(oldest) = settings.min_app_version {
        println!("apps older than {oldest} are told to update");
    }
    if let Some(app) = &settings.android_app {
        println!("invite links open the Android app {}", app.package);
    }
    let relay = ezcount_sync_server::Relay::open_with(&db, settings)?;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!(
        "ezcount sync relay listening on http://{} (data: {})",
        listener.local_addr()?,
        db.display()
    );
    tokio::select! {
        result = ezcount_sync_server::serve(listener, relay) => result?,
        _ = tokio::signal::ctrl_c() => println!("shutting down"),
    }
    Ok(())
}
