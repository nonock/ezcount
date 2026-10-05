//! A group's members: adding, renaming and removing them, and their picture and IBAN.

use super::*;

pub(super) fn insert_participant(doc: &LoroDoc, id: &str, name: &str, removed: bool) -> Res<()> {
    let participants = doc.get_map(PARTICIPANTS);
    let position = participants.len() as i64;
    let p = participants
        .insert_container(id, LoroMap::new())
        .map_err(doc_err)?;
    p.insert("name", name).map_err(doc_err)?;
    p.insert("removed", removed).map_err(doc_err)?;
    p.insert("position", position).map_err(doc_err)?;
    Ok(())
}

/// Sets or removes a participant's picture.
pub fn set_participant_avatar(
    doc: &LoroDoc,
    participant_id: &str,
    avatar: Option<&str>,
) -> Res<()> {
    let participant = child_map(&doc.get_map(PARTICIPANTS), participant_id)
        .ok_or_else(|| "Participant not found".to_string())?;
    let current = match participant.get("avatar") {
        Some(ValueOrContainer::Value(LoroValue::String(current))) => Some(current.to_string()),
        _ => None,
    };
    if current.as_deref() == avatar {
        return Ok(());
    }
    match avatar {
        Some(avatar) => {
            check_image(avatar)?;
            participant.insert("avatar", avatar).map_err(doc_err)
        }
        None => participant.delete("avatar").map_err(doc_err),
    }
}

/// Sets or removes the bank account a participant is paid back on.
pub fn set_participant_iban(doc: &LoroDoc, participant_id: &str, iban: Option<&str>) -> Res<()> {
    let participant = child_map(&doc.get_map(PARTICIPANTS), participant_id)
        .ok_or_else(|| "Participant not found".to_string())?;
    let current = match participant.get("iban") {
        Some(ValueOrContainer::Value(LoroValue::String(current))) => Some(current.to_string()),
        _ => None,
    };
    let iban = iban.map(check_iban).transpose()?;
    if current == iban {
        return Ok(());
    }
    match iban {
        Some(iban) => participant.insert("iban", iban).map_err(doc_err),
        None => participant.delete("iban").map_err(doc_err),
    }
}

/// Renames a participant. Payments keep their names in their title, so the ones still
/// titled as recorded get the new name too.
pub fn rename_participant(doc: &LoroDoc, participant_id: &str, name: &str) -> Res<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Participant name cannot be empty".to_string());
    }
    let participant = child_map(&doc.get_map(PARTICIPANTS), participant_id)
        .ok_or_else(|| "Participant not found".to_string())?;
    let group = read_group(doc)?;
    let old_name = |id: &str| {
        group
            .participants
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.as_str())
    };
    if old_name(participant_id) == Some(name) {
        return Ok(());
    }
    let new_name = |id: &str| {
        if id == participant_id {
            Some(name)
        } else {
            old_name(id)
        }
    };

    let expenses = doc.get_map(EXPENSES);
    for e in group.expenses.iter().filter(|e| e.is_reimbursement) {
        let [to] = e.splits.as_slice() else { continue };
        let (from, to) = (e.paid_by.as_str(), to.participant_id.as_str());
        if from != participant_id && to != participant_id {
            continue;
        }
        let (Some(old_from), Some(old_to), Some(new_from), Some(new_to)) =
            (old_name(from), old_name(to), new_name(from), new_name(to))
        else {
            continue;
        };
        let Some(notes) = e.title.strip_prefix(&payment_title(old_from, old_to)) else {
            continue;
        };
        if !notes.is_empty() && !notes.starts_with(" (") {
            continue;
        }
        if let Some(target) = child_map(&expenses, &e.id) {
            let title = format!("{}{notes}", payment_title(new_from, new_to));
            target.insert("title", title).map_err(doc_err)?;
        }
    }
    participant.insert("name", name).map_err(doc_err)
}

/// Who adds someone to a group.
pub enum AddedBy<'a> {
    /// A member, when the app knows who the user is in the group.
    Member(Option<&'a str>),
    /// The person themselves, joining.
    Themselves,
}

/// Adds a participant, noting when and by whom, and returns their id.
pub fn add_participant(doc: &LoroDoc, name: &str, by: AddedBy) -> Res<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Participant name cannot be empty".to_string());
    }
    let id = Uuid::new_v4().to_string();
    insert_participant(doc, &id, trimmed, false)?;
    let added = child_map(&doc.get_map(PARTICIPANTS), &id)
        .ok_or_else(|| "Participant not found".to_string())?;
    added
        .insert("added_at", timestamp(Utc::now()))
        .map_err(doc_err)?;
    let by = match by {
        AddedBy::Member(member) => member,
        AddedBy::Themselves => Some(id.as_str()),
    };
    if let Some(by) = by {
        added.insert("added_by", by).map_err(doc_err)?;
    }
    Ok(id)
}

/// Soft-deletes a participant, noting when and by which member (`by`, when the app knows who
/// the user is). Their past expenses and balance stay intact.
pub fn remove_participant(doc: &LoroDoc, participant_id: &str, by: Option<&str>) -> Res<()> {
    let p = child_map(&doc.get_map(PARTICIPANTS), participant_id)
        .ok_or_else(|| "Participant not found".to_string())?;
    p.insert("removed", true).map_err(doc_err)?;
    p.insert("removed_at", timestamp(Utc::now()))
        .map_err(doc_err)?;
    match by {
        Some(by) => p.insert("removed_by", by).map_err(doc_err),
        None => p.delete("removed_by").map_err(doc_err),
    }
}
