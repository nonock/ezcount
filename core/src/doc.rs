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
//! - `format` (map): format number -> true, for the formats the group needs (see "Formats")
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
//! # Formats
//!
//! Everything above was added without breaking the app versions from before: they skip what
//! they don't know, and what they read still means the same. Do the same whenever it can be
//! done. A change they could not live with (they would misread the group, or damage it by
//! editing it) raises the group's format instead: the version that makes it bumps `FORMAT`
//! and marks the groups it changes that way (`require_format`). A version that only knows an
//! older format then shows such a group as needing an update (`Group::needs_update`) and
//! changes nothing in it (`Store::update` refuses), while still syncing it for the others.
//! Whatever the format, `meta` keeps its `id`, `name`, `currency`, `created_at` and
//! `deleted` as they are: that much is always read. The account's document works the same
//! (`account::FORMAT`).
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

mod check;
mod comments;
mod edit;
mod expenses;
mod format;
mod group;
mod items;
mod participants;
mod read;
mod recurring;
mod trash;
mod values;

pub use self::check::*;
pub use self::comments::*;
pub use self::edit::*;
pub use self::expenses::*;
pub use self::format::*;
pub use self::group::*;
pub use self::items::*;
pub use self::participants::*;
pub use self::read::*;
pub use self::recurring::*;
pub use self::trash::*;
use self::values::*;

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

#[cfg(test)]
mod tests;
