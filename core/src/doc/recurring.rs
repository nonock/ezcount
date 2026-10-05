//! Repeated expenses: the models, and adding the occurrences that are due.

use super::*;

/// A repeated expense as stored.
#[derive(Deserialize)]
pub(super) struct DocRecurring {
    pub(super) title: String,
    #[serde(default)]
    pub(super) category: Option<String>,
    pub(super) amount_cents: i64,
    pub(super) paid_by: String,
    #[serde(default)]
    pub(super) payers: Vec<ExpensePayer>,
    pub(super) splits: Vec<DocSplit>,
    pub(super) every: String,
    pub(super) start: DateTime<Utc>,
    #[serde(default)]
    pub(super) made: i64,
    #[serde(default)]
    pub(super) added_by: Option<String>,
}

/// A repeated expense and where it is at: its first day, and how many were added.
pub(super) struct Repeated {
    pub(super) expense: RecurringExpense,
    pub(super) start: DateTime<Utc>,
    pub(super) made: u32,
}

/// The day of occurrence `n` of an expense that started on `start`. Counting from the start
/// keeps the day of the month: the 31st gives the 28th in February, then the 31st again.
pub(super) fn occurrence(start: DateTime<Utc>, every: &str, n: u32) -> Option<DateTime<Utc>> {
    match every {
        "week" => start.checked_add_days(Days::new(7 * u64::from(n))),
        "month" => start.checked_add_months(Months::new(n)),
        "year" => start.checked_add_months(Months::new(n.checked_mul(12)?)),
        _ => None,
    }
}

/// The repeated expenses, the next one due first. One naming someone who left is `paused`.
pub(super) fn read_recurring(doc: &LoroDoc, participants: &[Participant]) -> Vec<Repeated> {
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
