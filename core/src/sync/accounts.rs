//! Accounts: signing up, logging in and out, and changing or recovering the password.

use super::*;

#[derive(Serialize)]
pub(super) struct SignUpRequest<'a> {
    pub(super) username: &'a str,
    pub(super) login_token: &'a str,
    pub(super) wrapped_key: String,
    pub(super) account_id: &'a str,
    pub(super) doc_token: &'a str,
    pub(super) recovery_token: &'a str,
    pub(super) recovery_wrapped_key: String,
}

#[derive(Deserialize, Default)]
pub(super) struct SignUpResponse {
    /// Relays before recovery keys answer with an empty body, and ignore the key.
    #[serde(default)]
    pub(super) recovery: bool,
}

#[derive(Serialize)]
pub(super) struct LogInRequest<'a> {
    pub(super) username: &'a str,
    pub(super) login_token: &'a str,
}

#[derive(Deserialize)]
pub(super) struct LogInResponse {
    pub(super) account_id: String,
    pub(super) wrapped_key: String,
}

#[derive(Serialize)]
pub(super) struct RecoverRequest<'a> {
    pub(super) username: &'a str,
    pub(super) recovery_token: &'a str,
}

/// See `POST /v1/accounts/credentials` in the relay: one proof, one or two replacements.
#[derive(Serialize, Default)]
pub(super) struct CredentialsRequest<'a> {
    pub(super) username: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) login_token: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) recovery_token: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) new_login_token: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) new_wrapped_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) new_recovery_token: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) new_recovery_wrapped_key: Option<String>,
}

/// Creates an account on `server_url` and logs this device into it. Groups already on the
/// device become part of the account.
pub async fn sign_up(
    state: &AppState,
    server_url: &str,
    username: &str,
    password: &str,
) -> Res<Option<String>> {
    let server_url = normalize_server_url(server_url)?;
    let username = normalize_username(username)?;
    check_password(password, &username)?;
    ensure_logged_out(state)?;

    let keys = password_keys(&username, password).await?;
    let account_id = uuid::Uuid::new_v4().to_string();
    let account_key = new_secret()?;
    let doc_keys = GroupKeys::derive(&account_key)?;
    let recovery_key = new_recovery_key()?;
    let recovery = CredentialKeys::from_recovery_key(&recovery_key)?;

    let response = state
        .http
        .post(format!("{server_url}/v1/accounts"))
        .json(&SignUpRequest {
            username: &username,
            login_token: &keys.token,
            wrapped_key: STANDARD.encode(keys.wrap_account_key(&account_id, &account_key)?),
            account_id: &account_id,
            doc_token: &doc_keys.auth_token,
            recovery_token: &recovery.token,
            recovery_wrapped_key: STANDARD
                .encode(recovery.wrap_account_key(&account_id, &account_key)?),
        })
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(request_err)?;
    let created: SignUpResponse = match response.status().as_u16() {
        409 => return Err(format!("The username \"{username}\" is already taken")),
        404 => return Err(NO_ACCOUNTS.to_string()),
        _ => {
            let body = check_status(response)
                .await?
                .text()
                .await
                .unwrap_or_default();
            serde_json::from_str(&body).unwrap_or_default()
        }
    };

    let session = Session {
        server_url: server_url.clone(),
        username,
        account_id,
    };
    state.store().set_session(
        session,
        LoroDoc::new(),
        SyncMeta::new(server_url, account_key),
    )?;
    adopt_local_groups(state)?;
    // Upload the new account right away; the background loop retries if this fails.
    let _ = sync_account(state).await;
    // Only a relay that stored it makes the recovery key worth showing.
    Ok(created.recovery.then_some(recovery_key))
}

/// Sends a `POST /v1/accounts/credentials`. `wrong_proof` is the message for a rejected one.
pub(super) async fn update_credentials(
    state: &AppState,
    server_url: &str,
    request: &CredentialsRequest<'_>,
    wrong_proof: &str,
) -> Res<()> {
    let response = state
        .http
        .post(format!("{server_url}/v1/accounts/credentials"))
        .json(request)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(request_err)?;
    match response.status().as_u16() {
        401 => Err(wrong_proof.to_string()),
        404 => Err(OLD_RELAY.to_string()),
        429 => Err(TOO_MANY_ATTEMPTS.to_string()),
        _ => check_status(response).await.map(|_| ()),
    }
}

pub(super) const OLD_RELAY: &str =
    "This server can't change passwords or recovery keys yet. Update the ezcount relay.";

pub(super) const TOO_MANY_ATTEMPTS: &str =
    "Too many failed attempts. Wait a few minutes and try again.";

/// The logged-in account's username, relay, id and key.
pub(super) fn account_credentials(state: &AppState) -> Res<(String, String, String, Secret)> {
    let store = state.store();
    let session = store
        .session()
        .ok_or_else(|| "You are not logged in".to_string())?;
    let key = store
        .sync_meta(&session.account_id)
        .map(|meta| meta.secret.clone())
        .ok_or_else(|| "This device has no key for your account".to_string())?;
    Ok((
        session.username.clone(),
        session.server_url.clone(),
        session.account_id.clone(),
        key,
    ))
}

/// Changes the password. Other devices stay logged in: the account key doesn't change, only
/// the copy of it the password unlocks.
pub async fn change_password(state: &AppState, current: &str, new: &str) -> Res<()> {
    let (username, server_url, account_id, account_key) = account_credentials(state)?;
    check_password(new, &username)?;
    let old = password_keys(&username, current).await?;
    let keys = password_keys(&username, new).await?;
    let request = CredentialsRequest {
        username: &username,
        login_token: Some(&old.token),
        new_login_token: Some(&keys.token),
        new_wrapped_key: Some(STANDARD.encode(keys.wrap_account_key(&account_id, &account_key)?)),
        ..Default::default()
    };
    update_credentials(
        state,
        &server_url,
        &request,
        "Your current password is wrong",
    )
    .await
}

/// Replaces the recovery key, or gives an account created before recovery keys its first one.
/// The old key stops working. Returns the new one, to show once.
pub async fn replace_recovery_key(state: &AppState, password: &str) -> Res<String> {
    let (username, server_url, account_id, account_key) = account_credentials(state)?;
    let keys = password_keys(&username, password).await?;
    let recovery_key = new_recovery_key()?;
    let recovery = CredentialKeys::from_recovery_key(&recovery_key)?;
    let request = CredentialsRequest {
        username: &username,
        login_token: Some(&keys.token),
        new_recovery_token: Some(&recovery.token),
        new_recovery_wrapped_key: Some(
            STANDARD.encode(recovery.wrap_account_key(&account_id, &account_key)?),
        ),
        ..Default::default()
    };
    update_credentials(state, &server_url, &request, "Wrong password").await?;
    Ok(recovery_key)
}

/// Sets a new password with the recovery key, for a forgotten password, and logs this device
/// in. A recovery key works once: returns its replacement, to show once.
pub async fn recover_account(
    state: &AppState,
    server_url: &str,
    username: &str,
    recovery_key: &str,
    new_password: &str,
) -> Res<String> {
    let wrong = "Wrong username or recovery key";
    let server_url = normalize_server_url(server_url)?;
    let username = normalize_username(username).map_err(|_| wrong.to_string())?;
    let recovery = CredentialKeys::from_recovery_key(recovery_key)?;
    check_password(new_password, &username)?;
    ensure_logged_out(state)?;

    let response = state
        .http
        .post(format!("{server_url}/v1/accounts/recover"))
        .json(&RecoverRequest {
            username: &username,
            recovery_token: &recovery.token,
        })
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(request_err)?;
    let found: LogInResponse = match response.status().as_u16() {
        401 => return Err(wrong.to_string()),
        404 => return Err(OLD_RELAY.to_string()),
        429 => return Err(TOO_MANY_ATTEMPTS.to_string()),
        _ => check_status(response)
            .await?
            .json()
            .await
            .map_err(|e| format!("The sync server sent an unexpected response: {e}"))?,
    };
    let wrapped = STANDARD
        .decode(&found.wrapped_key)
        .map_err(|_| "The sync server sent corrupt account data".to_string())?;
    let account_key = recovery.unwrap_account_key(&found.account_id, &wrapped)?;

    let keys = password_keys(&username, new_password).await?;
    let next_key = new_recovery_key()?;
    let next = CredentialKeys::from_recovery_key(&next_key)?;
    let request = CredentialsRequest {
        username: &username,
        recovery_token: Some(&recovery.token),
        new_login_token: Some(&keys.token),
        new_wrapped_key: Some(
            STANDARD.encode(keys.wrap_account_key(&found.account_id, &account_key)?),
        ),
        new_recovery_token: Some(&next.token),
        new_recovery_wrapped_key: Some(
            STANDARD.encode(next.wrap_account_key(&found.account_id, &account_key)?),
        ),
        ..Default::default()
    };
    update_credentials(state, &server_url, &request, wrong).await?;

    // The new password is set; if this download fails, logging in with it finishes the job.
    let (doc, meta) = download(&state.http, &server_url, &account_key, &found.account_id).await?;
    let session = Session {
        server_url,
        username,
        account_id: found.account_id,
    };
    state.store().set_session(session, doc, meta)?;
    adopt_local_groups(state)?;
    state.sync_wakeup.notify_one();
    Ok(next_key)
}

/// Logs this device into an existing account. Its groups download in the background.
pub async fn log_in(state: &AppState, server_url: &str, username: &str, password: &str) -> Res<()> {
    let server_url = normalize_server_url(server_url)?;
    let username =
        normalize_username(username).map_err(|_| "Wrong username or password".to_string())?;
    ensure_logged_out(state)?;

    let keys = password_keys(&username, password).await?;
    let response = state
        .http
        .post(format!("{server_url}/v1/accounts/login"))
        .json(&LogInRequest {
            username: &username,
            login_token: &keys.token,
        })
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(request_err)?;
    let found: LogInResponse = match response.status().as_u16() {
        401 => return Err("Wrong username or password".to_string()),
        404 => return Err(NO_ACCOUNTS.to_string()),
        429 => return Err(TOO_MANY_ATTEMPTS.to_string()),
        _ => check_status(response)
            .await?
            .json()
            .await
            .map_err(|e| format!("The sync server sent an unexpected response: {e}"))?,
    };
    let wrapped = STANDARD
        .decode(&found.wrapped_key)
        .map_err(|_| "The sync server sent corrupt account data".to_string())?;
    let account_key = keys.unwrap_account_key(&found.account_id, &wrapped)?;

    // Download the account before saving anything, so a failure leaves the device as it was.
    let (doc, meta) = download(&state.http, &server_url, &account_key, &found.account_id).await?;
    let session = Session {
        server_url,
        username,
        account_id: found.account_id,
    };
    state.store().set_session(session, doc, meta)?;
    adopt_local_groups(state)?;
    state.sync_wakeup.notify_one();
    Ok(())
}

/// Logs out and removes the account's data from this device. Refuses while edits are not
/// uploaded yet, unless `force` is set.
pub async fn log_out(state: &AppState, force: bool) -> Res<()> {
    if !force {
        let _ = sync_account(state).await;
        let ids = state.store().synced_ids();
        for id in ids {
            let _ = sync_group(state, &id).await;
        }
        if state.store().has_unpushed_changes() {
            return Err(
                "Some changes on this device are not uploaded yet. Connect to the internet and \
                 try again, or log out anyway and lose them."
                    .to_string(),
            );
        }
    }
    let _guard = state.sync_lock.lock().await;
    state.store().wipe()
}

pub(super) fn ensure_logged_out(state: &AppState) -> Res<()> {
    if state.store().session().is_some() {
        return Err("This device is already logged in".to_string());
    }
    Ok(())
}

pub(super) fn require_session(state: &AppState) -> Res<Session> {
    state
        .store()
        .session()
        .cloned()
        .ok_or_else(|| "Log in first".to_string())
}

/// Adds the groups on this device to the account, sharing the ones that weren't yet.
pub(super) fn adopt_local_groups(state: &AppState) -> Res<()> {
    let mut store = state.store();
    let server_url = store
        .session()
        .map(|s| s.server_url.clone())
        .ok_or_else(|| "Log in first".to_string())?;
    let listed: Vec<String> = account::groups(store.account_doc()?)?
        .into_iter()
        .map(|g| g.group_id)
        .collect();
    for id in store.group_ids() {
        if listed.contains(&id) {
            continue;
        }
        if store.sync_meta(&id).is_none() {
            store.set_sync(&id, SyncMeta::new(server_url.clone(), new_secret()?))?;
        }
        let meta = store.sync_meta(&id).cloned().expect("just set");
        store.update_account(|d| account::add_group(d, &id, &meta.server_url, &meta.secret))?;
    }
    state.sync_wakeup.notify_one();
    Ok(())
}
