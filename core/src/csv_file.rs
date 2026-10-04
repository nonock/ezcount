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

/// A group read from a file. Its name is not in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedGroup {
    pub currency: String,
    pub participants: Vec<String>,
    pub expenses: Vec<ImportedExpense>,
}

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

/// The group as a CSV file.
pub fn export(group: &Group) -> Res<String> {
    // Everyone of the group, then any ID no participant matches (possible after merging edits
    // from several devices), so no money drops out of the file.
    let mut seen = HashSet::new();
    let ids: Vec<&str> = group
        .participants
        .iter()
        .map(|p| p.id.as_str())
        .chain(group.expenses.iter().flat_map(|e| {
            std::iter::once(e.paid_by.as_str())
                .chain(e.payers.iter().map(|p| p.participant_id.as_str()))
                .chain(e.splits.iter().map(|s| s.participant_id.as_str()))
        }))
        .filter(|id| seen.insert(*id))
        .collect();
    // Columns are told apart by name, so namesakes get a number.
    let mut used = HashSet::new();
    let names: Vec<String> = ids
        .iter()
        .map(|id| {
            let name = group
                .participants
                .iter()
                .find(|p| p.id == *id)
                .map_or("Unknown participant", |p| p.name.as_str());
            let mut unique = guard(name);
            let mut n = 2;
            while !used.insert(unique.clone()) {
                unique = format!("{} ({n})", guard(name));
                n += 1;
            }
            unique
        })
        .collect();
    let column = |id: &str| ids.iter().position(|x| *x == id);

    let mut out = csv::Writer::from_writer(Vec::new());
    let write_err = |e: csv::Error| format!("Could not write the CSV file: {e}");
    out.write_record(
        COLUMNS
            .iter()
            .copied()
            .chain(std::iter::once(CATEGORY))
            .chain(names.iter().map(String::as_str)),
    )
    .map_err(write_err)?;
    for e in &group.expenses {
        let original = e.original.as_ref();
        let mut parts = vec![String::new(); ids.len()];
        let mut split = vec![NOT_IN.to_string(); ids.len()];
        let amounts = owed(e.amount_cents, original.map(|o| o.amount_cents), &e.splits);
        for (s, amount) in e.splits.iter().zip(amounts) {
            if let Some(i) = column(&s.participant_id) {
                parts[i] = cents(amount);
                split[i] = s.fixed_cents.map_or_else(|| s.shares.to_string(), cents);
            }
        }
        let fixed = [
            e.created_at.to_rfc3339_opts(SecondsFormat::AutoSi, true),
            guard(&e.title),
            cents(e.amount_cents),
            group.currency.clone(),
            if e.payers.is_empty() {
                column(&e.paid_by)
                    .map(|i| names[i].clone())
                    .unwrap_or_default()
            } else {
                e.payers
                    .iter()
                    .filter_map(|p| {
                        let name = &names[column(&p.participant_id)?];
                        Some(format!("{name}={}", cents(p.amount_cents)))
                    })
                    .collect::<Vec<_>>()
                    .join(SEVERAL_PAYERS)
            },
            if e.is_reimbursement {
                PAYMENT
            } else if e.income {
                INCOME
            } else {
                EXPENSE
            }
            .to_string(),
            original.map(|o| cents(o.amount_cents)).unwrap_or_default(),
            original.map(|o| o.currency.clone()).unwrap_or_default(),
            original.map(|o| o.rate.clone()).unwrap_or_default(),
            // A payment's one column says it all.
            if e.is_reimbursement {
                String::new()
            } else {
                split.join(" ")
            },
        ];
        let category = e.category.clone().unwrap_or_default();
        out.write_record(
            fixed
                .iter()
                .chain(std::iter::once(&category))
                .chain(parts.iter()),
        )
        .map_err(write_err)?;
    }
    let bytes = out
        .into_inner()
        .map_err(|e| format!("Could not write the CSV file: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("Could not write the CSV file: {e}"))
}

/// An amount as a spreadsheet may write it: `12.5`, `12,50`, `1 234,50`, `1,234.50`.
fn parse_cents(text: &str) -> Option<i64> {
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    // The last separator starts the decimals when one or two digits follow it.
    let (whole, decimals) = match text.rfind([',', '.']) {
        Some(i) if (1..=2).contains(&(text.len() - i - 1)) => (&text[..i], &text[i + 1..]),
        _ => (text.as_str(), ""),
    };
    let whole: String = whole.chars().filter(|c| !matches!(c, ',' | '.')).collect();
    if (whole.is_empty() && decimals.is_empty())
        || !whole
            .chars()
            .chain(decimals.chars())
            .all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let whole: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let decimals: i64 = format!("{decimals:0<2}").parse().ok()?;
    whole.checked_mul(100)?.checked_add(decimals)
}

/// Times without a zone are read as UTC; a date alone is noon, the same day in most places.
fn parse_date(text: &str) -> Option<DateTime<Utc>> {
    if let Ok(date) = DateTime::parse_from_rfc3339(text) {
        return Some(date.with_timezone(&Utc));
    }
    for format in [
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%d/%m/%Y %H:%M:%S",
    ] {
        if let Ok(date) = NaiveDateTime::parse_from_str(text, format) {
            return Some(date.and_utc());
        }
    }
    ["%Y-%m-%d", "%d/%m/%Y"]
        .iter()
        .find_map(|format| NaiveDate::parse_from_str(text, format).ok())
        .and_then(|date| date.and_hms_opt(12, 0, 0))
        .map(|date| date.and_utc())
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// The rate that turned `original` into `amount`, to six decimals.
fn rate_between(amount: i64, original: i64) -> String {
    let original = i128::from(original);
    let millionths = (i128::from(amount) * 1_000_000 + original / 2) / original;
    let rate = format!("{}.{:06}", millionths / 1_000_000, millionths % 1_000_000);
    rate.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The small shares that split `amount` into `owed` (all above zero, adding up to it), if
/// there are any: 1/1/1 for 3.34/3.33/3.33, 2/1 for 89.33/44.67.
///
/// Another app may give the odd cent to someone else, so shares that are right to the cent
/// are preferred, then ones a cent off.
fn small_shares(amount: i64, owed: &[i64]) -> Option<Vec<u32>> {
    let smallest = i128::from(*owed.iter().min()?);
    // The smallest part is worth 1 to 10 shares; the others follow, rounded.
    let candidates: Vec<Vec<u32>> = (1..=10)
        .filter_map(|k| {
            owed.iter()
                .map(|o| u32::try_from((i128::from(*o) * k * 2 + smallest) / (smallest * 2)).ok())
                .collect()
        })
        // Nobody types 13 parts: such amounts were typed as amounts.
        .filter(|shares: &Vec<u32>| shares.iter().all(|s| *s <= 10))
        .collect();
    [0, 1].into_iter().find_map(|tolerance| {
        candidates
            .iter()
            .find(|shares| {
                split_amount(amount, shares)
                    .iter()
                    .zip(owed)
                    .all(|(a, b)| (a - b).abs() <= tolerance)
            })
            .cloned()
    })
}

/// The split behind what `people` owe (`owed`, in the group's currency), when the file
/// doesn't say: small shares if some fit, else these amounts as they are.
fn split_from_amounts(
    amount: i64,
    foreign: bool,
    people: &[usize],
    owed: &[i64],
) -> Option<Vec<Split>> {
    if let Some(shares) = small_shares(amount, owed) {
        return Some(
            people
                .iter()
                .zip(shares)
                .map(|(person, shares)| (*person, shares, None))
                .collect(),
        );
    }
    if !foreign {
        return Some(
            people
                .iter()
                .zip(owed)
                .map(|(person, owed)| (*person, 0, Some(*owed)))
                .collect(),
        );
    }
    // Fixed amounts would be in the other currency, which the file doesn't have: shares in
    // the same proportions, then.
    let divisor = owed.iter().fold(0, |a, b| gcd(a, *b));
    people
        .iter()
        .zip(owed)
        .map(|(person, owed)| Some((*person, u32::try_from(owed / divisor).ok()?, None)))
        .collect()
}

/// A `Split` cell: an entry per person.
fn parse_split(text: &str, people: usize) -> Option<Vec<Split>> {
    let entries: Vec<&str> = text.split_whitespace().collect();
    if entries.len() != people {
        return None;
    }
    let mut splits = Vec::new();
    for (person, entry) in entries.into_iter().enumerate() {
        if entry == NOT_IN {
            continue;
        }
        if entry.contains(['.', ',']) {
            splits.push((person, 0, Some(parse_cents(entry).filter(|a| *a > 0)?)));
        } else {
            match entry.parse().ok()? {
                0 => {}
                shares => splits.push((person, shares, None)),
            }
        }
    }
    Some(splits)
}

/// For the checks and sums that don't look at who a split is for.
fn anonymous(splits: &[Split]) -> Vec<ExpenseSplit> {
    splits
        .iter()
        .map(|(_, shares, fixed_cents)| ExpenseSplit {
            participant_id: String::new(),
            shares: *shares,
            fixed_cents: *fixed_cents,
        })
        .collect()
}

/// Reads a group from a CSV file in the format `export` writes.
pub fn import(text: &str) -> Res<ImportedGroup> {
    let text = text.trim_start_matches('\u{feff}');
    // Spreadsheets save with the separator of their language.
    let first_line = text.lines().next().unwrap_or_default();
    let delimiter = b",;\t"
        .iter()
        .copied()
        .max_by_key(|d| first_line.bytes().filter(|b| b == d).count())
        .unwrap_or(b',');
    let mut records = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes())
        .into_records();

    let not_ours = || {
        format!(
            "This is not an ezcount CSV file: its first line should be {}, then one column per person",
            COLUMNS.join(", ")
        )
    };
    let header = records
        .next()
        .ok_or_else(not_ours)?
        .map_err(|_| not_ours())?;
    if header.len() <= COLUMNS.len()
        || !COLUMNS
            .iter()
            .zip(header.iter())
            .all(|(expected, found)| found.trim().eq_ignore_ascii_case(expected))
    {
        return Err(not_ours());
    }
    // Files from before categories, or from elsewhere, have the people right after.
    let has_category = header
        .get(COLUMNS.len())
        .is_some_and(|name| name.trim().eq_ignore_ascii_case(CATEGORY));
    let first_person = COLUMNS.len() + usize::from(has_category);
    if header.len() <= first_person {
        return Err(not_ours());
    }
    let participants: Vec<String> = header
        .iter()
        .skip(first_person)
        .map(|name| unguard(name.trim()).to_string())
        .collect();
    let mut seen = HashSet::new();
    for name in &participants {
        if name.is_empty() {
            return Err("A person's column has no name".to_string());
        }
        if !seen.insert(name.as_str()) {
            return Err(format!("Two columns are named {name}"));
        }
    }

    let mut currency: Option<String> = None;
    let mut expenses = Vec::new();
    for (index, record) in records.enumerate() {
        // As a spreadsheet numbers them, the header being line 1.
        let line = index + 2;
        let record = record.map_err(|e| format!("Line {line} can't be read: {e}"))?;
        if record.iter().all(|cell| cell.trim().is_empty()) {
            continue;
        }
        let cell = |i: usize| record.get(i).unwrap_or_default().trim();
        let at_line = |e: String| format!("Line {line}: {e}");

        let created_at = parse_date(cell(0)).ok_or_else(|| {
            format!(
                "Line {line}: \"{}\" is not a date, such as 2026-12-31",
                cell(0)
            )
        })?;
        let amount_cents = parse_cents(cell(2))
            .filter(|amount| *amount > 0)
            .ok_or_else(|| format!("Line {line}: \"{}\" is not an amount above zero", cell(2)))?;
        if !cell(3).is_empty() {
            match &currency {
                None => currency = Some(cell(3).to_uppercase()),
                Some(c) if c.eq_ignore_ascii_case(cell(3)) => {}
                Some(c) => {
                    return Err(format!(
                        "Line {line} is in {}, the lines before in {c}: a group has one currency \
                         (another one goes in Original currency)",
                        cell(3)
                    ))
                }
            }
        }
        let payer = unguard(cell(4));
        let column_of = |name: &str| participants.iter().position(|n| n == name);
        let (paid_by, payers) = match column_of(payer) {
            Some(paid_by) => (paid_by, Vec::new()),
            // Not a name: several payers, each with their amount.
            None => {
                let payers = payer
                    .split(SEVERAL_PAYERS)
                    .map(|entry| {
                        let (name, amount) = entry.rsplit_once('=')?;
                        Some((
                            column_of(unguard(name.trim()))?,
                            parse_cents(amount.trim())?,
                        ))
                    })
                    .collect::<Option<Vec<(usize, i64)>>>()
                    .filter(|payers| payers.len() > 1)
                    .ok_or_else(|| format!("Line {line}: {payer} paid, but has no column"))?;
                (payers[0].0, payers)
            }
        };
        let (is_reimbursement, income) = match cell(5).to_lowercase().as_str() {
            "" | EXPENSE => (false, false),
            PAYMENT => (true, false),
            INCOME => (false, true),
            other => {
                return Err(format!(
                    "Line {line}: the type is \"{other}\", not {EXPENSE}, {PAYMENT} or {INCOME}"
                ))
            }
        };
        let original = match (cell(6), cell(7)) {
            ("", "") => None,
            ("", _) | (_, "") => {
                return Err(format!(
                    "Line {line}: the original amount and its currency go together"
                ))
            }
            (amount, original_currency) => {
                let original_cents = parse_cents(amount)
                    .filter(|amount| *amount > 0)
                    .ok_or_else(|| {
                        format!("Line {line}: \"{amount}\" is not an amount above zero")
                    })?;
                Some(OriginalAmount {
                    currency: original_currency.to_uppercase(),
                    amount_cents: original_cents,
                    rate: match cell(8) {
                        "" => rate_between(amount_cents, original_cents),
                        rate => rate.to_string(),
                    },
                })
            }
        };

        let mut people = Vec::new();
        let mut owes = Vec::new();
        for (person, name) in participants.iter().enumerate() {
            let text = cell(first_person + person);
            if text.is_empty() {
                continue;
            }
            let amount = parse_cents(text)
                .ok_or_else(|| format!("Line {line}: \"{text}\" is not an amount, for {name}"))?;
            if amount > 0 {
                people.push(person);
                owes.push(amount);
            }
        }
        if !people.is_empty() {
            let total = owes
                .iter()
                .try_fold(0i64, |sum, o| sum.checked_add(*o))
                .unwrap_or(i64::MAX);
            if total != amount_cents {
                return Err(format!(
                    "Line {line}: the people's parts add up to {}, not {}",
                    cents(total),
                    cents(amount_cents)
                ));
            }
        }

        let mut title = unguard(cell(1)).to_string();
        let splits = if is_reimbursement {
            let [to] = people.as_slice() else {
                return Err(format!(
                    "Line {line}: a payment goes to one person, with the whole amount in their column"
                ));
            };
            if title.is_empty() {
                title = payment_title(payer, &participants[*to]);
            }
            vec![(*to, 1, None)]
        } else {
            let declared = match cell(9) {
                "" => None,
                split => Some(parse_split(split, participants.len()).ok_or_else(|| {
                    format!(
                        "Line {line}: Split needs an entry per person: a number of parts, \
                         an amount such as 12.50, or {NOT_IN}"
                    )
                })?),
            };
            // What the split gives each person, to compare with the file's amounts.
            let gives = |splits: &[Split]| -> Vec<i64> {
                let amounts = owed(
                    amount_cents,
                    original.as_ref().map(|o| o.amount_cents),
                    &anonymous(splits),
                );
                let mut per_person = vec![0; participants.len()];
                for ((person, ..), amount) in splits.iter().zip(amounts) {
                    per_person[*person] = amount;
                }
                per_person
            };
            let mut in_file = vec![0; participants.len()];
            for (person, amount) in people.iter().zip(&owes) {
                in_file[*person] = *amount;
            }
            match declared {
                Some(declared) if people.is_empty() => {
                    check_amounts(amount_cents, original.as_ref(), &anonymous(&declared))
                        .map_err(at_line)?;
                    declared
                }
                // Another app rounds its cents elsewhere; more than that, and someone
                // changed the amounts.
                Some(declared)
                    if check_amounts(amount_cents, original.as_ref(), &anonymous(&declared))
                        .is_ok()
                        && gives(&declared)
                            .iter()
                            .zip(&in_file)
                            .all(|(a, b)| (a - b).abs() <= 1) =>
                {
                    declared
                }
                _ if people.is_empty() => {
                    return Err(format!("Line {line}: nobody shares this expense"))
                }
                _ => split_from_amounts(amount_cents, original.is_some(), &people, &owes)
                    .ok_or_else(|| format!("Line {line}: the parts are too uneven to import"))?,
            }
        };
        let category = Some(cell(COLUMNS.len()).to_lowercase())
            .filter(|category| has_category && !category.is_empty());
        expenses.push(ImportedExpense {
            title,
            category,
            amount_cents,
            original,
            paid_by,
            payers,
            splits,
            created_at,
            is_reimbursement,
            income,
        });
    }

    Ok(ImportedGroup {
        // A file without expenses doesn't say; the group's currency can be changed later.
        currency: currency.unwrap_or_else(|| "EUR".to_string()),
        participants,
        expenses,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc;
    use crate::engine::calculate_balances;

    const HEADER: &str = "Date,Title,Amount,Currency,Paid by,Type,Original amount,\
                          Original currency,Exchange rate,Split";

    fn group_of(file: &str) -> Group {
        let imported = import(file).unwrap();
        let doc = doc::imported_group_doc(
            "Imported",
            &imported.currency,
            &imported.participants,
            &imported.expenses,
        )
        .unwrap();
        doc::read_group(&doc).unwrap()
    }

    #[test]
    fn categories_survive_export_and_import() {
        let file = format!(
            "{HEADER},Category,Alice,Bob\n\
             2026-08-29T10:00:00Z,Dinner,30.00,EUR,Alice,expense,,,,1 1,Food,15.00,15.00\n\
             2026-08-30T10:00:00Z,Taxi,10.00,EUR,Bob,expense,,,,1 1,,5.00,5.00\n"
        );
        let group = group_of(&file);
        let categories = |group: &Group| -> Vec<Option<String>> {
            group.expenses.iter().map(|e| e.category.clone()).collect()
        };
        assert_eq!(group.participants.len(), 2, "Category is not a person");
        assert_eq!(categories(&group), vec![Some("food".to_string()), None]);

        let exported = export(&group).unwrap();
        assert!(exported.starts_with(&format!("{HEADER},Category,Alice,Bob")));
        assert_eq!(categories(&group_of(&exported)), categories(&group));

        // A file from before categories has the people right after Split.
        let old = format!(
            "{HEADER},Alice,Bob\n2026-08-29T10:00:00Z,Dinner,30.00,EUR,Alice,expense,,,,1 1,15.00,15.00\n"
        );
        let group = group_of(&old);
        assert_eq!(group.participants.len(), 2);
        assert_eq!(categories(&group), vec![None]);
    }

    #[test]
    fn several_payers_survive_export_and_import() {
        let file = format!(
            "{HEADER},Alice,Bob,Carol\n\
             2026-08-29T10:00:00Z,Dinner,90.00,EUR,Alice=30.00 + Bob=60.00,expense,,,,1 1 1,30.00,30.00,30.00\n\
             2026-08-30T10:00:00Z,Taxi,46.17,EUR,Bob=40.00 + Carol=10.00,expense,50.00,USD,0.9234,1 1 -,23.09,23.08,\n"
        );
        let group = group_of(&file);
        let payers = |i: usize| -> Vec<(String, i64)> {
            group.expenses[i]
                .payers
                .iter()
                .map(|p| {
                    let name = &group.participants.iter().find(|x| x.id == p.participant_id);
                    (name.unwrap().name.clone(), p.amount_cents)
                })
                .collect()
        };
        // Who paid the most first.
        assert_eq!(
            payers(0),
            vec![("Bob".to_string(), 6000), ("Alice".to_string(), 3000)]
        );
        assert_eq!(
            payers(1),
            vec![("Bob".to_string(), 4000), ("Carol".to_string(), 1000)]
        );
        assert_eq!(
            nets(&group),
            vec![
                ("Alice".to_string(), 3000 - 3000 - 2309),
                ("Bob".to_string(), 6000 + 3694 - 3000 - 2308),
                ("Carol".to_string(), 923 - 3000),
            ]
        );

        let again = group_of(&export(&group).unwrap());
        assert_eq!(nets(&again), nets(&group));
        assert!(export(&group).unwrap().contains("Bob=60.00 + Alice=30.00"));

        // Amounts that aren't the expense's, or someone without a column.
        for bad in [
            "Alice=30.00 + Bob=50.00",
            "Alice=30.00 + Dave=60.00",
            "Alice + Bob",
        ] {
            let file = format!(
                "{HEADER},Alice,Bob\n2026-08-29T10:00:00Z,Dinner,90.00,EUR,{bad},expense,,,,1 1,45.00,45.00\n"
            );
            let imported = import(&file).and_then(|g| {
                doc::imported_group_doc("x", &g.currency, &g.participants, &g.expenses)
            });
            assert!(imported.is_err(), "{bad}");
        }
    }

    /// (name, net balance), in the group's order.
    fn nets(group: &Group) -> Vec<(String, i64)> {
        calculate_balances(group)
            .into_iter()
            .map(|b| (b.participant_name, b.net_cents))
            .collect()
    }

    /// (person's position, shares, fixed amount) of each expense.
    fn splits(group: &Group) -> Vec<Vec<Split>> {
        let position = |id: &str| group.participants.iter().position(|p| p.id == id).unwrap();
        group
            .expenses
            .iter()
            .map(|e| {
                e.splits
                    .iter()
                    .map(|s| (position(&s.participant_id), s.shares, s.fixed_cents))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn a_group_survives_export_and_import() {
        let names = ["Alice", "Bob", "=Carol, \"C\""].map(String::from);
        let doc = doc::new_group_doc("Trip", "CHF", &names).unwrap();
        let ids: Vec<String> = doc::read_group(&doc)
            .unwrap()
            .participants
            .into_iter()
            .map(|p| p.id)
            .collect();
        let split = |i: usize, shares: u32| ExpenseSplit {
            participant_id: ids[i].clone(),
            shares,
            fixed_cents: None,
        };
        let fixed = |i: usize, amount: i64| ExpenseSplit {
            participant_id: ids[i].clone(),
            shares: 0,
            fixed_cents: Some(amount),
        };
        let at = |day: u32| parse_date(&format!("2026-03-{day:02}T08:30:00Z"));
        doc::add_expense(
            &doc,
            "Dinner, with \"wine\"",
            1000,
            ids[0].clone(),
            vec![split(0, 1), split(1, 1), split(2, 1)],
            at(1),
            None,
        )
        .unwrap();
        doc::add_expense(
            &doc,
            "-50% tickets",
            13400,
            ids[1].clone(),
            vec![split(1, 2), split(2, 1)],
            at(2),
            None,
        )
        .unwrap();
        // Two fixed amounts, and the rest by parts.
        doc::add_expense(
            &doc,
            "Groceries",
            14660,
            ids[2].clone(),
            vec![fixed(0, 4436), fixed(1, 5550), split(2, 5)],
            at(3),
            None,
        )
        .unwrap();
        // Paid in dollars: the fixed amount is in dollars too.
        doc::add_expense(
            &doc,
            "Taxi",
            4617,
            ids[1].clone(),
            vec![fixed(0, 2000), split(1, 1)],
            at(4),
            Some(OriginalAmount {
                currency: "USD".to_string(),
                amount_cents: 5000,
                rate: "0.9234".to_string(),
            }),
        )
        .unwrap();
        doc::record_reimbursement(&doc, ids[1].clone(), ids[0].clone(), 250, None, None).unwrap();
        // Money that came in: a refund Alice received for the three.
        doc::add_expense_as(
            &doc,
            "Refund",
            900,
            ids[0].clone(),
            vec![split(0, 1), split(1, 1), split(2, 1)],
            at(6),
            None,
            doc::Adding {
                income: true,
                ..doc::Adding::default()
            },
        )
        .unwrap();
        let group = doc::read_group(&doc).unwrap();

        let file = export(&group).unwrap();
        assert!(
            file.contains(",Refund,9.00,CHF,Alice,income,,,,1 1 1,,3.00,3.00,3.00\n"),
            "{file}"
        );
        let expected = format!(
            "{HEADER},Category,Alice,Bob,\"'=Carol, \"\"C\"\"\"\n\
             2026-03-01T08:30:00Z,\"Dinner, with \"\"wine\"\"\",10.00,CHF,Alice,expense,,,,1 1 1,,3.34,3.33,3.33\n\
             2026-03-02T08:30:00Z,'-50% tickets,134.00,CHF,Bob,expense,,,,- 2 1,,,89.33,44.67\n\
             2026-03-03T08:30:00Z,Groceries,146.60,CHF,\"'=Carol, \"\"C\"\"\",expense,,,,44.36 55.50 5,,44.36,55.50,46.74\n\
             2026-03-04T08:30:00Z,Taxi,46.17,CHF,Bob,expense,50.00,USD,0.9234,20.00 1 -,,18.47,27.70,\n"
        );
        assert_eq!(&file[..expected.len()], expected);

        let back = group_of(&file);
        assert_eq!(back.currency, "CHF");
        assert_eq!(nets(&back), nets(&group));
        assert_eq!(splits(&back), splits(&group));
        let fields = |g: &Group| -> Vec<_> {
            g.expenses
                .iter()
                .map(|e| {
                    (
                        e.title.clone(),
                        e.amount_cents,
                        e.original.clone(),
                        e.created_at,
                        e.is_reimbursement,
                    )
                })
                .collect()
        };
        assert_eq!(fields(&back), fields(&group));
    }

    #[test]
    fn reads_what_spreadsheets_and_other_apps_write() {
        // Semicolons, decimal commas, a BOM, dates in several forms, an empty line, and
        // equal parts whose odd cent went to someone else than here.
        let group = group_of(
            "\u{feff}date;title;amount;currency;paid by;type;original amount;original currency;\
             exchange rate;split;Ann;Ben;Cleo\r\n\
             2026-08-29 02:00:00.000000;Gift;8,50;eur;Ann;;;;;;2,83;2,83;2,84\r\n\
             31/12/2025;Train;1 234,5;EUR;Ben;Expense;;;;;;617,25;617,25\r\n\
             ;;;;;;;;;;;;\r\n\
             2026-01-02;;10;EUR;Cleo;payment;;;;;10;0;\r\n\
             2026-02-01;Hotel;92,34;EUR;Ann;;100;usd;;;46,17;46,17;\r\n",
        );
        assert_eq!(group.currency, "EUR");
        assert_eq!(
            splits(&group),
            [
                vec![(1, 1, None), (2, 1, None)],
                vec![(0, 1, None)],
                vec![(0, 1, None), (1, 1, None)],
                vec![(0, 1, None), (1, 1, None), (2, 1, None)]
            ]
        );
        assert_eq!(group.expenses[1].title, "Payment: Cleo → Ann");
        assert!(group.expenses[1].is_reimbursement);
        assert_eq!(
            group.expenses[1].created_at.to_rfc3339(),
            "2026-01-02T12:00:00+00:00"
        );
        assert_eq!(group.expenses[0].amount_cents, 123_450);
        // The rate is worked out when the file has none.
        assert_eq!(
            group.expenses[2].original,
            Some(OriginalAmount {
                currency: "USD".to_string(),
                amount_cents: 10_000,
                rate: "0.9234".to_string(),
            })
        );
    }

    #[test]
    fn the_split_column_is_a_hint() {
        let file = |line: &str| format!("{HEADER},Ann,Ben,Cleo\n{line}\n");
        let split = |line: &str| splits(&group_of(&file(line))).remove(0);
        // Alone, it says everything.
        assert_eq!(
            split("2026-01-01,Gift,10,EUR,Ann,,,,,2.50 1 2,,,"),
            [(0, 0, Some(250)), (1, 1, None), (2, 2, None)]
        );
        // With amounts a cent off, as another app rounds: still the split.
        assert_eq!(
            split("2026-01-01,Gift,10,EUR,Ann,,,,,1 1 1,3.33,3.33,3.34"),
            [(0, 1, None), (1, 1, None), (2, 1, None)]
        );
        // With other amounts, someone edited them: they win.
        assert_eq!(
            split("2026-01-01,Gift,10,EUR,Ann,,,,,1 1 1,5.00,5.00,"),
            [(0, 1, None), (1, 1, None)]
        );
        assert_eq!(
            split("2026-01-01,Gift,10,EUR,Ann,,,,,1 1 1,1.00,2.50,6.50"),
            [(0, 0, Some(100)), (1, 0, Some(250)), (2, 0, Some(650))]
        );
        let error = |line: &str| import(&file(line)).unwrap_err();
        assert!(error("2026-01-01,Gift,10,EUR,Ann,,,,,1 1,,,").starts_with("Line 2: Split needs"));
        assert_eq!(
            error("2026-01-01,Gift,10,EUR,Ann,,,,,7.00 4.00 -,,,"),
            "Line 2: The fixed amounts add up to 11.00, more than the expense's 10.00"
        );
        assert_eq!(
            error("2026-01-01,Gift,10,EUR,Ann,,,,,,,,"),
            "Line 2: nobody shares this expense"
        );
    }

    #[test]
    fn finds_the_shares_behind_amounts() {
        assert_eq!(small_shares(1000, &[334, 333, 333]), Some(vec![1, 1, 1]));
        assert_eq!(small_shares(13400, &[8933, 4467]), Some(vec![2, 1]));
        assert_eq!(
            small_shares(12149, &[2429, 2430, 4860, 2430]),
            Some(vec![1, 1, 2, 1])
        );
        assert_eq!(small_shares(700, &[300, 400]), Some(vec![3, 4]));
        assert_eq!(small_shares(14660, &[4436, 3830, 1720, 4674]), None);
        // In another currency there are no fixed amounts to fall back on.
        assert_eq!(
            split_from_amounts(14660, true, &[0, 1, 2, 3], &[4436, 3830, 1720, 4674]),
            Some(vec![
                (0, 2218, None),
                (1, 1915, None),
                (2, 860, None),
                (3, 2337, None)
            ])
        );
    }

    #[test]
    fn says_what_is_wrong_with_a_file() {
        let header = format!("{HEADER},Ann,Ben\n");
        let error = |lines: &str| import(&format!("{header}{lines}")).unwrap_err();
        assert!(import("Who,What\nAnn,Gift\n")
            .unwrap_err()
            .starts_with("This is not an ezcount CSV file"));
        assert!(import("")
            .unwrap_err()
            .starts_with("This is not an ezcount"));
        assert_eq!(
            import(&format!("{HEADER},Ann,Ann\n")).unwrap_err(),
            "Two columns are named Ann"
        );
        assert_eq!(
            error("soon,Gift,10,EUR,Ann,,,,,,5,5"),
            "Line 2: \"soon\" is not a date, such as 2026-12-31"
        );
        assert_eq!(
            error("2026-01-01,Gift,-10,EUR,Ann,,,,,,5,5"),
            "Line 2: \"-10\" is not an amount above zero"
        );
        assert_eq!(
            error("2026-01-01,Gift,10,EUR,Zoe,,,,,,5,5"),
            "Line 2: Zoe paid, but has no column"
        );
        assert_eq!(
            error("2026-01-01,Gift,10,EUR,Ann,,,,,,5,4"),
            "Line 2: the people's parts add up to 9.00, not 10.00"
        );
        assert!(
            error("2026-01-01,Gift,10,EUR,Ann,,,,,,5,5\n2026-01-01,Gift,10,USD,Ann,,,,,,5,5")
                .starts_with("Line 3 is in USD, the lines before in EUR")
        );
        assert_eq!(
            error("2026-01-01,Gift,10,EUR,Ann,,12,,,,5,5"),
            "Line 2: the original amount and its currency go together"
        );
        assert!(
            error("2026-01-01,,10,EUR,Ann,payment,,,,,5,5").starts_with("Line 2: a payment goes")
        );
        assert!(error("2026-01-01,,10,EUR,Ann,refund,,,,,5,5").starts_with("Line 2: the type is"));
        // A file with only people is a group without expenses.
        assert_eq!(import(&header).unwrap().participants, ["Ann", "Ben"]);
    }
}
