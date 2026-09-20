use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

use crate::models::{Expense, Group, Participant};

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
        split_among: Vec<String>,
    ) -> Result<Group, String> {
        if amount_cents <= 0 {
            return Err("Amount must be greater than zero".to_string());
        }
        if split_among.is_empty() {
            return Err("Expense must be split among at least one participant".to_string());
        }

        let mut groups = self.groups.lock().unwrap();
        let group = groups
            .iter_mut()
            .find(|g| g.id == group_id)
            .ok_or_else(|| "Group not found".to_string())?;

        let expense = Expense {
            id: Uuid::new_v4().to_string(),
            group_id: group_id.to_string(),
            title: title.trim().to_string(),
            amount_cents,
            paid_by,
            split_among,
            created_at: Utc::now(),
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
