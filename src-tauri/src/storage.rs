//! Local persistence: one Loro snapshot per group, stored in SQLite.
//!
//! SQLite gives atomic, crash-safe writes (important on Android, where the OS kills apps
//! at any moment). Data that cannot be read is reported and left in place, never dropped.

use chrono::{DateTime, Utc};
use loro::{ExportMode, IdSpan, LoroDoc, VersionVector};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::doc;
use crate::models::Group;

type Res<T> = Result<T, String>;

fn db_err(e: rusqlite::Error) -> String {
    format!("Database error: {e}")
}

const SCHEMA: &str = "
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
        last_error     TEXT
    );
";

/// Sync state of a shared group.
#[derive(Debug, Clone)]
pub struct SyncMeta {
    pub server_url: String,
    pub secret: String,
    /// Operations the server is known to hold. Local operations beyond this still need pushing.
    pub server_vv: VersionVector,
    /// Highest server sequence number already imported.
    pub cursor: i64,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

impl SyncMeta {
    pub fn new(server_url: String, secret: String) -> Self {
        Self {
            server_url,
            secret,
            server_vv: VersionVector::new(),
            cursor: 0,
            last_synced_at: None,
            last_error: None,
        }
    }
}

/// Imports one update received from the server and records its operations as server-held.
/// Fails if the update depends on history this device has not received yet.
pub fn import_remote_update(doc: &LoroDoc, bytes: &[u8], server_vv: &mut VersionVector) -> Res<()> {
    let status = doc
        .import(bytes)
        .map_err(|e| format!("Could not apply a change from the server: {e}"))?;
    for (peer, (start, end)) in status.success.iter() {
        server_vv.extend_to_include(IdSpan::new(*peer, *start, *end));
    }
    if status.pending.is_some() {
        return Err(
            "A change from the server is missing part of its history; will retry".to_string(),
        );
    }
    Ok(())
}

pub struct Store {
    conn: Connection,
    docs: HashMap<String, LoroDoc>,
    sync: HashMap<String, SyncMeta>,
}

impl Store {
    /// Opens (or creates) the database and migrates the old JSON file if present.
    /// Returns warnings about anything that could not be loaded; that data stays on disk.
    pub fn open(db_path: &Path, legacy_json: &Path) -> Res<(Self, Vec<String>)> {
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Could not create data folder {}: {e}", parent.display()))?;
        }
        let conn = Connection::open(db_path).map_err(db_err)?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = FULL;")
            .map_err(db_err)?;
        conn.execute_batch(SCHEMA).map_err(db_err)?;
        let mut warnings = Vec::new();
        upgrade_schema(&conn, &mut warnings)?;

        let mut store = Self {
            conn,
            docs: HashMap::new(),
            sync: HashMap::new(),
        };
        store.load(&mut warnings)?;
        store.migrate_legacy_json(legacy_json, &mut warnings);
        Ok((store, warnings))
    }

    fn load(&mut self, warnings: &mut Vec<String>) -> Res<()> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, snapshot FROM groups")
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
            })
            .map_err(db_err)?;
        for row in rows {
            let (id, snapshot) = row.map_err(db_err)?;
            match LoroDoc::from_snapshot(&snapshot) {
                Ok(doc) => {
                    self.docs.insert(id, doc);
                }
                Err(e) => warnings.push(format!(
                    "Group {id} could not be loaded ({e}). Its data was kept in the database."
                )),
            }
        }

        let mut stmt = self
            .conn
            .prepare(
                "SELECT group_id, server_url, secret, server_vv, cursor, last_synced_at, last_error
                 FROM group_sync",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    SyncMeta {
                        server_url: r.get(1)?,
                        secret: r.get(2)?,
                        // An unreadable version only means everything gets pushed again.
                        server_vv: VersionVector::decode(&r.get::<_, Vec<u8>>(3)?)
                            .unwrap_or_default(),
                        cursor: r.get(4)?,
                        last_synced_at: r
                            .get::<_, Option<String>>(5)?
                            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                            .map(|d| d.with_timezone(&Utc)),
                        last_error: r.get(6)?,
                    },
                ))
            })
            .map_err(db_err)?;
        for row in rows {
            let (id, meta) = row.map_err(db_err)?;
            self.sync.insert(id, meta);
        }
        Ok(())
    }

    /// One-time import of the pre-SQLite `ezcount_data.json`. On success the file is renamed,
    /// not deleted. If it cannot be parsed it is left untouched and a warning is returned.
    fn migrate_legacy_json(&mut self, path: &Path, warnings: &mut Vec<String>) {
        if !path.exists() {
            return;
        }
        let groups: Vec<Group> = match fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
        {
            Ok(groups) => groups,
            Err(e) => {
                warnings.push(format!(
                    "The old data file {} could not be read ({e}). It was left untouched.",
                    path.display()
                ));
                return;
            }
        };

        let result = (|| -> Res<Vec<(String, LoroDoc)>> {
            let tx = self.conn.unchecked_transaction().map_err(db_err)?;
            let mut migrated = Vec::new();
            for group in groups.iter().filter(|g| !self.docs.contains_key(&g.id)) {
                let doc = doc::doc_from_legacy(group)?;
                save_snapshot(&tx, &group.id, &doc)?;
                migrated.push((group.id.clone(), doc));
            }
            tx.commit().map_err(db_err)?;
            Ok(migrated)
        })();

        match result {
            Ok(migrated) => {
                self.docs.extend(migrated);
                let done = path.with_extension("migrated.json");
                if let Err(e) = fs::rename(path, &done) {
                    // Harmless: already-migrated groups are skipped on the next launch.
                    eprintln!("[storage] could not rename {}: {e}", path.display());
                }
            }
            Err(e) => warnings.push(format!(
                "The old data file {} could not be migrated ({e}). It was left untouched.",
                path.display()
            )),
        }
    }

    // -- Groups --------------------------------------------------------------

    pub fn contains(&self, id: &str) -> bool {
        self.docs.contains_key(id)
    }

    pub fn doc(&self, id: &str) -> Res<&LoroDoc> {
        self.docs
            .get(id)
            .ok_or_else(|| "Group not found".to_string())
    }

    pub fn group(&self, id: &str) -> Res<Group> {
        doc::read_group(self.doc(id)?)
    }

    /// All readable groups, oldest first.
    pub fn groups(&self) -> Vec<Group> {
        let mut groups: Vec<Group> = self
            .docs
            .iter()
            .filter_map(|(id, d)| match doc::read_group(d) {
                Ok(g) => Some(g),
                Err(e) => {
                    eprintln!("[storage] group {id} is unreadable: {e}");
                    None
                }
            })
            .collect();
        groups.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        groups
    }

    /// Adds a group created or joined on this device.
    pub fn insert(&mut self, doc: LoroDoc, sync: Option<SyncMeta>) -> Res<Group> {
        let group = doc::read_group(&doc)?;
        let tx = self.conn.unchecked_transaction().map_err(db_err)?;
        save_snapshot(&tx, &group.id, &doc)?;
        if let Some(meta) = &sync {
            save_sync(&tx, &group.id, meta)?;
        }
        tx.commit().map_err(db_err)?;
        self.docs.insert(group.id.clone(), doc);
        if let Some(meta) = sync {
            self.sync.insert(group.id.clone(), meta);
        }
        Ok(group)
    }

    /// Applies a change to a group's document and saves it.
    pub fn update(&mut self, id: &str, change: impl FnOnce(&LoroDoc) -> Res<()>) -> Res<Group> {
        let doc = self.doc(id)?;
        change(doc)?;
        doc.commit();
        save_snapshot(&self.conn, id, doc)?;
        doc::read_group(doc)
    }

    /// Removes a group from this device. For a shared group, other members keep it.
    pub fn delete(&mut self, id: &str) -> Res<bool> {
        let tx = self.conn.unchecked_transaction().map_err(db_err)?;
        let deleted = tx
            .execute("DELETE FROM groups WHERE id = ?1", [id])
            .map_err(db_err)?
            > 0;
        tx.execute("DELETE FROM group_sync WHERE group_id = ?1", [id])
            .map_err(db_err)?;
        tx.commit().map_err(db_err)?;
        self.docs.remove(id);
        self.sync.remove(id);
        Ok(deleted)
    }

    // -- Sync ----------------------------------------------------------------

    pub fn sync_meta(&self, id: &str) -> Option<&SyncMeta> {
        self.sync.get(id)
    }

    pub fn synced_ids(&self) -> Vec<String> {
        self.sync.keys().cloned().collect()
    }

    pub fn set_sync(&mut self, id: &str, meta: SyncMeta) -> Res<()> {
        self.doc(id)?;
        save_sync(&self.conn, id, &meta)?;
        self.sync.insert(id.to_string(), meta);
        Ok(())
    }

    pub fn clear_sync(&mut self, id: &str) -> Res<()> {
        self.conn
            .execute("DELETE FROM group_sync WHERE group_id = ?1", [id])
            .map_err(db_err)?;
        self.sync.remove(id);
        Ok(())
    }

    fn sync_mut(&mut self, id: &str) -> Res<&mut SyncMeta> {
        self.sync
            .get_mut(id)
            .ok_or_else(|| "This group is not shared".to_string())
    }

    /// Records that the server now holds everything up to `pushed`.
    pub fn mark_pushed(&mut self, id: &str, pushed: &VersionVector) -> Res<()> {
        let meta = self.sync_mut(id)?;
        meta.server_vv.merge(pushed);
        let meta = meta.clone();
        save_sync(&self.conn, id, &meta)
    }

    /// Applies updates pulled from the server, in order. Progress is saved even if a later
    /// update fails, so nothing is fetched twice. Returns whether the group changed.
    pub fn import_remote(&mut self, id: &str, updates: &[(i64, Vec<u8>)]) -> Res<bool> {
        let doc = self
            .docs
            .get(id)
            .ok_or_else(|| "Group not found".to_string())?;
        let meta = self
            .sync
            .get_mut(id)
            .ok_or_else(|| "This group is not shared".to_string())?;
        let before = doc.oplog_vv();
        let mut failure = None;
        for (seq, bytes) in updates {
            if let Err(e) = import_remote_update(doc, bytes, &mut meta.server_vv) {
                failure = Some(e);
                break;
            }
            meta.cursor = *seq;
        }
        let changed = doc.oplog_vv() != before;

        let tx = self.conn.unchecked_transaction().map_err(db_err)?;
        save_snapshot(&tx, id, doc)?;
        save_sync(&tx, id, meta)?;
        tx.commit().map_err(db_err)?;

        match failure {
            Some(e) => Err(e),
            None => Ok(changed),
        }
    }

    /// Saves the outcome of a sync attempt for display.
    pub fn record_sync_result(&mut self, id: &str, result: &Res<bool>) {
        let Some(meta) = self.sync.get_mut(id) else {
            return;
        };
        match result {
            Ok(_) => {
                meta.last_synced_at = Some(Utc::now());
                meta.last_error = None;
            }
            Err(e) => meta.last_error = Some(e.clone()),
        }
        let meta = meta.clone();
        if let Err(e) = save_sync(&self.conn, id, &meta) {
            eprintln!("[storage] could not save sync status for {id}: {e}");
        }
    }
}

/// Current schema version, stored in SQLite's `user_version`.
/// 2: sync is end-to-end encrypted.
const SCHEMA_VERSION: i64 = 2;

fn upgrade_schema(conn: &Connection, warnings: &mut Vec<String>) -> Res<()> {
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
    conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
        .map_err(db_err)
}

fn save_snapshot(conn: &Connection, id: &str, doc: &LoroDoc) -> Res<()> {
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

fn save_sync(conn: &Connection, id: &str, meta: &SyncMeta) -> Res<()> {
    conn.execute(
        "INSERT INTO group_sync (group_id, server_url, secret, server_vv, cursor, last_synced_at, last_error)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(group_id) DO UPDATE SET
            server_url = excluded.server_url, secret = excluded.secret,
            server_vv = excluded.server_vv, cursor = excluded.cursor,
            last_synced_at = excluded.last_synced_at, last_error = excluded.last_error",
        params![
            id,
            meta.server_url,
            meta.secret,
            meta.server_vv.encode(),
            meta.cursor,
            meta.last_synced_at.map(|d| d.to_rfc3339()),
            meta.last_error,
        ],
    )
    .map_err(db_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("ezcount-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn open(&self) -> (Store, Vec<String>) {
            Store::open(
                &self.0.join("db.sqlite3"),
                &self.0.join("ezcount_data.json"),
            )
            .unwrap()
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn changes_survive_reopen() {
        let dir = TempDir::new();
        let id = {
            let (mut store, _) = dir.open();
            let group = store
                .insert(
                    doc::new_group_doc("Trip", "EUR", &["Alice".into()]).unwrap(),
                    None,
                )
                .unwrap();
            store
                .update(&group.id, |d| doc::add_participant(d, "Bob"))
                .unwrap();
            group.id
        };
        let (store, warnings) = dir.open();
        assert!(warnings.is_empty());
        assert_eq!(store.group(&id).unwrap().participants.len(), 2);
    }

    #[test]
    fn migrates_legacy_json_once() {
        let dir = TempDir::new();
        let legacy = r#"[{"id":"g1","name":"Old","currency":"EUR","created_at":"2025-01-01T00:00:00Z",
            "participants":[{"id":"p1","name":"Alice"}],
            "expenses":[{"id":"e1","group_id":"g1","title":"Tea","amount_cents":250,"paid_by":"p1",
              "splits":[{"participant_id":"p1","shares":1}],
              "created_at":"2025-01-02T00:00:00Z","updated_at":"2025-01-02T00:00:00Z"}]}]"#;
        fs::write(dir.0.join("ezcount_data.json"), legacy).unwrap();

        let (store, warnings) = dir.open();
        assert!(warnings.is_empty(), "{warnings:?}");
        let g = store.group("g1").unwrap();
        assert_eq!(g.participants[0].id, "p1");
        assert_eq!(g.expenses[0].amount_cents, 250);
        assert!(!dir.0.join("ezcount_data.json").exists());
        assert!(dir.0.join("ezcount_data.migrated.json").exists());
        drop(store);

        let (store, _) = dir.open();
        assert_eq!(store.groups().len(), 1);
    }

    #[test]
    fn unreadable_legacy_json_is_kept_and_reported() {
        let dir = TempDir::new();
        let path = dir.0.join("ezcount_data.json");
        fs::write(&path, "{ not json").unwrap();

        let (mut store, warnings) = dir.open();
        assert_eq!(warnings.len(), 1);
        // Creating a group must not touch the old file.
        store
            .insert(
                doc::new_group_doc("New", "EUR", &["A".into()]).unwrap(),
                None,
            )
            .unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");
    }

    #[test]
    fn pre_encryption_sharing_is_reset_but_groups_kept() {
        let dir = TempDir::new();
        let id = {
            let (mut store, _) = dir.open();
            let id = store
                .insert(
                    doc::new_group_doc("Old", "EUR", &["A".into()]).unwrap(),
                    None,
                )
                .unwrap()
                .id;
            store
                .set_sync(&id, SyncMeta::new("http://relay".into(), "old".into()))
                .unwrap();
            // Simulate a database written before encryption existed.
            store.conn.execute_batch("PRAGMA user_version = 0").unwrap();
            id
        };

        let (store, warnings) = dir.open();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(store.contains(&id));
        assert!(store.sync_meta(&id).is_none());

        drop(store);
        let (_, warnings) = dir.open();
        assert!(warnings.is_empty(), "reset happens only once");
    }

    #[test]
    fn corrupt_snapshot_is_reported_not_dropped() {
        let dir = TempDir::new();
        {
            let (store, _) = dir.open();
            store
                .conn
                .execute(
                    "INSERT INTO groups (id, snapshot, updated_at) VALUES ('bad', x'00ff', '')",
                    [],
                )
                .unwrap();
        }
        let (store, warnings) = dir.open();
        assert_eq!(warnings.len(), 1);
        let rows: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM groups WHERE id = 'bad'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(rows, 1);
    }
}
