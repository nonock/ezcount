//! Deleted expenses: they wait in the trash, from which they are put back or removed for good.

use super::*;

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
pub(super) fn old_trash(
    group: &Group,
    now: DateTime<Utc>,
) -> impl Iterator<Item = &DeletedExpense> {
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
