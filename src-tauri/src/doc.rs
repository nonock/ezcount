//! Loro document schema for a group.
//!
//! Each group is one `LoroDoc`. It is the source of truth and the unit of sync:
//!
//! - `meta` (map): `id`, `name`, `currency`, `created_at`
//! - `participants` (map): participant id -> map { `name`, `removed`, `position` }
//! - `expenses` (map): expense id -> map { `title`, `amount_cents`, `paid_by`, `splits`,
//!   `created_at`, `updated_at`, `is_reimbursement`, `history` (list) }
//!
//! Every field is its own last-writer-wins register, so concurrent edits to different fields
//! of one expense both survive a merge. `splits` is stored as a single plain value so an
//! allocation is always replaced as a whole and never merged into a mix of two edits.
//! Plain values are built by hand rather than through serde so that user text can never be
//! mistaken for a Loro container reference.

use chrono::{DateTime, SecondsFormat, Utc};
use loro::{Container, LoroDoc, LoroList, LoroMap, LoroValue, ValueOrContainer};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::models::{Expense, ExpenseHistoryEntry, ExpenseSplit, Group, Participant};

const META: &str = "meta";
const PARTICIPANTS: &str = "participants";
const EXPENSES: &str = "expenses";
const HISTORY: &str = "history";

type Res<T> = Result<T, String>;

fn doc_err(e: impl std::fmt::Display) -> String {
    format!("Document error: {e}")
}

fn timestamp(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

fn map_value<const N: usize>(entries: [(&str, LoroValue); N]) -> LoroValue {
    let map: HashMap<String, LoroValue> = entries
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    LoroValue::Map(map.into())
}

fn splits_value(splits: &[ExpenseSplit]) -> LoroValue {
    let list: Vec<LoroValue> = splits
        .iter()
        .map(|s| {
            map_value([
                ("participant_id", s.participant_id.as_str().into()),
                ("shares", i64::from(s.shares).into()),
            ])
        })
        .collect();
    LoroValue::List(list.into())
}

fn history_value(entry: &ExpenseHistoryEntry) -> LoroValue {
    map_value([
        ("edited_at", timestamp(entry.edited_at).into()),
        ("previous_title", entry.previous_title.as_str().into()),
        ("previous_amount_cents", entry.previous_amount_cents.into()),
        ("previous_paid_by", entry.previous_paid_by.as_str().into()),
        ("previous_splits", splits_value(&entry.previous_splits)),
        ("summary", entry.summary.as_str().into()),
    ])
}

fn child_map(parent: &LoroMap, key: &str) -> Option<LoroMap> {
    match parent.get(key)? {
        ValueOrContainer::Container(Container::Map(map)) => Some(map),
        _ => None,
    }
}

fn child_list(parent: &LoroMap, key: &str) -> Res<LoroList> {
    match parent.get(key) {
        Some(ValueOrContainer::Container(Container::List(list))) => Ok(list),
        _ => parent
            .insert_container(key, LoroList::new())
            .map_err(doc_err),
    }
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct DocMeta {
    id: String,
    name: String,
    currency: String,
    created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct DocParticipant {
    name: String,
    #[serde(default)]
    removed: bool,
    #[serde(default)]
    position: i64,
}

#[derive(Deserialize)]
struct DocExpense {
    title: String,
    amount_cents: i64,
    paid_by: String,
    splits: Vec<ExpenseSplit>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    #[serde(default)]
    is_reimbursement: bool,
    #[serde(default)]
    history: Vec<ExpenseHistoryEntry>,
}

/// Entries of a root map, skipping (and logging) any that don't match the schema.
/// A single malformed entry must not make the whole group unreadable.
fn entries<T: DeserializeOwned>(root: &serde_json::Value, key: &str) -> Vec<(String, T)> {
    let Some(map) = root.get(key).and_then(|v| v.as_object()) else {
        return Vec::new();
    };
    map.iter()
        .filter_map(|(id, value)| match serde_json::from_value(value.clone()) {
            Ok(parsed) => Some((id.clone(), parsed)),
            Err(e) => {
                eprintln!("[doc] skipping malformed {key} entry {id}: {e}");
                None
            }
        })
        .collect()
}

/// Materializes the document into the `Group` shape the frontend and engine use.
pub fn read_group(doc: &LoroDoc) -> Res<Group> {
    let root = serde_json::to_value(doc.get_deep_value()).map_err(doc_err)?;
    let meta: DocMeta = serde_json::from_value(root.get(META).cloned().unwrap_or_default())
        .map_err(|e| format!("Group document has no valid metadata: {e}"))?;

    let mut participants: Vec<(i64, Participant)> = entries::<DocParticipant>(&root, PARTICIPANTS)
        .into_iter()
        .map(|(id, p)| {
            (
                p.position,
                Participant {
                    id,
                    name: p.name,
                    removed: p.removed,
                },
            )
        })
        .collect();
    // Concurrent additions can share a position; the id keeps the order stable everywhere.
    participants.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));

    let mut expenses: Vec<Expense> = entries::<DocExpense>(&root, EXPENSES)
        .into_iter()
        .map(|(id, e)| Expense {
            id,
            group_id: meta.id.clone(),
            title: e.title,
            amount_cents: e.amount_cents,
            paid_by: e.paid_by,
            splits: e.splits,
            created_at: e.created_at,
            updated_at: e.updated_at,
            history: e.history,
            is_reimbursement: e.is_reimbursement,
        })
        .collect();
    expenses.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });

    Ok(Group {
        id: meta.id,
        name: meta.name,
        currency: meta.currency,
        participants: participants.into_iter().map(|(_, p)| p).collect(),
        expenses,
        created_at: meta.created_at,
    })
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

fn write_meta(
    doc: &LoroDoc,
    id: &str,
    name: &str,
    currency: &str,
    created_at: DateTime<Utc>,
) -> Res<()> {
    let meta = doc.get_map(META);
    meta.insert("id", id).map_err(doc_err)?;
    meta.insert("name", name).map_err(doc_err)?;
    meta.insert("currency", currency).map_err(doc_err)?;
    meta.insert("created_at", timestamp(created_at))
        .map_err(doc_err)?;
    Ok(())
}

fn insert_participant(doc: &LoroDoc, id: &str, name: &str, removed: bool) -> Res<()> {
    let participants = doc.get_map(PARTICIPANTS);
    let position = participants.len() as i64;
    let p = participants
        .insert_container(id, LoroMap::new())
        .map_err(doc_err)?;
    p.insert("name", name).map_err(doc_err)?;
    p.insert("removed", removed).map_err(doc_err)?;
    p.insert("position", position).map_err(doc_err)?;
    Ok(())
}

fn insert_expense(doc: &LoroDoc, expense: &Expense) -> Res<()> {
    let e = doc
        .get_map(EXPENSES)
        .insert_container(&expense.id, LoroMap::new())
        .map_err(doc_err)?;
    e.insert("title", expense.title.as_str()).map_err(doc_err)?;
    e.insert("amount_cents", expense.amount_cents)
        .map_err(doc_err)?;
    e.insert("paid_by", expense.paid_by.as_str())
        .map_err(doc_err)?;
    e.insert("splits", splits_value(&expense.splits))
        .map_err(doc_err)?;
    e.insert("created_at", timestamp(expense.created_at))
        .map_err(doc_err)?;
    e.insert("updated_at", timestamp(expense.updated_at))
        .map_err(doc_err)?;
    e.insert("is_reimbursement", expense.is_reimbursement)
        .map_err(doc_err)?;
    let history = e
        .insert_container(HISTORY, LoroList::new())
        .map_err(doc_err)?;
    for entry in &expense.history {
        history.push(history_value(entry)).map_err(doc_err)?;
    }
    Ok(())
}

/// Creates the document for a brand-new group.
pub fn new_group_doc(name: &str, currency: &str, participant_names: &[String]) -> Res<LoroDoc> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Group name cannot be empty".to_string());
    }
    let doc = LoroDoc::new();
    write_meta(
        &doc,
        &Uuid::new_v4().to_string(),
        name,
        &currency.trim().to_uppercase(),
        Utc::now(),
    )?;
    for participant_name in participant_names {
        let trimmed = participant_name.trim();
        if !trimmed.is_empty() {
            insert_participant(&doc, &Uuid::new_v4().to_string(), trimmed, false)?;
        }
    }
    doc.commit();
    Ok(doc)
}

/// Converts a group from the old JSON file, keeping every ID so saved preferences
/// (such as "viewing as") still match.
pub fn doc_from_legacy(group: &Group) -> Res<LoroDoc> {
    let doc = LoroDoc::new();
    write_meta(
        &doc,
        &group.id,
        &group.name,
        &group.currency,
        group.created_at,
    )?;
    for p in &group.participants {
        insert_participant(&doc, &p.id, &p.name, p.removed)?;
    }
    for e in &group.expenses {
        insert_expense(&doc, e)?;
    }
    doc.commit();
    Ok(doc)
}

pub fn add_participant(doc: &LoroDoc, name: &str) -> Res<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Participant name cannot be empty".to_string());
    }
    insert_participant(doc, &Uuid::new_v4().to_string(), trimmed, false)
}

/// Soft-deletes a participant. Their past expenses and balance stay intact.
pub fn remove_participant(doc: &LoroDoc, participant_id: &str) -> Res<()> {
    let p = child_map(&doc.get_map(PARTICIPANTS), participant_id)
        .ok_or_else(|| "Participant not found".to_string())?;
    p.insert("removed", true).map_err(doc_err)
}

/// `grandfathered` lists IDs that may be used even if removed: the people already on an
/// expense being edited, so editing an old expense doesn't force dropping them.
fn validate_expense(
    group: &Group,
    amount_cents: i64,
    paid_by: &str,
    splits: &[ExpenseSplit],
    grandfathered: &HashSet<&str>,
) -> Res<()> {
    if amount_cents <= 0 {
        return Err("Amount must be greater than zero".to_string());
    }
    if splits.is_empty() {
        return Err("Expense must be split among at least one participant".to_string());
    }
    if splits.iter().any(|s| s.shares == 0) {
        return Err("Shares must be at least 1".to_string());
    }
    let mut seen = HashSet::new();
    if !splits
        .iter()
        .all(|s| seen.insert(s.participant_id.as_str()))
    {
        return Err("A participant appears twice in the split".to_string());
    }
    let usable = |id: &str| {
        group
            .participants
            .iter()
            .any(|p| p.id == id && (!p.removed || grandfathered.contains(id)))
    };
    if !usable(paid_by) {
        return Err("The payer is not an active member of this group".to_string());
    }
    if !splits.iter().all(|s| usable(&s.participant_id)) {
        return Err(
            "The split includes someone who is not an active member of this group".to_string(),
        );
    }
    Ok(())
}

pub fn add_expense(
    doc: &LoroDoc,
    title: &str,
    amount_cents: i64,
    paid_by: String,
    splits: Vec<ExpenseSplit>,
    created_at: Option<DateTime<Utc>>,
) -> Res<()> {
    let group = read_group(doc)?;
    validate_expense(&group, amount_cents, &paid_by, &splits, &HashSet::new())?;
    let now = Utc::now();
    insert_expense(
        doc,
        &Expense {
            id: Uuid::new_v4().to_string(),
            group_id: group.id,
            title: title.trim().to_string(),
            amount_cents,
            paid_by,
            splits,
            created_at: created_at.unwrap_or(now),
            updated_at: now,
            history: Vec::new(),
            is_reimbursement: false,
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn update_expense(
    doc: &LoroDoc,
    expense_id: &str,
    title: &str,
    amount_cents: i64,
    paid_by: String,
    splits: Vec<ExpenseSplit>,
    created_at: Option<DateTime<Utc>>,
) -> Res<()> {
    let group = read_group(doc)?;
    let expense = group
        .expenses
        .iter()
        .find(|e| e.id == expense_id)
        .ok_or_else(|| "Expense not found".to_string())?;
    let grandfathered: HashSet<&str> = std::iter::once(expense.paid_by.as_str())
        .chain(expense.splits.iter().map(|s| s.participant_id.as_str()))
        .collect();
    validate_expense(&group, amount_cents, &paid_by, &splits, &grandfathered)?;

    let name_of = |id: &str| {
        group
            .participants
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.as_str())
            .unwrap_or("Unknown")
    };

    let target = child_map(&doc.get_map(EXPENSES), expense_id)
        .ok_or_else(|| "Expense not found".to_string())?;

    // Only changed fields are written, so a concurrent edit to another field survives the merge.
    let mut changes = Vec::new();
    let title = title.trim();
    if expense.title != title {
        changes.push(format!(
            "Title changed from '{}' to '{}'",
            expense.title, title
        ));
        target.insert("title", title).map_err(doc_err)?;
    }
    if expense.amount_cents != amount_cents {
        changes.push(format!(
            "Amount changed from {:.2} to {:.2}",
            expense.amount_cents as f64 / 100.0,
            amount_cents as f64 / 100.0
        ));
        target
            .insert("amount_cents", amount_cents)
            .map_err(doc_err)?;
    }
    if expense.paid_by != paid_by {
        changes.push(format!(
            "Payer changed from {} to {}",
            name_of(&expense.paid_by),
            name_of(&paid_by)
        ));
        target
            .insert("paid_by", paid_by.as_str())
            .map_err(doc_err)?;
    }
    if expense.splits != splits {
        changes.push("Participants / parts allocation updated".to_string());
        target
            .insert("splits", splits_value(&splits))
            .map_err(doc_err)?;
    }
    if let Some(new_created_at) = created_at {
        if expense.created_at != new_created_at {
            changes.push(format!(
                "Date changed from {} to {}",
                expense.created_at.format("%Y-%m-%d"),
                new_created_at.format("%Y-%m-%d")
            ));
            target
                .insert("created_at", timestamp(new_created_at))
                .map_err(doc_err)?;
        }
    }

    let summary = if changes.is_empty() {
        "Updated without major changes".to_string()
    } else {
        changes.join("; ")
    };
    let now = Utc::now();
    let entry = ExpenseHistoryEntry {
        edited_at: now,
        previous_title: expense.title.clone(),
        previous_amount_cents: expense.amount_cents,
        previous_paid_by: expense.paid_by.clone(),
        previous_splits: expense.splits.clone(),
        summary,
    };
    child_list(&target, HISTORY)?
        .push(history_value(&entry))
        .map_err(doc_err)?;
    target
        .insert("updated_at", timestamp(now))
        .map_err(doc_err)?;
    Ok(())
}

pub fn delete_expense(doc: &LoroDoc, expense_id: &str) -> Res<()> {
    let expenses = doc.get_map(EXPENSES);
    if child_map(&expenses, expense_id).is_none() {
        return Err("Expense not found".to_string());
    }
    expenses.delete(expense_id).map_err(doc_err)
}

/// Records a payment between two people. Removed participants are allowed here, since
/// settling up with someone who left the group is exactly when this is needed.
pub fn record_reimbursement(
    doc: &LoroDoc,
    from_id: String,
    to_id: String,
    amount_cents: i64,
    notes: Option<String>,
) -> Res<()> {
    if amount_cents <= 0 {
        return Err("Reimbursement amount must be greater than zero".to_string());
    }
    if from_id == to_id {
        return Err("Sender and recipient cannot be the same person".to_string());
    }
    let group = read_group(doc)?;
    let name_of = |id: &str, role: &str| {
        group
            .participants
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
            .ok_or_else(|| format!("{role} participant not found"))
    };
    let from_name = name_of(&from_id, "Sender")?;
    let to_name = name_of(&to_id, "Recipient")?;

    let title = match notes {
        Some(n) if !n.trim().is_empty() => {
            format!("Payment: {} → {} ({})", from_name, to_name, n.trim())
        }
        _ => format!("Payment: {} → {}", from_name, to_name),
    };

    let now = Utc::now();
    insert_expense(
        doc,
        &Expense {
            id: Uuid::new_v4().to_string(),
            group_id: group.id,
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
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use loro::ExportMode;

    fn split(id: &str, shares: u32) -> ExpenseSplit {
        ExpenseSplit {
            participant_id: id.to_string(),
            shares,
        }
    }

    fn sample() -> (LoroDoc, Group) {
        let doc =
            new_group_doc(" Trip ", "eur", &["Alice".into(), " ".into(), "Bob".into()]).unwrap();
        let group = read_group(&doc).unwrap();
        (doc, group)
    }

    /// Makes an independent replica, as another device would have after syncing.
    fn fork(doc: &LoroDoc) -> LoroDoc {
        let other = LoroDoc::new();
        other
            .import(&doc.export(ExportMode::Snapshot).unwrap())
            .unwrap();
        other
    }

    fn merge(a: &LoroDoc, b: &LoroDoc) {
        a.import(&b.export(ExportMode::Snapshot).unwrap()).unwrap();
        b.import(&a.export(ExportMode::Snapshot).unwrap()).unwrap();
    }

    #[test]
    fn new_group_round_trips() {
        let (_, group) = sample();
        assert_eq!(group.name, "Trip");
        assert_eq!(group.currency, "EUR");
        let names: Vec<_> = group.participants.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Alice", "Bob"], "blank names dropped, order kept");
    }

    #[test]
    fn expense_edit_records_history() {
        let (doc, g) = sample();
        let (alice, bob) = (&g.participants[0].id, &g.participants[1].id);
        add_expense(
            &doc,
            "Dinner",
            3000,
            alice.clone(),
            vec![split(alice, 1), split(bob, 1)],
            None,
        )
        .unwrap();
        let expense_id = read_group(&doc).unwrap().expenses[0].id.clone();

        update_expense(
            &doc,
            &expense_id,
            "Dinner",
            4500,
            bob.clone(),
            vec![split(bob, 1)],
            None,
        )
        .unwrap();

        let e = &read_group(&doc).unwrap().expenses[0];
        assert_eq!(e.amount_cents, 4500);
        assert_eq!(e.paid_by, *bob);
        assert_eq!(e.history.len(), 1);
        assert_eq!(e.history[0].previous_amount_cents, 3000);
        assert!(e.history[0]
            .summary
            .contains("Payer changed from Alice to Bob"));
    }

    #[test]
    fn rejects_unknown_and_removed_participants() {
        let (doc, g) = sample();
        let (alice, bob) = (&g.participants[0].id, &g.participants[1].id);
        let err =
            add_expense(&doc, "X", 100, "ghost".into(), vec![split(alice, 1)], None).unwrap_err();
        assert!(err.contains("payer"));

        add_expense(
            &doc,
            "Old",
            100,
            alice.clone(),
            vec![split(alice, 1), split(bob, 1)],
            None,
        )
        .unwrap();
        remove_participant(&doc, bob).unwrap();
        let err =
            add_expense(&doc, "New", 100, alice.clone(), vec![split(bob, 1)], None).unwrap_err();
        assert!(err.contains("not an active member"));

        // Editing an expense Bob was already on is still allowed.
        let old = read_group(&doc).unwrap().expenses[0].id.clone();
        update_expense(
            &doc,
            &old,
            "Old (edited)",
            200,
            alice.clone(),
            vec![split(alice, 1), split(bob, 1)],
            None,
        )
        .unwrap();

        // And a removed participant can still settle up.
        record_reimbursement(&doc, bob.clone(), alice.clone(), 100, None).unwrap();
    }

    #[test]
    fn concurrent_field_edits_both_survive() {
        let (a, g) = sample();
        let alice = &g.participants[0].id;
        add_expense(&a, "Taxi", 1000, alice.clone(), vec![split(alice, 1)], None).unwrap();
        a.commit();
        let b = fork(&a);
        let id = read_group(&a).unwrap().expenses[0].id.clone();

        update_expense(
            &a,
            &id,
            "Taxi",
            1200,
            alice.clone(),
            vec![split(alice, 1)],
            None,
        )
        .unwrap();
        update_expense(
            &b,
            &id,
            "Airport taxi",
            1000,
            alice.clone(),
            vec![split(alice, 1)],
            None,
        )
        .unwrap();
        a.commit();
        b.commit();
        merge(&a, &b);

        let (ga, gb) = (read_group(&a).unwrap(), read_group(&b).unwrap());
        assert_eq!(ga, gb, "replicas converge");
        let e = &ga.expenses[0];
        assert_eq!(e.amount_cents, 1200);
        assert_eq!(e.title, "Airport taxi");
        assert_eq!(e.history.len(), 2, "both edits are in the history");
    }

    #[test]
    fn concurrent_additions_merge() {
        let (a, g) = sample();
        let alice = &g.participants[0].id;
        let b = fork(&a);
        add_expense(
            &a,
            "Coffee",
            400,
            alice.clone(),
            vec![split(alice, 1)],
            None,
        )
        .unwrap();
        add_participant(&b, "Charlie").unwrap();
        a.commit();
        b.commit();
        merge(&a, &b);

        let g = read_group(&a).unwrap();
        assert_eq!(g.expenses.len(), 1);
        assert_eq!(g.participants.len(), 3);
        assert_eq!(g, read_group(&b).unwrap());
    }

    #[test]
    fn legacy_import_keeps_ids() {
        let (doc, g) = sample();
        let alice = &g.participants[0].id;
        add_expense(
            &doc,
            "Snacks",
            999,
            alice.clone(),
            vec![split(alice, 1)],
            None,
        )
        .unwrap();
        let original = read_group(&doc).unwrap();

        let migrated = read_group(&doc_from_legacy(&original).unwrap()).unwrap();
        assert_eq!(migrated, original);
    }
}
