//! The plain values a group's document holds, built by hand (see the parent module).

use super::*;

pub(super) fn map_value<const N: usize>(entries: [(&str, LoroValue); N]) -> LoroValue {
    let map: HashMap<String, LoroValue> = entries
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    LoroValue::Map(map.into())
}

/// Shares that split an amount into `owed`, for app versions from before fixed amounts:
/// they read only `shares`, and still get everyone's balance right.
pub(super) fn legacy_shares(owed: &[i64]) -> Vec<u32> {
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

pub(super) fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// With fixed amounts, each split says what it is in `parts` and `fixed_cents`, and `shares`
/// holds `legacy_shares`. Without, a split is only its `shares`, as it always was.
pub(super) fn splits_value(
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

pub(super) fn payers_value(payers: &[ExpensePayer]) -> LoroValue {
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

pub(super) fn items_value(items: &[ExpenseItem]) -> LoroValue {
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

pub(super) fn original_value(original: &OriginalAmount) -> LoroValue {
    map_value([
        ("currency", original.currency.as_str().into()),
        ("amount_cents", original.amount_cents.into()),
        ("rate", original.rate.as_str().into()),
    ])
}

pub(super) fn history_value(entry: &ExpenseHistoryEntry) -> LoroValue {
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

pub(super) fn child_map(parent: &LoroMap, key: &str) -> Option<LoroMap> {
    match parent.get(key)? {
        ValueOrContainer::Container(Container::Map(map)) => Some(map),
        _ => None,
    }
}

pub(super) fn child_list(parent: &LoroMap, key: &str) -> Res<LoroList> {
    match parent.get(key) {
        Some(ValueOrContainer::Container(Container::List(list))) => Ok(list),
        _ => parent
            .insert_container(key, LoroList::new())
            .map_err(doc_err),
    }
}
