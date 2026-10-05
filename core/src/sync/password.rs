//! Usernames and passwords: what is accepted, and the keys a password gives.

use super::*;

pub(super) const MIN_PASSWORD_LEN: usize = 8;

/// Lowercases and checks a username: 3 to 32 of `a-z 0-9 . _ -`, starting with a letter or
/// digit. The relay applies the same rule.
pub fn normalize_username(raw: &str) -> Res<String> {
    let username = raw.trim().to_lowercase();
    let bytes = username.as_bytes();
    let valid = (3..=32).contains(&bytes.len())
        && bytes[0].is_ascii_alphanumeric()
        && bytes.iter().all(|&b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        });
    if valid {
        Ok(username)
    } else {
        Err("Usernames are 3 to 32 letters, digits, dots, dashes or underscores".to_string())
    }
}

/// Lowest zxcvbn score accepted at sign-up. 3 is "safely unguessable": over 10^8 guesses,
/// which matters because a stolen relay database lets an attacker guess offline.
pub(super) const MIN_PASSWORD_SCORE: u8 = 3;

/// How hard `password` is to guess, with the username and the app's name counted as known
/// to an attacker. zxcvbn looks at the first 100 characters only, so this stays fast.
pub fn password_strength(password: &str, username: &str) -> PasswordStrength {
    let username = username.trim().to_lowercase();
    let entropy = zxcvbn::zxcvbn(password, &[&username, "ezcount"]);
    let score = u8::from(entropy.score());
    let feedback = entropy.feedback();
    PasswordStrength {
        score,
        acceptable: password.chars().count() >= MIN_PASSWORD_LEN && score >= MIN_PASSWORD_SCORE,
        warning: feedback.and_then(|f| f.warning()).map(|w| w.to_string()),
        suggestions: feedback
            .map(|f| f.suggestions().iter().map(ToString::to_string).collect())
            .unwrap_or_default(),
    }
}

pub(super) fn check_password(password: &str, username: &str) -> Res<()> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(format!(
            "Use a password of at least {MIN_PASSWORD_LEN} characters"
        ));
    }
    let strength = password_strength(password, username);
    if !strength.acceptable {
        let why = strength
            .warning
            .map(|w| format!(" {w}"))
            .unwrap_or_default();
        return Err(format!(
            "This password is too easy to guess.{why} Try a few unrelated words."
        ));
    }
    Ok(())
}

/// Argon2 takes a noticeable fraction of a second, so it runs on a blocking thread.
#[cfg(not(target_family = "wasm"))]
pub(super) async fn password_keys(username: &str, password: &str) -> Res<CredentialKeys> {
    let (username, password) = (username.to_string(), password.to_string());
    tokio::task::spawn_blocking(move || CredentialKeys::from_password(&username, &password))
        .await
        .map_err(|e| format!("Could not derive keys from the password: {e}"))?
}

/// The browser has no blocking threads, but the core runs in a Web Worker there, so hashing
/// doesn't freeze the page.
#[cfg(target_family = "wasm")]
pub(super) async fn password_keys(username: &str, password: &str) -> Res<CredentialKeys> {
    CredentialKeys::from_password(username, password)
}
