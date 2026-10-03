use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct Participant {
    pub id: String,
    pub name: String,
    // Soft-deleted: kept so existing expenses and balances still resolve,
    // but no longer offered for new expenses.
    #[serde(default)]
    pub removed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct ExpenseSplit {
    pub participant_id: String,
    // Parts of what is left of the expense once the fixed amounts are taken. 0 with
    // `fixed_cents`.
    pub shares: u32,
    // Owes exactly this instead of parts, in the currency the expense was paid in.
    #[serde(default)]
    pub fixed_cents: Option<i64>,
}

/// What an expense paid in another currency than the group's cost there.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct OriginalAmount {
    pub currency: String,
    pub amount_cents: i64,
    // Units of the group's currency for one of this currency, as typed: "0.9234".
    pub rate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct ExpenseHistoryEntry {
    pub edited_at: DateTime<Utc>,
    pub previous_title: String,
    pub previous_amount_cents: i64,
    pub previous_paid_by: String,
    pub previous_splits: Vec<ExpenseSplit>,
    #[serde(default)]
    pub previous_original: Option<OriginalAmount>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct Expense {
    pub id: String,
    pub group_id: String,
    pub title: String,
    // In the group's currency, also when `original` is set.
    pub amount_cents: i64,
    #[serde(default)]
    pub original: Option<OriginalAmount>,
    pub paid_by: String, // Participant ID
    pub splits: Vec<ExpenseSplit>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub history: Vec<ExpenseHistoryEntry>,
    #[serde(default)]
    pub is_reimbursement: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub currency: String,
    pub participants: Vec<Participant>,
    pub expenses: Vec<Expense>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct ParticipantBalance {
    pub participant_id: String,
    pub participant_name: String,
    pub paid_cents: i64,
    pub owed_cents: i64,
    pub net_cents: i64,
    // True for removed participants and for IDs that match no participant.
    pub removed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct SettlementTransfer {
    pub from_id: String,
    pub from_name: String,
    pub to_id: String,
    pub to_name: String,
    pub amount_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct SyncInfo {
    pub group_id: String,
    pub enabled: bool,
    pub server_url: Option<String>,
    // Share this with other members so they can join the group.
    pub invite_code: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct PasswordStrength {
    // 0 (guessed at once) to 4 (very hard to guess), from zxcvbn.
    pub score: u8,
    // Strong and long enough to sign up with.
    pub acceptable: bool,
    // What makes it weak, like "This is a top-10 common password.".
    pub warning: Option<String>,
    // How to make it stronger.
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct NativeFeatures {
    // The system share sheet (`share_text`).
    pub share: bool,
    // QR code scanning with the camera (barcode-scanner plugin).
    pub scan: bool,
}

/// After signing up or recovering an account.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct SignedIn {
    pub account: AccountInfo,
    // The new recovery key, to show once. None when the relay doesn't support them.
    pub recovery_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct AccountInfo {
    pub username: String,
    pub server_url: String,
    // Group id -> id of the participant the user is in that group.
    pub identities: std::collections::HashMap<String, String>,
}
