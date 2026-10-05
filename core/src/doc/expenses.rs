//! Adding expenses, money that came in and payments between members (`edit` changes them).

use super::*;

/// What an amount is stored as: below zero for money that came in.
pub(super) fn stored_amount(amount_cents: i64, income: bool) -> i64 {
    if income {
        -amount_cents
    } else {
        amount_cents
    }
}

/// Writes an expense under its id into `parent`: the expenses, or the trash.
pub(super) fn write_expense(parent: &LoroMap, expense: &Expense) -> Res<LoroMap> {
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

pub(super) fn insert_expense(doc: &LoroDoc, expense: &Expense) -> Res<()> {
    write_expense(&doc.get_map(EXPENSES), expense).map(|_| ())
}

/// The title `record_reimbursement` gives a payment, before any notes.
pub(crate) fn payment_title(from_name: &str, to_name: &str) -> String {
    format!("Payment: {from_name} → {to_name}")
}

/// What an expense is: its title, and the category it counts under.
pub struct Label {
    pub(super) title: String,
    pub(super) category: Option<String>,
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
    pub(super) paid_by: String,
    pub(super) payers: Vec<ExpensePayer>,
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
