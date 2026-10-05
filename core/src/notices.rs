//! What the other members did in a group between two readings of it, for the notifications
//! a phone shows when a sync brings changes while the app isn't on screen.

use serde::Serialize;
use std::collections::HashSet;

use crate::models::{Expense, Group};

/// One thing to tell the user about. The app that shows it writes the sentence, in the
/// phone's language.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Notice {
    pub group_id: String,
    pub group: String,
    // "expense", "income", "payment" or "comment".
    pub kind: &'static str,
    // The member who added the expense or wrote the comment, by name, when the group knows.
    // For a payment, who paid.
    pub by: Option<String>,
    // The expense's title.
    pub title: String,
    pub amount_cents: i64,
    pub currency: String,
    // A comment's text. For a payment, the name of who received it.
    pub text: Option<String>,
}

/// What appeared in `after` that `before` didn't have, leaving out what `me` (the participant
/// the user is) did, on this device or another of theirs.
pub fn news(before: &Group, after: &Group, me: Option<&str>) -> Vec<Notice> {
    let all = |group: &'_ Group| -> Vec<Expense> {
        group
            .expenses
            .iter()
            .chain(group.trash.iter().map(|deleted| &deleted.expense))
            .cloned()
            .collect()
    };
    let known = all(before);
    let expenses: HashSet<&str> = known.iter().map(|e| e.id.as_str()).collect();
    let comments: HashSet<&str> = known
        .iter()
        .flat_map(|e| e.comments.iter().map(|c| c.id.as_str()))
        .collect();
    let name = |id: &str| {
        after
            .participants
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
    };
    let mine = |by: &Option<String>| me.is_some() && by.as_deref() == me;
    let notice = |e: &Expense, kind, by, text| Notice {
        group_id: after.id.clone(),
        group: after.name.clone(),
        kind,
        by,
        title: e.title.clone(),
        amount_cents: e.amount_cents,
        currency: after.currency.clone(),
        text,
    };

    let mut notices = Vec::new();
    for e in &after.expenses {
        // An occurrence of a repeated expense is added by the app, on whichever device.
        let repeated = e
            .recurring
            .as_ref()
            .is_some_and(|model| e.id.starts_with(model.as_str()));
        if !expenses.contains(e.id.as_str()) && !repeated && !mine(&e.added_by) {
            notices.push(if e.is_reimbursement {
                let to = e.splits.first().and_then(|s| name(&s.participant_id));
                notice(e, "payment", name(&e.paid_by), to)
            } else {
                let kind = if e.income { "income" } else { "expense" };
                notice(e, kind, e.added_by.as_deref().and_then(name), None)
            });
        }
        for c in &e.comments {
            if !comments.contains(c.id.as_str()) && !mine(&c.by) {
                let by = c.by.as_deref().and_then(name);
                notices.push(notice(e, "comment", by, Some(c.text.clone())));
            }
        }
    }
    notices
}

#[cfg(test)]
mod tests;
