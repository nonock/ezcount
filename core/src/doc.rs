//! Loro document schema for a group.
//!
//! Each group is one `LoroDoc`. It is the source of truth and the unit of sync:
//!
//! - `meta` (map): `id`, `name`, `currency`, `created_at`, `description`, `image`, `deleted`
//! - `deletion` (map): participant id -> true, for the members who agreed to delete the group
//! - `participants` (map): participant id -> map { `name`, `removed`, `position`, `avatar`,
//!   `iban`, `added_at`, `added_by`, `removed_at`, `removed_by` }
//! - `expenses` (map): expense id -> map { `title`, `category`, `amount_cents`, `paid_by`, `splits`,
//!   `created_at`, `updated_at`, `is_reimbursement`, `history` (list), `original`, `payers`,
//!   `added_at`, `added_by`, `recurring`, `items` }
//! - `comments` (map): comment id -> plain map { `expense`, `text`, `at`, `by` }
//! - `trash` (map): expense id -> the expense as above, with `deleted_at` and `deleted_by`
//! - `recurring` (map): id -> map { `title`, `category`, `amount_cents`, `paid_by`, `payers`,
//!   `splits`, `every`, `start`, `made`, `added_by` }
//!
//! `amount_cents` is always in the group's currency. An expense paid in another one also has
//! `original` (a plain value: `currency`, `amount_cents`, `rate`). A split is a number of
//! `shares`, or a fixed amount in the currency paid, the shares dividing what the fixed
//! amounts leave. App versions from before fixed amounts read only `shares`, so with fixed
//! amounts that field holds shares giving everyone the same amount (see `splits_value`).
//!
//! An expense several people paid also has `payers` (a plain value: who and how much, in the
//! currency paid), with the one who paid the most in `paid_by`. App versions from before that
//! read only `paid_by` and credit them the whole amount; when one of them edits the expense
//! so that `payers` no longer fits it, `paid_by` alone counts (see `check_payers`).
//!
//! Money that came in (`Expense::income`) is stored as an `amount_cents` below zero, the
//! rest as for an expense. App versions from before leave such an entry out, rather than
//! count it as money spent.
//!
//! Deleting an expense moves it to `trash`, so that app versions from before see it gone and
//! this one can put it back (`restore_expense`). It stays there `TRASH_DAYS`, after which any
//! device removes it for good (`empty_old_trash`).
//!
//! An expense entered line by line keeps its lines in `items` (a plain value: `name`,
//! `amount_cents`, `participants`), next to `splits` holding what each person owes of them as
//! fixed amounts: that is all the balances, and app versions from before, read. Lines that no
//! longer give those splits (an older version edited the expense) are left out when reading.
//!
//! Comments are in a map of their own, by comment id, rather than in a list under each
//! expense: two people commenting an expense at once then both keep theirs.
//!
//! A repeated expense is a model in `recurring`: its occurrence number `n` is due `n` weeks,
//! months or years (`every`) after `start`, and `made` counts the ones added. Any device adds
//! the ones due (`add_due_expenses`), under an id made of the model's and the day, so two
//! devices doing it at once add the same expense, not two.
//!
//! A group is deleted for everyone by setting `deleted`: devices stop listing it and drop it
//! once that is synced. While someone still owes something, that takes every member's
//! agreement, recorded in `deletion` (see `delete_or_vote`).
//!
//! Pictures (`image`, `avatar`) are `data:` URLs of small images, which the app shrinks before
//! saving them.
//!
//! Every field is its own last-writer-wins register, so concurrent edits to different fields
//! of one expense both survive a merge. `splits` is stored as a single plain value so an
//! allocation is always replaced as a whole and never merged into a mix of two edits.
//! Plain values are built by hand rather than through serde so that user text can never be
//! mistaken for a Loro container reference.

use chrono::{DateTime, Days, Months, SecondsFormat, Utc};
use loro::{Container, LoroDoc, LoroList, LoroMap, LoroValue, ValueOrContainer};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::engine;
use crate::models::{
    DeletedExpense, Expense, ExpenseComment, ExpenseHistoryEntry, ExpenseItem, ExpensePayer,
    ExpenseSplit, Group, OriginalAmount, Participant, RecurringExpense,
};

const META: &str = "meta";
const PARTICIPANTS: &str = "participants";
const EXPENSES: &str = "expenses";
const HISTORY: &str = "history";
const ORIGINAL: &str = "original";
const DELETION: &str = "deletion";
const TRASH: &str = "trash";
const RECURRING: &str = "recurring";
const COMMENTS: &str = "comments";
const ITEMS: &str = "items";

/// Days a deleted expense stays in the trash.
pub const TRASH_DAYS: u64 = 30;

/// Most lines in an expense entered item by item, and the longest name of one.
pub const MAX_ITEMS: usize = 100;
pub const MAX_ITEM_NAME_CHARS: usize = 80;

/// Longest comment, in characters.
pub const MAX_COMMENT_CHARS: usize = 500;

/// Most occurrences of one repeated expense added in a pass: a group nobody opened for years
/// catches up over a few.
const MAX_DUE: usize = 60;

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

fn payers_value(payers: &[ExpensePayer]) -> LoroValue {
    let list: Vec<LoroValue> = payers
        .iter()
        .map(|p| {
            map_value([
                ("participant_id", p.participant_id.as_str().into()),
                ("amount_cents", p.amount_cents.into()),
            ])
        })
        .collect();
    LoroValue::List(list.into())
}

fn items_value(items: &[ExpenseItem]) -> LoroValue {
    let list: Vec<LoroValue> = items
        .iter()
        .map(|item| {
            let people: Vec<LoroValue> = item
                .participants
                .iter()
                .map(|p| p.as_str().into())
                .collect();
            map_value([
                ("name", item.name.as_str().into()),
                ("amount_cents", item.amount_cents.into()),
                ("participants", LoroValue::List(people.into())),
            ])
        })
        .collect();
    LoroValue::List(list.into())
}

/// What each person owes of an expense entered line by line, in the order they first appear.
/// A line is shared equally between its people, the cents left over going to the first ones.
pub fn items_owed(items: &[ExpenseItem]) -> Vec<(String, i64)> {
    let mut owed: Vec<(String, i64)> = Vec::new();
    for item in items {
        let people = item.participants.len() as i64;
        if people == 0 {
            continue;
        }
        let (each, extra) = (item.amount_cents / people, item.amount_cents % people);
        for (i, id) in item.participants.iter().enumerate() {
            let part = each + i64::from((i as i64) < extra);
            match owed.iter_mut().find(|(who, _)| who == id) {
                Some((_, total)) => *total += part,
                None => owed.push((id.clone(), part)),
            }
        }
    }
    owed
}

/// The splits of an expense entered line by line: what each person owes, as a fixed amount.
fn items_splits(items: &[ExpenseItem]) -> Vec<ExpenseSplit> {
    items_owed(items)
        .into_iter()
        .filter(|(_, cents)| *cents > 0)
        .map(|(participant_id, cents)| ExpenseSplit {
            participant_id,
            shares: 0,
            fixed_cents: Some(cents),
        })
        .collect()
}

/// Lines add up to what was paid (`paid_cents`, in the currency paid), and each has an
/// amount and people.
fn check_items(paid_cents: i64, items: &[ExpenseItem]) -> Res<()> {
    if items.len() > MAX_ITEMS {
        return Err(format!("An expense can't have more than {MAX_ITEMS} items"));
    }
    let mut total: i128 = 0;
    for item in items {
        if item.name.chars().count() > MAX_ITEM_NAME_CHARS {
            return Err(format!(
                "An item's name is too long ({MAX_ITEM_NAME_CHARS} characters at most)"
            ));
        }
        if item.amount_cents <= 0 || item.amount_cents > MAX_AMOUNT_CENTS {
            return Err("An item's amount must be above zero".to_string());
        }
        if item.participants.is_empty() {
            return Err("Each item needs at least one person".to_string());
        }
        let mut seen = HashSet::new();
        if !item.participants.iter().all(|p| seen.insert(p.as_str())) {
            return Err("A participant appears twice on an item".to_string());
        }
        total += i128::from(item.amount_cents);
    }
    let paid = i128::from(paid_cents);
    if total != paid {
        return Err(format!(
            "The items add up to {}, not the expense's {}",
            money(total),
            money(paid)
        ));
    }
    Ok(())
}

/// The lines as stored, when they still give the expense's splits: an older app version
/// changes the amount or the splits without them, and then the splits alone count.
fn read_items(
    value: serde_json::Value,
    paid_cents: i64,
    splits: &[ExpenseSplit],
) -> Vec<ExpenseItem> {
    let Ok(items) = serde_json::from_value::<Vec<ExpenseItem>>(value) else {
        return Vec::new();
    };
    let sorted = |mut splits: Vec<ExpenseSplit>| {
        splits.sort_by(|a, b| a.participant_id.cmp(&b.participant_id));
        splits
    };
    let fits = !items.is_empty()
        && check_items(paid_cents, &items).is_ok()
        && sorted(items_splits(&items)) == sorted(splits.to_vec());
    if fits {
        items
    } else {
        Vec::new()
    }
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
        (
            "previous_category",
            entry
                .previous_category
                .as_deref()
                .map_or(LoroValue::Null, Into::into),
        ),
        ("previous_paid_by", entry.previous_paid_by.as_str().into()),
        ("previous_payers", payers_value(&entry.previous_payers)),
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
        (
            "edited_by",
            entry
                .edited_by
                .as_deref()
                .map_or(LoroValue::Null, Into::into),
        ),
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
    #[serde(default)]
    deleted: bool,
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
    #[serde(default)]
    iban: Option<String>,
    // Read as text: a date that isn't one is left out rather than taking the member with it.
    #[serde(default)]
    added_at: Option<String>,
    #[serde(default)]
    added_by: Option<String>,
    #[serde(default)]
    removed_at: Option<String>,
    #[serde(default)]
    removed_by: Option<String>,
}

fn read_time(text: Option<String>) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&text?)
        .ok()
        .map(|time| time.with_timezone(&Utc))
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
    #[serde(default)]
    previous_payers: Vec<ExpensePayer>,
    previous_splits: Vec<DocSplit>,
    #[serde(default)]
    previous_original: Option<OriginalAmount>,
    #[serde(default)]
    previous_category: Option<String>,
    summary: String,
    #[serde(default)]
    edited_by: Option<String>,
}

#[derive(Deserialize)]
struct DocExpense {
    title: String,
    #[serde(default)]
    category: Option<String>,
    amount_cents: i64,
    #[serde(default)]
    original: Option<OriginalAmount>,
    paid_by: String,
    #[serde(default)]
    payers: Vec<ExpensePayer>,
    splits: Vec<DocSplit>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    #[serde(default)]
    is_reimbursement: bool,
    #[serde(default)]
    history: Vec<DocHistoryEntry>,
    #[serde(default)]
    added_at: Option<String>,
    #[serde(default)]
    added_by: Option<String>,
    #[serde(default)]
    recurring: Option<String>,
    // Read as it comes: lines that aren't usable are left out, not the expense.
    #[serde(default)]
    items: serde_json::Value,
    // In the trash only.
    #[serde(default)]
    deleted_at: Option<String>,
    #[serde(default)]
    deleted_by: Option<String>,
}

/// A comment as stored.
#[derive(Deserialize)]
struct DocComment {
    expense: String,
    text: String,
    at: String,
    #[serde(default)]
    by: Option<String>,
}

/// The comments of each expense, the oldest first.
fn read_comments(doc: &LoroDoc) -> HashMap<String, Vec<ExpenseComment>> {
    let mut comments: HashMap<String, Vec<ExpenseComment>> = HashMap::new();
    for (id, c) in entries::<DocComment>(&doc.get_map(COMMENTS), COMMENTS) {
        let Some(created_at) = read_time(Some(c.at)) else {
            continue;
        };
        comments.entry(c.expense).or_default().push(ExpenseComment {
            id,
            text: c.text.chars().take(MAX_COMMENT_CHARS).collect(),
            created_at,
            by: c.by,
        });
    }
    for list in comments.values_mut() {
        list.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
    }
    comments
}

/// A repeated expense as stored.
#[derive(Deserialize)]
struct DocRecurring {
    title: String,
    #[serde(default)]
    category: Option<String>,
    amount_cents: i64,
    paid_by: String,
    #[serde(default)]
    payers: Vec<ExpensePayer>,
    splits: Vec<DocSplit>,
    every: String,
    start: DateTime<Utc>,
    #[serde(default)]
    made: i64,
    #[serde(default)]
    added_by: Option<String>,
}

/// A repeated expense and where it is at: its first day, and how many were added.
struct Repeated {
    expense: RecurringExpense,
    start: DateTime<Utc>,
    made: u32,
}

/// The day of occurrence `n` of an expense that started on `start`. Counting from the start
/// keeps the day of the month: the 31st gives the 28th in February, then the 31st again.
fn occurrence(start: DateTime<Utc>, every: &str, n: u32) -> Option<DateTime<Utc>> {
    match every {
        "week" => start.checked_add_days(Days::new(7 * u64::from(n))),
        "month" => start.checked_add_months(Months::new(n)),
        "year" => start.checked_add_months(Months::new(n.checked_mul(12)?)),
        _ => None,
    }
}

/// An expense as stored, checked and in the shape the app uses. Synced edits skip
/// `validate_expense`, and the balance engine relies on these checks.
fn read_expense(id: String, group_id: &str, mut e: DocExpense) -> Option<Expense> {
    // Money that came in is stored below zero.
    let income = e.amount_cents < 0;
    e.amount_cents = e.amount_cents.saturating_abs();
    // What the expense cost elsewhere is a note next to its amount: unusable, it is left out
    // rather than taking the expense with it.
    e.original = e.original.filter(|o| check_original(o).is_ok());
    let splits = match e.checked_splits() {
        Ok(splits) => splits,
        Err(err) => {
            eprintln!("[doc] skipping expense {id}: {err}");
            return None;
        }
    };
    let payers = e.checked_payers();
    let paid_cents = e
        .original
        .as_ref()
        .map_or(e.amount_cents, |o| o.amount_cents);
    let items = read_items(std::mem::take(&mut e.items), paid_cents, &splits);
    Some(Expense {
        id,
        group_id: group_id.to_string(),
        title: e.title,
        category: e.category.filter(|c| check_category(c).is_ok()),
        amount_cents: e.amount_cents,
        original: e.original,
        paid_by: e.paid_by,
        payers,
        splits,
        created_at: e.created_at,
        updated_at: e.updated_at,
        history: e
            .history
            .into_iter()
            .map(|h| ExpenseHistoryEntry {
                edited_at: h.edited_at,
                previous_title: h.previous_title,
                previous_amount_cents: h.previous_amount_cents.saturating_abs(),
                previous_paid_by: h.previous_paid_by,
                previous_payers: h.previous_payers,
                previous_splits: read_splits(&h.previous_splits, false),
                previous_original: h.previous_original,
                previous_category: h.previous_category,
                summary: h.summary,
                edited_by: h.edited_by,
            })
            .collect(),
        is_reimbursement: e.is_reimbursement,
        income,
        added_at: read_time(e.added_at),
        added_by: e.added_by,
        recurring: e.recurring,
        items,
        comments: Vec::new(),
    })
}

/// The repeated expenses, the next one due first. One naming someone who left is `paused`.
fn read_recurring(doc: &LoroDoc, participants: &[Participant]) -> Vec<Repeated> {
    let member = |id: &str| participants.iter().any(|p| p.id == id && !p.removed);
    let mut repeated: Vec<Repeated> = entries::<DocRecurring>(&doc.get_map(RECURRING), RECURRING)
        .into_iter()
        .filter_map(|(id, r)| {
            let income = r.amount_cents < 0;
            let amount_cents = r.amount_cents.saturating_abs();
            let splits = read_splits(&r.splits, false);
            check_amounts(amount_cents, None, &splits).ok()?;
            let payers = match check_payers(amount_cents, None, &r.paid_by, &r.payers) {
                Ok(()) => r.payers,
                Err(_) => Vec::new(),
            };
            let made = u32::try_from(r.made).ok()?;
            let next = occurrence(r.start, &r.every, made)?;
            let paused = !(member(&r.paid_by)
                && payers.iter().all(|p| member(&p.participant_id))
                && splits.iter().all(|s| member(&s.participant_id)));
            Some(Repeated {
                expense: RecurringExpense {
                    id,
                    title: r.title,
                    category: r.category.filter(|c| check_category(c).is_ok()),
                    amount_cents,
                    income,
                    paid_by: r.paid_by,
                    payers,
                    splits,
                    every: r.every,
                    next,
                    paused,
                    added_by: r.added_by,
                },
                start: r.start,
                made,
            })
        })
        .collect();
    repeated.sort_by(|a, b| {
        a.expense
            .next
            .cmp(&b.expense.next)
            .then_with(|| a.expense.id.cmp(&b.expense.id))
    });
    repeated
}

impl DocExpense {
    /// The several payers, when they still fit the expense: an older app version changes the
    /// amount or `paid_by` without them, and then `paid_by` paid it all.
    fn checked_payers(&self) -> Vec<ExpensePayer> {
        let fits = check_payers(
            self.amount_cents,
            self.original.as_ref(),
            &self.paid_by,
            &self.payers,
        );
        match fits {
            Ok(()) => self.payers.clone(),
            Err(_) => Vec::new(),
        }
    }

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
                        iban: p.iban.and_then(|iban| check_iban(&iban).ok()),
                        added_at: read_time(p.added_at),
                        added_by: p.added_by,
                        removed_at: read_time(p.removed_at),
                        removed_by: p.removed_by,
                    },
                )
            })
            .collect();
    // Concurrent additions can share a position; the id keeps the order stable everywhere.
    participants.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));

    let participants: Vec<Participant> = participants.into_iter().map(|(_, p)| p).collect();

    let mut comments = read_comments(doc);
    let mut commented = |mut expense: Expense| {
        expense.comments = comments.remove(&expense.id).unwrap_or_default();
        expense
    };
    let mut expenses: Vec<Expense> = entries::<DocExpense>(&doc.get_map(EXPENSES), EXPENSES)
        .into_iter()
        .filter_map(|(id, e)| read_expense(id, &meta.id, e))
        .map(&mut commented)
        .collect();
    expenses.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut trash: Vec<DeletedExpense> = entries::<DocExpense>(&doc.get_map(TRASH), TRASH)
        .into_iter()
        .filter_map(|(id, mut e)| {
            let deleted_at = read_time(e.deleted_at.take())?;
            let deleted_by = e.deleted_by.take();
            Some(DeletedExpense {
                expense: read_expense(id, &meta.id, e)?,
                deleted_at,
                deleted_by,
            })
        })
        // Put back on one device while deleted on another: it is back.
        .filter(|d| !expenses.iter().any(|e| e.id == d.expense.id))
        .map(|d| DeletedExpense {
            expense: commented(d.expense),
            ..d
        })
        .collect();
    trash.sort_by(|a, b| {
        b.deleted_at
            .cmp(&a.deleted_at)
            .then_with(|| a.expense.id.cmp(&b.expense.id))
    });

    // Votes of people who are still members, in the group's order.
    let voted: HashSet<String> = entries::<bool>(&doc.get_map(DELETION), "deletion vote")
        .into_iter()
        .filter_map(|(id, agreed)| agreed.then_some(id))
        .collect();
    let deletion_votes = participants
        .iter()
        .filter(|p| !p.removed && voted.contains(&p.id))
        .map(|p| p.id.clone())
        .collect();
    let recurring = read_recurring(doc, &participants)
        .into_iter()
        .map(|r| r.expense)
        .collect();

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
        participants,
        expenses,
        created_at: meta.created_at,
        deleted: meta.deleted,
        deletion_votes,
        trash,
        recurring,
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

/// What an amount is stored as: below zero for money that came in.
fn stored_amount(amount_cents: i64, income: bool) -> i64 {
    if income {
        -amount_cents
    } else {
        amount_cents
    }
}

/// Writes an expense under its id into `parent`: the expenses, or the trash.
fn write_expense(parent: &LoroMap, expense: &Expense) -> Res<LoroMap> {
    let e = parent
        .insert_container(&expense.id, LoroMap::new())
        .map_err(doc_err)?;
    e.insert("title", expense.title.as_str()).map_err(doc_err)?;
    if let Some(category) = &expense.category {
        e.insert("category", category.as_str()).map_err(doc_err)?;
    }
    e.insert(
        "amount_cents",
        stored_amount(expense.amount_cents, expense.income),
    )
    .map_err(doc_err)?;
    e.insert("paid_by", expense.paid_by.as_str())
        .map_err(doc_err)?;
    if !expense.payers.is_empty() {
        e.insert("payers", payers_value(&expense.payers))
            .map_err(doc_err)?;
    }
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
    if let Some(added_at) = expense.added_at {
        e.insert("added_at", timestamp(added_at)).map_err(doc_err)?;
    }
    if let Some(added_by) = &expense.added_by {
        e.insert("added_by", added_by.as_str()).map_err(doc_err)?;
    }
    if let Some(recurring) = &expense.recurring {
        e.insert("recurring", recurring.as_str()).map_err(doc_err)?;
    }
    if !expense.items.is_empty() {
        e.insert(ITEMS, items_value(&expense.items))
            .map_err(doc_err)?;
    }
    let history = e
        .insert_container(HISTORY, LoroList::new())
        .map_err(doc_err)?;
    for entry in &expense.history {
        history.push(history_value(entry)).map_err(doc_err)?;
    }
    Ok(e)
}

fn insert_expense(doc: &LoroDoc, expense: &Expense) -> Res<()> {
    write_expense(&doc.get_map(EXPENSES), expense).map(|_| ())
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

/// An IBAN as stored: without spaces, upper-cased, with check digits that fit it.
pub fn check_iban(iban: &str) -> Res<String> {
    let iban = iban
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase();
    let bytes = iban.as_bytes();
    let shaped = (15..=34).contains(&bytes.len())
        && bytes.iter().all(u8::is_ascii_alphanumeric)
        && bytes[..2].iter().all(u8::is_ascii_uppercase)
        && bytes[2..4].iter().all(u8::is_ascii_digit);
    // The country and check digits go last, letters count as 10 to 35, and what that number
    // leaves when divided by 97 is 1.
    let valid = shaped
        && bytes[4..].iter().chain(&bytes[..4]).fold(0u32, |rest, b| {
            if b.is_ascii_digit() {
                (rest * 10 + u32::from(b - b'0')) % 97
            } else {
                (rest * 100 + u32::from(b - b'A') + 10) % 97
            }
        }) == 1;
    if valid {
        Ok(iban)
    } else {
        Err("This IBAN is not valid".to_string())
    }
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
    pub category: Option<String>,
    pub amount_cents: i64,
    pub paid_by: usize,
    // (person, amount) for each of several payers, as in `ExpensePayer`
    pub payers: Vec<(usize, i64)>,
    pub original: Option<OriginalAmount>,
    // (person, shares, fixed amount), as in `ExpenseSplit`
    pub splits: Vec<(usize, u32, Option<i64>)>,
    pub created_at: DateTime<Utc>,
    pub is_reimbursement: bool,
    pub income: bool,
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
        let payers = e
            .payers
            .iter()
            .map(|(position, amount_cents)| {
                Ok(ExpensePayer {
                    participant_id: id(*position)?,
                    amount_cents: *amount_cents,
                })
            })
            .collect::<Res<Vec<_>>>()?;
        let PaidBy { paid_by, payers } = PaidBy::new(id(e.paid_by)?, payers);
        let original = normal_original(&group, e.original.clone())?;
        validate_expense(
            &group,
            e.amount_cents,
            original.as_ref(),
            &paid_by,
            &payers,
            &splits,
            &HashSet::new(),
        )?;
        insert_expense(
            &doc,
            &Expense {
                id: Uuid::new_v4().to_string(),
                group_id: group.id.clone(),
                title: e.title.trim().to_string(),
                category: e.category.clone().filter(|c| check_category(c).is_ok()),
                amount_cents: e.amount_cents,
                original,
                paid_by,
                payers,
                splits,
                created_at: e.created_at,
                updated_at: now,
                history: Vec::new(),
                is_reimbursement: e.is_reimbursement,
                income: e.income,
                added_at: None,
                added_by: None,
                recurring: None,
                items: Vec::new(),
                comments: Vec::new(),
            },
        )?;
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

/// Deletes the group for everyone when nobody owes anything. Otherwise it takes every
/// member's agreement: this records `me`'s, and deletes once all the members have agreed.
/// Returns whether the group is now deleted.
pub fn delete_or_vote(doc: &LoroDoc, me: Option<&str>) -> Res<bool> {
    let group = read_group(doc)?;
    let settled = engine::calculate_balances(&group)
        .iter()
        .all(|b| b.net_cents == 0);
    let members = || group.participants.iter().filter(|p| !p.removed);
    if !settled {
        let me = me
            .filter(|me| members().any(|p| p.id == *me))
            .ok_or_else(|| {
                "Say who you are in this group before asking to delete it".to_string()
            })?;
        doc.get_map(DELETION).insert(me, true).map_err(doc_err)?;
        let agreed = |id: &str| id == me || group.deletion_votes.iter().any(|voted| voted == id);
        if !members().all(|p| agreed(&p.id)) {
            return Ok(false);
        }
    }
    doc.get_map(META).insert("deleted", true).map_err(doc_err)?;
    Ok(true)
}

/// Drops the request to delete the group: someone refused, or changed their mind.
pub fn refuse_deletion(doc: &LoroDoc) -> Res<()> {
    let votes = doc.get_map(DELETION);
    let mut voters = Vec::new();
    votes.for_each(|id, _| voters.push(id.to_string()));
    for id in voters {
        votes.delete(&id).map_err(doc_err)?;
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

/// Sets or removes the bank account a participant is paid back on.
pub fn set_participant_iban(doc: &LoroDoc, participant_id: &str, iban: Option<&str>) -> Res<()> {
    let participant = child_map(&doc.get_map(PARTICIPANTS), participant_id)
        .ok_or_else(|| "Participant not found".to_string())?;
    let current = match participant.get("iban") {
        Some(ValueOrContainer::Value(LoroValue::String(current))) => Some(current.to_string()),
        _ => None,
    };
    let iban = iban.map(check_iban).transpose()?;
    if current == iban {
        return Ok(());
    }
    match iban {
        Some(iban) => participant.insert("iban", iban).map_err(doc_err),
        None => participant.delete("iban").map_err(doc_err),
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

/// Who adds someone to a group.
pub enum AddedBy<'a> {
    /// A member, when the app knows who the user is in the group.
    Member(Option<&'a str>),
    /// The person themselves, joining.
    Themselves,
}

/// Adds a participant, noting when and by whom, and returns their id.
pub fn add_participant(doc: &LoroDoc, name: &str, by: AddedBy) -> Res<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Participant name cannot be empty".to_string());
    }
    let id = Uuid::new_v4().to_string();
    insert_participant(doc, &id, trimmed, false)?;
    let added = child_map(&doc.get_map(PARTICIPANTS), &id)
        .ok_or_else(|| "Participant not found".to_string())?;
    added
        .insert("added_at", timestamp(Utc::now()))
        .map_err(doc_err)?;
    let by = match by {
        AddedBy::Member(member) => member,
        AddedBy::Themselves => Some(id.as_str()),
    };
    if let Some(by) = by {
        added.insert("added_by", by).map_err(doc_err)?;
    }
    Ok(id)
}

/// Soft-deletes a participant, noting when and by which member (`by`, when the app knows who
/// the user is). Their past expenses and balance stay intact.
pub fn remove_participant(doc: &LoroDoc, participant_id: &str, by: Option<&str>) -> Res<()> {
    let p = child_map(&doc.get_map(PARTICIPANTS), participant_id)
        .ok_or_else(|| "Participant not found".to_string())?;
    p.insert("removed", true).map_err(doc_err)?;
    p.insert("removed_at", timestamp(Utc::now()))
        .map_err(doc_err)?;
    match by {
        Some(by) => p.insert("removed_by", by).map_err(doc_err),
        None => p.delete("removed_by").map_err(doc_err),
    }
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

/// Longest category key.
const MAX_CATEGORY_LEN: usize = 30;

/// A category is a short key ("food", "transport"): lower-case letters, digits, `-` and `_`.
/// The interface names the ones it knows, so that each language has its own words.
fn check_category(category: &str) -> Res<()> {
    let valid = !category.is_empty()
        && category.len() <= MAX_CATEGORY_LEN
        && category
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'));
    if valid {
        Ok(())
    } else {
        Err("This category can't be used".to_string())
    }
}

/// What an expense is: its title, and the category it counts under.
pub struct Label {
    title: String,
    category: Option<String>,
}

impl Label {
    /// An empty category is none.
    pub fn new(title: &str, category: Option<&str>) -> Self {
        Self {
            title: title.to_string(),
            category: category
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .map(str::to_string),
        }
    }
}

impl From<&str> for Label {
    fn from(title: &str) -> Self {
        Self::new(title, None)
    }
}

/// Who paid an expense: one person, or several with what each paid, in the currency paid.
pub struct PaidBy {
    paid_by: String,
    payers: Vec<ExpensePayer>,
}

impl PaidBy {
    /// Without `payers`, `paid_by` paid it all; so did a lone payer.
    pub fn new(paid_by: String, mut payers: Vec<ExpensePayer>) -> Self {
        if payers.len() < 2 {
            let paid_by = payers.pop().map_or(paid_by, |p| p.participant_id);
            return Self {
                paid_by,
                payers: Vec::new(),
            };
        }
        // Who paid the most comes first and is `paid_by`, the only payer older app versions
        // know.
        payers.sort_by_key(|p| std::cmp::Reverse(p.amount_cents));
        Self {
            paid_by: payers[0].participant_id.clone(),
            payers,
        }
    }
}

impl From<String> for PaidBy {
    fn from(paid_by: String) -> Self {
        Self::new(paid_by, Vec::new())
    }
}

/// Several payers are at least two different people, `paid_by` first, whose amounts add up to
/// what was paid (`original`'s amount for an expense in another currency).
fn check_payers(
    amount_cents: i64,
    original: Option<&OriginalAmount>,
    paid_by: &str,
    payers: &[ExpensePayer],
) -> Res<()> {
    let Some(first) = payers.first() else {
        return Ok(());
    };
    if payers.len() < 2 || first.participant_id != paid_by {
        return Err("The payers of this expense are not valid".to_string());
    }
    let mut seen = HashSet::new();
    if !payers
        .iter()
        .all(|p| seen.insert(p.participant_id.as_str()))
    {
        return Err("A participant appears twice among the payers".to_string());
    }
    if !payers
        .iter()
        .all(|p| p.amount_cents > 0 && p.amount_cents <= MAX_AMOUNT_CENTS)
    {
        return Err("What each payer paid must be above zero".to_string());
    }
    let total: i128 = payers.iter().map(|p| i128::from(p.amount_cents)).sum();
    let paid = i128::from(original.map_or(amount_cents, |o| o.amount_cents));
    if total != paid {
        return Err(format!(
            "The payers paid {} between them, not the expense's {}",
            money(total),
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
    payers: &[ExpensePayer],
    splits: &[ExpenseSplit],
    grandfathered: &HashSet<&str>,
) -> Res<()> {
    check_amounts(amount_cents, original, splits)?;
    check_payers(amount_cents, original, paid_by, payers)?;
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
    if !usable(paid_by) || !payers.iter().all(|p| usable(&p.participant_id)) {
        return Err("The payer is not an active member of this group".to_string());
    }
    if !splits.iter().all(|s| usable(&s.participant_id)) {
        return Err(
            "The split includes someone who is not an active member of this group".to_string(),
        );
    }
    Ok(())
}

/// The lines of an expense as stored: names trimmed, on people who can be on the expense
/// (see `validate_expense` for `grandfathered`), adding up to what was paid.
fn normal_items(
    group: &Group,
    amount_cents: i64,
    original: Option<&OriginalAmount>,
    items: Vec<ExpenseItem>,
    grandfathered: &HashSet<&str>,
) -> Res<Vec<ExpenseItem>> {
    if items.is_empty() {
        return Ok(items);
    }
    let items: Vec<ExpenseItem> = items
        .into_iter()
        .map(|item| ExpenseItem {
            name: item.name.trim().to_string(),
            ..item
        })
        .collect();
    check_items(original.map_or(amount_cents, |o| o.amount_cents), &items)?;
    let usable = |id: &str| {
        group
            .participants
            .iter()
            .any(|p| p.id == id && (!p.removed || grandfathered.contains(id)))
    };
    if !items
        .iter()
        .all(|item| item.participants.iter().all(|p| usable(p)))
    {
        return Err(
            "The split includes someone who is not an active member of this group".to_string(),
        );
    }
    Ok(items)
}

/// What else there is to say about an expense being added.
#[derive(Default)]
pub struct Adding<'a> {
    /// The expense line by line: the splits are then worked out from these.
    pub items: Vec<ExpenseItem>,
    /// Money that came in rather than out.
    pub income: bool,
    /// The member adding it, when the app knows who the user is.
    pub by: Option<&'a str>,
    /// "week", "month" or "year": it comes back each time, counting from the day it is paid.
    pub repeat: Option<&'a str>,
}

pub fn add_expense(
    doc: &LoroDoc,
    title: impl Into<Label>,
    amount_cents: i64,
    paid_by: impl Into<PaidBy>,
    splits: Vec<ExpenseSplit>,
    created_at: Option<DateTime<Utc>>,
    original: Option<OriginalAmount>,
) -> Res<()> {
    add_expense_as(
        doc,
        title,
        amount_cents,
        paid_by,
        splits,
        created_at,
        original,
        Adding::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn add_expense_as(
    doc: &LoroDoc,
    title: impl Into<Label>,
    amount_cents: i64,
    paid_by: impl Into<PaidBy>,
    splits: Vec<ExpenseSplit>,
    created_at: Option<DateTime<Utc>>,
    original: Option<OriginalAmount>,
    adding: Adding,
) -> Res<()> {
    let PaidBy { paid_by, payers } = paid_by.into();
    let Label { title, category } = title.into();
    if let Some(category) = &category {
        check_category(category)?;
    }
    let group = read_group(doc)?;
    let original = normal_original(&group, original)?;
    let items = normal_items(
        &group,
        amount_cents,
        original.as_ref(),
        adding.items,
        &HashSet::new(),
    )?;
    let splits = if items.is_empty() {
        splits
    } else {
        items_splits(&items)
    };
    validate_expense(
        &group,
        amount_cents,
        original.as_ref(),
        &paid_by,
        &payers,
        &splits,
        &HashSet::new(),
    )?;
    let now = Utc::now();
    let created_at = created_at.unwrap_or(now);
    let repeat = adding.repeat.filter(|every| !every.is_empty());
    if let Some(every) = repeat {
        if occurrence(created_at, every, 1).is_none() {
            return Err("An expense repeats every week, month or year".to_string());
        }
        // Its rate would be the first day's for good.
        if original.is_some() {
            return Err("A repeated expense has to be in the group's currency".to_string());
        }
    }
    let recurring = repeat.map(|_| Uuid::new_v4().to_string());
    let expense = Expense {
        id: Uuid::new_v4().to_string(),
        group_id: group.id,
        title: title.trim().to_string(),
        category,
        amount_cents,
        original,
        paid_by,
        payers,
        splits,
        created_at,
        updated_at: now,
        history: Vec::new(),
        is_reimbursement: false,
        income: adding.income,
        added_at: Some(now),
        added_by: adding.by.map(str::to_string),
        recurring: recurring.clone(),
        items,
        comments: Vec::new(),
    };
    insert_expense(doc, &expense)?;

    let (Some(id), Some(every)) = (recurring, repeat) else {
        return Ok(());
    };
    let model = doc
        .get_map(RECURRING)
        .insert_container(&id, LoroMap::new())
        .map_err(doc_err)?;
    model
        .insert("title", expense.title.as_str())
        .map_err(doc_err)?;
    if let Some(category) = &expense.category {
        model
            .insert("category", category.as_str())
            .map_err(doc_err)?;
    }
    model
        .insert(
            "amount_cents",
            stored_amount(expense.amount_cents, expense.income),
        )
        .map_err(doc_err)?;
    model
        .insert("paid_by", expense.paid_by.as_str())
        .map_err(doc_err)?;
    if !expense.payers.is_empty() {
        model
            .insert("payers", payers_value(&expense.payers))
            .map_err(doc_err)?;
    }
    model
        .insert(
            "splits",
            splits_value(expense.amount_cents, None, &expense.splits),
        )
        .map_err(doc_err)?;
    model.insert("every", every).map_err(doc_err)?;
    model
        .insert("start", timestamp(created_at))
        .map_err(doc_err)?;
    // The one just added is the first.
    model.insert("made", 1).map_err(doc_err)?;
    if let Some(by) = &expense.added_by {
        model.insert("added_by", by.as_str()).map_err(doc_err)?;
    }
    // Started in the past, some are due already.
    add_due_expenses(doc, now).map(|_| ())
}

/// Whether a repeated expense has an occurrence to add.
pub fn has_due_expenses(doc: &LoroDoc, now: DateTime<Utc>) -> bool {
    let Ok(group) = read_group(doc) else {
        return false;
    };
    !group.deleted && group.recurring.iter().any(|r| !r.paused && r.next <= now)
}

/// Adds the occurrences of the repeated expenses whose day has come, and says whether there
/// were any. Each has an id made of its model's and its day, so that devices doing this at
/// the same time add the same expense.
pub fn add_due_expenses(doc: &LoroDoc, now: DateTime<Utc>) -> Res<bool> {
    let group = read_group(doc)?;
    if group.deleted {
        return Ok(false);
    }
    // One deleted since isn't added again.
    let mut taken: HashSet<String> = group
        .expenses
        .iter()
        .chain(group.trash.iter().map(|d| &d.expense))
        .map(|e| e.id.clone())
        .collect();
    let models = doc.get_map(RECURRING);
    let mut added = false;
    for repeated in read_recurring(doc, &group.participants) {
        let model = &repeated.expense;
        if model.paused {
            continue;
        }
        let mut made = repeated.made;
        let mut next = model.next;
        while next <= now && (made - repeated.made) as usize <= MAX_DUE {
            let id = format!("{}-{}", model.id, next.format("%Y%m%d"));
            if taken.insert(id.clone()) {
                insert_expense(
                    doc,
                    &Expense {
                        id,
                        group_id: group.id.clone(),
                        title: model.title.clone(),
                        category: model.category.clone(),
                        amount_cents: model.amount_cents,
                        original: None,
                        paid_by: model.paid_by.clone(),
                        payers: model.payers.clone(),
                        splits: model.splits.clone(),
                        created_at: next,
                        updated_at: next,
                        history: Vec::new(),
                        is_reimbursement: false,
                        income: model.income,
                        added_at: Some(next),
                        added_by: model.added_by.clone(),
                        recurring: Some(model.id.clone()),
                        items: Vec::new(),
                        comments: Vec::new(),
                    },
                )?;
            }
            made += 1;
            let Some(following) = occurrence(repeated.start, &model.every, made) else {
                break;
            };
            next = following;
        }
        if made != repeated.made {
            if let Some(target) = child_map(&models, &model.id) {
                target.insert("made", i64::from(made)).map_err(doc_err)?;
                added = true;
            }
        }
    }
    Ok(added)
}

/// Stops a repeated expense: the ones already added stay.
pub fn stop_recurring(doc: &LoroDoc, recurring_id: &str) -> Res<()> {
    let models = doc.get_map(RECURRING);
    if child_map(&models, recurring_id).is_none() {
        return Err("This repeated expense no longer exists".to_string());
    }
    models.delete(recurring_id).map_err(doc_err)
}

/// What else there is to say about an expense being replaced.
#[derive(Default)]
pub struct Editing<'a> {
    /// The member editing it, when the app knows who the user is.
    pub by: Option<&'a str>,
    /// The expense line by line: the splits are then worked out from these. Without, any
    /// lines it had are dropped.
    pub items: Vec<ExpenseItem>,
}

#[allow(clippy::too_many_arguments)]
pub fn update_expense(
    doc: &LoroDoc,
    expense_id: &str,
    title: impl Into<Label>,
    amount_cents: i64,
    paid_by: impl Into<PaidBy>,
    splits: Vec<ExpenseSplit>,
    created_at: Option<DateTime<Utc>>,
    original: Option<OriginalAmount>,
    by: Option<&str>,
) -> Res<()> {
    update_expense_as(
        doc,
        expense_id,
        title,
        amount_cents,
        paid_by,
        splits,
        created_at,
        original,
        Editing {
            by,
            items: Vec::new(),
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn update_expense_as(
    doc: &LoroDoc,
    expense_id: &str,
    title: impl Into<Label>,
    amount_cents: i64,
    paid_by: impl Into<PaidBy>,
    splits: Vec<ExpenseSplit>,
    created_at: Option<DateTime<Utc>>,
    original: Option<OriginalAmount>,
    editing: Editing,
) -> Res<()> {
    let by = editing.by;
    let PaidBy { paid_by, payers } = paid_by.into();
    let Label { title, category } = title.into();
    if let Some(category) = &category {
        check_category(category)?;
    }
    let group = read_group(doc)?;
    let expense = group
        .expenses
        .iter()
        .find(|e| e.id == expense_id)
        .ok_or_else(|| "Expense not found".to_string())?;
    let grandfathered: HashSet<&str> = std::iter::once(expense.paid_by.as_str())
        .chain(expense.payers.iter().map(|p| p.participant_id.as_str()))
        .chain(expense.splits.iter().map(|s| s.participant_id.as_str()))
        .chain(
            expense
                .items
                .iter()
                .flat_map(|item| item.participants.iter().map(String::as_str)),
        )
        .collect();
    let original = normal_original(&group, original)?;
    let items = normal_items(
        &group,
        amount_cents,
        original.as_ref(),
        editing.items,
        &grandfathered,
    )?;
    let splits = if items.is_empty() {
        splits
    } else {
        items_splits(&items)
    };
    validate_expense(
        &group,
        amount_cents,
        original.as_ref(),
        &paid_by,
        &payers,
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
    if expense.category != category {
        let named = |category: &Option<String>| category.clone().unwrap_or_else(|| "none".into());
        changes.push(format!(
            "Category changed from {} to {}",
            named(&expense.category),
            named(&category)
        ));
        match &category {
            Some(category) => target.insert("category", category.as_str()),
            None => target.delete("category"),
        }
        .map_err(doc_err)?;
    }
    if expense.amount_cents != amount_cents {
        changes.push(format!(
            "Amount changed from {:.2} to {:.2}",
            expense.amount_cents as f64 / 100.0,
            amount_cents as f64 / 100.0
        ));
        target
            .insert("amount_cents", stored_amount(amount_cents, expense.income))
            .map_err(doc_err)?;
    }
    if expense.paid_by != paid_by || expense.payers != payers {
        // One payer by name, several with what each paid.
        let describe = |paid_by: &str, payers: &[ExpensePayer]| {
            if payers.is_empty() {
                return name_of(paid_by).to_string();
            }
            payers
                .iter()
                .map(|p| {
                    format!(
                        "{} ({})",
                        name_of(&p.participant_id),
                        money(i128::from(p.amount_cents))
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        changes.push(format!(
            "Payer changed from {} to {}",
            describe(&expense.paid_by, &expense.payers),
            describe(&paid_by, &payers)
        ));
        if expense.paid_by != paid_by {
            target
                .insert("paid_by", paid_by.as_str())
                .map_err(doc_err)?;
        }
        if expense.payers != payers {
            if payers.is_empty() {
                target.delete("payers")
            } else {
                target.insert("payers", payers_value(&payers))
            }
            .map_err(doc_err)?;
        }
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
    if expense.items != items {
        changes.push("Items updated".to_string());
        if items.is_empty() {
            target.delete(ITEMS)
        } else {
            target.insert(ITEMS, items_value(&items))
        }
        .map_err(doc_err)?;
    } else if expense.splits != splits {
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
        previous_payers: expense.payers.clone(),
        previous_splits: expense.splits.clone(),
        previous_original: expense.original.clone(),
        previous_category: expense.category.clone(),
        summary,
        edited_by: by.map(str::to_string),
    };
    child_list(&target, HISTORY)?
        .push(history_value(&entry))
        .map_err(doc_err)?;
    target
        .insert("updated_at", timestamp(now))
        .map_err(doc_err)?;
    Ok(())
}

/// Deletes an expense: it goes to the trash, with when and by which member (`by`, when the
/// app knows who the user is), from where it can be put back.
pub fn delete_expense(doc: &LoroDoc, expense_id: &str, by: Option<&str>) -> Res<()> {
    let expenses = doc.get_map(EXPENSES);
    if child_map(&expenses, expense_id).is_none() {
        return Err("Expense not found".to_string());
    }
    // One this version can't read is only removed.
    let group = read_group(doc)?;
    if let Some(expense) = group.expenses.iter().find(|e| e.id == expense_id) {
        let kept = write_expense(&doc.get_map(TRASH), expense)?;
        kept.insert("deleted_at", timestamp(Utc::now()))
            .map_err(doc_err)?;
        if let Some(by) = by {
            kept.insert("deleted_by", by).map_err(doc_err)?;
        }
    }
    expenses.delete(expense_id).map_err(doc_err)
}

/// Puts a deleted expense back, as it was, and notes it in its history.
pub fn restore_expense(doc: &LoroDoc, expense_id: &str, by: Option<&str>) -> Res<()> {
    let group = read_group(doc)?;
    let deleted = group
        .trash
        .iter()
        .find(|d| d.expense.id == expense_id)
        .ok_or_else(|| "This expense is no longer in the trash".to_string())?;
    let now = Utc::now();
    let mut expense = deleted.expense.clone();
    expense.history.push(ExpenseHistoryEntry {
        edited_at: now,
        previous_title: expense.title.clone(),
        previous_amount_cents: expense.amount_cents,
        previous_paid_by: expense.paid_by.clone(),
        previous_payers: expense.payers.clone(),
        previous_splits: expense.splits.clone(),
        previous_original: expense.original.clone(),
        previous_category: expense.category.clone(),
        summary: "Restored from the trash".to_string(),
        edited_by: by.map(str::to_string),
    });
    expense.updated_at = now;
    insert_expense(doc, &expense)?;
    doc.get_map(TRASH).delete(expense_id).map_err(doc_err)
}

/// Removes a deleted expense for good, with its comments.
pub fn purge_expense(doc: &LoroDoc, expense_id: &str) -> Res<()> {
    let trash = doc.get_map(TRASH);
    if child_map(&trash, expense_id).is_none() {
        return Err("This expense is no longer in the trash".to_string());
    }
    let comments = doc.get_map(COMMENTS);
    for (id, comment) in entries::<DocComment>(&comments, COMMENTS) {
        if comment.expense == expense_id {
            comments.delete(&id).map_err(doc_err)?;
        }
    }
    trash.delete(expense_id).map_err(doc_err)
}

/// The expenses deleted more than `TRASH_DAYS` ago.
fn old_trash(group: &Group, now: DateTime<Utc>) -> impl Iterator<Item = &DeletedExpense> {
    group.trash.iter().filter(move |deleted| {
        deleted
            .deleted_at
            .checked_add_days(Days::new(TRASH_DAYS))
            .is_some_and(|until| until <= now)
    })
}

/// Whether the trash holds expenses to remove for good.
pub fn has_old_trash(doc: &LoroDoc, now: DateTime<Utc>) -> bool {
    read_group(doc).is_ok_and(|group| old_trash(&group, now).next().is_some())
}

/// Removes for good the expenses that were deleted more than `TRASH_DAYS` ago.
pub fn empty_old_trash(doc: &LoroDoc, now: DateTime<Utc>) -> Res<()> {
    let group = read_group(doc)?;
    for deleted in old_trash(&group, now) {
        purge_expense(doc, &deleted.expense.id)?;
    }
    Ok(())
}

/// Adds a comment under an expense, from `by` when the app knows who the user is.
pub fn add_comment(doc: &LoroDoc, expense_id: &str, text: &str, by: Option<&str>) -> Res<()> {
    let text = text.trim();
    if text.is_empty() {
        return Err("A comment can't be empty".to_string());
    }
    if text.chars().count() > MAX_COMMENT_CHARS {
        return Err(format!(
            "This comment is too long ({MAX_COMMENT_CHARS} characters at most)"
        ));
    }
    if child_map(&doc.get_map(EXPENSES), expense_id).is_none() {
        return Err("Expense not found".to_string());
    }
    let mut comment = HashMap::from([
        ("expense".to_string(), LoroValue::from(expense_id)),
        ("text".to_string(), text.into()),
        ("at".to_string(), timestamp(Utc::now()).into()),
    ]);
    if let Some(by) = by {
        comment.insert("by".to_string(), by.into());
    }
    doc.get_map(COMMENTS)
        .insert(&Uuid::new_v4().to_string(), LoroValue::Map(comment.into()))
        .map_err(doc_err)
}

/// Removes a comment.
pub fn delete_comment(doc: &LoroDoc, comment_id: &str) -> Res<()> {
    let comments = doc.get_map(COMMENTS);
    if comments.get(comment_id).is_none() {
        return Err("This comment no longer exists".to_string());
    }
    comments.delete(comment_id).map_err(doc_err)
}

/// Records a payment between two people. Removed participants are allowed here, since
/// settling up with someone who left the group is exactly when this is needed.
pub fn record_reimbursement(
    doc: &LoroDoc,
    from_id: String,
    to_id: String,
    amount_cents: i64,
    notes: Option<String>,
    by: Option<&str>,
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
            category: None,
            amount_cents,
            paid_by: from_id,
            payers: Vec::new(),
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
            income: false,
            added_at: Some(now),
            added_by: by.map(str::to_string),
            recurring: None,
            items: Vec::new(),
            comments: Vec::new(),
        },
    )
}

#[cfg(test)]
mod tests;
