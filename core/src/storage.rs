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

use crate::account;
use crate::crypto::Secret;
use crate::doc;
use crate::models::Group;

mod schema;
mod session;
mod sync;

use self::schema::*;
pub use self::session::*;
pub use self::sync::*;

type Res<T> = Result<T, String>;

fn db_err(e: rusqlite::Error) -> String {
    format!("Database error: {e}")
}

pub struct Store {
    conn: Connection,
    docs: HashMap<String, LoroDoc>,
    sync: HashMap<String, SyncMeta>,
    session: Option<Session>,
}

impl Store {
    /// Opens (or creates) the database. Returns warnings about anything that could not be
    /// loaded; that data stays on disk.
    pub fn open(db_path: &Path) -> Res<(Self, Vec<String>)> {
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Could not create data folder {}: {e}", parent.display()))?;
        }
        let conn = Connection::open(db_path).map_err(db_err)?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = FULL;")
            .map_err(db_err)?;
        Self::from_connection(conn)
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
        // A document in a format this version doesn't know is synced, never changed.
        let (known, refusal) = match self.is_account(id) {
            true => (account::FORMAT, account::NEWER_FORMAT),
            false => (doc::FORMAT, doc::NEWER_FORMAT),
        };
        if doc::format_needed(doc) > known {
            return Err(refusal.to_string());
        }
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
}

#[cfg(test)]
mod tests;
