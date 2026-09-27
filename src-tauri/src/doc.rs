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

/// Largest amount of one expense: ten trillion units, enough for any currency, and small
/// enough that JavaScript numbers hold it exactly.
pub const MAX_AMOUNT_CENTS: i64 = 1_000_000_000_000_000;

/// Deepest nesting read from a document. The schemas need 6 levels; anything deeper comes
/// from a crafted update, and following it without a bound could overflow the stack.
const MAX_DEPTH: usize = 16;

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

// Documents are read value by value with a depth bound rather than through
// `LoroDoc::get_deep_value`, which follows containers nested to any depth: a group member
// could otherwise crash every other member's app with one deeply nested update.

/// A document value as JSON, or `None` if it nests deeper than `MAX_DEPTH` or holds
/// something the schemas never use (binary data, other container types).
fn value_json(value: ValueOrContainer, depth: usize) -> Option<serde_json::Value> {
    if depth > MAX_DEPTH {
        return None;
    }
    match value {
        ValueOrContainer::Value(v) => plain_json(&v, depth),
        ValueOrContainer::Container(Container::Map(map)) => map_json(&map, depth),
        ValueOrContainer::Container(Container::List(list)) => {
            let mut items = Some(Vec::with_capacity(list.len()));
            list.for_each(|v| {
                items = items.take().and_then(|mut items| {
                    items.push(value_json(v, depth + 1)?);
                    Some(items)
                });
            });
            items.map(serde_json::Value::Array)
        }
        ValueOrContainer::Container(_) => None,
    }
}

fn map_json(map: &LoroMap, depth: usize) -> Option<serde_json::Value> {
    let mut fields = Some(serde_json::Map::new());
    map.for_each(|key, v| {
        fields = fields.take().and_then(|mut fields| {
            fields.insert(key.to_string(), value_json(v, depth + 1)?);
            Some(fields)
        });
    });
    fields.map(serde_json::Value::Object)
}

fn plain_json(value: &LoroValue, depth: usize) -> Option<serde_json::Value> {
    use serde_json::Value as Json;
    if depth > MAX_DEPTH {
        return None;
    }
    Some(match value {
        LoroValue::Null => Json::Null,
        LoroValue::Bool(b) => Json::Bool(*b),
        LoroValue::I64(n) => Json::from(*n),
        LoroValue::Double(n) => Json::from(*n),
        LoroValue::String(s) => Json::String(s.to_string()),
        LoroValue::List(items) => Json::Array(
            items
                .iter()
                .map(|v| plain_json(v, depth + 1))
                .collect::<Option<_>>()?,
        ),
        LoroValue::Map(fields) => Json::Object(
            fields
                .iter()
                .map(|(k, v)| Some((k.clone(), plain_json(v, depth + 1)?)))
                .collect::<Option<_>>()?,
        ),
        LoroValue::Binary(_) | LoroValue::Container(_) => return None,
    })
}

/// Entries of a root map, by id, skipping (and logging) any that don't match the schema.
/// A single malformed entry must not make the whole document unreadable.
pub(crate) fn entries<T: DeserializeOwned>(map: &LoroMap, what: &str) -> Vec<(String, T)> {
    let mut out = Vec::new();
    map.for_each(|id, value| {
        let parsed = value_json(value, 1)
            .ok_or_else(|| "unexpected shape or nesting".to_string())
            .and_then(|json| serde_json::from_value(json).map_err(|e| e.to_string()));
        match parsed {
            Ok(parsed) => out.push((id.to_string(), parsed)),
            Err(e) => eprintln!("[doc] skipping malformed {what} entry {id}: {e}"),
        }
    });
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Materializes the document into the `Group` shape the frontend and engine use.
pub fn read_group(doc: &LoroDoc) -> Res<Group> {
    let meta: DocMeta = map_json(&doc.get_map(META), 0)
        .ok_or_else(|| "unexpected shape or nesting".to_string())
        .and_then(|json| serde_json::from_value(json).map_err(|e| e.to_string()))
        .map_err(|e| format!("Group document has no valid metadata: {e}"))?;

    let mut participants: Vec<(i64, Participant)> =
        entries::<DocParticipant>(&doc.get_map(PARTICIPANTS), PARTICIPANTS)
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

    let mut expenses: Vec<Expense> = entries::<DocExpense>(&doc.get_map(EXPENSES), EXPENSES)
        .into_iter()
        // Synced edits skip `validate_expense`, and the balance engine relies on these.
        .filter(|(id, e)| match check_amounts(e.amount_cents, &e.splits) {
            Ok(()) => true,
            Err(err) => {
                eprintln!("[doc] skipping expense {id}: {err}");
                false
            }
        })
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

fn group_name(name: &str) -> Res<&str> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Group name cannot be empty".to_string());
    }
    Ok(name)
}

/// A currency code as stored: three letters, upper-cased.
fn currency_code(currency: &str) -> Res<String> {
    let code = currency.trim().to_uppercase();
    if code.len() == 3 && code.chars().all(|c| c.is_ascii_alphabetic()) {
        Ok(code)
    } else {
        Err("The currency must be a three-letter code, such as EUR".to_string())
    }
}

/// Creates the document for a brand-new group.
pub fn new_group_doc(name: &str, currency: &str, participant_names: &[String]) -> Res<LoroDoc> {
    let name = group_name(name)?;
    let currency = currency_code(currency)?;
    let doc = LoroDoc::new();
    write_meta(
        &doc,
        &Uuid::new_v4().to_string(),
        name,
        &currency,
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

/// Renames the group and sets its currency. Amounts are kept as they are, not converted.
pub fn update_group(doc: &LoroDoc, name: &str, currency: &str) -> Res<()> {
    let name = group_name(name)?;
    let currency = currency_code(currency)?;
    let group = read_group(doc)?;
    let meta = doc.get_map(META);
    // Only changed fields are written, so a concurrent change to the other one survives.
    if group.name != name {
        meta.insert("name", name).map_err(doc_err)?;
    }
    if group.currency != currency {
        meta.insert("currency", currency.as_str())
            .map_err(doc_err)?;
    }
    Ok(())
}

/// The title `record_reimbursement` gives a payment, before any notes.
fn payment_title(from_name: &str, to_name: &str) -> String {
    format!("Payment: {from_name} → {to_name}")
}

/// Renames a participant. Payments keep their names in their title, so the ones still
/// titled as recorded get the new name too.
pub fn rename_participant(doc: &LoroDoc, participant_id: &str, name: &str) -> Res<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Participant name cannot be empty".to_string());
    }
    let participant = child_map(&doc.get_map(PARTICIPANTS), participant_id)
        .ok_or_else(|| "Participant not found".to_string())?;
    let group = read_group(doc)?;
    let old_name = |id: &str| {
        group
            .participants
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.as_str())
    };
    if old_name(participant_id) == Some(name) {
        return Ok(());
    }
    let new_name = |id: &str| {
        if id == participant_id {
            Some(name)
        } else {
            old_name(id)
        }
    };

    let expenses = doc.get_map(EXPENSES);
    for e in group.expenses.iter().filter(|e| e.is_reimbursement) {
        let [to] = e.splits.as_slice() else { continue };
        let (from, to) = (e.paid_by.as_str(), to.participant_id.as_str());
        if from != participant_id && to != participant_id {
            continue;
        }
        let (Some(old_from), Some(old_to), Some(new_from), Some(new_to)) =
            (old_name(from), old_name(to), new_name(from), new_name(to))
        else {
            continue;
        };
        let Some(notes) = e.title.strip_prefix(&payment_title(old_from, old_to)) else {
            continue;
        };
        if !notes.is_empty() && !notes.starts_with(" (") {
            continue;
        }
        if let Some(target) = child_map(&expenses, &e.id) {
            let title = format!("{}{notes}", payment_title(new_from, new_to));
            target.insert("title", title).map_err(doc_err)?;
        }
    }
    participant.insert("name", name).map_err(doc_err)
}

/// Adds a participant and returns their id.
pub fn add_participant(doc: &LoroDoc, name: &str) -> Res<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Participant name cannot be empty".to_string());
    }
    let id = Uuid::new_v4().to_string();
    insert_participant(doc, &id, trimmed, false)?;
    Ok(id)
}

/// Soft-deletes a participant. Their past expenses and balance stay intact.
pub fn remove_participant(doc: &LoroDoc, participant_id: &str) -> Res<()> {
    let p = child_map(&doc.get_map(PARTICIPANTS), participant_id)
        .ok_or_else(|| "Participant not found".to_string())?;
    p.insert("removed", true).map_err(doc_err)
}

fn check_amount(amount_cents: i64) -> Res<()> {
    if amount_cents <= 0 {
        return Err("Amount must be greater than zero".to_string());
    }
    if amount_cents > MAX_AMOUNT_CENTS {
        return Err("Amount is too large".to_string());
    }
    Ok(())
}

fn check_amounts(amount_cents: i64, splits: &[ExpenseSplit]) -> Res<()> {
    check_amount(amount_cents)?;
    if splits.is_empty() {
        return Err("Expense must be split among at least one participant".to_string());
    }
    if splits.iter().any(|s| s.shares == 0) {
        return Err("Shares must be at least 1".to_string());
    }
    Ok(())
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
    check_amounts(amount_cents, splits)?;
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
    check_amount(amount_cents)?;
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
            format!("{} ({})", payment_title(&from_name, &to_name), n.trim())
        }
        _ => payment_title(&from_name, &to_name),
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
    fn group_name_and_currency_can_change() {
        let (doc, g) = sample();
        let alice = &g.participants[0].id;
        add_expense(
            &doc,
            "Taxi",
            1000,
            alice.clone(),
            vec![split(alice, 1)],
            None,
        )
        .unwrap();

        update_group(&doc, " Lisbon ", "usd").unwrap();
        let g = read_group(&doc).unwrap();
        assert_eq!((g.name.as_str(), g.currency.as_str()), ("Lisbon", "USD"));
        assert_eq!(
            g.expenses[0].amount_cents, 1000,
            "amounts are not converted"
        );

        assert!(update_group(&doc, "  ", "EUR").is_err());
        for bad in ["", "EURO", "€", "E1R"] {
            assert!(
                update_group(&doc, "Lisbon", bad).is_err(),
                "{bad:?} accepted"
            );
        }
        assert!(new_group_doc("Trip", "euro", &[]).is_err());
    }

    #[test]
    fn concurrent_rename_and_currency_change_both_survive() {
        let (a, _) = sample();
        a.commit();
        let b = fork(&a);
        update_group(&a, "Lisbon", "EUR").unwrap();
        update_group(&b, "Trip", "CHF").unwrap();
        a.commit();
        b.commit();
        merge(&a, &b);

        let g = read_group(&a).unwrap();
        assert_eq!((g.name.as_str(), g.currency.as_str()), ("Lisbon", "CHF"));
        assert_eq!(g, read_group(&b).unwrap());
    }

    #[test]
    fn renaming_updates_payment_titles_only() {
        let (doc, g) = sample();
        let (alice, bob) = (g.participants[0].id.clone(), g.participants[1].id.clone());
        record_reimbursement(&doc, alice.clone(), bob.clone(), 500, None).unwrap();
        record_reimbursement(&doc, bob.clone(), alice.clone(), 300, Some("cash".into())).unwrap();
        add_expense(
            &doc,
            "Alice's birthday",
            2000,
            alice.clone(),
            vec![split(&bob, 1)],
            None,
        )
        .unwrap();
        // A payment whose title someone rewrote keeps it.
        record_reimbursement(&doc, alice.clone(), bob.clone(), 100, None).unwrap();
        let custom = read_group(&doc)
            .unwrap()
            .expenses
            .into_iter()
            .find(|e| e.amount_cents == 100)
            .unwrap();
        update_expense(
            &doc,
            &custom.id,
            "Beers",
            100,
            alice.clone(),
            custom.splits.clone(),
            None,
        )
        .unwrap();

        rename_participant(&doc, &alice, " Alicia ").unwrap();

        let g = read_group(&doc).unwrap();
        assert_eq!(g.participants[0].name, "Alicia");
        let mut titles: Vec<_> = g.expenses.iter().map(|e| e.title.as_str()).collect();
        titles.sort();
        assert_eq!(
            titles,
            [
                "Alice's birthday",
                "Beers",
                "Payment: Alicia → Bob",
                "Payment: Bob → Alicia (cash)",
            ]
        );

        assert!(rename_participant(&doc, &alice, " ").is_err());
        assert!(rename_participant(&doc, "ghost", "Zoe").is_err());
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

    /// What a group member could sync in: writes that never went through the checks above.
    #[test]
    fn crafted_synced_data_is_skipped_not_fatal() {
        let (doc, g) = sample();
        let alice = &g.participants[0].id;
        add_expense(
            &doc,
            "Real",
            1000,
            alice.clone(),
            vec![split(alice, 1)],
            None,
        )
        .unwrap();

        let now = Utc::now();
        let unchecked = |title: &str, amount_cents: i64, splits: Vec<ExpenseSplit>| {
            let expense = Expense {
                id: Uuid::new_v4().to_string(),
                group_id: g.id.clone(),
                title: title.to_string(),
                amount_cents,
                paid_by: alice.clone(),
                splits,
                created_at: now,
                updated_at: now,
                history: Vec::new(),
                is_reimbursement: false,
            };
            insert_expense(&doc, &expense).unwrap();
        };
        unchecked("Too big", i64::MAX, vec![split(alice, 1)]);
        unchecked("Negative", -500, vec![split(alice, 1)]);
        unchecked("Nobody", 500, vec![]);
        unchecked("Zero shares", 500, vec![split(alice, 0)]);

        // Containers nested far deeper than any schema, and a deeply nested plain value.
        let participants = doc.get_map(PARTICIPANTS);
        let mut deep = participants
            .insert_container("deep", LoroMap::new())
            .unwrap();
        for _ in 0..5_000 {
            deep = deep.insert_container("name", LoroMap::new()).unwrap();
        }
        let mut nested = LoroValue::from("x");
        for _ in 0..100 {
            nested = LoroValue::List(vec![nested].into());
        }
        participants.insert("nested", nested).unwrap();

        let group = read_group(&doc).unwrap();
        assert_eq!(group.participants.len(), 2, "only Alice and Bob");
        let titles: Vec<_> = group.expenses.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, ["Real"]);

        let err = add_expense(
            &doc,
            "Huge",
            MAX_AMOUNT_CENTS + 1,
            alice.clone(),
            vec![split(alice, 1)],
            None,
        )
        .unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }
}
