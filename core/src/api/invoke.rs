//! The commands by name, with JSON arguments and a JSON answer, as Tauri's own IPC takes
//! them: how the web version calls the core.

use super::*;

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
        "send_feedback" => json(
            send_feedback(
                state,
                &s("message")?,
                arg::<Option<String>>(&args, "contact")?.as_deref(),
                arg::<Option<String>>(&args, "app")?.as_deref(),
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
        "add_expense" => json(add_expense(state, &s("groupId")?, arg(&args, "expense")?)?),
        "update_expense" => json(update_expense(
            state,
            &s("groupId")?,
            &s("expenseId")?,
            arg(&args, "expense")?,
        )?),
        "delete_expense" => json(delete_expense(state, &s("groupId")?, &s("expenseId")?)?),
        "restore_expense" => json(restore_expense(state, &s("groupId")?, &s("expenseId")?)?),
        "purge_expense" => json(purge_expense(state, &s("groupId")?, &s("expenseId")?)?),
        "add_expense_comment" => json(add_expense_comment(
            state,
            &s("groupId")?,
            &s("expenseId")?,
            &s("text")?,
        )?),
        "delete_expense_comment" => json(delete_expense_comment(
            state,
            &s("groupId")?,
            &s("commentId")?,
        )?),
        "stop_recurring_expense" => json(stop_recurring_expense(
            state,
            &s("groupId")?,
            &s("recurringId")?,
        )?),
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
        "receive_link" => json(receive_link(&s("serverUrl")?, &s("purpose")?)?),
        "receive" => json(receive(state, &s("link")?).await?),
        "send_login" => json(send_login(state, &s("link")?, &s("password")?).await?),
        "send_group_invite" => json(send_group_invite(state, &s("groupId")?, &s("link")?).await?),
        "log_out" => json(log_out(state, arg(&args, "force")?).await?),
        "set_identity" => json(set_identity(state, &s("groupId")?, &s("participantId")?)?),
        "update_profile" => json(update_profile(
            state,
            &s("name")?,
            arg::<Option<String>>(&args, "avatar")?.as_deref(),
            arg::<Option<String>>(&args, "iban")?.as_deref(),
        )?),
        "add_self" => json(add_self(state, &s("groupId")?, &s("name")?)?),
        "password_strength" => json(password_strength(&s("password")?, &s("username")?)),
        _ => Err(format!("Unknown command {command}")),
    }
}

/// One argument; a missing one reads as `null`, which suits `Option`s.
pub(super) fn arg<T: DeserializeOwned>(args: &Value, name: &str) -> Res<T> {
    let value = args.get(name).cloned().unwrap_or(Value::Null);
    serde_json::from_value(value).map_err(|e| format!("Invalid argument {name}: {e}"))
}

pub(super) fn json(value: impl Serialize) -> Res<String> {
    serde_json::to_string(&value).map_err(|e| format!("Could not encode the result: {e}"))
}
