//! Reading what a CSV file's cells hold: amounts, dates, and how an expense is split.

use super::*;

/// An amount as a spreadsheet may write it: `12.5`, `12,50`, `1 234,50`, `1,234.50`.
pub(super) fn parse_cents(text: &str) -> Option<i64> {
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
pub(super) fn parse_date(text: &str) -> Option<DateTime<Utc>> {
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

pub(super) fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// The rate that turned `original` into `amount`, to six decimals.
pub(super) fn rate_between(amount: i64, original: i64) -> String {
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
pub(super) fn small_shares(amount: i64, owed: &[i64]) -> Option<Vec<u32>> {
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
pub(super) fn split_from_amounts(
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
pub(super) fn parse_split(text: &str, people: usize) -> Option<Vec<Split>> {
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
pub(super) fn anonymous(splits: &[Split]) -> Vec<ExpenseSplit> {
    splits
        .iter()
        .map(|(_, shares, fixed_cents)| ExpenseSplit {
            participant_id: String::new(),
            shares: *shares,
            fixed_cents: *fixed_cents,
        })
        .collect()
}
