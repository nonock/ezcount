//! Groups as CSV files, to move them in and out of spreadsheets and other apps.
//!
//! One line per expense or payment, after a header line:
//!
//! ```csv
//! Date,Title,Amount,Currency,Paid by,Type,Original amount,Original currency,Exchange rate,Split,Alice,Bob,Carol
//! 2026-08-29T10:00:00Z,Dinner,30.00,EUR,Alice,expense,,,,1 1 1,10.00,10.00,10.00
//! 2026-08-30T10:00:00Z,Taxi,46.17,EUR,Bob,expense,50.00,USD,0.9234,20.00 1 -,18.47,27.70,
//! 2026-08-31T09:00:00Z,,10.00,EUR,Bob,payment,,,,,10.00,,
//! ```
//!
//! `Type` is `expense`, `payment` (from one person to another) or `income`: money that came in,
//! which `Paid by` received and the people share.
//!
//! The columns after `Split` are the people of the group, and hold what each one owes of the
//! line's `Amount`, in the group's currency (for a payment: who received it). They add up to
//! it.
//!
//! `Split` says how the app came to these amounts, with an entry per person in the same
//! order: a number of parts, a fixed amount with its decimals (`20.00`, in the currency the
//! expense was paid in), or `-` for someone who isn't in. It is optional: without it, or when
//! it doesn't give the people's amounts (someone edited them), the split is worked out from
//! the amounts. `Exchange rate` is optional too.
//!
//! A `Category` column may follow `Split`, with the expense's category key (`food`,
//! `transport`…); the app writes it, and reads files without it.
//!
//! `Paid by` names one of the people. An expense several people paid lists them with what each
//! paid, in the currency the expense was paid in: `Alice=30.00 + Bob=20.00`.
//!
//! Reading is lenient about what spreadsheets do to such a file: `;` or tabs as separators,
//! decimal commas, dates without a time or as `31/12/2026`.

use chrono::{DateTime, NaiveDate, NaiveDateTime, SecondsFormat, Utc};
use std::collections::HashSet;

use crate::doc::{check_amounts, payment_title, ImportedExpense};
use crate::engine::{owed, split_amount};
use crate::models::{ExpenseSplit, Group, OriginalAmount};

type Res<T> = Result<T, String>;

mod export;
mod import;
mod parse;

pub use self::export::*;
pub use self::import::*;
use self::parse::*;

const COLUMNS: [&str; 10] = [
    "Date",
    "Title",
    "Amount",
    "Currency",
    "Paid by",
    "Type",
    "Original amount",
    "Original currency",
    "Exchange rate",
    "Split",
];
/// An optional column after the others, before the people.
const CATEGORY: &str = "Category";
const EXPENSE: &str = "expense";
const PAYMENT: &str = "payment";
const INCOME: &str = "income";
const NOT_IN: &str = "-";
/// Between the payers of an expense several people paid.
const SEVERAL_PAYERS: &str = " + ";

/// (person, shares, fixed amount), as in `ImportedExpense`.
type Split = (usize, u32, Option<i64>);

fn cents(amount: i64) -> String {
    format!("{}.{:02}", amount / 100, amount % 100)
}

/// Spreadsheets run cells starting with these as formulas, and a group's texts come from all
/// its members. A leading quote makes them plain text; `unguard` removes it.
const FORMULA_STARTS: [char; 6] = ['=', '+', '-', '@', '\t', '\r'];

fn guard(text: &str) -> String {
    if text.starts_with(FORMULA_STARTS) {
        format!("'{text}")
    } else {
        text.to_string()
    }
}

fn unguard(text: &str) -> &str {
    match text.strip_prefix('\'') {
        Some(rest) if rest.starts_with(FORMULA_STARTS) => rest,
        _ => text,
    }
}

#[cfg(test)]
mod tests;
