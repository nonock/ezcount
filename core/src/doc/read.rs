//! Reading a group's document into the `Group` the app shows. What doesn't fit the schema
//! (a crafted update, a newer version's data) is left out, never fatal.

use super::*;

#[derive(Deserialize)]
pub(super) struct DocMeta {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) currency: String,
    pub(super) created_at: DateTime<Utc>,
    #[serde(default)]
    pub(super) description: String,
    #[serde(default)]
    pub(super) image: Option<String>,
    #[serde(default)]
    pub(super) deleted: bool,
}

#[derive(Deserialize)]
pub(super) struct DocParticipant {
    pub(super) name: String,
    #[serde(default)]
    pub(super) removed: bool,
    #[serde(default)]
    pub(super) position: i64,
    #[serde(default)]
    pub(super) avatar: Option<String>,
    #[serde(default)]
    pub(super) iban: Option<String>,
    // Read as text: a date that isn't one is left out rather than taking the member with it.
    #[serde(default)]
    pub(super) added_at: Option<String>,
    #[serde(default)]
    pub(super) added_by: Option<String>,
    #[serde(default)]
    pub(super) removed_at: Option<String>,
    #[serde(default)]
    pub(super) removed_by: Option<String>,
}

pub(super) fn read_time(text: Option<String>) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&text?)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

/// A picture read from a document, or nothing when it isn't one the app would have saved.
pub(super) fn read_image(image: Option<String>) -> Option<String> {
    image.filter(|image| check_image(image).is_ok())
}

/// A split as stored (see `splits_value`).
#[derive(Deserialize)]
pub(super) struct DocSplit {
    pub(super) participant_id: String,
    pub(super) shares: u32,
    #[serde(default)]
    pub(super) parts: Option<u32>,
    #[serde(default)]
    pub(super) fixed_cents: Option<i64>,
}

/// `legacy` reads only what app versions from before fixed amounts do.
pub(super) fn read_splits(splits: &[DocSplit], legacy: bool) -> Vec<ExpenseSplit> {
    splits
        .iter()
        .map(|s| ExpenseSplit {
            participant_id: s.participant_id.clone(),
            shares: if legacy {
                s.shares
            } else {
                s.parts.unwrap_or(s.shares)
            },
            fixed_cents: s.fixed_cents.filter(|_| !legacy),
        })
        .collect()
}

#[derive(Deserialize)]
pub(super) struct DocHistoryEntry {
    pub(super) edited_at: DateTime<Utc>,
    pub(super) previous_title: String,
    pub(super) previous_amount_cents: i64,
    pub(super) previous_paid_by: String,
    #[serde(default)]
    pub(super) previous_payers: Vec<ExpensePayer>,
    pub(super) previous_splits: Vec<DocSplit>,
    #[serde(default)]
    pub(super) previous_original: Option<OriginalAmount>,
    #[serde(default)]
    pub(super) previous_category: Option<String>,
    pub(super) summary: String,
    #[serde(default)]
    pub(super) edited_by: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct DocExpense {
    pub(super) title: String,
    #[serde(default)]
    pub(super) category: Option<String>,
    pub(super) amount_cents: i64,
    #[serde(default)]
    pub(super) original: Option<OriginalAmount>,
    pub(super) paid_by: String,
    #[serde(default)]
    pub(super) payers: Vec<ExpensePayer>,
    pub(super) splits: Vec<DocSplit>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    #[serde(default)]
    pub(super) is_reimbursement: bool,
    #[serde(default)]
    pub(super) history: Vec<DocHistoryEntry>,
    #[serde(default)]
    pub(super) added_at: Option<String>,
    #[serde(default)]
    pub(super) added_by: Option<String>,
    #[serde(default)]
    pub(super) recurring: Option<String>,
    // Read as it comes: lines that aren't usable are left out, not the expense.
    #[serde(default)]
    pub(super) items: serde_json::Value,
    // In the trash only.
    #[serde(default)]
    pub(super) deleted_at: Option<String>,
    #[serde(default)]
    pub(super) deleted_by: Option<String>,
}

/// An expense as stored, checked and in the shape the app uses. Synced edits skip
/// `validate_expense`, and the balance engine relies on these checks.
pub(super) fn read_expense(id: String, group_id: &str, mut e: DocExpense) -> Option<Expense> {
    // Money that came in is stored below zero.
    let income = e.amount_cents < 0;
    e.amount_cents = e.amount_cents.saturating_abs();
    // What the expense cost elsewhere is a note next to its amount: unusable, it is left out
    // rather than taking the expense with it.
    e.original = e.original.filter(|o| check_original(o).is_ok());
    let splits = match e.checked_splits() {
        Ok(splits) => splits,
        Err(err) => {
            eprintln!("[doc] skipping expense {id}: {err}");
            return None;
        }
    };
    let payers = e.checked_payers();
    let paid_cents = e
        .original
        .as_ref()
        .map_or(e.amount_cents, |o| o.amount_cents);
    let items = read_items(std::mem::take(&mut e.items), paid_cents, &splits);
    Some(Expense {
        id,
        group_id: group_id.to_string(),
        title: e.title,
        category: e.category.filter(|c| check_category(c).is_ok()),
        amount_cents: e.amount_cents,
        original: e.original,
        paid_by: e.paid_by,
        payers,
        splits,
        created_at: e.created_at,
        updated_at: e.updated_at,
        history: e
            .history
            .into_iter()
            .map(|h| ExpenseHistoryEntry {
                edited_at: h.edited_at,
                previous_title: h.previous_title,
                previous_amount_cents: h.previous_amount_cents.saturating_abs(),
                previous_paid_by: h.previous_paid_by,
                previous_payers: h.previous_payers,
                previous_splits: read_splits(&h.previous_splits, false),
                previous_original: h.previous_original,
                previous_category: h.previous_category,
                summary: h.summary,
                edited_by: h.edited_by,
            })
            .collect(),
        is_reimbursement: e.is_reimbursement,
        income,
        added_at: read_time(e.added_at),
        added_by: e.added_by,
        recurring: e.recurring,
        items,
        comments: Vec::new(),
    })
}

impl DocExpense {
    /// The several payers, when they still fit the expense: an older app version changes the
    /// amount or `paid_by` without them, and then `paid_by` paid it all.
    pub(super) fn checked_payers(&self) -> Vec<ExpensePayer> {
        let fits = check_payers(
            self.amount_cents,
            self.original.as_ref(),
            &self.paid_by,
            &self.payers,
        );
        match fits {
            Ok(()) => self.payers.clone(),
            Err(_) => Vec::new(),
        }
    }

    /// The splits, checked. An older app version changes the amount without the splits; if
    /// the fixed amounts no longer fit it, the shares that version reads are used.
    pub(super) fn checked_splits(&self) -> Res<Vec<ExpenseSplit>> {
        let splits = read_splits(&self.splits, false);
        if check_amounts(self.amount_cents, self.original.as_ref(), &splits).is_ok() {
            return Ok(splits);
        }
        let splits = read_splits(&self.splits, true);
        check_amounts(self.amount_cents, self.original.as_ref(), &splits)?;
        Ok(splits)
    }
}

// Documents are read value by value with a depth bound rather than through
// `LoroDoc::get_deep_value`, which follows containers nested to any depth: a group member
// could otherwise crash every other member's app with one deeply nested update.

/// A document value as JSON, or `None` if it nests deeper than `MAX_DEPTH` or holds
/// something the schemas never use (binary data, other container types).
pub(super) fn value_json(value: ValueOrContainer, depth: usize) -> Option<serde_json::Value> {
    if depth > MAX_DEPTH {
        return None;
    }
    match value {
        ValueOrContainer::Value(v) => plain_json(&v, depth),
        ValueOrContainer::Container(Container::Map(map)) => map_json(&map, depth),
        ValueOrContainer::Container(Container::List(list)) => {
            let mut items = Some(Vec::with_capacity(list.len()));
            list.for_each(|v| {
                items = items.take().and_then(|mut items| {
                    items.push(value_json(v, depth + 1)?);
                    Some(items)
                });
            });
            items.map(serde_json::Value::Array)
        }
        ValueOrContainer::Container(_) => None,
    }
}

pub(super) fn map_json(map: &LoroMap, depth: usize) -> Option<serde_json::Value> {
    let mut fields = Some(serde_json::Map::new());
    map.for_each(|key, v| {
        fields = fields.take().and_then(|mut fields| {
            fields.insert(key.to_string(), value_json(v, depth + 1)?);
            Some(fields)
        });
    });
    fields.map(serde_json::Value::Object)
}

pub(super) fn plain_json(value: &LoroValue, depth: usize) -> Option<serde_json::Value> {
    use serde_json::Value as Json;
    if depth > MAX_DEPTH {
        return None;
    }
    Some(match value {
        LoroValue::Null => Json::Null,
        LoroValue::Bool(b) => Json::Bool(*b),
        LoroValue::I64(n) => Json::from(*n),
        LoroValue::Double(n) => Json::from(*n),
        LoroValue::String(s) => Json::String(s.to_string()),
        LoroValue::List(items) => Json::Array(
            items
                .iter()
                .map(|v| plain_json(v, depth + 1))
                .collect::<Option<_>>()?,
        ),
        LoroValue::Map(fields) => Json::Object(
            fields
                .iter()
                .map(|(k, v)| Some((k.clone(), plain_json(v, depth + 1)?)))
                .collect::<Option<_>>()?,
        ),
        LoroValue::Binary(_) | LoroValue::Container(_) => return None,
    })
}

/// Entries of a root map, by id, skipping (and logging) any that don't match the schema.
/// A single malformed entry must not make the whole document unreadable.
pub(crate) fn entries<T: DeserializeOwned>(map: &LoroMap, what: &str) -> Vec<(String, T)> {
    let mut out = Vec::new();
    map.for_each(|id, value| {
        let parsed = value_json(value, 1)
            .ok_or_else(|| "unexpected shape or nesting".to_string())
            .and_then(|json| serde_json::from_value(json).map_err(|e| e.to_string()));
        match parsed {
            Ok(parsed) => out.push((id.to_string(), parsed)),
            Err(e) => eprintln!("[doc] skipping malformed {what} entry {id}: {e}"),
        }
    });
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Materializes the document into the `Group` shape the frontend and engine use.
pub fn read_group(doc: &LoroDoc) -> Res<Group> {
    let meta: DocMeta = map_json(&doc.get_map(META), 0)
        .ok_or_else(|| "unexpected shape or nesting".to_string())
        .and_then(|json| serde_json::from_value(json).map_err(|e| e.to_string()))
        .map_err(|e| format!("Group document has no valid metadata: {e}"))?;

    let mut participants: Vec<(i64, Participant)> =
        entries::<DocParticipant>(&doc.get_map(PARTICIPANTS), PARTICIPANTS)
            .into_iter()
            .map(|(id, p)| {
                (
                    p.position,
                    Participant {
                        id,
                        name: p.name,
                        removed: p.removed,
                        avatar: read_image(p.avatar),
                        iban: p.iban.and_then(|iban| check_iban(&iban).ok()),
                        added_at: read_time(p.added_at),
                        added_by: p.added_by,
                        removed_at: read_time(p.removed_at),
                        removed_by: p.removed_by,
                    },
                )
            })
            .collect();
    // Concurrent additions can share a position; the id keeps the order stable everywhere.
    participants.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));

    let participants: Vec<Participant> = participants.into_iter().map(|(_, p)| p).collect();

    let mut comments = read_comments(doc);
    let mut commented = |mut expense: Expense| {
        expense.comments = comments.remove(&expense.id).unwrap_or_default();
        expense
    };
    let mut expenses: Vec<Expense> = entries::<DocExpense>(&doc.get_map(EXPENSES), EXPENSES)
        .into_iter()
        .filter_map(|(id, e)| read_expense(id, &meta.id, e))
        .map(&mut commented)
        .collect();
    expenses.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut trash: Vec<DeletedExpense> = entries::<DocExpense>(&doc.get_map(TRASH), TRASH)
        .into_iter()
        .filter_map(|(id, mut e)| {
            let deleted_at = read_time(e.deleted_at.take())?;
            let deleted_by = e.deleted_by.take();
            Some(DeletedExpense {
                expense: read_expense(id, &meta.id, e)?,
                deleted_at,
                deleted_by,
            })
        })
        // Put back on one device while deleted on another: it is back.
        .filter(|d| !expenses.iter().any(|e| e.id == d.expense.id))
        .map(|d| DeletedExpense {
            expense: commented(d.expense),
            ..d
        })
        .collect();
    trash.sort_by(|a, b| {
        b.deleted_at
            .cmp(&a.deleted_at)
            .then_with(|| a.expense.id.cmp(&b.expense.id))
    });

    // Votes of people who are still members, in the group's order.
    let voted: HashSet<String> = entries::<bool>(&doc.get_map(DELETION), "deletion vote")
        .into_iter()
        .filter_map(|(id, agreed)| agreed.then_some(id))
        .collect();
    let deletion_votes = participants
        .iter()
        .filter(|p| !p.removed && voted.contains(&p.id))
        .map(|p| p.id.clone())
        .collect();
    let recurring = read_recurring(doc, &participants)
        .into_iter()
        .map(|r| r.expense)
        .collect();

    Ok(Group {
        id: meta.id,
        name: meta.name,
        description: meta
            .description
            .chars()
            .take(MAX_DESCRIPTION_CHARS)
            .collect(),
        image: read_image(meta.image),
        currency: meta.currency,
        participants,
        expenses,
        created_at: meta.created_at,
        deleted: meta.deleted,
        deletion_votes,
        trash,
        recurring,
    })
}
