//! Editing an expense: only what changed is written, so that a concurrent edit to another
//! field survives the merge, and each edit is kept in the expense's history.

use super::*;

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

/// An expense as its edit leaves it: checked, and as the document stores it.
struct Edited<'a> {
    title: &'a str,
    category: Option<String>,
    amount_cents: i64,
    paid_by: String,
    payers: Vec<ExpensePayer>,
    original: Option<OriginalAmount>,
    items: Vec<ExpenseItem>,
    splits: Vec<ExpenseSplit>,
    /// The day it was made, when the edit sets it.
    created_at: Option<DateTime<Utc>>,
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
    let grandfathered = people_of(expense);
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
    let edited = Edited {
        title: title.trim(),
        category,
        amount_cents,
        paid_by,
        payers,
        original,
        items,
        splits,
        created_at,
    };

    let target = child_map(&doc.get_map(EXPENSES), expense_id)
        .ok_or_else(|| "Expense not found".to_string())?;
    let changes = write_changes(&target, &group, expense, &edited)?;
    record_edit(&target, expense, &changes, editing.by)
}

/// The people already on an expense: they may stay on it once removed from the group, so
/// that editing an old expense doesn't force dropping them.
pub(super) fn people_of(expense: &Expense) -> HashSet<&str> {
    std::iter::once(expense.paid_by.as_str())
        .chain(expense.payers.iter().map(|p| p.participant_id.as_str()))
        .chain(expense.splits.iter().map(|s| s.participant_id.as_str()))
        .chain(
            expense
                .items
                .iter()
                .flat_map(|item| item.participants.iter().map(String::as_str)),
        )
        .collect()
}

/// Writes the fields the edit changes, and returns what to say of each change, in the order
/// the history shows them.
fn write_changes(target: &LoroMap, group: &Group, old: &Expense, new: &Edited) -> Res<Vec<String>> {
    let changes = [
        change_title(target, old, new)?,
        change_category(target, old, new)?,
        change_amount(target, old, new)?,
        change_payers(target, group, old, new)?,
        change_original(target, group, old, new)?,
        change_split(target, old, new)?,
        change_date(target, old, new)?,
    ];
    Ok(changes.into_iter().flatten().collect())
}

fn change_title(target: &LoroMap, old: &Expense, new: &Edited) -> Res<Option<String>> {
    if old.title == new.title {
        return Ok(None);
    }
    target.insert("title", new.title).map_err(doc_err)?;
    Ok(Some(format!(
        "Title changed from '{}' to '{}'",
        old.title, new.title
    )))
}

fn change_category(target: &LoroMap, old: &Expense, new: &Edited) -> Res<Option<String>> {
    if old.category == new.category {
        return Ok(None);
    }
    match &new.category {
        Some(category) => target.insert("category", category.as_str()),
        None => target.delete("category"),
    }
    .map_err(doc_err)?;
    let named = |category: &Option<String>| category.clone().unwrap_or_else(|| "none".into());
    Ok(Some(format!(
        "Category changed from {} to {}",
        named(&old.category),
        named(&new.category)
    )))
}

fn change_amount(target: &LoroMap, old: &Expense, new: &Edited) -> Res<Option<String>> {
    if old.amount_cents == new.amount_cents {
        return Ok(None);
    }
    target
        .insert("amount_cents", stored_amount(new.amount_cents, old.income))
        .map_err(doc_err)?;
    Ok(Some(format!(
        "Amount changed from {:.2} to {:.2}",
        old.amount_cents as f64 / 100.0,
        new.amount_cents as f64 / 100.0
    )))
}

fn change_payers(
    target: &LoroMap,
    group: &Group,
    old: &Expense,
    new: &Edited,
) -> Res<Option<String>> {
    if old.paid_by == new.paid_by && old.payers == new.payers {
        return Ok(None);
    }
    if old.paid_by != new.paid_by {
        target
            .insert("paid_by", new.paid_by.as_str())
            .map_err(doc_err)?;
    }
    if old.payers != new.payers {
        if new.payers.is_empty() {
            target.delete("payers")
        } else {
            target.insert("payers", payers_value(&new.payers))
        }
        .map_err(doc_err)?;
    }
    Ok(Some(format!(
        "Payer changed from {} to {}",
        payers_named(group, &old.paid_by, &old.payers),
        payers_named(group, &new.paid_by, &new.payers)
    )))
}

/// One payer by name, several with what each paid.
pub(super) fn payers_named(group: &Group, paid_by: &str, payers: &[ExpensePayer]) -> String {
    let name_of = |id: &str| {
        group
            .participants
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.as_str())
            .unwrap_or("Unknown")
    };
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
}

fn change_original(
    target: &LoroMap,
    group: &Group,
    old: &Expense,
    new: &Edited,
) -> Res<Option<String>> {
    if old.original == new.original {
        return Ok(None);
    }
    match &new.original {
        Some(original) => target.insert(ORIGINAL, original_value(original)),
        None => target.delete(ORIGINAL),
    }
    .map_err(doc_err)?;
    let described = |original: &Option<OriginalAmount>| match original {
        Some(o) => format!(
            "{} {} at {}",
            money(i128::from(o.amount_cents)),
            o.currency,
            o.rate
        ),
        None => group.currency.clone(),
    };
    Ok(Some(format!(
        "Paid in {} instead of {}",
        described(&new.original),
        described(&old.original)
    )))
}

/// The lines and the splits: lines give the splits, so a change of lines says it for both.
fn change_split(target: &LoroMap, old: &Expense, new: &Edited) -> Res<Option<String>> {
    let change = if old.items != new.items {
        if new.items.is_empty() {
            target.delete(ITEMS)
        } else {
            target.insert(ITEMS, items_value(&new.items))
        }
        .map_err(doc_err)?;
        Some("Items updated".to_string())
    } else if old.splits != new.splits {
        Some("Participants / parts allocation updated".to_string())
    } else {
        None
    };
    // With fixed amounts, the stored splits also depend on the amounts (see `splits_value`).
    let fixed = new.splits.iter().any(|s| s.fixed_cents.is_some());
    if old.splits != new.splits
        || (fixed && (old.amount_cents != new.amount_cents || old.original != new.original))
    {
        target
            .insert(
                "splits",
                splits_value(new.amount_cents, new.original.as_ref(), &new.splits),
            )
            .map_err(doc_err)?;
    }
    Ok(change)
}

fn change_date(target: &LoroMap, old: &Expense, new: &Edited) -> Res<Option<String>> {
    let Some(created_at) = new.created_at.filter(|at| *at != old.created_at) else {
        return Ok(None);
    };
    target
        .insert("created_at", timestamp(created_at))
        .map_err(doc_err)?;
    Ok(Some(format!(
        "Date changed from {} to {}",
        old.created_at.format("%Y-%m-%d"),
        created_at.format("%Y-%m-%d")
    )))
}

/// Adds the edit to the expense's history, with what the expense was before it.
fn record_edit(target: &LoroMap, old: &Expense, changes: &[String], by: Option<&str>) -> Res<()> {
    let summary = if changes.is_empty() {
        "Updated without major changes".to_string()
    } else {
        changes.join("; ")
    };
    let now = Utc::now();
    let entry = ExpenseHistoryEntry {
        edited_at: now,
        previous_title: old.title.clone(),
        previous_amount_cents: old.amount_cents,
        previous_paid_by: old.paid_by.clone(),
        previous_payers: old.payers.clone(),
        previous_splits: old.splits.clone(),
        previous_original: old.original.clone(),
        previous_category: old.category.clone(),
        summary,
        edited_by: by.map(str::to_string),
    };
    child_list(target, HISTORY)?
        .push(history_value(&entry))
        .map_err(doc_err)?;
    target
        .insert("updated_at", timestamp(now))
        .map_err(doc_err)?;
    Ok(())
}
