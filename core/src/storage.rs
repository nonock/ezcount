//! Local persistence: one Loro snapshot per group, stored in SQLite.
//!
//! SQLite gives atomic, crash-safe writes (important on Android, where the OS kills apps
//! at any moment). Data that cannot be read is reported and left in place, never dropped.
//!
//! When logged in, the account document (see `account`) is stored and synced exactly like a
//! group, under the account id, but it is never listed as a group.

use chrono::{DateTime, Utc};
use loro::{ExportMode, IdSpan, LoroDoc, VersionVector};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::crypto::Secret;
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

/// The account this device is logged into. Its key is the secret of the account document's
/// sync row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub server_url: String,
    pub username: String,
    pub account_id: String,
}

/// Sync state of a shared group.
#[derive(Debug, Clone)]
pub struct SyncMeta {
    pub server_url: String,
    pub secret: Secret,
    /// Operations the server is known to hold. Local operations beyond this still need pushing.
    pub server_vv: VersionVector,
    /// Highest server sequence number already imported.
    pub cursor: i64,
    /// Identity of the relay database that `server_vv` and `cursor` refer to.
    pub relay_id: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

impl SyncMeta {
    /// Whether this device ever uploaded to or read from the relay for this document.
    pub fn used_relay(&self) -> bool {
        self.cursor > 0 || self.server_vv.iter().next().is_some()
    }

    /// Forgets what the relay was thought to hold, so everything is uploaded and read again.
    fn start_over(&mut self, relay_id: Option<String>) {
        self.server_vv = VersionVector::new();
        self.cursor = 0;
        self.relay_id = relay_id;
    }

    pub fn new(server_url: String, secret: Secret) -> Self {
        Self {
            server_url,
            secret,
            server_vv: VersionVector::new(),
            cursor: 0,
            relay_id: None,
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
    session: Option<Session>,
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
        let (mut store, mut warnings) = Self::from_connection(conn)?;
        store.migrate_legacy_json(legacy_json, &mut warnings);
        Ok((store, warnings))
    }

    /// In the browser: opens (or creates) `name` in the SQLite VFS the web app registered as
    /// the default. No WAL, which needs shared memory the browser VFSs don't have, and no
    /// legacy JSON file to migrate.
    #[cfg(target_family = "wasm")]
    pub fn open_in_browser(name: &str) -> Res<(Self, Vec<String>)> {
        let conn = Connection::open(name).map_err(db_err)?;
        conn.execute_batch("PRAGMA synchronous = FULL;")
            .map_err(db_err)?;
        Self::from_connection(conn)
    }

    fn from_connection(conn: Connection) -> Res<(Self, Vec<String>)> {
        conn.execute_batch(SCHEMA).map_err(db_err)?;
        let mut warnings = Vec::new();
        upgrade_schema(&conn, &mut warnings)?;

        let mut store = Self {
            conn,
            docs: HashMap::new(),
            sync: HashMap::new(),
            session: None,
        };
        store.load(&mut warnings)?;
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
                "SELECT group_id, server_url, secret, server_vv, cursor, last_synced_at, last_error,
                        relay_id
                 FROM group_sync",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    SyncMeta {
                        server_url: r.get(1)?,
                        secret: Secret::new(r.get(2)?),
                        // An unreadable version only means everything gets pushed again.
                        server_vv: VersionVector::decode(&r.get::<_, Vec<u8>>(3)?)
                            .unwrap_or_default(),
                        cursor: r.get(4)?,
                        relay_id: r.get(7)?,
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

        self.session = self
            .conn
            .query_row(
                "SELECT server_url, username, account_id FROM account WHERE id = 1",
                [],
                |r| {
                    Ok(Session {
                        server_url: r.get(0)?,
                        username: r.get(1)?,
                        account_id: r.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(db_err)?;
        if let Some(session) = &self.session {
            // Without its document the account cannot work; start again from the relay.
            if !self.docs.contains_key(&session.account_id)
                || !self.sync.contains_key(&session.account_id)
            {
                warnings.push(
                    "Your account data on this device was unreadable. Log in again to restore it."
                        .to_string(),
                );
                self.conn
                    .execute("DELETE FROM account", [])
                    .map_err(db_err)?;
                self.session = None;
            }
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

    fn is_account(&self, id: &str) -> bool {
        self.session.as_ref().is_some_and(|s| s.account_id == id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.docs.contains_key(id) && !self.is_account(id)
    }

    /// Ids of the groups on this device.
    pub fn group_ids(&self) -> Vec<String> {
        self.docs
            .keys()
            .filter(|id| !self.is_account(id))
            .cloned()
            .collect()
    }

    pub fn doc(&self, id: &str) -> Res<&LoroDoc> {
        self.docs
            .get(id)
            .ok_or_else(|| "Group not found".to_string())
    }

    pub fn group(&self, id: &str) -> Res<Group> {
        doc::read_group(self.doc(id)?)
    }

    /// All readable groups, oldest first. A deleted one waiting to be dropped isn't listed.
    pub fn groups(&self) -> Vec<Group> {
        let mut groups: Vec<Group> = self
            .docs
            .iter()
            .filter(|(id, _)| !self.is_account(id))
            .filter_map(|(id, d)| match doc::read_group(d) {
                Ok(g) if g.deleted => None,
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
        self.update_doc(id, change)?;
        doc::read_group(self.doc(id)?)
    }

    /// Applies a change to any stored document (group or account) and saves it.
    fn update_doc(&mut self, id: &str, change: impl FnOnce(&LoroDoc) -> Res<()>) -> Res<()> {
        let doc = self
            .docs
            .get(id)
            .ok_or_else(|| "Document not found".to_string())?;
        change(doc)?;
        doc.commit();
        save_snapshot(&self.conn, id, doc)
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

    /// Shared groups, not including the account document.
    pub fn synced_ids(&self) -> Vec<String> {
        self.sync
            .keys()
            .filter(|id| !self.is_account(id))
            .cloned()
            .collect()
    }

    pub fn set_sync(&mut self, id: &str, meta: SyncMeta) -> Res<()> {
        self.docs
            .get(id)
            .ok_or_else(|| "Document not found".to_string())?;
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

    /// Compares the relay's id with the one this device last saw for `id`. A different id
    /// means the relay's database was reset or replaced and its sequence numbers started over,
    /// so everything is uploaded and read again. Returns whether that happened.
    pub fn check_relay(&mut self, id: &str, relay_id: Option<&str>) -> Res<bool> {
        let Some(relay_id) = relay_id else {
            // A relay too old to say; nothing to compare.
            return Ok(false);
        };
        let meta = self.sync_mut(id)?;
        let reset = match &meta.relay_id {
            Some(known) if known == relay_id => return Ok(false),
            Some(_) => {
                meta.start_over(Some(relay_id.to_string()));
                true
            }
            None => {
                meta.relay_id = Some(relay_id.to_string());
                false
            }
        };
        let meta = meta.clone();
        save_sync(&self.conn, id, &meta)?;
        Ok(reset)
    }

    /// The relay lost `id` although this device used it before: upload everything again.
    pub fn restart_sync(&mut self, id: &str) -> Res<()> {
        let meta = self.sync_mut(id)?;
        meta.start_over(None);
        let meta = meta.clone();
        save_sync(&self.conn, id, &meta)
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

impl Store {
    // -- Account -------------------------------------------------------------

    pub fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    /// Logs this device into an account whose document is `doc`, synced with `meta`.
    pub fn set_session(&mut self, session: Session, doc: LoroDoc, meta: SyncMeta) -> Res<()> {
        if self.session.is_some() {
            return Err("This device is already logged in".to_string());
        }
        let tx = self.conn.unchecked_transaction().map_err(db_err)?;
        save_snapshot(&tx, &session.account_id, &doc)?;
        save_sync(&tx, &session.account_id, &meta)?;
        tx.execute(
            "INSERT INTO account (id, server_url, username, account_id) VALUES (1, ?1, ?2, ?3)",
            params![session.server_url, session.username, session.account_id],
        )
        .map_err(db_err)?;
        tx.commit().map_err(db_err)?;
        self.docs.insert(session.account_id.clone(), doc);
        self.sync.insert(session.account_id.clone(), meta);
        self.session = Some(session);
        Ok(())
    }

    fn account_id(&self) -> Res<&str> {
        self.session
            .as_ref()
            .map(|s| s.account_id.as_str())
            .ok_or_else(|| "You are not logged in".to_string())
    }

    pub fn account_doc(&self) -> Res<&LoroDoc> {
        let id = self.account_id()?;
        self.docs
            .get(id)
            .ok_or_else(|| "Account data is missing".to_string())
    }

    pub fn update_account(&mut self, change: impl FnOnce(&LoroDoc) -> Res<()>) -> Res<()> {
        let id = self.account_id()?.to_string();
        self.update_doc(&id, change)
    }

    /// Whether the synced document `id` has edits the server doesn't hold yet.
    pub fn has_unpushed(&self, id: &str) -> bool {
        match (self.sync.get(id), self.docs.get(id)) {
            (Some(meta), Some(doc)) => !meta.server_vv.includes_vv(&doc.oplog_vv()),
            _ => false,
        }
    }

    /// Whether any synced document (group or account) has edits the server doesn't hold yet.
    pub fn has_unpushed_changes(&self) -> bool {
        self.sync.keys().any(|id| self.has_unpushed(id))
    }

    /// Logs out: removes the account and every group from this device.
    pub fn wipe(&mut self) -> Res<()> {
        let tx = self.conn.unchecked_transaction().map_err(db_err)?;
        tx.execute_batch("DELETE FROM groups; DELETE FROM group_sync; DELETE FROM account;")
            .map_err(db_err)?;
        tx.commit().map_err(db_err)?;
        self.docs.clear();
        self.sync.clear();
        self.session = None;
        Ok(())
    }
}

/// Current schema version, stored in SQLite's `user_version`.
/// 2: sync is end-to-end encrypted.
/// 3: accounts (a new table, nothing to migrate).
/// 4: `group_sync.relay_id`.
const SCHEMA_VERSION: i64 = 4;

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
                .update(&group.id, |d| {
                    doc::add_participant(d, "Bob", doc::AddedBy::Member(None)).map(|_| ())
                })
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
                .set_sync(
                    &id,
                    SyncMeta::new("http://relay".into(), Secret::new("old".into())),
                )
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
