use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

use crate::models::{Expense, ExpenseHistoryEntry, ExpenseSplit, Group, Participant};

pub struct AppState {
    pub groups: Mutex<Vec<Group>>,
    pub file_path: PathBuf,
}

impl AppState {
    pub fn new(file_path: PathBuf) -> Self {
        let groups = if file_path.exists() {
            match fs::read_to_string(&file_path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };

        Self {
            groups: Mutex::new(groups),
            file_path,
        }
    }

    fn persist(&self, groups: &[Group]) -> Result<(), String> {
        if let Some(parent) = self.file_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(groups)
            .map_err(|e| format!("Serialization error: {}", e))?;
        fs::write(&self.file_path, json).map_err(|e| format!("Save error: {}", e))?;
        Ok(())
    }

    pub fn get_groups(&self) -> Vec<Group> {
        self.groups.lock().unwrap().clone()
    }

    pub fn get_group(&self, id: &str) -> Option<Group> {
        self.groups
            .lock()
            .unwrap()
            .iter()
            .find(|g| g.id == id)
            .cloned()
    }

    pub fn create_group(
        &self,
        name: String,
        currency: String,
        participant_names: Vec<String>,
    ) -> Result<Group, String> {
        let mut groups = self.groups.lock().unwrap();

        let participants = participant_names
            .into_iter()
            .filter(|n| !n.trim().is_empty())
            .map(|name| Participant {
                id: Uuid::new_v4().to_string(),
                name: name.trim().to_string(),
            })
            .collect();

        let new_group = Group {
            id: Uuid::new_v4().to_string(),
            name: name.trim().to_string(),
            currency: currency.trim().to_uppercase(),
            participants,
            expenses: Vec::new(),
            created_at: Utc::now(),
        };

        groups.push(new_group.clone());
        self.persist(&groups)?;

        Ok(new_group)
    }

    pub fn delete_group(&self, id: &str) -> Result<bool, String> {
        let mut groups = self.groups.lock().unwrap();
        let initial_len = groups.len();
        groups.retain(|g| g.id != id);
        let deleted = groups.len() < initial_len;
        if deleted {
            self.persist(&groups)?;
        }
        Ok(deleted)
    }

    pub fn add_participant(&self, group_id: &str, name: String) -> Result<Group, String> {
        let mut groups = self.groups.lock().unwrap();
        let group = groups
            .iter_mut()
            .find(|g| g.id == group_id)
            .ok_or_else(|| "Group not found".to_string())?;

        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return Err("Participant name cannot be empty".to_string());
        }

        let participant = Participant {
            id: Uuid::new_v4().to_string(),
            name: trimmed_name.to_string(),
        };

        group.participants.push(participant);
        let updated_group = group.clone();
        self.persist(&groups)?;

        Ok(updated_group)
    }

    pub fn add_expense(
        &self,
        group_id: &str,
        title: String,
        amount_cents: i64,
        paid_by: String,
        splits: Vec<ExpenseSplit>,
    ) -> Result<Group, String> {
        if amount_cents <= 0 {
            return Err("Amount must be greater than zero".to_string());
        }
        if splits.is_empty() {
            return Err("Expense must be split among at least one participant".to_string());
        }
        if splits.iter().any(|s| s.shares == 0) {
            return Err("Shares must be at least 1".to_string());
        }

        let mut groups = self.groups.lock().unwrap();
        let group = groups
            .iter_mut()
            .find(|g| g.id == group_id)
            .ok_or_else(|| "Group not found".to_string())?;

        let now = Utc::now();
        let expense = Expense {
            id: Uuid::new_v4().to_string(),
            group_id: group_id.to_string(),
            title: title.trim().to_string(),
            amount_cents,
            paid_by,
            splits,
            created_at: now,
            updated_at: now,
            history: Vec::new(),
            is_reimbursement: false,
        };

        group.expenses.push(expense);
        let updated = group.clone();
        self.persist(&groups)?;

        Ok(updated)
    }

    pub fn update_expense(
        &self,
        group_id: &str,
        expense_id: &str,
        title: String,
        amount_cents: i64,
        paid_by: String,
        splits: Vec<ExpenseSplit>,
    ) -> Result<Group, String> {
        if amount_cents <= 0 {
            return Err("Amount must be greater than zero".to_string());
        }
        if splits.is_empty() {
            return Err("Expense must be split among at least one participant".to_string());
        }
        if splits.iter().any(|s| s.shares == 0) {
            return Err("Shares must be at least 1".to_string());
        }

        let mut groups = self.groups.lock().unwrap();
        let group = groups
            .iter_mut()
            .find(|g| g.id == group_id)
            .ok_or_else(|| "Group not found".to_string())?;

        let expense = group
            .expenses
            .iter_mut()
            .find(|e| e.id == expense_id)
            .ok_or_else(|| "Expense not found".to_string())?;

        // Build human-readable change summary
        let mut changes = Vec::new();
        let trimmed_title = title.trim();
        if expense.title != trimmed_title {
            changes.push(format!(
                "Title changed from '{}' to '{}'",
                expense.title, trimmed_title
            ));
        }
        if expense.amount_cents != amount_cents {
            changes.push(format!(
                "Amount changed from {:.2} to {:.2}",
                expense.amount_cents as f64 / 100.0,
                amount_cents as f64 / 100.0
            ));
        }
        if expense.paid_by != paid_by {
            let old_payer = group
                .participants
                .iter()
                .find(|p| p.id == expense.paid_by)
                .map(|p| p.name.as_str())
                .unwrap_or("Unknown");
            let new_payer = group
                .participants
                .iter()
                .find(|p| p.id == paid_by)
                .map(|p| p.name.as_str())
                .unwrap_or("Unknown");
            changes.push(format!("Payer changed from {} to {}", old_payer, new_payer));
        }
        if expense.splits != splits {
            changes.push("Participants / parts allocation updated".to_string());
        }

        let summary = if changes.is_empty() {
            "Updated without major changes".to_string()
        } else {
            changes.join("; ")
        };

        let history_entry = ExpenseHistoryEntry {
            edited_at: Utc::now(),
            previous_title: expense.title.clone(),
            previous_amount_cents: expense.amount_cents,
            previous_paid_by: expense.paid_by.clone(),
            previous_splits: expense.splits.clone(),
            summary,
        };

        expense.history.push(history_entry);
        expense.updated_at = Utc::now();
        expense.title = trimmed_title.to_string();
        expense.amount_cents = amount_cents;
        expense.paid_by = paid_by;
        expense.splits = splits;

        let updated = group.clone();
        self.persist(&groups)?;

        Ok(updated)
    }

    pub fn record_reimbursement(
        &self,
        group_id: &str,
        from_id: String,
        to_id: String,
        amount_cents: i64,
        notes: Option<String>,
    ) -> Result<Group, String> {
        if amount_cents <= 0 {
            return Err("Reimbursement amount must be greater than zero".to_string());
        }
        if from_id == to_id {
            return Err("Sender and recipient cannot be the same person".to_string());
        }

        let mut groups = self.groups.lock().unwrap();
        let group = groups
            .iter_mut()
            .find(|g| g.id == group_id)
            .ok_or_else(|| "Group not found".to_string())?;

        let from_name = group
            .participants
            .iter()
            .find(|p| p.id == from_id)
            .map(|p| p.name.clone())
            .ok_or_else(|| "Sender participant not found".to_string())?;

        let to_name = group
            .participants
            .iter()
            .find(|p| p.id == to_id)
            .map(|p| p.name.clone())
            .ok_or_else(|| "Recipient participant not found".to_string())?;

        let title = match notes {
            Some(n) if !n.trim().is_empty() => {
                format!("Payment: {} → {} ({})", from_name, to_name, n.trim())
            }
            _ => format!("Payment: {} → {}", from_name, to_name),
        };

        let now = Utc::now();
        let expense = Expense {
            id: Uuid::new_v4().to_string(),
            group_id: group_id.to_string(),
            title,
            amount_cents,
            paid_by: from_id,
            splits: vec![ExpenseSplit {
                participant_id: to_id,
                shares: 1,
            }],
            created_at: now,
            updated_at: now,
            history: Vec::new(),
            is_reimbursement: true,
        };

        group.expenses.push(expense);
        let updated = group.clone();
        self.persist(&groups)?;

        Ok(updated)
    }

    pub fn delete_expense(&self, group_id: &str, expense_id: &str) -> Result<Group, String> {
        let mut groups = self.groups.lock().unwrap();
        let group = groups
            .iter_mut()
            .find(|g| g.id == group_id)
            .ok_or_else(|| "Group not found".to_string())?;

        let initial_len = group.expenses.len();
        group.expenses.retain(|e| e.id != expense_id);

        if group.expenses.len() == initial_len {
            return Err("Expense not found".to_string());
        }

        let updated = group.clone();
        self.persist(&groups)?;

        Ok(updated)
    }
}
