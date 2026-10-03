//! Loro document schema for an account: the private list of one person's groups.
//!
//! - `groups` (map): group id -> plain map { `server_url`, `secret`, `added_at` }
//! - `identities` (map): group id -> id of the participant this person is in that group
//! - `archived` (map): group id -> true, for the groups this person put away
//! - `profile` (map): `name` and `avatar` (a `data:` URL), what this person shows in their
//!   groups
//!
//! The document is synced through the relay like a group, end-to-end encrypted with the
//! account key, so every device logged into the account sees the same groups and knows who
//! "you" are in each. Identities live in their own map so that choosing who you are never
//! conflicts with joining or leaving a group on another device.

use crate::crypto::Secret;
use crate::doc::{self, entries};
use chrono::{SecondsFormat, Utc};
use loro::{LoroDoc, LoroValue, ValueOrContainer};
use serde::Deserialize;
use std::collections::HashMap;

type Res<T> = Result<T, String>;

const GROUPS: &str = "groups";
const IDENTITIES: &str = "identities";
const PROFILE: &str = "profile";
const ARCHIVED: &str = "archived";

/// Longest name in a profile, in characters.
pub const MAX_NAME_CHARS: usize = 50;

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

/// The account's groups. Malformed entries are skipped, never fatal.
pub fn groups(doc: &LoroDoc) -> Res<Vec<AccountGroup>> {
    Ok(
        entries::<AccountGroup>(&doc.get_map(GROUPS), "account group")
            .into_iter()
            .map(|(group_id, group)| AccountGroup { group_id, ..group })
            .collect(),
    )
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
    doc.get_map(ARCHIVED).delete(group_id).map_err(doc_err)?;
    doc.get_map(IDENTITIES).delete(group_id).map_err(doc_err)
}

/// Group id -> participant id, for the groups where this person said who they are.
pub fn identities(doc: &LoroDoc) -> Res<HashMap<String, String>> {
    Ok(entries::<String>(&doc.get_map(IDENTITIES), "identity")
        .into_iter()
        .collect())
}

pub fn set_identity(doc: &LoroDoc, group_id: &str, participant_id: &str) -> Res<()> {
    doc.get_map(IDENTITIES)
        .insert(group_id, participant_id)
        .map_err(doc_err)
}

/// The groups this person put away, by id.
pub fn archived(doc: &LoroDoc) -> Vec<String> {
    entries::<bool>(&doc.get_map(ARCHIVED), "archived group")
        .into_iter()
        .filter_map(|(group_id, archived)| archived.then_some(group_id))
        .collect()
}

/// Puts a group away, or back among the others.
pub fn set_archived(doc: &LoroDoc, group_id: &str, archived: bool) -> Res<()> {
    let map = doc.get_map(ARCHIVED);
    if archived {
        map.insert(group_id, true).map_err(doc_err)
    } else {
        map.delete(group_id).map_err(doc_err)
    }
}

/// The name and picture this person shows in their groups.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Profile {
    pub name: Option<String>,
    pub avatar: Option<String>,
}

/// The profile. Anything unusable in it reads as not set.
pub fn profile(doc: &LoroDoc) -> Profile {
    let map = doc.get_map(PROFILE);
    let text = |key: &str| match map.get(key) {
        Some(ValueOrContainer::Value(LoroValue::String(text))) => Some(text.to_string()),
        _ => None,
    };
    Profile {
        name: text("name").filter(|name| !name.trim().is_empty()),
        avatar: text("avatar").filter(|avatar| doc::check_image(avatar).is_ok()),
    }
}

/// Sets the profile. An empty name, or no picture, removes it.
pub fn set_profile(doc: &LoroDoc, name: &str, avatar: Option<&str>) -> Res<()> {
    let name = name.trim();
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!(
            "This name is too long ({MAX_NAME_CHARS} characters at most)"
        ));
    }
    if let Some(avatar) = avatar {
        doc::check_image(avatar)?;
    }
    let current = profile(doc);
    let map = doc.get_map(PROFILE);
    // Only what changed is written, so a change to the other one elsewhere survives.
    if current.name.as_deref().unwrap_or("") != name {
        if name.is_empty() {
            map.delete("name").map_err(doc_err)?;
        } else {
            map.insert("name", name).map_err(doc_err)?;
        }
    }
    if current.avatar.as_deref() != avatar {
        match avatar {
            Some(avatar) => map.insert("avatar", avatar).map_err(doc_err)?,
            None => map.delete("avatar").map_err(doc_err)?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_profile_is_set_and_removed() {
        let doc = LoroDoc::new();
        assert_eq!(profile(&doc), Profile::default());
        let picture = "data:image/png;base64,AAAA";
        set_profile(&doc, " Alice ", Some(picture)).unwrap();
        assert_eq!(
            profile(&doc),
            Profile {
                name: Some("Alice".into()),
                avatar: Some(picture.into())
            }
        );
        assert!(set_profile(&doc, &"a".repeat(MAX_NAME_CHARS + 1), None).is_err());
        assert!(set_profile(&doc, "Alice", Some("data:text/html;base64,AAAA")).is_err());
        set_profile(&doc, "", None).unwrap();
        assert_eq!(profile(&doc), Profile::default());
    }

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

        set_archived(&doc, "g1", true).unwrap();
        set_archived(&doc, "g2", true).unwrap();
        set_archived(&doc, "g2", false).unwrap();
        assert_eq!(archived(&doc), vec!["g1".to_string()]);

        remove_group(&doc, "g1").unwrap();
        assert_eq!(groups(&doc).unwrap().len(), 1);
        assert!(identities(&doc).unwrap().is_empty());
        assert!(archived(&doc).is_empty());
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
