//! What a shared document knows of the relay: what it holds, and how the last sync went.

use super::*;

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

impl Store {
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
