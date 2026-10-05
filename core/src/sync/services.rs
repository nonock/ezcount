//! What else the relay does for the app: exchange rates and messages to whoever runs it.

use super::*;

#[derive(Deserialize)]
pub(super) struct RateResponse {
    pub(super) rate: String,
}

/// The exchange rate the account's relay suggests from one currency to another on a day
/// (`YYYY-MM-DD`): how much of `to` one unit of `from` is worth. `None` when it has no
/// suggestion, as on a relay from before them.
pub async fn suggested_rate(
    state: &AppState,
    from: &str,
    to: &str,
    date: Option<&str>,
) -> Res<Option<String>> {
    let session = require_session(state)?;
    // They go into the address.
    let code = |c: &str| c.len() == 3 && c.bytes().all(|b| b.is_ascii_alphabetic());
    if !code(from) || !code(to) {
        return Ok(None);
    }
    let mut url = format!("{}/v1/rates/{from}/{to}", session.server_url);
    if let Some(date) = date.filter(|d| d.bytes().all(|b| b.is_ascii_digit() || b == b'-')) {
        url.push_str(&format!("?date={date}"));
    }
    let response = state.http.get(url).send().await.map_err(request_err)?;
    match response.status().as_u16() {
        200 => {}
        404 => return Ok(None),
        status => return Err(format!("The sync server answered {status} for the rate")),
    }
    let answer: RateResponse = response.json().await.map_err(request_err)?;
    // Only what an expense accepts.
    Ok(doc::exchange_rate(&answer.rate).ok())
}

/// Longest message the feedback form sends, in characters.
pub const MAX_FEEDBACK_CHARS: usize = 2000;

/// Sends an idea or a problem to whoever runs the account's relay, with how to answer
/// (`contact`) when the user wants an answer, and which app it comes from.
pub async fn send_feedback(
    state: &AppState,
    message: &str,
    contact: Option<&str>,
    app: Option<&str>,
) -> Res<()> {
    let session = require_session(state)?;
    let message = message.trim();
    if message.is_empty() {
        return Err("Write a message first".to_string());
    }
    if message.chars().count() > MAX_FEEDBACK_CHARS {
        return Err(format!(
            "This message is too long ({MAX_FEEDBACK_CHARS} characters at most)"
        ));
    }
    let short = |text: Option<&str>| {
        text.map(|t| t.trim().chars().take(200).collect::<String>())
            .filter(|t| !t.is_empty())
    };
    let body = serde_json::json!({
        "message": message,
        "contact": short(contact),
        "app": short(app),
    });
    let response = state
        .http
        .post(format!("{}/v1/feedback", session.server_url))
        .json(&body)
        .send()
        .await
        .map_err(request_err)?;
    match response.status().as_u16() {
        200..=299 => Ok(()),
        // A relay from before the form.
        404 | 405 => Err("This sync server doesn't take messages yet".to_string()),
        429 => Err("Too many messages were sent from your network: try again later".to_string()),
        status => Err(format!("The sync server answered {status} to the message")),
    }
}
