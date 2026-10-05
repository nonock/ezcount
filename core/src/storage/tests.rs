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
        Store::open(&self.0.join("db.sqlite3")).unwrap()
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
