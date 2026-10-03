//! What the app's commands do, for both shells: the Tauri app wraps each function in a
//! `#[tauri::command]` (which is what generates `src/bindings.ts`), and the web build calls
//! them through `invoke`, by command name with JSON arguments, like Tauri's own IPC.
//!
//! Commands that only make sense natively (`native_features`, `share_text`, `save_download`)
//! stay in each shell.

use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::models::{
    AccountInfo, ExpensePayer, ExpenseSplit, Group, LoginLink, OriginalAmount, ParticipantBalance,
    PasswordStrength, SettlementTransfer, SignedIn, SyncInfo,
};
use crate::{csv_file, doc, engine, sync, AppState};

type Res<T> = Result<T, String>;

pub fn get_groups(state: &AppState) -> Vec<Group> {
    state.store().groups()
}

pub fn get_group(state: &AppState, group_id: &str) -> Res<Group> {
    state.store().group(group_id)
}

/// The first participant is the user.
pub fn create_group(
    state: &AppState,
    name: &str,
    currency: &str,
    participants: &[String],
) -> Res<Group> {
    sync::create_group(state, name, currency, participants)
}

/// Creates a group from a CSV file's text, in the format `export_group_csv` writes. The user
/// then says who they are in it.
pub fn import_group_csv(state: &AppState, name: &str, csv: &str) -> Res<Group> {
    sync::import_group(state, name, csv)
}

/// The group as a CSV file's text: a line per expense, a column per person.
pub fn export_group_csv(state: &AppState, group_id: &str) -> Res<String> {
    csv_file::export(&state.store().group(group_id)?)
}

/// The exchange rate to suggest for an expense paid in `from` on `date` (`YYYY-MM-DD`) in a
/// group counting in `to`, from the account's relay. `None` when it has none.
pub async fn suggest_exchange_rate(
    state: &AppState,
    from: &str,
    to: &str,
    date: Option<&str>,
) -> Res<Option<String>> {
    sync::suggested_rate(state, from, to, date).await
}

/// Deletes the group for every member. While someone still owes something it takes
/// everyone's agreement: this gives the user's, and returns the group still waiting for the
/// others. Returns nothing once the group is deleted.
pub fn delete_group(state: &AppState, group_id: &str) -> Res<Option<Group>> {
    sync::delete_group(state, group_id)
}

/// Refuses the deletion some members asked for, or takes the request back.
pub fn refuse_group_deletion(state: &AppState, group_id: &str) -> Res<Group> {
    state.mutate(group_id, doc::refuse_deletion)
}

/// Puts a group away for the user (it stays theirs, listed apart), or back.
pub fn set_group_archived(state: &AppState, group_id: &str, archived: bool) -> Res<AccountInfo> {
    sync::set_group_archived(state, group_id, archived)?;
    state.require_account_info()
}

/// Removes the group from the account, on all the user's devices. Other members keep it.
pub async fn leave_group(state: &AppState, group_id: &str) -> Res<()> {
    sync::leave_group(state, group_id).await
}

/// Sets the group's name, currency, description and picture (a `data:` URL). Amounts are not
/// converted.
pub fn update_group(
    state: &AppState,
    group_id: &str,
    name: &str,
    currency: &str,
    description: &str,
    image: Option<&str>,
) -> Res<Group> {
    state.mutate(group_id, |d| {
        doc::update_group(d, name, currency, description, image)
    })
}

pub fn add_participant(state: &AppState, group_id: &str, name: &str) -> Res<Group> {
    state.mutate(group_id, |d| doc::add_participant(d, name).map(|_| ()))
}

pub fn remove_participant(state: &AppState, group_id: &str, participant_id: &str) -> Res<Group> {
    state.mutate(group_id, |d| doc::remove_participant(d, participant_id))
}

pub fn rename_participant(
    state: &AppState,
    group_id: &str,
    participant_id: &str,
    name: &str,
) -> Res<Group> {
    state.mutate(group_id, |d| {
        doc::rename_participant(d, participant_id, name)
    })
}

#[allow(clippy::too_many_arguments)]
pub fn add_expense(
    state: &AppState,
    group_id: &str,
    title: &str,
    amount_cents: i64,
    paid_by: String,
    payers: Vec<ExpensePayer>,
    splits: Vec<ExpenseSplit>,
    created_at: Option<DateTime<Utc>>,
    original: Option<OriginalAmount>,
) -> Res<Group> {
    state.mutate(group_id, |d| {
        doc::add_expense(
            d,
            title,
            amount_cents,
            doc::PaidBy::new(paid_by, payers),
            splits,
            created_at,
            original,
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub fn update_expense(
    state: &AppState,
    group_id: &str,
    expense_id: &str,
    title: &str,
    amount_cents: i64,
    paid_by: String,
    payers: Vec<ExpensePayer>,
    splits: Vec<ExpenseSplit>,
    created_at: Option<DateTime<Utc>>,
    original: Option<OriginalAmount>,
) -> Res<Group> {
    state.mutate(group_id, |d| {
        doc::update_expense(
            d,
            expense_id,
            title,
            amount_cents,
            doc::PaidBy::new(paid_by, payers),
            splits,
            created_at,
            original,
        )
    })
}

pub fn delete_expense(state: &AppState, group_id: &str, expense_id: &str) -> Res<Group> {
    state.mutate(group_id, |d| doc::delete_expense(d, expense_id))
}

pub fn get_balances(state: &AppState, group_id: &str) -> Res<Vec<ParticipantBalance>> {
    Ok(engine::calculate_balances(&state.store().group(group_id)?))
}

pub fn get_settlements(state: &AppState, group_id: &str) -> Res<Vec<SettlementTransfer>> {
    Ok(engine::calculate_settlements(
        &state.store().group(group_id)?,
    ))
}

pub fn record_reimbursement(
    state: &AppState,
    group_id: &str,
    from_id: String,
    to_id: String,
    amount_cents: i64,
    notes: Option<String>,
) -> Res<Group> {
    state.mutate(group_id, |d| {
        doc::record_reimbursement(d, from_id, to_id, amount_cents, notes)
    })
}

pub fn get_storage_warnings(state: &AppState) -> Vec<String> {
    state.warnings.clone()
}

pub fn get_sync_info(state: &AppState, group_id: &str) -> Res<SyncInfo> {
    state.sync_info(group_id)
}

/// Syncs one group immediately. Failures are reported in the returned `last_error`.
pub async fn sync_now(state: &AppState, group_id: &str) -> Res<SyncInfo> {
    let _ = sync::sync_group(state, group_id).await;
    state.sync_info(group_id)
}

pub async fn join_group(state: &AppState, invite_code: &str) -> Res<Group> {
    sync::join_group(state, invite_code).await
}

pub fn get_account(state: &AppState) -> Res<Option<AccountInfo>> {
    state.account_info()
}

pub async fn sign_up(
    state: &AppState,
    server_url: &str,
    username: &str,
    password: &str,
) -> Res<SignedIn> {
    let recovery_key = sync::sign_up(state, server_url, username, password).await?;
    Ok(SignedIn {
        account: state.require_account_info()?,
        recovery_key,
    })
}

/// Sets a new password with the recovery key and logs in. Returns the replacement recovery
/// key: each one works once.
pub async fn recover_account(
    state: &AppState,
    server_url: &str,
    username: &str,
    recovery_key: &str,
    new_password: &str,
) -> Res<SignedIn> {
    let next =
        sync::recover_account(state, server_url, username, recovery_key, new_password).await?;
    Ok(SignedIn {
        account: state.require_account_info()?,
        recovery_key: Some(next),
    })
}

pub async fn change_password(
    state: &AppState,
    current_password: &str,
    new_password: &str,
) -> Res<()> {
    sync::change_password(state, current_password, new_password).await
}

/// A new recovery key, replacing the old one. Returned to show once.
pub async fn replace_recovery_key(state: &AppState, password: &str) -> Res<String> {
    sync::replace_recovery_key(state, password).await
}

pub async fn log_in(
    state: &AppState,
    server_url: &str,
    username: &str,
    password: &str,
) -> Res<AccountInfo> {
    sync::log_in(state, server_url, username, password).await?;
    state.require_account_info()
}

/// A link that logs another device into the account, once and for a short time.
pub async fn create_login_link(state: &AppState, password: &str) -> Res<LoginLink> {
    sync::create_login_link(state, password).await
}

/// Logs in with a link made by `create_login_link` on another device.
pub async fn log_in_with_link(state: &AppState, link: &str) -> Res<AccountInfo> {
    sync::log_in_with_link(state, link).await?;
    state.require_account_info()
}

/// Removes the account and its groups from this device. Fails while changes are not uploaded,
/// unless `force` is set.
pub async fn log_out(state: &AppState, force: bool) -> Res<()> {
    sync::log_out(state, force).await
}

/// Records which participant the user is in a group.
pub fn set_identity(state: &AppState, group_id: &str, participant_id: &str) -> Res<AccountInfo> {
    sync::set_identity(state, group_id, participant_id)?;
    state.require_account_info()
}

/// Sets the name and picture (a `data:` URL) the user shows, in every group where they said
/// who they are. An empty name keeps the names the groups have.
pub fn update_profile(state: &AppState, name: &str, avatar: Option<&str>) -> Res<AccountInfo> {
    sync::update_profile(state, name, avatar)?;
    state.require_account_info()
}

/// Adds the user to a group as a new participant.
pub fn add_self(state: &AppState, group_id: &str, name: &str) -> Res<Group> {
    sync::add_self(state, group_id, name)
}

/// How hard a password is to guess. Signing up requires `acceptable`.
pub fn password_strength(password: &str, username: &str) -> PasswordStrength {
    sync::password_strength(password, username)
}

// ---------------------------------------------------------------------------
// Calls by name
// ---------------------------------------------------------------------------

/// Runs `command` with `args`, a JSON object of its arguments in camelCase (as Tauri's IPC
/// and `src/bindings.ts` send them), and returns its result as JSON. An `Err` is the message
/// the interface shows, as with a failing Tauri command.
pub async fn invoke(state: &AppState, command: &str, args: &str) -> Res<String> {
    let args: Value = if args.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(args).map_err(|e| format!("Invalid arguments for {command}: {e}"))?
    };
    let s = |name: &str| arg::<String>(&args, name);
    match command {
        "get_groups" => json(get_groups(state)),
        "get_group" => json(get_group(state, &s("groupId")?)?),
        "create_group" => json(create_group(
            state,
            &s("name")?,
            &s("currency")?,
            &arg::<Vec<String>>(&args, "participants")?,
        )?),
        "import_group_csv" => json(import_group_csv(state, &s("name")?, &s("csv")?)?),
        "export_group_csv" => json(export_group_csv(state, &s("groupId")?)?),
        "suggest_exchange_rate" => json(
            suggest_exchange_rate(
                state,
                &s("from")?,
                &s("to")?,
                arg::<Option<String>>(&args, "date")?.as_deref(),
            )
            .await?,
        ),
        "leave_group" => json(leave_group(state, &s("groupId")?).await?),
        "delete_group" => json(delete_group(state, &s("groupId")?)?),
        "refuse_group_deletion" => json(refuse_group_deletion(state, &s("groupId")?)?),
        "set_group_archived" => json(set_group_archived(
            state,
            &s("groupId")?,
            arg(&args, "archived")?,
        )?),
        "update_group" => json(update_group(
            state,
            &s("groupId")?,
            &s("name")?,
            &s("currency")?,
            &s("description")?,
            arg::<Option<String>>(&args, "image")?.as_deref(),
        )?),
        "add_participant" => json(add_participant(state, &s("groupId")?, &s("name")?)?),
        "remove_participant" => json(remove_participant(
            state,
            &s("groupId")?,
            &s("participantId")?,
        )?),
        "rename_participant" => json(rename_participant(
            state,
            &s("groupId")?,
            &s("participantId")?,
            &s("name")?,
        )?),
        "add_expense" => json(add_expense(
            state,
            &s("groupId")?,
            &s("title")?,
            arg(&args, "amountCents")?,
            s("paidBy")?,
            arg::<Option<Vec<ExpensePayer>>>(&args, "payers")?.unwrap_or_default(),
            arg(&args, "splits")?,
            arg(&args, "createdAt")?,
            arg(&args, "original")?,
        )?),
        "update_expense" => json(update_expense(
            state,
            &s("groupId")?,
            &s("expenseId")?,
            &s("title")?,
            arg(&args, "amountCents")?,
            s("paidBy")?,
            arg::<Option<Vec<ExpensePayer>>>(&args, "payers")?.unwrap_or_default(),
            arg(&args, "splits")?,
            arg(&args, "createdAt")?,
            arg(&args, "original")?,
        )?),
        "delete_expense" => json(delete_expense(state, &s("groupId")?, &s("expenseId")?)?),
        "record_reimbursement" => json(record_reimbursement(
            state,
            &s("groupId")?,
            s("fromId")?,
            s("toId")?,
            arg(&args, "amountCents")?,
            arg(&args, "notes")?,
        )?),
        "get_balances" => json(get_balances(state, &s("groupId")?)?),
        "get_settlements" => json(get_settlements(state, &s("groupId")?)?),
        "get_storage_warnings" => json(get_storage_warnings(state)),
        "get_sync_info" => json(get_sync_info(state, &s("groupId")?)?),
        "sync_now" => json(sync_now(state, &s("groupId")?).await?),
        "join_group" => json(join_group(state, &s("inviteCode")?).await?),
        "get_account" => json(get_account(state)?),
        "sign_up" => {
            json(sign_up(state, &s("serverUrl")?, &s("username")?, &s("password")?).await?)
        }
        "recover_account" => json(
            recover_account(
                state,
                &s("serverUrl")?,
                &s("username")?,
                &s("recoveryKey")?,
                &s("newPassword")?,
            )
            .await?,
        ),
        "change_password" => {
            json(change_password(state, &s("currentPassword")?, &s("newPassword")?).await?)
        }
        "replace_recovery_key" => json(replace_recovery_key(state, &s("password")?).await?),
        "log_in" => json(log_in(state, &s("serverUrl")?, &s("username")?, &s("password")?).await?),
        "create_login_link" => json(create_login_link(state, &s("password")?).await?),
        "log_in_with_link" => json(log_in_with_link(state, &s("link")?).await?),
        "log_out" => json(log_out(state, arg(&args, "force")?).await?),
        "set_identity" => json(set_identity(state, &s("groupId")?, &s("participantId")?)?),
        "update_profile" => json(update_profile(
            state,
            &s("name")?,
            arg::<Option<String>>(&args, "avatar")?.as_deref(),
        )?),
        "add_self" => json(add_self(state, &s("groupId")?, &s("name")?)?),
        "password_strength" => json(password_strength(&s("password")?, &s("username")?)),
        _ => Err(format!("Unknown command {command}")),
    }
}

/// One argument; a missing one reads as `null`, which suits `Option`s.
fn arg<T: DeserializeOwned>(args: &Value, name: &str) -> Res<T> {
    let value = args.get(name).cloned().unwrap_or(Value::Null);
    serde_json::from_value(value).map_err(|e| format!("Invalid argument {name}: {e}"))
}

fn json(value: impl Serialize) -> Res<String> {
    serde_json::to_string(&value).map_err(|e| format!("Could not encode the result: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Store;

    fn state() -> (AppState, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("ezcount-api-{}", uuid::Uuid::new_v4()));
        let (store, _) =
            Store::open(&dir.join("db.sqlite3"), &dir.join("ezcount_data.json")).unwrap();
        (AppState::new(store, Vec::new()).unwrap(), dir)
    }

    /// The commands the frontend calls, from the generated bindings.
    fn bound_commands() -> Vec<String> {
        let bindings = include_str!("../../src/bindings.ts");
        bindings
            .split("TAURI_INVOKE(\"")
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            .map(str::to_string)
            .collect()
    }

    #[tokio::test]
    async fn every_bound_command_is_dispatched() {
        let (state, dir) = state();
        let native_only = ["native_features", "share_text", "save_download"];
        let commands = bound_commands();
        assert!(commands.len() > 20, "found {commands:?}");
        for command in commands
            .iter()
            .filter(|c| !native_only.contains(&c.as_str()))
        {
            // Missing arguments fail, but not as an unknown command.
            if let Err(e) = invoke(&state, command, "{}").await {
                assert!(!e.starts_with("Unknown command"), "{e}");
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn runs_commands_by_name() {
        let (state, dir) = state();
        // Creating groups needs an account; a local one is enough here.
        let names = ["Alice".to_string(), "Bob".to_string()];
        let doc = doc::new_group_doc("Trip", "EUR", &names).unwrap();
        let group = state.store().insert(doc, None).unwrap();
        assert_eq!(
            invoke(
                &state,
                "create_group",
                r#"{"name":"Trip","currency":"EUR"}"#
            )
            .await
            .unwrap_err(),
            "Invalid argument participants: invalid type: null, expected a sequence"
        );
        let alice = &group.participants[0].id;
        let bob = &group.participants[1].id;
        let args = serde_json::json!({
            "groupId": group.id,
            "title": "Dinner",
            "amountCents": 3000,
            "paidBy": alice,
            "splits": [{"participant_id": alice, "shares": 1}, {"participant_id": bob, "shares": 2}],
            "createdAt": null,
        });
        invoke(&state, "add_expense", &args.to_string())
            .await
            .unwrap();
        let balances: Vec<ParticipantBalance> = serde_json::from_str(
            &invoke(
                &state,
                "get_balances",
                &serde_json::json!({ "groupId": group.id }).to_string(),
            )
            .await
            .unwrap(),
        )
        .unwrap();
        let net = |id: &str| {
            balances
                .iter()
                .find(|b| b.participant_id == id)
                .unwrap()
                .net_cents
        };
        assert_eq!((net(alice), net(bob)), (2000, -2000));

        assert_eq!(
            invoke(&state, "get_group", r#"{"groupId":"nope"}"#)
                .await
                .unwrap_err(),
            get_group(&state, "nope").unwrap_err()
        );
        assert_eq!(
            invoke(&state, "get_group", "{}").await.unwrap_err(),
            "Invalid argument groupId: invalid type: null, expected a string"
        );
        assert_eq!(
            invoke(&state, "nope", "{}").await.unwrap_err(),
            "Unknown command nope"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
