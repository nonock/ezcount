//! End-to-end encryption of sync updates.
//!
//! The group secret from the invite code never leaves the device. Two independent keys are
//! derived from it with HKDF-SHA256:
//!
//! - an **auth token**, sent to the relay as a bearer token (the relay stores only its hash),
//! - an **encryption key**, used with XChaCha20-Poly1305 to seal every update before upload.
//!
//! So the relay can check who may read and write a group, but can neither read the data nor
//! derive the encryption key. The group ID is bound as associated data, so a blob cannot be
//! moved into another group, and any modification is detected on decryption.
//!
//! Sealed blob layout: `[format version: 1 byte][nonce: 24 bytes][ciphertext + 16-byte tag]`.
//!
//! Accounts add one layer on top (see `PasswordKeys`): a random account key, which is the
//! secret of the account document, is stored on the relay encrypted with a key stretched
//! from the password. The relay can check logins but can never recover the account key.

use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};

type Res<T> = Result<T, String>;

const FORMAT_V1: u8 = 1;
const NONCE_LEN: usize = 24;
const SECRET_LEN: usize = 32;
const SALT: &[u8] = b"ezcount group keys";
/// Argon2id cost: 64 MiB and 3 passes, well above the OWASP minimum. Roughly half a second
/// on a laptop, a bit more on a phone; it only runs when signing up or logging in.
const ARGON2_MEMORY_KIB: u32 = 64 * 1024;
const ARGON2_PASSES: u32 = 3;

enum OpenError {
    UnknownFormat(u8),
    Unreadable,
}

fn seal_with(cipher: &XChaCha20Poly1305, aad: &[u8], plaintext: &[u8]) -> Res<Vec<u8>> {
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).map_err(|e| format!("Could not generate a nonce: {e}"))?;
    let ciphertext = cipher
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| "Could not encrypt data".to_string())?;

    let mut blob = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
    blob.push(FORMAT_V1);
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(&ciphertext);
    Ok(blob)
}

fn open_with(cipher: &XChaCha20Poly1305, aad: &[u8], blob: &[u8]) -> Result<Vec<u8>, OpenError> {
    let (&version, rest) = blob.split_first().ok_or(OpenError::Unreadable)?;
    if version != FORMAT_V1 {
        return Err(OpenError::UnknownFormat(version));
    }
    if rest.len() < NONCE_LEN {
        return Err(OpenError::Unreadable);
    }
    let (nonce, ciphertext) = rest.split_at(NONCE_LEN);
    let nonce = XNonce::try_from(nonce).map_err(|_| OpenError::Unreadable)?;
    cipher
        .decrypt(
            &nonce,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| OpenError::Unreadable)
}

pub struct GroupKeys {
    /// Bearer token proving access to the relay. Safe to send; reveals nothing about the data.
    pub auth_token: String,
    cipher: XChaCha20Poly1305,
}

impl GroupKeys {
    /// Derives the keys from a group secret (32 random bytes, base64url, as in invite codes).
    pub fn derive(secret: &str) -> Res<Self> {
        let invalid = || "The group key is malformed".to_string();
        let secret = URL_SAFE_NO_PAD.decode(secret).map_err(|_| invalid())?;
        if secret.len() != SECRET_LEN {
            return Err(invalid());
        }
        let hkdf = Hkdf::<Sha256>::new(Some(SALT), &secret);

        let mut auth = [0u8; 32];
        let mut enc = [0u8; 32];
        hkdf.expand(b"ezcount/v1/relay-auth", &mut auth)
            .and_then(|_| hkdf.expand(b"ezcount/v1/update-encryption", &mut enc))
            .map_err(|_| invalid())?;

        Ok(Self {
            auth_token: URL_SAFE_NO_PAD.encode(auth),
            cipher: XChaCha20Poly1305::new_from_slice(&enc).map_err(|_| invalid())?,
        })
    }

    /// Encrypts an update for upload. Random 192-bit nonces make reuse practically impossible.
    pub fn seal(&self, group_id: &str, plaintext: &[u8]) -> Res<Vec<u8>> {
        seal_with(&self.cipher, group_id.as_bytes(), plaintext)
    }

    /// Decrypts an update downloaded from the relay.
    pub fn open(&self, group_id: &str, blob: &[u8]) -> Res<Vec<u8>> {
        open_with(&self.cipher, group_id.as_bytes(), blob).map_err(|e| match e {
            OpenError::UnknownFormat(version) => format!(
                "A change from the server uses an unknown format ({version}); update ezcount"
            ),
            OpenError::Unreadable => {
                "A change from the server could not be decrypted (wrong key or altered data)"
                    .to_string()
            }
        })
    }
}

/// Keys stretched from an account password.
pub struct PasswordKeys {
    /// Proves the password to the relay, which stores only its hash.
    pub login_token: String,
    /// Encrypts the account key for storage on the relay. Never leaves the device.
    wrap: XChaCha20Poly1305,
}

impl PasswordKeys {
    /// Slow on purpose (Argon2id); call it off the async runtime. The salt is derived from the
    /// username, which is unique per relay, so no extra round trip is needed before logging in.
    pub fn derive(username: &str, password: &str) -> Res<Self> {
        let failed = |e: argon2::Error| format!("Could not derive keys from the password: {e}");
        let salt = Sha256::digest(format!("ezcount/v1/account-salt/{username}"));
        let params = Params::new(ARGON2_MEMORY_KIB, ARGON2_PASSES, 1, Some(32)).map_err(failed)?;
        let mut stretched = [0u8; 32];
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
            .hash_password_into(password.as_bytes(), &salt, &mut stretched)
            .map_err(failed)?;

        let hkdf = Hkdf::<Sha256>::new(Some(b"ezcount account keys"), &stretched);
        let mut login = [0u8; 32];
        let mut wrap = [0u8; 32];
        hkdf.expand(b"ezcount/v1/account-login", &mut login)
            .and_then(|_| hkdf.expand(b"ezcount/v1/account-key-wrap", &mut wrap))
            .map_err(|_| "Could not derive keys from the password".to_string())?;
        Ok(Self {
            login_token: URL_SAFE_NO_PAD.encode(login),
            wrap: XChaCha20Poly1305::new_from_slice(&wrap)
                .map_err(|_| "Could not derive keys from the password".to_string())?,
        })
    }

    /// Encrypts the account key (a secret in the same format as group secrets).
    pub fn wrap_account_key(&self, account_id: &str, account_key: &str) -> Res<Vec<u8>> {
        seal_with(&self.wrap, account_id.as_bytes(), account_key.as_bytes())
    }

    pub fn unwrap_account_key(&self, account_id: &str, blob: &[u8]) -> Res<String> {
        let unreadable = || "Your account key could not be decrypted".to_string();
        let key = open_with(&self.wrap, account_id.as_bytes(), blob).map_err(|_| unreadable())?;
        let key = String::from_utf8(key).map_err(|_| unreadable())?;
        GroupKeys::derive(&key).map_err(|_| unreadable())?;
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
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
        assert_ne!(a.auth_token, secret, "the secret itself is never sent");
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
        let keys = PasswordKeys::derive("alice", "correct horse").unwrap();
        let blob = keys.wrap_account_key("acc-1", &account_key).unwrap();
        assert!(!blob
            .windows(account_key.len())
            .any(|w| w == account_key.as_bytes()));

        // Same username and password on another device: same token, same key.
        let again = PasswordKeys::derive("alice", "correct horse").unwrap();
        assert_eq!(again.login_token, keys.login_token);
        assert_eq!(
            again.unwrap_account_key("acc-1", &blob).unwrap(),
            account_key
        );

        let wrong_password = PasswordKeys::derive("alice", "wrong horse").unwrap();
        assert_ne!(wrong_password.login_token, keys.login_token);
        assert!(wrong_password.unwrap_account_key("acc-1", &blob).is_err());
        let other_user = PasswordKeys::derive("bob", "correct horse").unwrap();
        assert_ne!(
            other_user.login_token, keys.login_token,
            "salted per username"
        );
        assert!(
            keys.unwrap_account_key("acc-2", &blob).is_err(),
            "bound to the account"
        );
    }

    #[test]
    fn rejects_malformed_secrets() {
        assert!(GroupKeys::derive("short").is_err());
        assert!(GroupKeys::derive("not base64 !!").is_err());
    }
}
