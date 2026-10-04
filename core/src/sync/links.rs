//! Logging a device in, or handing it a group, with a code one device shows and another scans.

use super::*;

// ---------------------------------------------------------------------------
// Login links
// ---------------------------------------------------------------------------

/// What a login link hands to the other device, encrypted with the link's code.
#[derive(Serialize, Deserialize)]
struct LinkedAccount {
    username: String,
    account_id: String,
    account_key: String,
}

#[derive(Serialize)]
struct CreateLinkRequest<'a> {
    username: &'a str,
    login_token: &'a str,
    ticket: &'a str,
    data: String,
}

#[derive(Deserialize)]
struct CreateLinkResponse {
    expires_in: u32,
}

#[derive(Serialize)]
struct ClaimLinkRequest<'a> {
    ticket: &'a str,
}

#[derive(Deserialize)]
struct ClaimLinkResponse {
    data: String,
}

/// A relay from before login links has no such endpoints.
const NO_LINKS: &str =
    "This server can't connect devices with a code yet. Update the ezcount relay.";

/// A login link: `ezcount://login?server=<relay>&code=<code>`. The code never reaches the
/// relay, only the device that scans the link.
fn login_link(server_url: &str, code: &Secret) -> String {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("server", server_url)
        .append_pair("code", code.expose())
        .finish();
    format!("ezcount://login?{query}")
}

/// The relay and the code of a login link.
pub(super) fn parse_login_link(link: &str) -> Res<(String, Secret)> {
    let invalid = || "This is not an ezcount login code".to_string();
    let url = Url::parse(link.trim()).map_err(|_| invalid())?;
    if url.scheme() != "ezcount" || url.host_str() != Some("login") {
        return Err(invalid());
    }
    let param = |name: &str| {
        url.query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
            .ok_or_else(invalid)
    };
    Ok((
        normalize_server_url(&param("server")?)?,
        Secret::new(param("code")?),
    ))
}

/// Makes a link that logs another device into this account, once and for a short time (the
/// relay says how long). It asks for the password, so an app left open isn't enough to take
/// the account elsewhere.
///
/// The account's key waits on the relay encrypted with the link's code, which only the link
/// carries: the relay can't read it, and forgets it when it is fetched or expires.
pub async fn create_login_link(state: &AppState, password: &str) -> Res<LoginLink> {
    let (username, server_url, account_id, account_key) = account_credentials(state)?;
    let keys = password_keys(&username, password).await?;
    let code = new_secret()?;
    let link_keys = LinkKeys::derive(&code)?;
    let account = serde_json::to_vec(&LinkedAccount {
        username: username.clone(),
        account_id,
        account_key: account_key.expose().to_string(),
    })
    .map_err(|e| format!("Could not encode the account: {e}"))?;

    let response = state
        .http
        .post(format!("{server_url}/v1/accounts/links"))
        .json(&CreateLinkRequest {
            username: &username,
            login_token: &keys.token,
            ticket: &link_keys.ticket,
            data: STANDARD.encode(link_keys.seal(&account)?),
        })
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(request_err)?;
    let created: CreateLinkResponse = match response.status().as_u16() {
        401 => return Err("Wrong password".to_string()),
        404 | 405 => return Err(NO_LINKS.to_string()),
        429 => return Err(TOO_MANY_ATTEMPTS.to_string()),
        _ => check_status(response)
            .await?
            .json()
            .await
            .map_err(|e| format!("The sync server sent an unexpected response: {e}"))?,
    };
    Ok(LoginLink {
        link: login_link(&server_url, &code),
        expires_in: created.expires_in,
    })
}

/// Logs this device into the account a login link is for (see [`create_login_link`]). Its
/// groups download in the background.
pub async fn log_in_with_link(state: &AppState, link: &str) -> Res<()> {
    let (server_url, code) = parse_login_link(link)?;
    let link_keys = LinkKeys::derive(&code)?;
    ensure_logged_out(state)?;

    let response = state
        .http
        .post(format!("{server_url}/v1/accounts/links/claim"))
        .json(&ClaimLinkRequest {
            ticket: &link_keys.ticket,
        })
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(request_err)?;
    let claimed: ClaimLinkResponse = match response.status().as_u16() {
        410 => {
            return Err(
                "This code has expired or was already used. Show a new one and scan it."
                    .to_string(),
            )
        }
        404 | 405 => return Err(NO_LINKS.to_string()),
        _ => check_status(response)
            .await?
            .json()
            .await
            .map_err(|e| format!("The sync server sent an unexpected response: {e}"))?,
    };
    let sealed = STANDARD
        .decode(&claimed.data)
        .map_err(|_| "The sync server sent corrupt account data".to_string())?;
    log_in_as(state, server_url, &link_keys.open(&sealed)?).await
}

/// Logs this device into the account another device handed over, as `LinkedAccount`.
async fn log_in_as(state: &AppState, server_url: String, account: &[u8]) -> Res<()> {
    let corrupt = || "The sync server sent corrupt account data".to_string();
    let account: LinkedAccount = serde_json::from_slice(account).map_err(|_| corrupt())?;
    let account_key = Secret::new(account.account_key);
    GroupKeys::derive(&account_key).map_err(|_| corrupt())?;

    // Download the account before saving anything, so a failure leaves the device as it was.
    let (doc, meta) = download(&state.http, &server_url, &account_key, &account.account_id).await?;
    let session = Session {
        server_url,
        username: account.username,
        account_id: account.account_id,
    };
    state.store().set_session(session, doc, meta)?;
    adopt_local_groups(state)?;
    state.sync_wakeup.notify_one();
    Ok(())
}

// ---------------------------------------------------------------------------
// Codes shown by the device that receives
// ---------------------------------------------------------------------------
//
// A computer can't scan a phone's QR code, so it shows one itself: `receive_link` makes it,
// a phone that is logged in scans it and leaves the account (`send_login`) or a group's
// invite (`send_group_invite`) on the relay, encrypted with the code, and the computer asks
// the relay until it is there (`receive`). The relay never sees the code.

/// A relay from before these codes has no such endpoints.
const NO_HANDOFFS: &str = "This sync server can't pass things between devices yet";

/// The link a device shows as a QR code to get something from a phone: `purpose` is
/// "login" on the login screen, "group" to join a group.
pub fn receive_link(server_url: &str, purpose: &str) -> Res<String> {
    if !matches!(purpose, "login" | "group") {
        return Err("This code can't be made".to_string());
    }
    let server_url = normalize_server_url(server_url)?;
    let code = new_secret()?;
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("server", &server_url)
        .append_pair("code", code.expose())
        .append_pair("for", purpose)
        .finish();
    Ok(format!("ezcount://receive?{query}"))
}

/// The relay, the code and the purpose of a link made by `receive_link`.
fn parse_receive_link(link: &str) -> Res<(String, Secret, String)> {
    let invalid = || "This is not a code shown by ezcount to receive something".to_string();
    let url = Url::parse(link.trim()).map_err(|_| invalid())?;
    if url.scheme() != "ezcount" || url.host_str() != Some("receive") {
        return Err(invalid());
    }
    let param = |name: &str| {
        url.query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
            .ok_or_else(invalid)
    };
    Ok((
        normalize_server_url(&param("server")?)?,
        Secret::new(param("code")?),
        param("for")?,
    ))
}

/// The scanned code of a device waiting for `purpose`, on this account's relay.
fn scanned_for(state: &AppState, link: &str, purpose: &str) -> Res<(Session, LinkKeys)> {
    let session = require_session(state)?;
    let (server_url, code, wanted) = parse_receive_link(link)?;
    if wanted != purpose {
        return Err(match wanted.as_str() {
            "login" => "This code is for logging in: scan it from Connect a device",
            _ => "This code is for joining a group: scan it from the group's Invite window",
        }
        .to_string());
    }
    if server_url != session.server_url {
        return Err("The other device uses another sync server than your account".to_string());
    }
    Ok((session, LinkKeys::derive(&code)?))
}

/// Logs the device that shows `link` into this account. It asks for the password, like
/// `create_login_link`.
pub async fn send_login(state: &AppState, link: &str, password: &str) -> Res<()> {
    let (_, link_keys) = scanned_for(state, link, "login")?;
    let (username, server_url, account_id, account_key) = account_credentials(state)?;
    let keys = password_keys(&username, password).await?;
    let account = serde_json::to_vec(&LinkedAccount {
        username: username.clone(),
        account_id,
        account_key: account_key.expose().to_string(),
    })
    .map_err(|e| format!("Could not encode the account: {e}"))?;
    let response = state
        .http
        .post(format!("{server_url}/v1/accounts/links"))
        .json(&CreateLinkRequest {
            username: &username,
            login_token: &keys.token,
            ticket: &link_keys.ticket,
            data: STANDARD.encode(link_keys.seal(&account)?),
        })
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(request_err)?;
    match response.status().as_u16() {
        401 => Err("Wrong password".to_string()),
        404 | 405 => Err(NO_LINKS.to_string()),
        429 => Err(TOO_MANY_ATTEMPTS.to_string()),
        _ => check_status(response).await.map(|_| ()),
    }
}

/// Gives a group's invite to the device that shows `link`, which then joins it.
pub async fn send_group_invite(state: &AppState, group_id: &str, link: &str) -> Res<()> {
    let (session, link_keys) = scanned_for(state, link, "group")?;
    let invite = state
        .sync_info(group_id)?
        .invite_code
        .ok_or_else(|| "This group is not shared yet".to_string())?;
    let response = state
        .http
        .post(format!("{}/v1/handoffs", session.server_url))
        .json(&ClaimedLink {
            ticket: &link_keys.ticket,
            data: Some(STANDARD.encode(link_keys.seal(invite.as_bytes())?)),
        })
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(request_err)?;
    match response.status().as_u16() {
        404 | 405 => Err(NO_HANDOFFS.to_string()),
        429 => Err(TOO_MANY_ATTEMPTS.to_string()),
        _ => check_status(response).await.map(|_| ()),
    }
}

#[derive(Serialize)]
struct ClaimedLink<'a> {
    ticket: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<String>,
}

/// Asks the relay once for what a phone left for the code this device shows. `None` while
/// no phone has scanned it; otherwise the device is logged in, or has joined the group.
pub async fn receive(state: &AppState, link: &str) -> Res<Option<Received>> {
    let (server_url, code, purpose) = parse_receive_link(link)?;
    let link_keys = LinkKeys::derive(&code)?;
    match purpose.as_str() {
        "login" => ensure_logged_out(state)?,
        _ => {
            require_session(state)?;
        }
    }
    let response = state
        .http
        .post(format!("{server_url}/v1/handoffs/claim"))
        .json(&ClaimedLink {
            ticket: &link_keys.ticket,
            data: None,
        })
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(request_err)?;
    let claimed: ClaimLinkResponse = match response.status().as_u16() {
        204 => return Ok(None),
        404 | 405 => return Err(NO_HANDOFFS.to_string()),
        429 => return Err(TOO_MANY_ATTEMPTS.to_string()),
        _ => check_status(response)
            .await?
            .json()
            .await
            .map_err(|e| format!("The sync server sent an unexpected response: {e}"))?,
    };
    let corrupt = || "The sync server sent corrupt data".to_string();
    let sealed = STANDARD.decode(&claimed.data).map_err(|_| corrupt())?;
    let opened = link_keys.open(&sealed).map_err(|_| corrupt())?;
    if purpose == "login" {
        log_in_as(state, server_url, &opened).await?;
        return Ok(Some(Received {
            account: Some(state.require_account_info()?),
            group: None,
        }));
    }
    let invite = String::from_utf8(opened).map_err(|_| corrupt())?;
    Ok(Some(Received {
        account: None,
        group: Some(join_group(state, &invite).await?),
    }))
}
