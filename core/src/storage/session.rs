//! The account this device is logged into, whose document is stored like a group's.

use super::*;

/// The account this device is logged into. Its key is the secret of the account document's
/// sync row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub server_url: String,
    pub username: String,
    pub account_id: String,
}

impl Store {
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
