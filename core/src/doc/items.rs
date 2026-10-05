//! An expense entered line by line: its lines, and the splits they give.

use super::*;

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
pub(super) fn items_splits(items: &[ExpenseItem]) -> Vec<ExpenseSplit> {
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
pub(super) fn check_items(paid_cents: i64, items: &[ExpenseItem]) -> Res<()> {
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
pub(super) fn read_items(
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

/// The lines of an expense as stored: names trimmed, on people who can be on the expense
/// (see `validate_expense` for `grandfathered`), adding up to what was paid.
pub(super) fn normal_items(
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
