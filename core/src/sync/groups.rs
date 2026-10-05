//! The groups of an account: making, joining, leaving and deleting them, and who the user
//! is in each.

use super::*;

/// Creates a group shared through the account's relay. The first participant is the user.
pub fn create_group(
    state: &AppState,
    name: &str,
    currency: &str,
    participants: &[String],
) -> Res<Group> {
    require_session(state)?;
    let doc = doc::new_group_doc(name, currency, participants)?;
    let me = doc::read_group(&doc)?
        .participants
        .first()
        .ok_or_else(|| "Add yourself to the group".to_string())?
        .id
        .clone();
    add_new_group(state, doc, Some(&me))
}

/// Creates a group from a CSV file (see `csv_file`). Who the user is in it isn't known yet.
pub fn import_group(state: &AppState, name: &str, csv: &str) -> Res<Group> {
    require_session(state)?;
    let imported = csv_file::import(csv)?;
    let doc = doc::imported_group_doc(
        name,
        &imported.currency,
        &imported.participants,
        &imported.expenses,
    )?;
    add_new_group(state, doc, None)
}

/// Adds a group made on this device to the account, with `me` as the user in it.
pub(super) fn add_new_group(state: &AppState, doc: LoroDoc, me: Option<&str>) -> Res<Group> {
    let session = require_session(state)?;
    let group = doc::read_group(&doc)?;
    let meta = SyncMeta::new(session.server_url, new_secret()?);

    // Account first: if the app stops in between, the group is fetched again from the
    // account instead of being dropped as "left on another device".
    let mut store = state.store();
    store.update_account(|d| {
        account::add_group(d, &group.id, &meta.server_url, &meta.secret)?;
        match me {
            Some(me) => account::set_identity(d, &group.id, me),
            None => Ok(()),
        }
    })?;
    let mut inserted = store.insert(doc, Some(meta));
    if inserted.is_err() {
        // Best effort: the error reported is the insert's, and a group left in the account
        // is only fetched again.
        let _ = store.update_account(|d| account::remove_group(d, &group.id));
    } else if let Some(me) = me {
        let profile = account::profile(store.account_doc()?);
        // The name typed for the group is kept: only the picture comes from the profile.
        let picture = profile.avatar.as_deref();
        if let Ok(with_picture) =
            store.update(&group.id, |d| doc::set_participant_avatar(d, me, picture))
        {
            inserted = Ok(with_picture);
        }
    }
    drop(store);
    state.sync_wakeup.notify_one();
    inserted
}

/// Downloads a shared group from its invite code and adds it to the account.
pub async fn join_group(state: &AppState, code: &str) -> Res<Group> {
    let invite = parse_invite(code)?;
    require_session(state)?;
    let already = || "This group is already in your account".to_string();
    if state.store().contains(&invite.group_id) {
        return Err(already());
    }

    let (doc, meta) = download(
        &state.http,
        &invite.server_url,
        &invite.secret,
        &invite.group_id,
    )
    .await?;
    let group = doc::read_group(&doc)
        .map_err(|_| "The sync server has no usable data for this group".to_string())?;
    if group.deleted {
        return Err("This group was deleted".to_string());
    }
    if group.id != invite.group_id {
        return Err("The invite code does not match the group on the server".to_string());
    }

    let mut store = state.store();
    if store.contains(&invite.group_id) {
        return Err(already());
    }
    store.update_account(|d| {
        account::add_group(d, &invite.group_id, &invite.server_url, &invite.secret)
    })?;
    let group = store.insert(doc, Some(meta))?;
    drop(store);
    state.sync_wakeup.notify_one();
    Ok(group)
}

/// Removes a group from the account, on every device. Other members keep it.
pub async fn leave_group(state: &AppState, group_id: &str) -> Res<()> {
    // Hand over this device's last edits to the other members first, if possible.
    if state.store().sync_meta(group_id).is_some() {
        let _ = sync_group(state, group_id).await;
    }
    let mut store = state.store();
    if store.session().is_some() {
        store.update_account(|d| account::remove_group(d, group_id))?;
    }
    store.delete(group_id)?;
    drop(store);
    state.sync_wakeup.notify_one();
    Ok(())
}

/// Deletes a group for everyone, or asks to: with balances that aren't settled it takes every
/// member's agreement, and this gives the user's. Returns the group while it waits for the
/// others, and nothing once it is deleted.
pub fn delete_group(state: &AppState, group_id: &str) -> Res<Option<Group>> {
    require_session(state)?;
    let mut store = state.store();
    let me = account::identities(store.account_doc()?)?.remove(group_id);
    let mut deleted = false;
    let group = store.update(group_id, |d| {
        deleted = doc::delete_or_vote(d, me.as_deref())?;
        Ok(())
    })?;
    drop(store);
    // The next sync tells the others, then drops the group here (`drop_deleted`).
    state.sync_wakeup.notify_one();
    Ok((!deleted).then_some(group))
}

/// Puts a group away for the user, on all their devices, or back among the others.
pub fn set_group_archived(state: &AppState, group_id: &str, archived: bool) -> Res<()> {
    let mut store = state.store();
    store.doc(group_id)?;
    store.update_account(|d| account::set_archived(d, group_id, archived))?;
    drop(store);
    state.sync_wakeup.notify_one();
    Ok(())
}

/// Drops a group that was deleted for everyone, once this device has nothing left to upload
/// for it (its own deletion included). Returns whether it did.
pub(super) fn drop_deleted(state: &AppState, group_id: &str) -> bool {
    let mut store = state.store();
    if !store.group(group_id).is_ok_and(|g| g.deleted) || store.has_unpushed(group_id) {
        return false;
    }
    // Out of the account first: left in it, the group would be downloaded again. A failure
    // leaves both as they are for the next pass.
    if store.session().is_some()
        && store
            .update_account(|d| account::remove_group(d, group_id))
            .is_err()
    {
        return false;
    }
    store.delete(group_id).is_ok()
}

/// Records which participant the user is in a group, for all their devices.
pub fn set_identity(state: &AppState, group_id: &str, participant_id: &str) -> Res<()> {
    let mut store = state.store();
    let group = store.group(group_id)?;
    if !group
        .participants
        .iter()
        .any(|p| p.id == participant_id && !p.removed)
    {
        return Err("This person is not a member of the group".to_string());
    }
    let previous = account::identities(store.account_doc()?)?.remove(group_id);
    store.update_account(|d| account::set_identity(d, group_id, participant_id))?;
    // The profile follows the user: off the member they said they were before, onto this one.
    if let Some(previous) = previous.filter(|previous| previous != participant_id) {
        let _ = store.update(group_id, |d| {
            doc::set_participant_avatar(d, &previous, None)?;
            doc::set_participant_iban(d, &previous, None)
        });
    }
    let profile = account::profile(store.account_doc()?);
    let _ = show_profile(&mut store, group_id, participant_id, &profile);
    drop(store);
    state.sync_wakeup.notify_one();
    Ok(())
}

/// Gives a member of a group the profile's name, picture and bank account. Without a name in
/// the profile, the member keeps theirs.
pub(super) fn show_profile(
    store: &mut Store,
    group_id: &str,
    participant_id: &str,
    profile: &account::Profile,
) -> Res<()> {
    store
        .update(group_id, |d| {
            if let Some(name) = &profile.name {
                doc::rename_participant(d, participant_id, name)?;
            }
            doc::set_participant_avatar(d, participant_id, profile.avatar.as_deref())?;
            doc::set_participant_iban(d, participant_id, profile.iban.as_deref())
        })
        .map(|_| ())
}

/// Sets the name, picture and bank account the user shows, and gives them to the member they
/// are in each of their groups, for the other members to see.
pub fn update_profile(
    state: &AppState,
    name: &str,
    avatar: Option<&str>,
    iban: Option<&str>,
) -> Res<()> {
    require_session(state)?;
    let mut store = state.store();
    store.update_account(|d| account::set_profile(d, name, avatar, iban))?;
    let profile = account::profile(store.account_doc()?);
    for (group_id, participant_id) in account::identities(store.account_doc()?)? {
        // A group that is gone, or a member that was removed, doesn't keep the others from
        // getting it.
        let _ = show_profile(&mut store, &group_id, &participant_id, &profile);
    }
    drop(store);
    state.sync_wakeup.notify_one();
    Ok(())
}

/// Adds the user to a group as a new participant.
pub fn add_self(state: &AppState, group_id: &str, name: &str) -> Res<Group> {
    require_session(state)?;
    let mut new_id = String::new();
    state.mutate(group_id, |d| {
        new_id = doc::add_participant(d, name, doc::AddedBy::Themselves)?;
        Ok(())
    })?;
    set_identity(state, group_id, &new_id)?;
    state.store().group(group_id)
}
