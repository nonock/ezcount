use super::*;
use crate::sync::new_secret;

#[test]
fn round_trips() {
    let keys = GroupKeys::derive(&new_secret().unwrap()).unwrap();
    let blob = keys.seal("g1", b"hello group").unwrap();
    assert_eq!(keys.open("g1", &blob).unwrap(), b"hello group");
    assert_ne!(
        keys.seal("g1", b"hello group").unwrap(),
        blob,
        "nonces are fresh each time"
    );
}

#[test]
fn derivation_is_deterministic_and_separates_keys() {
    let secret = new_secret().unwrap();
    let (a, b) = (
        GroupKeys::derive(&secret).unwrap(),
        GroupKeys::derive(&secret).unwrap(),
    );
    assert_eq!(a.auth_token, b.auth_token);
    assert_ne!(
        a.auth_token,
        secret.expose(),
        "the secret itself is never sent"
    );
    assert_eq!(b.open("g", &a.seal("g", b"x").unwrap()).unwrap(), b"x");
}

#[test]
fn rejects_wrong_key_group_or_tampering() {
    let keys = GroupKeys::derive(&new_secret().unwrap()).unwrap();
    let other = GroupKeys::derive(&new_secret().unwrap()).unwrap();
    let blob = keys.seal("g1", b"amount: 42").unwrap();

    assert!(other.open("g1", &blob).is_err(), "wrong key");
    assert!(keys.open("g2", &blob).is_err(), "moved to another group");
    let mut tampered = blob.clone();
    *tampered.last_mut().unwrap() ^= 1;
    assert!(keys.open("g1", &tampered).is_err(), "altered data");
    assert!(keys.open("g1", &blob[..10]).is_err(), "truncated");
    assert!(keys.open("g1", b"").is_err(), "empty");
}

#[test]
fn password_keys_wrap_the_account_key() {
    let account_key = new_secret().unwrap();
    let keys = CredentialKeys::from_password("alice", "correct horse").unwrap();
    let blob = keys.wrap_account_key("acc-1", &account_key).unwrap();
    assert!(!blob
        .windows(account_key.expose().len())
        .any(|w| w == account_key.expose().as_bytes()));

    // Same username and password on another device: same token, same key.
    let again = CredentialKeys::from_password("alice", "correct horse").unwrap();
    assert_eq!(again.token, keys.token);
    assert_eq!(
        again.unwrap_account_key("acc-1", &blob).unwrap(),
        account_key
    );

    let wrong_password = CredentialKeys::from_password("alice", "wrong horse").unwrap();
    assert_ne!(wrong_password.token, keys.token);
    assert!(wrong_password.unwrap_account_key("acc-1", &blob).is_err());
    let other_user = CredentialKeys::from_password("bob", "correct horse").unwrap();
    assert_ne!(other_user.token, keys.token, "salted per username");
    assert!(
        keys.unwrap_account_key("acc-2", &blob).is_err(),
        "bound to the account"
    );
}

#[test]
fn recovery_keys_are_readable_and_forgiving() {
    let key = new_recovery_key().unwrap();
    assert_eq!(key.len(), 39, "{key}");
    assert!(key.split('-').all(|g| g.len() == 4), "{key}");
    assert!(!key.contains(['I', 'L', 'O', 'U']), "{key}");
    assert_ne!(key, new_recovery_key().unwrap());

    let account_key = new_secret().unwrap();
    let keys = CredentialKeys::from_recovery_key(&key).unwrap();
    let blob = keys.wrap_account_key("acc-1", &account_key).unwrap();
    // Typed back in lowercase, without dashes, O and I for 0 and 1: same keys.
    let typed = key
        .to_lowercase()
        .replace('-', " ")
        .replace('0', "o")
        .replace('1', "I");
    let again = CredentialKeys::from_recovery_key(&typed).unwrap();
    assert_eq!(again.token, keys.token);
    assert_eq!(
        again.unwrap_account_key("acc-1", &blob).unwrap(),
        account_key
    );

    // Its token and key have nothing to do with a password's.
    assert_ne!(
        keys.token,
        CredentialKeys::from_password("alice", &key).unwrap().token
    );
    let other = CredentialKeys::from_recovery_key(&new_recovery_key().unwrap()).unwrap();
    assert!(other.unwrap_account_key("acc-1", &blob).is_err());

    for bad in [
        "",
        "ABCD-EFGH",
        &format!("{key}-7"),
        &key.replace(|c| c != '-', "U"),
    ] {
        let err = CredentialKeys::from_recovery_key(bad).err().unwrap();
        assert!(err.contains("isn't a valid recovery key"), "{bad}: {err}");
    }
}

#[test]
fn rejects_malformed_secrets() {
    assert!(GroupKeys::derive(&Secret::new("short".into())).is_err());
    assert!(GroupKeys::derive(&Secret::new("not base64 !!".into())).is_err());
}
