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
        delete_expense(&doc, &e.id, None).unwrap();

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
            "ghost".to_string(),
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
        remove_participant(&doc, bob, None).unwrap();
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
            None,
        )
        .unwrap();

        // And a removed participant can still settle up.
        record_reimbursement(&doc, bob.clone(), alice.clone(), 100, None, None).unwrap();
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
        add_participant(&b, "Charlie", AddedBy::Member(None)).unwrap();
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
    fn deleting_takes_settled_balances_or_everyone() {
        // Nobody owes anything: anyone deletes, without saying who they are.
        let (doc, _) = sample();
        assert!(delete_or_vote(&doc, None).unwrap());
        assert!(read_group(&doc).unwrap().deleted);

        let (doc, group) = sample();
        let ids: Vec<String> = group.participants.iter().map(|p| p.id.clone()).collect();
        let (alice, bob) = (&ids[0], &ids[1]);
        add_expense(
            &doc,
            "Dinner",
            9000,
            alice.clone(),
            vec![split(alice, 1), split(bob, 1)],
            None,
            None,
        )
        .unwrap();
        assert!(delete_or_vote(&doc, None).is_err(), "who asks?");
        assert!(delete_or_vote(&doc, Some("ghost")).is_err());

        // Everyone agrees one after the other; the last one deletes.
        for (i, id) in ids.iter().enumerate() {
            let last = i == ids.len() - 1;
            assert_eq!(delete_or_vote(&doc, Some(id)).unwrap(), last, "{i}");
            let read = read_group(&doc).unwrap();
            assert_eq!(read.deleted, last);
            assert_eq!(read.deletion_votes, ids[..=i]);
        }

        // A refusal starts over, and a removed member has no say.
        let (doc, group) = sample();
        let ids: Vec<String> = group.participants.iter().map(|p| p.id.clone()).collect();
        add_expense(
            &doc,
            "Dinner",
            9000,
            ids[0].clone(),
            vec![split(&ids[0], 1), split(&ids[1], 1)],
            None,
            None,
        )
        .unwrap();
        assert!(!delete_or_vote(&doc, Some(&ids[0])).unwrap());
        refuse_deletion(&doc).unwrap();
        assert!(read_group(&doc).unwrap().deletion_votes.is_empty());
        for id in &ids[2..] {
            remove_participant(&doc, id, None).unwrap();
        }
        assert!(!delete_or_vote(&doc, Some(&ids[0])).unwrap());
        assert!(delete_or_vote(&doc, Some(&ids[1])).unwrap());
    }

    #[test]
    fn expenses_have_a_category() {
        let (doc, group) = sample();
        let alice = &group.participants[0].id;
        let add = |category: Option<&str>| {
            add_expense(
                &doc,
                Label::new("Dinner", category),
                9000,
                alice.clone(),
                vec![split(alice, 1)],
                None,
                None,
            )
        };
        add(Some("food")).unwrap();
        // No category, and an empty one, are the same.
        add(Some("  ")).unwrap();
        for bad in ["Food", "a b", "<b>", &"a".repeat(31)] {
            assert!(add(Some(bad)).is_err(), "{bad}");
        }
        let mut expenses = read_group(&doc).unwrap().expenses;
        expenses.sort_by_key(|e| e.category.is_none());
        assert_eq!(expenses[0].category.as_deref(), Some("food"));
        assert_eq!(expenses[1].category, None);

        let update = |category: Option<&str>| {
            update_expense(
                &doc,
                &expenses[0].id,
                Label::new("Dinner", category),
                9000,
                alice.clone(),
                vec![split(alice, 1)],
                None,
                None,
                None,
            )
            .unwrap();
            let group = read_group(&doc).unwrap();
            group
                .expenses
                .into_iter()
                .find(|e| e.id == expenses[0].id)
                .unwrap()
        };
        let moved = update(Some("transport"));
        assert_eq!(moved.category.as_deref(), Some("transport"));
        assert_eq!(
            moved.history[0].summary,
            "Category changed from food to transport"
        );
        assert_eq!(moved.history[0].previous_category.as_deref(), Some("food"));
        let cleared = update(None);
        assert_eq!(cleared.category, None);
        assert_eq!(
            cleared.history[1].summary,
            "Category changed from transport to none"
        );

        // A category written by something else than the app is left out.
        let stored = child_map(&doc.get_map(EXPENSES), &expenses[0].id).unwrap();
        stored.insert("category", "Not A Key").unwrap();
        assert_eq!(read_group(&doc).unwrap().expenses[0].category, None);
    }

    #[test]
    fn members_remember_who_added_and_removed_them() {
        let (doc, group) = sample();
        let (alice, bob) = (&group.participants[0].id, &group.participants[1].id);
        // The members the group started with weren't added by anyone.
        assert!(group
            .participants
            .iter()
            .all(|p| p.added_at.is_none() && p.added_by.is_none()));

        let carol = add_participant(&doc, "Carol", AddedBy::Member(Some(alice))).unwrap();
        let dave = add_participant(&doc, "Dave", AddedBy::Themselves).unwrap();
        let eve = add_participant(&doc, "Eve", AddedBy::Member(None)).unwrap();
        remove_participant(&doc, &carol, Some(bob)).unwrap();
        remove_participant(&doc, &eve, None).unwrap();

        let read = read_group(&doc).unwrap();
        let of = |id: &str| {
            read.participants
                .iter()
                .find(|p| p.id == id)
                .unwrap()
                .clone()
        };
        let (carol, dave, eve) = (of(&carol), of(&dave), of(&eve));
        assert!(carol.added_at.is_some() && carol.removed_at.is_some());
        assert_eq!(carol.added_by.as_ref(), Some(alice));
        assert_eq!(carol.removed_by.as_ref(), Some(bob));
        assert_eq!(dave.added_by.as_ref(), Some(&dave.id), "joined");
        assert_eq!((dave.removed_at, dave.removed_by), (None, None));
        assert!(eve.added_at.is_some() && eve.removed_at.is_some());
        assert_eq!((eve.added_by, eve.removed_by), (None, None));

        // A date that isn't one doesn't take the member with it.
        let stored = child_map(&doc.get_map(PARTICIPANTS), &dave.id).unwrap();
        stored.insert("added_at", "yesterday").unwrap();
        let read = read_group(&doc).unwrap();
        let dave = read.participants.iter().find(|p| p.id == dave.id).unwrap();
        assert_eq!(dave.added_at, None);
    }

    #[test]
    fn several_people_pay_one_expense() {
        let (doc, group) = sample();
        let (alice, bob) = (&group.participants[0].id, &group.participants[1].id);
        let payer = |id: &str, amount_cents: i64| ExpensePayer {
            participant_id: id.to_string(),
            amount_cents,
        };
        let everyone = || vec![split(alice, 1), split(bob, 1)];
        let add = |payers: Vec<ExpensePayer>| {
            add_expense(
                &doc,
                "Dinner",
                9000,
                PaidBy::new(alice.clone(), payers),
                everyone(),
                None,
                None,
            )
        };

        // Who paid the most is `paid_by`, whatever the order given.
        add(vec![payer(alice, 3000), payer(bob, 6000)]).unwrap();
        let e = read_group(&doc).unwrap().expenses.remove(0);
        assert_eq!(e.paid_by, *bob);
        assert_eq!(e.payers, vec![payer(bob, 6000), payer(alice, 3000)]);
        assert_eq!(net(&doc, bob), 1500);
        assert_eq!(net(&doc, alice), -1500);

        // A lone payer paid it all.
        add(vec![payer(bob, 1)]).unwrap();
        let lone = read_group(&doc).unwrap().expenses.remove(1);
        assert_eq!(
            (lone.paid_by.as_str(), lone.payers.len()),
            (bob.as_str(), 0)
        );
        delete_expense(&doc, &lone.id, None).unwrap();

        for (bad, why) in [
            (vec![payer(alice, 3000), payer(bob, 5000)], "not the total"),
            (vec![payer(alice, 9000), payer(bob, 0)], "nothing paid"),
            (vec![payer(alice, 4500), payer(alice, 4500)], "twice"),
            (
                vec![payer(alice, 4500), payer("ghost", 4500)],
                "not a member",
            ),
        ] {
            assert!(add(bad).is_err(), "{why}");
        }

        // Editing the payers is recorded with what each paid, and going back to one payer
        // removes them.
        let update = |paid_by: PaidBy, amount_cents: i64| {
            update_expense(
                &doc,
                &e.id,
                "Dinner",
                amount_cents,
                paid_by,
                everyone(),
                None,
                None,
                None,
            )
        };
        update(
            PaidBy::new(alice.clone(), vec![payer(alice, 7000), payer(bob, 2000)]),
            9000,
        )
        .unwrap();
        let edited = read_group(&doc).unwrap().expenses.remove(0);
        assert_eq!(edited.paid_by, *alice);
        assert_eq!(
            edited.history[0].summary,
            "Payer changed from Bob (60.00), Alice (30.00) to Alice (70.00), Bob (20.00)"
        );
        assert_eq!(edited.history[0].previous_payers, e.payers);
        // The amount can't change without the payers' amounts.
        assert!(update(PaidBy::new(alice.clone(), edited.payers.clone()), 8000).is_err());
        update(bob.clone().into(), 8000).unwrap();
        let single = read_group(&doc).unwrap().expenses.remove(0);
        assert_eq!(
            (single.paid_by.as_str(), single.payers.len()),
            (bob.as_str(), 0)
        );
        assert_eq!(net(&doc, bob), 4000);

        // An older app version changes the amount, or who paid, and knows nothing of the
        // payers: they no longer fit, so `paid_by` paid it all.
        add(vec![payer(alice, 3000), payer(bob, 6000)]).unwrap();
        let shared = read_group(&doc).unwrap().expenses.remove(1);
        let stored = child_map(&doc.get_map(EXPENSES), &shared.id).unwrap();
        stored.insert("amount_cents", 5000).unwrap();
        assert!(read_group(&doc).unwrap().expenses[1].payers.is_empty());
        stored.insert("amount_cents", 9000).unwrap();
        assert_eq!(read_group(&doc).unwrap().expenses[1].payers.len(), 2);
        stored.insert("paid_by", alice.as_str()).unwrap();
        let read = read_group(&doc).unwrap().expenses.remove(1);
        assert_eq!(
            (read.paid_by.as_str(), read.payers.len()),
            (alice.as_str(), 0)
        );
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
        record_reimbursement(&doc, alice.clone(), bob.clone(), 500, None, None).unwrap();
        record_reimbursement(
            &doc,
            bob.clone(),
            alice.clone(),
            300,
            Some("cash".into()),
            None,
        )
        .unwrap();
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
        record_reimbursement(&doc, alice.clone(), bob.clone(), 100, None, None).unwrap();
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
                category: None,
                amount_cents,
                paid_by: alice.clone(),
                payers: Vec::new(),
                splits,
                created_at: now,
                updated_at: now,
                history: Vec::new(),
                is_reimbursement: false,
                original: None,
                income: false,
                added_at: None,
                added_by: None,
                recurring: None,
                items: Vec::new(),
                comments: Vec::new(),
            };
            insert_expense(&doc, &expense).unwrap();
        };
        unchecked("Too big", i64::MAX, vec![split(alice, 1)]);
        unchecked("Nothing", 0, vec![split(alice, 1)]);
        unchecked("Too big the other way", i64::MIN, vec![split(alice, 1)]);
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

    #[test]
    fn money_that_came_in_counts_the_other_way() {
        let (doc, g) = sample();
        let (alice, bob) = (&g.participants[0].id, &g.participants[1].id);
        add_expense_as(
            &doc,
            "Deposit back",
            1000,
            alice.clone(),
            vec![split(alice, 1), split(bob, 1)],
            None,
            None,
            Adding {
                income: true,
                by: Some(bob.as_str()),
                ..Adding::default()
            },
        )
        .unwrap();
        let e = read_group(&doc).unwrap().expenses.remove(0);
        assert!(e.income);
        assert_eq!(e.amount_cents, 1000);
        assert_eq!(e.added_by.as_deref(), Some(bob.as_str()));
        assert!(e.added_at.is_some());
        // Alice holds 10.00, of which 5.00 are Bob's.
        assert_eq!((net(&doc, alice), net(&doc, bob)), (-500, 500));
        // An app version from before finds an amount below zero, and leaves the entry out.
        let stored = entries::<DocExpense>(&doc.get_map(EXPENSES), EXPENSES);
        assert_eq!(stored[0].1.amount_cents, -1000);

        // Editing keeps it money that came in, and says who did.
        update_expense(
            &doc,
            &e.id,
            "Deposit back",
            800,
            alice.clone(),
            e.splits.clone(),
            None,
            None,
            Some(alice.as_str()),
        )
        .unwrap();
        let e = read_group(&doc).unwrap().expenses.remove(0);
        assert!(e.income);
        assert_eq!(e.amount_cents, 800);
        assert_eq!(e.history[0].previous_amount_cents, 1000);
        assert_eq!(e.history[0].edited_by.as_deref(), Some(alice.as_str()));
        assert_eq!(net(&doc, bob), 400);
    }

    #[test]
    fn a_deleted_expense_waits_in_the_trash() {
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
        let e = read_group(&doc).unwrap().expenses.remove(0);

        delete_expense(&doc, &e.id, Some(bob.as_str())).unwrap();
        let group = read_group(&doc).unwrap();
        assert!(group.expenses.is_empty());
        assert_eq!(net(&doc, bob), 0);
        assert_eq!(group.trash.len(), 1);
        assert_eq!(group.trash[0].expense, e);
        assert_eq!(group.trash[0].deleted_by.as_deref(), Some(bob.as_str()));
        // For an app version from before, it is deleted.
        assert!(child_map(&doc.get_map(EXPENSES), &e.id).is_none());

        restore_expense(&doc, &e.id, Some(alice.as_str())).unwrap();
        let group = read_group(&doc).unwrap();
        assert!(group.trash.is_empty());
        let back = &group.expenses[0];
        assert_eq!((back.id.as_str(), back.amount_cents), (e.id.as_str(), 3000));
        let noted = back.history.last().unwrap();
        assert_eq!(noted.summary, "Restored from the trash");
        assert_eq!(noted.edited_by.as_deref(), Some(alice.as_str()));
        assert_eq!(net(&doc, bob), -1500);

        delete_expense(&doc, &e.id, None).unwrap();
        purge_expense(&doc, &e.id).unwrap();
        assert!(read_group(&doc).unwrap().trash.is_empty());
        assert!(restore_expense(&doc, &e.id, None).is_err());
        assert!(purge_expense(&doc, &e.id).is_err());
    }

    #[test]
    fn the_trash_empties_itself_after_a_month() {
        let (doc, g) = sample();
        let (alice, bob) = (&g.participants[0].id, &g.participants[1].id);
        for title in ["Dinner", "Taxi"] {
            let splits = vec![split(alice, 1), split(bob, 1)];
            add_expense(&doc, title, 3000, alice.clone(), splits, None, None).unwrap();
        }
        let group = read_group(&doc).unwrap();
        let (dinner, taxi) = (&group.expenses[0].id, &group.expenses[1].id);
        add_comment(&doc, dinner, "Was it this much?", Some(bob.as_str())).unwrap();
        delete_expense(&doc, dinner, None).unwrap();
        delete_expense(&doc, taxi, None).unwrap();
        // The dinner was deleted 31 days ago.
        let long_ago = Utc::now() - chrono::Duration::days(31);
        child_map(&doc.get_map(TRASH), dinner)
            .unwrap()
            .insert("deleted_at", timestamp(long_ago))
            .unwrap();

        let now = Utc::now();
        assert!(has_old_trash(&doc, now));
        empty_old_trash(&doc, now).unwrap();
        assert!(!has_old_trash(&doc, now));
        let trash = read_group(&doc).unwrap().trash;
        assert_eq!(trash.len(), 1);
        assert_eq!(&trash[0].expense.id, taxi);
        // Its comments went with it.
        assert_eq!(doc.get_map(COMMENTS).len(), 0);
    }

    #[test]
    fn an_expense_entered_item_by_item() {
        let (doc, g) = sample();
        let (alice, bob) = (&g.participants[0].id, &g.participants[1].id);
        let item = |name: &str, amount_cents: i64, people: &[&String]| ExpenseItem {
            name: name.to_string(),
            amount_cents,
            participants: people.iter().map(|p| p.to_string()).collect(),
        };
        let add = |amount: i64, items: Vec<ExpenseItem>| {
            add_expense_as(
                &doc,
                "Groceries",
                amount,
                alice.clone(),
                Vec::new(),
                None,
                None,
                Adding {
                    items,
                    ..Adding::default()
                },
            )
        };
        // The lines must add up to the expense, and each be someone's.
        assert_eq!(
            add(3000, vec![item("Wine", 1201, &[bob])]).unwrap_err(),
            "The items add up to 12.01, not the expense's 30.00"
        );
        assert_eq!(
            add(1201, vec![item("Wine", 1201, &[])]).unwrap_err(),
            "Each item needs at least one person"
        );

        // Bob's wine, and bread for both: the odd cent goes to the first on the line.
        let items = vec![
            item(" Wine ", 1200, &[bob]),
            item("Bread", 301, &[alice, bob]),
        ];
        add(1501, items).unwrap();
        let e = read_group(&doc).unwrap().expenses.remove(0);
        assert_eq!(e.items[0].name, "Wine");
        assert_eq!(e.splits, [fixed(bob, 1350), fixed(alice, 151)]);
        assert_eq!(net(&doc, bob), -1350);

        // Editing the lines replaces the splits, and says so.
        update_expense_as(
            &doc,
            &e.id,
            "Groceries",
            1501,
            alice.clone(),
            Vec::new(),
            None,
            None,
            Editing {
                by: None,
                items: vec![item("Wine", 1501, &[alice, bob])],
            },
        )
        .unwrap();
        let e = read_group(&doc).unwrap().expenses.remove(0);
        assert_eq!(e.splits, [fixed(alice, 751), fixed(bob, 750)]);
        assert_eq!(e.history[0].summary, "Items updated");

        // An app version from before changes the split without the lines: they no longer
        // describe the expense, and are left out.
        child_map(&doc.get_map(EXPENSES), &e.id)
            .unwrap()
            .insert(
                "splits",
                splits_value(1501, None, &[split(alice, 1), split(bob, 2)]),
            )
            .unwrap();
        let e = read_group(&doc).unwrap().expenses.remove(0);
        assert!(e.items.is_empty());
        assert_eq!(e.splits, [split(alice, 1), split(bob, 2)]);
    }

    #[test]
    fn comments_under_an_expense() {
        let (doc, g) = sample();
        let (alice, bob) = (&g.participants[0].id, &g.participants[1].id);
        let splits = vec![split(alice, 1), split(bob, 1)];
        add_expense(&doc, "Dinner", 3000, alice.clone(), splits, None, None).unwrap();
        doc.commit();
        let e = read_group(&doc).unwrap().expenses.remove(0);
        assert!(add_comment(&doc, &e.id, "  ", None).is_err());
        assert!(add_comment(&doc, &e.id, &"a".repeat(MAX_COMMENT_CHARS + 1), None).is_err());
        assert!(add_comment(&doc, "nope", "Hello", None).is_err());

        // Two people comment at once, each on their device: both stay.
        let other = fork(&doc);
        add_comment(&doc, &e.id, " Was it this much? ", Some(bob.as_str())).unwrap();
        add_comment(&other, &e.id, "With the tip", Some(alice.as_str())).unwrap();
        doc.commit();
        other.commit();
        merge(&doc, &other);
        let comments = read_group(&doc).unwrap().expenses.remove(0).comments;
        let mut texts: Vec<&str> = comments.iter().map(|c| c.text.as_str()).collect();
        texts.sort_unstable();
        assert_eq!(texts, ["Was it this much?", "With the tip"]);
        let bobs = comments
            .iter()
            .find(|c| c.by.as_ref() == Some(bob))
            .unwrap();

        // They follow the expense to the trash and back.
        delete_expense(&doc, &e.id, None).unwrap();
        assert_eq!(read_group(&doc).unwrap().trash[0].expense.comments.len(), 2);
        restore_expense(&doc, &e.id, None).unwrap();

        delete_comment(&doc, &bobs.id).unwrap();
        assert!(delete_comment(&doc, &bobs.id).is_err());
        let comments = read_group(&doc).unwrap().expenses.remove(0).comments;
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].text, "With the tip");
    }

    #[test]
    fn ibans_are_checked() {
        assert_eq!(
            check_iban(" fr76 3000 6000 0112 3456 7890 189 ").unwrap(),
            "FR7630006000011234567890189"
        );
        assert!(check_iban("GB82WEST12345698765432").is_ok());
        for wrong in [
            "",
            "FR76",
            "GB82WEST12345698765433",
            "1234567890123456",
            "FR76 30é0",
        ] {
            assert!(check_iban(wrong).is_err(), "{wrong}");
        }
        let (doc, g) = sample();
        let alice = &g.participants[0].id;
        set_participant_iban(&doc, alice, Some("de89 3704 0044 0532 0130 00")).unwrap();
        let iban = read_group(&doc).unwrap().participants.remove(0).iban;
        assert_eq!(iban.as_deref(), Some("DE89370400440532013000"));
        set_participant_iban(&doc, alice, None).unwrap();
        assert_eq!(read_group(&doc).unwrap().participants[0].iban, None);
    }

    #[test]
    fn a_repeated_expense_comes_back_when_due() {
        let (doc, g) = sample();
        let (alice, bob) = (&g.participants[0].id, &g.participants[1].id);
        let at = |text: &str| text.parse::<DateTime<Utc>>().unwrap();
        let add = |repeat: &'static str, original| {
            add_expense_as(
                &doc,
                Label::new("Rent", Some("housing")),
                90000,
                alice.clone(),
                vec![split(alice, 1), split(bob, 1)],
                Some(at("2100-01-31T10:00:00Z")),
                original,
                Adding {
                    repeat: Some(repeat),
                    ..Adding::default()
                },
            )
        };
        assert!(add("day", None).unwrap_err().contains("every week"));
        let usd = OriginalAmount {
            currency: "USD".to_string(),
            amount_cents: 100000,
            rate: "0.9".to_string(),
        };
        assert!(add("month", Some(usd)).unwrap_err().contains("currency"));
        assert!(read_group(&doc).unwrap().expenses.is_empty());

        add("month", None).unwrap();
        let group = read_group(&doc).unwrap();
        let model = group.recurring[0].clone();
        assert_eq!(group.expenses[0].recurring.as_ref(), Some(&model.id));
        assert_eq!((model.every.as_str(), model.amount_cents), ("month", 90000));
        // The 31st, or the month's last day.
        assert_eq!(model.next, at("2100-02-28T10:00:00Z"));
        assert!(!has_due_expenses(&doc, at("2100-02-27T10:00:00Z")));
        assert!(has_due_expenses(&doc, at("2100-02-28T10:00:00Z")));

        // Two devices add the ones due: the same expenses, not twice each.
        doc.commit();
        let other = fork(&doc);
        assert!(add_due_expenses(&doc, at("2100-03-31T12:00:00Z")).unwrap());
        assert!(add_due_expenses(&other, at("2100-03-31T12:00:00Z")).unwrap());
        doc.commit();
        other.commit();
        merge(&doc, &other);
        let group = read_group(&doc).unwrap();
        let days: Vec<String> = group
            .expenses
            .iter()
            .map(|e| e.created_at.format("%Y-%m-%d").to_string())
            .collect();
        assert_eq!(days, ["2100-01-31", "2100-02-28", "2100-03-31"]);
        assert!(group.expenses.iter().all(|e| e.title == "Rent"
            && e.category.as_deref() == Some("housing")
            && e.recurring.as_ref() == Some(&model.id)));
        assert_eq!(group.recurring[0].next, at("2100-04-30T10:00:00Z"));
        assert_eq!(net(&doc, bob), -135000);

        // One deleted since isn't added again.
        delete_expense(&doc, &group.expenses[2].id, None).unwrap();
        assert!(!add_due_expenses(&doc, at("2100-03-31T12:00:00Z")).unwrap());
        assert_eq!(read_group(&doc).unwrap().expenses.len(), 2);

        // Bob leaves: nothing more is added for him.
        remove_participant(&doc, bob, None).unwrap();
        assert!(read_group(&doc).unwrap().recurring[0].paused);
        assert!(!has_due_expenses(&doc, at("2101-01-01T00:00:00Z")));
        assert!(!add_due_expenses(&doc, at("2101-01-01T00:00:00Z")).unwrap());

        stop_recurring(&doc, &model.id).unwrap();
        let group = read_group(&doc).unwrap();
        assert!(group.recurring.is_empty());
        assert_eq!(group.expenses.len(), 2);
        assert!(stop_recurring(&doc, &model.id).is_err());
    }
}
