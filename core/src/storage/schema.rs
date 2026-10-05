//! The database's tables, their upgrades, and the two rows a document has.

use super::*;

pub(super) const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS groups (
        id         TEXT PRIMARY KEY,
        snapshot   BLOB NOT NULL,
        updated_at TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS group_sync (
        group_id       TEXT PRIMARY KEY,
        server_url     TEXT NOT NULL,
        secret         TEXT NOT NULL,
        server_vv      BLOB NOT NULL,
        cursor         INTEGER NOT NULL,
        last_synced_at TEXT,
        last_error     TEXT,
        relay_id       TEXT
    );
    CREATE TABLE IF NOT EXISTS account (
        id         INTEGER PRIMARY KEY CHECK (id = 1),
        server_url TEXT NOT NULL,
        username   TEXT NOT NULL,
        account_id TEXT NOT NULL
    );
";

/// Current schema version, stored in SQLite's `user_version`.
/// 2: sync is end-to-end encrypted.
/// 3: accounts (a new table, nothing to migrate).
/// 4: `group_sync.relay_id`.
pub(super) const SCHEMA_VERSION: i64 = 4;

pub(super) fn upgrade_schema(conn: &Connection, warnings: &mut Vec<String>) -> Res<()> {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(db_err)?;
    if version < 2 {
        // Groups shared before encryption used a protocol that no longer exists. The groups
        // stay on this device; only their sharing is turned off so they can be shared again.
        let reset = conn.execute("DELETE FROM group_sync", []).map_err(db_err)?;
        if reset > 0 {
            warnings.push(format!(
                "Sharing was turned off for {reset} group(s) because sync is now end-to-end \
                 encrypted. The groups are still on this device; share them again to get new \
                 invite codes."
            ));
        }
    }
    let has_relay_id: bool = conn
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM pragma_table_info('group_sync') WHERE name = 'relay_id')",
            [],
            |r| r.get(0),
        )
        .map_err(db_err)?;
    if !has_relay_id {
        conn.execute("ALTER TABLE group_sync ADD COLUMN relay_id TEXT", [])
            .map_err(db_err)?;
    }
    conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
        .map_err(db_err)
}

pub(super) fn save_snapshot(conn: &Connection, id: &str, doc: &LoroDoc) -> Res<()> {
    let snapshot = doc
        .export(ExportMode::Snapshot)
        .map_err(|e| format!("Could not encode group: {e}"))?;
    conn.execute(
        "INSERT INTO groups (id, snapshot, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET snapshot = excluded.snapshot, updated_at = excluded.updated_at",
        params![id, snapshot, Utc::now().to_rfc3339()],
    )
    .map_err(db_err)?;
    Ok(())
}

pub(super) fn save_sync(conn: &Connection, id: &str, meta: &SyncMeta) -> Res<()> {
    conn.execute(
        "INSERT INTO group_sync
            (group_id, server_url, secret, server_vv, cursor, last_synced_at, last_error, relay_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(group_id) DO UPDATE SET
            server_url = excluded.server_url, secret = excluded.secret,
            server_vv = excluded.server_vv, cursor = excluded.cursor,
            last_synced_at = excluded.last_synced_at, last_error = excluded.last_error,
            relay_id = excluded.relay_id",
        params![
            id,
            meta.server_url,
            meta.secret.expose(),
            meta.server_vv.encode(),
            meta.cursor,
            meta.last_synced_at.map(|d| d.to_rfc3339()),
            meta.last_error,
            meta.relay_id,
        ],
    )
    .map_err(db_err)?;
    Ok(())
}
