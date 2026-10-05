//! Client side of accounts and group sync.
//!
//! The server is a dumb relay: it stores opaque Loro updates per group, in order, and hands
//! them back by sequence number. Each device pushes the operations the server doesn't have
//! yet and pulls everything after the last sequence number it imported. Loro merges the rest.
//!
//! Anyone holding a group's invite code (server URL, group ID and secret key) can read and
//! edit that group. The relay never sees the secret: updates are end-to-end encrypted and the
//! relay only receives a derived auth token (see `crypto`).
//!
//! An account is one more document synced the same way (see `account`). It lists the groups
//! of one person with their invite details, so every device logged into the account ends up
//! with the same groups: `reconcile` downloads groups added on another device and drops the
//! ones left there. Every group of a logged-in device is shared through the relay.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use loro::{ExportMode, LoroDoc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use url::Url;

use crate::account;
use crate::crypto::{new_recovery_key, CredentialKeys, GroupKeys, LinkKeys, Secret};
use crate::csv_file;
use crate::doc;
use crate::models::{Group, LoginLink, PasswordStrength, Received};
use crate::notices::{self, Notice};
use crate::storage::{import_remote_update, Session, Store, SyncMeta};
use crate::AppState;

mod accounts;
mod groups;
mod invite;
mod links;
mod pass;
mod password;
mod relay;
mod services;

pub use self::accounts::*;
pub use self::groups::*;
pub use self::invite::*;
pub use self::links::*;
pub use self::pass::*;
pub use self::password::*;
pub use self::relay::*;
pub use self::services::*;

type Res<T> = Result<T, String>;

#[cfg(test)]
mod tests;

/// Devices syncing through a real relay on a local port.
#[cfg(test)]
mod end_to_end;
