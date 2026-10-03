//! Loro document schema for a group.
//!
//! Each group is one `LoroDoc`. It is the source of truth and the unit of sync:
//!
//! - `meta` (map): `id`, `name`, `currency`, `created_at`, `description`, `image`
//! - `participants` (map): participant id -> map { `name`, `removed`, `position`, `avatar` }
//! - `expenses` (map): expense id -> map { `title`, `amount_cents`, `paid_by`, `splits`,
//!   `created_at`, `updated_at`, `is_reimbursement`, `history` (list), `original` }
//!
//! `amount_cents` is always in the group's currency. An expense paid in another one also has
//! `original` (a plain value: `currency`, `amount_cents`, `rate`). A split is a number of
//! `shares`, or a fixed amount in the currency paid, the shares dividing what the fixed
//! amounts leave. App versions from before fixed amounts read only `shares`, so with fixed
//! amounts that field holds shares giving everyone the same amount (see `splits_value`).
//!
//! Pictures (`image`, `avatar`) are `data:` URLs of small images, which the app shrinks before
//! saving them.
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

use crate::engine;
use crate::models::{
    Expense, ExpenseHistoryEntry, ExpenseSplit, Group, OriginalAmount, Participant,
};

const META: &str = "meta";
const PARTICIPANTS: &str = "participants";
const EXPENSES: &str = "expenses";
const HISTORY: &str = "history";
const ORIGINAL: &str = "original";

/// Largest amount of one expense: ten trillion units, enough for any currency, and small
/// enough that JavaScript numbers hold it exactly.
pub const MAX_AMOUNT_CENTS: i64 = 1_000_000_000_000_000;

/// Longest description of a group, in characters.
pub const MAX_DESCRIPTION_CHARS: usize = 500;

/// Largest picture, as the `data:` URL it is stored as. The app saves them far smaller; this
/// keeps a crafted one from weighing on every member's device.
pub const MAX_IMAGE_LEN: usize = 200_000;

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

/// Shares that split an amount into `owed`, for app versions from before fixed amounts:
/// they read only `shares`, and still get everyone's balance right.
fn legacy_shares(owed: &[i64]) -> Vec<u32> {
    // Someone owing nothing still needs a share to be read, so nothing is simplified then:
    // one share in a total counted in cents is the smallest error.
    let divisor = if owed.contains(&0) {
        1
    } else {
        owed.iter().fold(0, |a, b| gcd(a, *b)).max(1)
    };
    let mut shares: Vec<i64> = owed.iter().map(|o| (o / divisor).max(1)).collect();
    while shares.iter().any(|s| *s > i64::from(u32::MAX)) {
        shares = shares.iter().map(|s| (s / 2).max(1)).collect();
    }
    shares.into_iter().map(|s| s as u32).collect()
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// With fixed amounts, each split says what it is in `parts` and `fixed_cents`, and `shares`
/// holds `legacy_shares`. Without, a split is only its `shares`, as it always was.
fn splits_value(
    amount_cents: i64,
    original: Option<&OriginalAmount>,
    splits: &[ExpenseSplit],
) -> LoroValue {
    let legacy = splits.iter().any(|s| s.fixed_cents.is_some()).then(|| {
        legacy_shares(&engine::owed(
            amount_cents,
            original.map(|o| o.amount_cents),
            splits,
        ))
    });
    let list: Vec<LoroValue> = splits
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let mut fields = HashMap::from([
                (
                    "participant_id".to_string(),
                    s.participant_id.as_str().into(),
                ),
                ("shares".to_string(), i64::from(s.shares).into()),
            ]);
            if let Some(legacy) = &legacy {
                fields.insert("shares".to_string(), i64::from(legacy[i]).into());
                fields.insert("parts".to_string(), i64::from(s.shares).into());
            }
            if let Some(fixed) = s.fixed_cents {
                fields.insert("fixed_cents".to_string(), fixed.into());
            }
            LoroValue::Map(fields.into())
        })
        .collect();
    LoroValue::List(list.into())
}

fn original_value(original: &OriginalAmount) -> LoroValue {
    map_value([
        ("currency", original.currency.as_str().into()),
        ("amount_cents", original.amount_cents.into()),
        ("rate", original.rate.as_str().into()),
    ])
}

fn history_value(entry: &ExpenseHistoryEntry) -> LoroValue {
    let previous_original = entry.previous_original.as_ref();
    map_value([
        ("edited_at", timestamp(entry.edited_at).into()),
        ("previous_title", entry.previous_title.as_str().into()),
        ("previous_amount_cents", entry.previous_amount_cents.into()),
        ("previous_paid_by", entry.previous_paid_by.as_str().into()),
        (
            "previous_splits",
            splits_value(
                entry.previous_amount_cents,
                previous_original,
                &entry.previous_splits,
            ),
        ),
        (
            "previous_original",
            previous_original.map_or(LoroValue::Null, original_value),
        ),
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
    #[serde(default)]
    description: String,
    #[serde(default)]
    image: Option<String>,
}

#[derive(Deserialize)]
struct DocParticipant {
    name: String,
    #[serde(default)]
    removed: bool,
    #[serde(default)]
    position: i64,
    #[serde(default)]
    avatar: Option<String>,
}

/// A picture read from a document, or nothing when it isn't one the app would have saved.
fn read_image(image: Option<String>) -> Option<String> {
    image.filter(|image| check_image(image).is_ok())
}

/// A split as stored (see `splits_value`).
#[derive(Deserialize)]
struct DocSplit {
    participant_id: String,
    shares: u32,
    #[serde(default)]
    parts: Option<u32>,
    #[serde(default)]
    fixed_cents: Option<i64>,
}

/// `legacy` reads only what app versions from before fixed amounts do.
fn read_splits(splits: &[DocSplit], legacy: bool) -> Vec<ExpenseSplit> {
    splits
        .iter()
        .map(|s| ExpenseSplit {
            participant_id: s.participant_id.clone(),
            shares: if legacy {
                s.shares
            } else {
                s.parts.unwrap_or(s.shares)
            },
            fixed_cents: s.fixed_cents.filter(|_| !legacy),
        })
        .collect()
}

#[derive(Deserialize)]
struct DocHistoryEntry {
    edited_at: DateTime<Utc>,
    previous_title: String,
    previous_amount_cents: i64,
    previous_paid_by: String,
    previous_splits: Vec<DocSplit>,
    #[serde(default)]
    previous_original: Option<OriginalAmount>,
    summary: String,
}

#[derive(Deserialize)]
struct DocExpense {
    title: String,
    amount_cents: i64,
    #[serde(default)]
    original: Option<OriginalAmount>,
    paid_by: String,
    splits: Vec<DocSplit>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    #[serde(default)]
    is_reimbursement: bool,
    #[serde(default)]
    history: Vec<DocHistoryEntry>,
}

impl DocExpense {
    /// The splits, checked. An older app version changes the amount without the splits; if
    /// the fixed amounts no longer fit it, the shares that version reads are used.
    fn checked_splits(&self) -> Res<Vec<ExpenseSplit>> {
        let splits = read_splits(&self.splits, false);
        if check_amounts(self.amount_cents, self.original.as_ref(), &splits).is_ok() {
            return Ok(splits);
        }
        let splits = read_splits(&self.splits, true);
        check_amounts(self.amount_cents, self.original.as_ref(), &splits)?;
        Ok(splits)
    }
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
                        avatar: read_image(p.avatar),
                    },
                )
            })
            .collect();
    // Concurrent additions can share a position; the id keeps the order stable everywhere.
    participants.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));

    let mut expenses: Vec<Expense> = entries::<DocExpense>(&doc.get_map(EXPENSES), EXPENSES)
        .into_iter()
        // Synced edits skip `validate_expense`, and the balance engine relies on these.
        .filter_map(|(id, mut e)| {
            // What the expense cost elsewhere is a note next to its amount: unusable, it is
            // left out rather than taking the expense with it.
            e.original = e.original.filter(|o| check_original(o).is_ok());
            let splits = match e.checked_splits() {
                Ok(splits) => splits,
                Err(err) => {
                    eprintln!("[doc] skipping expense {id}: {err}");
                    return None;
                }
            };
            Some(Expense {
                id,
                group_id: meta.id.clone(),
                title: e.title,
                amount_cents: e.amount_cents,
                original: e.original,
                paid_by: e.paid_by,
                splits,
                created_at: e.created_at,
                updated_at: e.updated_at,
                history: e
                    .history
                    .into_iter()
                    .map(|h| ExpenseHistoryEntry {
                        edited_at: h.edited_at,
                        previous_title: h.previous_title,
                        previous_amount_cents: h.previous_amount_cents,
                        previous_paid_by: h.previous_paid_by,
                        previous_splits: read_splits(&h.previous_splits, false),
                        previous_original: h.previous_original,
                        summary: h.summary,
                    })
                    .collect(),
                is_reimbursement: e.is_reimbursement,
            })
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
        description: meta
            .description
            .chars()
            .take(MAX_DESCRIPTION_CHARS)
            .collect(),
        image: read_image(meta.image),
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
    let original = expense.original.as_ref();
    if let Some(original) = original {
        e.insert(ORIGINAL, original_value(original))
            .map_err(doc_err)?;
    }
    e.insert(
        "splits",
        splits_value(expense.amount_cents, original, &expense.splits),
    )
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

/// Checks a picture is one the app stores: a `data:` URL of a JPEG, PNG or WebP, not too big.
pub fn check_image(image: &str) -> Res<()> {
    let is_picture = ["jpeg", "png", "webp"].iter().any(|kind| {
        image
            .strip_prefix("data:image/")
            .and_then(|rest| rest.strip_prefix(kind))
            .and_then(|rest| rest.strip_prefix(";base64,"))
            .is_some_and(|data| {
                !data.is_empty()
                    && data
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
            })
    });
    if !is_picture {
        return Err("This picture can't be used: pick a JPEG, PNG or WebP image".to_string());
    }
    if image.len() > MAX_IMAGE_LEN {
        return Err("This picture is too big".to_string());
    }
    Ok(())
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

/// An expense read from a file, before the group exists: people are positions in the
/// imported list of names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedExpense {
    pub title: String,
    pub amount_cents: i64,
    pub paid_by: usize,
    pub original: Option<OriginalAmount>,
    // (person, shares, fixed amount), as in `ExpenseSplit`
    pub splits: Vec<(usize, u32, Option<i64>)>,
    pub created_at: DateTime<Utc>,
    pub is_reimbursement: bool,
}

/// Creates the document for a group read from a file, with its expenses.
pub fn imported_group_doc(
    name: &str,
    currency: &str,
    participant_names: &[String],
    expenses: &[ImportedExpense],
) -> Res<LoroDoc> {
    let doc = new_group_doc(name, currency, participant_names)?;
    let group = read_group(&doc)?;
    // `new_group_doc` skips blank names, which would shift the positions.
    if group.participants.len() != participant_names.len() {
        return Err("Participant name cannot be empty".to_string());
    }
    let id = |position: usize| {
        group
            .participants
            .get(position)
            .map(|p| p.id.clone())
            .ok_or_else(|| "Participant not found".to_string())
    };
    let now = Utc::now();
    for e in expenses {
        let splits = e
            .splits
            .iter()
            .map(|(position, shares, fixed_cents)| {
                Ok(ExpenseSplit {
                    participant_id: id(*position)?,
                    shares: *shares,
                    fixed_cents: *fixed_cents,
                })
            })
            .collect::<Res<Vec<_>>>()?;
        let paid_by = id(e.paid_by)?;
        let original = normal_original(&group, e.original.clone())?;
        validate_expense(
            &group,
            e.amount_cents,
            original.as_ref(),
            &paid_by,
            &splits,
            &HashSet::new(),
        )?;
        insert_expense(
            &doc,
            &Expense {
                id: Uuid::new_v4().to_string(),
                group_id: group.id.clone(),
                title: e.title.trim().to_string(),
                amount_cents: e.amount_cents,
                original,
                paid_by,
                splits,
                created_at: e.created_at,
                updated_at: now,
                history: Vec::new(),
                is_reimbursement: e.is_reimbursement,
            },
        )?;
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

/// Sets the group's name, currency, description and picture. Amounts are kept as they are,
/// not converted. An empty description, or no picture, removes it.
pub fn update_group(
    doc: &LoroDoc,
    name: &str,
    currency: &str,
    description: &str,
    image: Option<&str>,
) -> Res<()> {
    let name = group_name(name)?;
    let currency = currency_code(currency)?;
    let description = description.trim();
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(format!(
            "This description is too long ({MAX_DESCRIPTION_CHARS} characters at most)"
        ));
    }
    if let Some(image) = image {
        check_image(image)?;
    }
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
    if group.description != description {
        meta.insert("description", description).map_err(doc_err)?;
    }
    if group.image.as_deref() != image {
        match image {
            Some(image) => meta.insert("image", image).map_err(doc_err)?,
            None => meta.delete("image").map_err(doc_err)?,
        }
    }
    Ok(())
}

/// Sets or removes a participant's picture.
pub fn set_participant_avatar(
    doc: &LoroDoc,
    participant_id: &str,
    avatar: Option<&str>,
) -> Res<()> {
    let participant = child_map(&doc.get_map(PARTICIPANTS), participant_id)
        .ok_or_else(|| "Participant not found".to_string())?;
    let current = match participant.get("avatar") {
        Some(ValueOrContainer::Value(LoroValue::String(current))) => Some(current.to_string()),
        _ => None,
    };
    if current.as_deref() == avatar {
        return Ok(());
    }
    match avatar {
        Some(avatar) => {
            check_image(avatar)?;
            participant.insert("avatar", avatar).map_err(doc_err)
        }
        None => participant.delete("avatar").map_err(doc_err),
    }
}

/// The title `record_reimbursement` gives a payment, before any notes.
pub(crate) fn payment_title(from_name: &str, to_name: &str) -> String {
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

/// An exchange rate as stored: a positive decimal number with a point, such as "0.9234".
pub(crate) fn exchange_rate(rate: &str) -> Res<String> {
    let rate = rate.trim().replace(',', ".");
    let (whole, decimals) = rate.split_once('.').unwrap_or((&rate, ""));
    let digits = |s: &str| s.chars().all(|c| c.is_ascii_digit());
    if rate.len() > 20
        || !digits(whole)
        || !digits(decimals)
        || !rate.chars().any(|c| matches!(c, '1'..='9'))
    {
        return Err("The exchange rate must be a number above zero, such as 0.92".to_string());
    }
    Ok(rate)
}

fn check_original(original: &OriginalAmount) -> Res<()> {
    check_amount(original.amount_cents)?;
    if currency_code(&original.currency)? != original.currency
        || exchange_rate(&original.rate)? != original.rate
    {
        return Err("The expense's currency or exchange rate is not valid".to_string());
    }
    Ok(())
}

/// What the user typed for an expense in another currency, as stored. An expense in the
/// group's own currency has no original amount.
fn normal_original(group: &Group, original: Option<OriginalAmount>) -> Res<Option<OriginalAmount>> {
    let Some(original) = original else {
        return Ok(None);
    };
    let currency = currency_code(&original.currency)?;
    if currency == group.currency {
        return Err(format!(
            "The group is in {currency} already: leave out the exchange rate"
        ));
    }
    check_amount(original.amount_cents)?;
    Ok(Some(OriginalAmount {
        currency,
        amount_cents: original.amount_cents,
        rate: exchange_rate(&original.rate)?,
    }))
}

fn money(cents: i128) -> String {
    format!("{}.{:02}", cents / 100, cents % 100)
}

/// `original` is taken as checked. Fixed amounts are in its currency when there is one.
pub(crate) fn check_amounts(
    amount_cents: i64,
    original: Option<&OriginalAmount>,
    splits: &[ExpenseSplit],
) -> Res<()> {
    check_amount(amount_cents)?;
    if splits.is_empty() {
        return Err("Expense must be split among at least one participant".to_string());
    }
    let mut fixed: i128 = 0;
    let mut parts = false;
    for s in splits {
        match s.fixed_cents {
            Some(amount) if amount > 0 && amount <= MAX_AMOUNT_CENTS && s.shares == 0 => {
                fixed += i128::from(amount);
            }
            Some(_) => return Err("A fixed amount must be above zero".to_string()),
            None if s.shares == 0 => return Err("Shares must be at least 1".to_string()),
            None => parts = true,
        }
    }
    let paid = i128::from(original.map_or(amount_cents, |o| o.amount_cents));
    if fixed > paid {
        return Err(format!(
            "The fixed amounts add up to {}, more than the expense's {}",
            money(fixed),
            money(paid)
        ));
    }
    if !parts && fixed != paid {
        return Err(format!(
            "The amounts add up to {}, not the expense's {}",
            money(fixed),
            money(paid)
        ));
    }
    Ok(())
}

/// `grandfathered` lists IDs that may be used even if removed: the people already on an
/// expense being edited, so editing an old expense doesn't force dropping them.
fn validate_expense(
    group: &Group,
    amount_cents: i64,
    original: Option<&OriginalAmount>,
    paid_by: &str,
    splits: &[ExpenseSplit],
    grandfathered: &HashSet<&str>,
) -> Res<()> {
    check_amounts(amount_cents, original, splits)?;
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
    original: Option<OriginalAmount>,
) -> Res<()> {
    let group = read_group(doc)?;
    let original = normal_original(&group, original)?;
    validate_expense(
        &group,
        amount_cents,
        original.as_ref(),
        &paid_by,
        &splits,
        &HashSet::new(),
    )?;
    let now = Utc::now();
    insert_expense(
        doc,
        &Expense {
            id: Uuid::new_v4().to_string(),
            group_id: group.id,
            title: title.trim().to_string(),
            amount_cents,
            original,
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
    original: Option<OriginalAmount>,
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
    let original = normal_original(&group, original)?;
    validate_expense(
        &group,
        amount_cents,
        original.as_ref(),
        &paid_by,
        &splits,
        &grandfathered,
    )?;

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
    if expense.original != original {
        let describe = |o: &Option<OriginalAmount>| match o {
            Some(o) => format!(
                "{} {} at {}",
                money(i128::from(o.amount_cents)),
                o.currency,
                o.rate
            ),
            None => group.currency.clone(),
        };
        changes.push(format!(
            "Paid in {} instead of {}",
            describe(&original),
            describe(&expense.original)
        ));
        match &original {
            Some(original) => target.insert(ORIGINAL, original_value(original)),
            None => target.delete(ORIGINAL),
        }
        .map_err(doc_err)?;
    }
    if expense.splits != splits {
        changes.push("Participants / parts allocation updated".to_string());
    }
    // With fixed amounts, the stored splits also depend on the amounts (see `splits_value`).
    let fixed = |splits: &[ExpenseSplit]| splits.iter().any(|s| s.fixed_cents.is_some());
    if expense.splits != splits
        || (fixed(&splits)
            && (expense.amount_cents != amount_cents || expense.original != original))
    {
        target
            .insert(
                "splits",
                splits_value(amount_cents, original.as_ref(), &splits),
            )
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
        previous_original: expense.original.clone(),
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
                fixed_cents: None,
            }],
            created_at: now,
            updated_at: now,
            history: Vec::new(),
            is_reimbursement: true,
            original: None,
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
            fixed_cents: None,
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

    fn fixed(id: &str, amount: i64) -> ExpenseSplit {
        ExpenseSplit {
            participant_id: id.to_string(),
            shares: 0,
            fixed_cents: Some(amount),
        }
    }

    fn net(doc: &LoroDoc, id: &str) -> i64 {
        engine::calculate_balances(&read_group(doc).unwrap())
            .into_iter()
            .find(|b| b.participant_id == id)
            .unwrap()
            .net_cents
    }

    /// The splits as an app version from before fixed amounts reads them.
    fn legacy_splits(doc: &LoroDoc) -> Vec<ExpenseSplit> {
        entries::<DocExpense>(&doc.get_map(EXPENSES), EXPENSES)
            .into_iter()
            .flat_map(|(_, e)| read_splits(&e.splits, true))
            .collect()
    }

    #[test]
    fn fixed_amounts_and_another_currency() {
        let (doc, g) = sample();
        let (alice, bob) = (&g.participants[0].id, &g.participants[1].id);
        let usd = |amount_cents: i64, rate: &str| {
            Some(OriginalAmount {
                currency: "usd".to_string(),
                amount_cents,
                rate: rate.to_string(),
            })
        };
        let add = |amount: i64, splits: Vec<ExpenseSplit>, original| {
            add_expense(&doc, "Taxi", amount, alice.clone(), splits, None, original)
        };

        // Bob owes 12.50, Alice the rest.
        add(3000, vec![split(alice, 1), fixed(bob, 1250)], None).unwrap();
        assert_eq!(net(&doc, bob), -1250);
        let e = read_group(&doc).unwrap().expenses.remove(0);
        assert_eq!(e.splits, [split(alice, 1), fixed(bob, 1250)]);
        // An older app reads plain shares that give the same amounts.
        assert_eq!(legacy_splits(&doc), [split(alice, 7), split(bob, 5)]);

        // Changing the amount keeps the fixed part, and what the older app reads follows.
        update_expense(
            &doc,
            &e.id,
            "Taxi",
            2500,
            alice.clone(),
            e.splits.clone(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(net(&doc, bob), -1250);
        assert_eq!(legacy_splits(&doc), [split(alice, 1), split(bob, 1)]);

        // An older app changes the amount alone. The fixed part no longer fits in 10.00, so
        // the shares it reads are used here too.
        child_map(&doc.get_map(EXPENSES), &e.id)
            .unwrap()
            .insert("amount_cents", 1000)
            .unwrap();
        assert_eq!(
            read_group(&doc).unwrap().expenses[0].splits,
            [split(alice, 1), split(bob, 1)]
        );
        delete_expense(&doc, &e.id).unwrap();

        // 50.00 USD at 0.9234: Bob owes 20.00 USD of it, which is 18.47 of the 46.17.
        add(
            4617,
            vec![split(alice, 1), fixed(bob, 2000)],
            usd(5000, " 0,9234 "),
        )
        .unwrap();
        assert_eq!(net(&doc, bob), -1847);
        let e = read_group(&doc).unwrap().expenses.remove(0);
        assert_eq!(
            e.original,
            usd(5000, "0.9234").map(|o| OriginalAmount {
                currency: "USD".to_string(),
                ..o
            })
        );
        // Back to the group's currency: recorded in the history.
        update_expense(
            &doc,
            &e.id,
            "Taxi",
            4617,
            alice.clone(),
            vec![split(alice, 1), split(bob, 1)],
            None,
            None,
        )
        .unwrap();
        let e = read_group(&doc).unwrap().expenses.remove(0);
        assert_eq!(e.original, None);
        assert_eq!(
            e.history[0].previous_original,
            usd(5000, "0.9234").map(|o| OriginalAmount {
                currency: "USD".to_string(),
                ..o
            })
        );
        assert_eq!(
            e.history[0].previous_splits,
            [split(alice, 1), fixed(bob, 2000)]
        );
        assert!(e.history[0]
            .summary
            .contains("Paid in EUR instead of 50.00 USD at 0.9234"));

        let both = || vec![fixed(alice, 1000), fixed(bob, 1250)];
        assert_eq!(
            add(3000, both(), None).unwrap_err(),
            "The amounts add up to 22.50, not the expense's 30.00"
        );
        assert_eq!(
            add(2000, vec![split(alice, 1), fixed(bob, 2500)], None).unwrap_err(),
            "The fixed amounts add up to 25.00, more than the expense's 20.00"
        );
        assert_eq!(
            add(2000, vec![split(alice, 1), fixed(bob, 0)], None).unwrap_err(),
            "A fixed amount must be above zero"
        );
        assert_eq!(
            add(2000, vec![split(alice, 1)], usd(2200, "0")).unwrap_err(),
            "The exchange rate must be a number above zero, such as 0.92"
        );
        assert_eq!(
            add(
                2000,
                vec![split(alice, 1)],
                Some(OriginalAmount {
                    currency: "EUR".to_string(),
                    amount_cents: 2000,
                    rate: "1".to_string(),
                })
            )
            .unwrap_err(),
            "The group is in EUR already: leave out the exchange rate"
        );
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
        let err = add_expense(
            &doc,
            "X",
            100,
            "ghost".into(),
            vec![split(alice, 1)],
            None,
            None,
        )
        .unwrap_err();
        assert!(err.contains("payer"));

        add_expense(
            &doc,
            "Old",
            100,
            alice.clone(),
            vec![split(alice, 1), split(bob, 1)],
            None,
            None,
        )
        .unwrap();
        remove_participant(&doc, bob).unwrap();
        let err = add_expense(
            &doc,
            "New",
            100,
            alice.clone(),
            vec![split(bob, 1)],
            None,
            None,
        )
        .unwrap_err();
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
        add_expense(
            &a,
            "Taxi",
            1000,
            alice.clone(),
            vec![split(alice, 1)],
            None,
            None,
        )
        .unwrap();
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
            None,
        )
        .unwrap();

        update_group(&doc, " Lisbon ", "usd", "", None).unwrap();
        let g = read_group(&doc).unwrap();
        assert_eq!((g.name.as_str(), g.currency.as_str()), ("Lisbon", "USD"));
        assert_eq!(
            g.expenses[0].amount_cents, 1000,
            "amounts are not converted"
        );

        assert!(update_group(&doc, "  ", "EUR", "", None).is_err());
        for bad in ["", "EURO", "€", "E1R"] {
            assert!(
                update_group(&doc, "Lisbon", bad, "", None).is_err(),
                "{bad:?} accepted"
            );
        }
        assert!(new_group_doc("Trip", "euro", &[]).is_err());
    }

    #[test]
    fn a_group_and_its_people_have_pictures() {
        let (doc, group) = sample();
        let picture = "data:image/webp;base64,UklGRg==";
        assert_eq!((group.description.as_str(), &group.image), ("", &None));

        update_group(&doc, "Trip", "EUR", " A week away ", Some(picture)).unwrap();
        let alice = group.participants[0].id.clone();
        set_participant_avatar(&doc, &alice, Some(picture)).unwrap();
        let read = read_group(&doc).unwrap();
        assert_eq!(read.description, "A week away");
        assert_eq!(read.image.as_deref(), Some(picture));
        assert_eq!(read.participants[0].avatar.as_deref(), Some(picture));
        assert_eq!(read.participants[1].avatar, None);

        // Not pictures, or too big.
        for bad in [
            "https://example.com/a.png",
            "data:image/svg+xml;base64,AAAA",
            "data:image/png;base64,<script>",
            "data:image/png;base64,",
        ] {
            assert!(
                update_group(&doc, "Trip", "EUR", "", Some(bad)).is_err(),
                "{bad}"
            );
        }
        let big = format!("data:image/png;base64,{}", "A".repeat(MAX_IMAGE_LEN));
        assert!(set_participant_avatar(&doc, &alice, Some(&big)).is_err());
        let long = "a".repeat(MAX_DESCRIPTION_CHARS + 1);
        assert!(update_group(&doc, "Trip", "EUR", &long, None).is_err());

        update_group(&doc, "Trip", "EUR", "", None).unwrap();
        set_participant_avatar(&doc, &alice, None).unwrap();
        let read = read_group(&doc).unwrap();
        assert_eq!((read.description.as_str(), &read.image), ("", &None));
        assert_eq!(read.participants[0].avatar, None);

        // A picture written by something else than the app is left out, not shown.
        doc.get_map(META)
            .insert("image", "javascript:alert(1)")
            .unwrap();
        assert_eq!(read_group(&doc).unwrap().image, None);
    }

    #[test]
    fn concurrent_rename_and_currency_change_both_survive() {
        let (a, _) = sample();
        a.commit();
        let b = fork(&a);
        update_group(&a, "Lisbon", "EUR", "", None).unwrap();
        update_group(&b, "Trip", "CHF", "", None).unwrap();
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
                original: None,
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
            None,
        )
        .unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }
}
