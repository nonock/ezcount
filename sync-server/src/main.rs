//! Runs the ezcount sync relay.
//!
//! Configuration (environment variables):
//! - `EZCOUNT_SYNC_ADDR`: listen address, default `0.0.0.0:8787`
//! - `EZCOUNT_SYNC_DB`: SQLite file, default `ezcount-sync.sqlite3`

use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::var("EZCOUNT_SYNC_ADDR").unwrap_or_else(|_| "0.0.0.0:8787".into());
    let db = PathBuf::from(
        std::env::var("EZCOUNT_SYNC_DB").unwrap_or_else(|_| "ezcount-sync.sqlite3".into()),
    );

    let relay = ezcount_sync_server::Relay::open(&db)?;
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
