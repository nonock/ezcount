//! Loro document schema for an account: the private list of one person's groups.
//!
//! - `groups` (map): group id -> plain map { `server_url`, `secret`, `added_at` }
//! - `identities` (map): group id -> id of the participant this person is in that group
//!
//! The document is synced through the relay like a group, end-to-end encrypted with the
//! account key, so every device logged into the account sees the same groups and knows who
//! "you" are in each. Identities live in their own map so that choosing who you are never
//! conflicts with joining or leaving a group on another device.

use crate::crypto::Secret;
use chrono::{SecondsFormat, Utc};
use loro::{LoroDoc, LoroValue};
use serde::Deserialize;
use std::collections::HashMap;

type Res<T> = Result<T, String>;

const GROUPS: &str = "groups";
const IDENTITIES: &str = "identities";

fn doc_err(e: impl std::fmt::Display) -> String {
    format!("Account document error: {e}")
}

/// Everything needed to sync one group: the same details as its invite code.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AccountGroup {
    #[serde(skip)]
    pub group_id: String,
    pub server_url: String,
    pub secret: Secret,
}

fn root(doc: &LoroDoc) -> Res<serde_json::Value> {
    serde_json::to_value(doc.get_deep_value()).map_err(doc_err)
}

/// The account's groups. Malformed entries are skipped, never fatal.
pub fn groups(doc: &LoroDoc) -> Res<Vec<AccountGroup>> {
    let root = root(doc)?;
    let Some(map) = root.get(GROUPS).and_then(|v| v.as_object()) else {
        return Ok(Vec::new());
    };
    Ok(map
        .iter()
        .filter_map(
            |(id, value)| match serde_json::from_value::<AccountGroup>(value.clone()) {
                Ok(group) => Some(AccountGroup {
                    group_id: id.clone(),
                    ..group
                }),
                Err(e) => {
                    eprintln!("[account] skipping malformed group entry {id}: {e}");
                    None
                }
            },
        )
        .collect())
}

pub fn add_group(doc: &LoroDoc, group_id: &str, server_url: &str, secret: &Secret) -> Res<()> {
    let entry: HashMap<String, LoroValue> = [
        ("server_url", server_url.into()),
        ("secret", secret.expose().into()),
        (
            "added_at",
            Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true).into(),
        ),
    ]
    .into_iter()
    .map(|(k, v): (&str, LoroValue)| (k.to_string(), v))
    .collect();
    doc.get_map(GROUPS)
        .insert(group_id, LoroValue::Map(entry.into()))
        .map_err(doc_err)
}

/// Removes a group from the account, so every device of the account drops it.
pub fn remove_group(doc: &LoroDoc, group_id: &str) -> Res<()> {
    doc.get_map(GROUPS).delete(group_id).map_err(doc_err)?;
    doc.get_map(IDENTITIES).delete(group_id).map_err(doc_err)
}

/// Group id -> participant id, for the groups where this person said who they are.
pub fn identities(doc: &LoroDoc) -> Res<HashMap<String, String>> {
    let root = root(doc)?;
    Ok(root
        .get(IDENTITIES)
        .and_then(|v| v.as_object())
        .map(|map| {
            map.iter()
                .filter_map(|(gid, pid)| Some((gid.clone(), pid.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default())
}

pub fn set_identity(doc: &LoroDoc, group_id: &str, participant_id: &str) -> Res<()> {
    doc.get_map(IDENTITIES)
        .insert(group_id, participant_id)
        .map_err(doc_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_and_identities_round_trip() {
        let doc = LoroDoc::new();
        let secret = |s: &str| Secret::new(s.to_string());
        add_group(&doc, "g1", "http://relay", &secret("secret-1")).unwrap();
        add_group(&doc, "g2", "http://relay", &secret("secret-2")).unwrap();
        set_identity(&doc, "g1", "p-alice").unwrap();

        let mut list = groups(&doc).unwrap();
        list.sort_by(|a, b| a.group_id.cmp(&b.group_id));
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].group_id, "g1");
        assert_eq!(list[0].secret.expose(), "secret-1");
        assert_eq!(identities(&doc).unwrap().get("g1").unwrap(), "p-alice");

        remove_group(&doc, "g1").unwrap();
        assert_eq!(groups(&doc).unwrap().len(), 1);
        assert!(identities(&doc).unwrap().is_empty());
    }

    #[test]
    fn concurrent_join_and_identity_both_survive() {
        let (a, b) = (LoroDoc::new(), LoroDoc::new());
        add_group(&a, "g1", "http://relay", &Secret::new("s".into())).unwrap();
        a.commit();
        b.import(&a.export(loro::ExportMode::Snapshot).unwrap())
            .unwrap();

        // Phone joins another group while the laptop picks an identity in the first.
        add_group(&a, "g2", "http://relay", &Secret::new("s2".into())).unwrap();
        set_identity(&b, "g1", "p-bob").unwrap();
        a.commit();
        b.commit();
        a.import(&b.export(loro::ExportMode::Snapshot).unwrap())
            .unwrap();

        assert_eq!(groups(&a).unwrap().len(), 2);
        assert_eq!(identities(&a).unwrap().get("g1").unwrap(), "p-bob");
    }
}
