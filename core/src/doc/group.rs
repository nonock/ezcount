//! Making a group's document, and what changes the group itself: its name, currency,
//! description and picture, and its deletion.

use super::*;

pub(super) fn write_meta(
    doc: &LoroDoc,
    id: &str,
    name: &str,
    currency: &str,
    created_at: DateTime<Utc>,
) -> Res<()> {
    let meta = doc.get_map(META);
    meta.insert("id", id).map_err(doc_err)?;
    meta.insert("name", name).map_err(doc_err)?;
    meta.insert("currency", currency).map_err(doc_err)?;
    meta.insert("created_at", timestamp(created_at))
        .map_err(doc_err)?;
    Ok(())
}

/// Creates the document for a brand-new group.
pub fn new_group_doc(name: &str, currency: &str, participant_names: &[String]) -> Res<LoroDoc> {
    let name = group_name(name)?;
    let currency = currency_code(currency)?;
    let doc = LoroDoc::new();
    write_meta(
        &doc,
        &Uuid::new_v4().to_string(),
        name,
        &currency,
        Utc::now(),
    )?;
    for participant_name in participant_names {
        let trimmed = participant_name.trim();
        if !trimmed.is_empty() {
            insert_participant(&doc, &Uuid::new_v4().to_string(), trimmed, false)?;
        }
    }
    doc.commit();
    Ok(doc)
}

/// An expense read from a file, before the group exists: people are positions in the
/// imported list of names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedExpense {
    pub title: String,
    pub category: Option<String>,
    pub amount_cents: i64,
    pub paid_by: usize,
    // (person, amount) for each of several payers, as in `ExpensePayer`
    pub payers: Vec<(usize, i64)>,
    pub original: Option<OriginalAmount>,
    // (person, shares, fixed amount), as in `ExpenseSplit`
    pub splits: Vec<(usize, u32, Option<i64>)>,
    pub created_at: DateTime<Utc>,
    pub is_reimbursement: bool,
    pub income: bool,
}

/// Creates the document for a group read from a file, with its expenses.
pub fn imported_group_doc(
    name: &str,
    currency: &str,
    participant_names: &[String],
    expenses: &[ImportedExpense],
) -> Res<LoroDoc> {
    let doc = new_group_doc(name, currency, participant_names)?;
    let group = read_group(&doc)?;
    // `new_group_doc` skips blank names, which would shift the positions.
    if group.participants.len() != participant_names.len() {
        return Err("Participant name cannot be empty".to_string());
    }
    let id = |position: usize| {
        group
            .participants
            .get(position)
            .map(|p| p.id.clone())
            .ok_or_else(|| "Participant not found".to_string())
    };
    let now = Utc::now();
    for e in expenses {
        let splits = e
            .splits
            .iter()
            .map(|(position, shares, fixed_cents)| {
                Ok(ExpenseSplit {
                    participant_id: id(*position)?,
                    shares: *shares,
                    fixed_cents: *fixed_cents,
                })
            })
            .collect::<Res<Vec<_>>>()?;
        let payers = e
            .payers
            .iter()
            .map(|(position, amount_cents)| {
                Ok(ExpensePayer {
                    participant_id: id(*position)?,
                    amount_cents: *amount_cents,
                })
            })
            .collect::<Res<Vec<_>>>()?;
        let PaidBy { paid_by, payers } = PaidBy::new(id(e.paid_by)?, payers);
        let original = normal_original(&group, e.original.clone())?;
        validate_expense(
            &group,
            e.amount_cents,
            original.as_ref(),
            &paid_by,
            &payers,
            &splits,
            &HashSet::new(),
        )?;
        insert_expense(
            &doc,
            &Expense {
                id: Uuid::new_v4().to_string(),
                group_id: group.id.clone(),
                title: e.title.trim().to_string(),
                category: e.category.clone().filter(|c| check_category(c).is_ok()),
                amount_cents: e.amount_cents,
                original,
                paid_by,
                payers,
                splits,
                created_at: e.created_at,
                updated_at: now,
                history: Vec::new(),
                is_reimbursement: e.is_reimbursement,
                income: e.income,
                added_at: None,
                added_by: None,
                recurring: None,
                items: Vec::new(),
                comments: Vec::new(),
            },
        )?;
    }
    doc.commit();
    Ok(doc)
}

/// Sets the group's name, currency, description and picture. Amounts are kept as they are,
/// not converted. An empty description, or no picture, removes it.
pub fn update_group(
    doc: &LoroDoc,
    name: &str,
    currency: &str,
    description: &str,
    image: Option<&str>,
) -> Res<()> {
    let name = group_name(name)?;
    let currency = currency_code(currency)?;
    let description = description.trim();
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(format!(
            "This description is too long ({MAX_DESCRIPTION_CHARS} characters at most)"
        ));
    }
    if let Some(image) = image {
        check_image(image)?;
    }
    let group = read_group(doc)?;
    let meta = doc.get_map(META);
    // Only changed fields are written, so a concurrent change to the other one survives.
    if group.name != name {
        meta.insert("name", name).map_err(doc_err)?;
    }
    if group.currency != currency {
        meta.insert("currency", currency.as_str())
            .map_err(doc_err)?;
    }
    if group.description != description {
        meta.insert("description", description).map_err(doc_err)?;
    }
    if group.image.as_deref() != image {
        match image {
            Some(image) => meta.insert("image", image).map_err(doc_err)?,
            None => meta.delete("image").map_err(doc_err)?,
        }
    }
    Ok(())
}

/// Deletes the group for everyone when nobody owes anything. Otherwise it takes every
/// member's agreement: this records `me`'s, and deletes once all the members have agreed.
/// Returns whether the group is now deleted.
pub fn delete_or_vote(doc: &LoroDoc, me: Option<&str>) -> Res<bool> {
    let group = read_group(doc)?;
    let settled = engine::calculate_balances(&group)
        .iter()
        .all(|b| b.net_cents == 0);
    let members = || group.participants.iter().filter(|p| !p.removed);
    if !settled {
        let me = me
            .filter(|me| members().any(|p| p.id == *me))
            .ok_or_else(|| {
                "Say who you are in this group before asking to delete it".to_string()
            })?;
        doc.get_map(DELETION).insert(me, true).map_err(doc_err)?;
        let agreed = |id: &str| id == me || group.deletion_votes.iter().any(|voted| voted == id);
        if !members().all(|p| agreed(&p.id)) {
            return Ok(false);
        }
    }
    doc.get_map(META).insert("deleted", true).map_err(doc_err)?;
    Ok(true)
}

/// Drops the request to delete the group: someone refused, or changed their mind.
pub fn refuse_deletion(doc: &LoroDoc) -> Res<()> {
    let votes = doc.get_map(DELETION);
    let mut voters = Vec::new();
    votes.for_each(|id, _| voters.push(id.to_string()));
    for id in voters {
        votes.delete(&id).map_err(doc_err)?;
    }
    Ok(())
}
