//! Runs the ezcount sync relay.
//!
//! Configuration (environment variables):
//! - `EZCOUNT_SYNC_ADDR`: listen address, default `0.0.0.0:8787`
//! - `EZCOUNT_SYNC_DB`: SQLite file, default `ezcount-sync.sqlite3`
//! - `EZCOUNT_ANDROID_PACKAGE` and `EZCOUNT_ANDROID_CERT_SHA256` (comma-separated
//!   fingerprints): the Android app allowed to open this relay's invite links directly

use ezcount_sync_server::AndroidApp;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::var("EZCOUNT_SYNC_ADDR").unwrap_or_else(|_| "0.0.0.0:8787".into());
    let db = PathBuf::from(
        std::env::var("EZCOUNT_SYNC_DB").unwrap_or_else(|_| "ezcount-sync.sqlite3".into()),
    );
    let android_app = match (
        std::env::var("EZCOUNT_ANDROID_PACKAGE"),
        std::env::var("EZCOUNT_ANDROID_CERT_SHA256"),
    ) {
        (Ok(package), Ok(certs)) => Some(AndroidApp {
            package,
            cert_sha256: certs.split(',').map(|c| c.trim().to_string()).collect(),
        }),
        _ => None,
    };

    let relay = ezcount_sync_server::Relay::open(&db)?;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!(
        "ezcount sync relay listening on http://{} (data: {})",
        listener.local_addr()?,
        db.display()
    );
    if let Some(app) = &android_app {
        println!("invite links open the Android app {}", app.package);
    }
    tokio::select! {
        result = ezcount_sync_server::serve(listener, relay, android_app) => result?,
        _ = tokio::signal::ctrl_c() => println!("shutting down"),
    }
    Ok(())
}
