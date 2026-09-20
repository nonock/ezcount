use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Participant {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Expense {
    pub id: String,
    pub group_id: String,
    pub title: String,
    pub amount_cents: i64,
    pub paid_by: String,          // Participant ID
    pub split_among: Vec<String>, // List of Participant IDs
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub is_reimbursement: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub currency: String,
    pub participants: Vec<Participant>,
    pub expenses: Vec<Expense>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParticipantBalance {
    pub participant_id: String,
    pub participant_name: String,
    pub paid_cents: i64,
    pub owed_cents: i64,
    pub net_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettlementTransfer {
    pub from_id: String,
    pub from_name: String,
    pub to_id: String,
    pub to_name: String,
    pub amount_cents: i64,
}
