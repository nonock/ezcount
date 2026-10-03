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
//! Accounts add one layer on top (see `CredentialKeys`): a random account key, which is the
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

/// A group secret or account key (32 random bytes, base64url, as in invite links).
///
/// Its own type, so it can't be passed where a group id or URL is expected, or the other way
/// round; `Debug` hides it, so logging a struct doesn't leak it. Get the text with `expose()`,
/// only where it has to leave memory (storage, invite links, derivation).
#[derive(Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// A new random secret.
    pub fn generate() -> Res<Self> {
        let mut bytes = [0u8; SECRET_LEN];
        getrandom::fill(&mut bytes).map_err(|e| format!("Could not generate a key: {e}"))?;
        Ok(Self(URL_SAFE_NO_PAD.encode(bytes)))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(…)")
    }
}

pub struct GroupKeys {
    /// Bearer token proving access to the relay. Safe to send; reveals nothing about the data.
    pub auth_token: String,
    cipher: XChaCha20Poly1305,
}

impl GroupKeys {
    /// Derives the keys from a group secret.
    pub fn derive(secret: &Secret) -> Res<Self> {
        let invalid = || "The group key is malformed".to_string();
        let secret = URL_SAFE_NO_PAD
            .decode(secret.expose())
            .map_err(|_| invalid())?;
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

/// Recovery keys hold this many random bytes: 160 bits, 32 base32 characters.
const RECOVERY_KEY_LEN: usize = 20;
/// Crockford's base32: no I, L, O or U, so a key read back from paper can't be misspelled.
const RECOVERY_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// A new random recovery key, as the user sees it: `7KQ2-M9XD-…`, eight groups of four.
pub fn new_recovery_key() -> Res<String> {
    let mut bytes = [0u8; RECOVERY_KEY_LEN];
    getrandom::fill(&mut bytes).map_err(|e| format!("Could not generate a key: {e}"))?;
    let mut bits = 0u64;
    let mut pending = 0;
    let mut chars = String::new();
    for byte in bytes {
        bits = (bits << 8) | u64::from(byte);
        pending += 8;
        while pending >= 5 {
            pending -= 5;
            chars.push(RECOVERY_ALPHABET[((bits >> pending) & 31) as usize] as char);
        }
    }
    let groups: Vec<&str> = (0..chars.len())
        .step_by(4)
        .map(|i| &chars[i..i + 4])
        .collect();
    Ok(groups.join("-"))
}

/// Reads a recovery key the way people type it: any case, with or without dashes and
/// spaces, and O, I or L for the digits they look like.
fn parse_recovery_key(key: &str) -> Res<[u8; RECOVERY_KEY_LEN]> {
    let invalid = || "This isn't a valid recovery key. Check it for typos.".to_string();
    let chars: Vec<char> = key
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect();
    // Exactly the characters for the key's bits: an extra one would only add unused bits.
    if chars.len() != RECOVERY_KEY_LEN * 8 / 5 {
        return Err(invalid());
    }
    let mut bits = 0u64;
    let mut pending = 0;
    let mut bytes = Vec::with_capacity(RECOVERY_KEY_LEN);
    for c in chars {
        let c = match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        };
        let value = RECOVERY_ALPHABET
            .iter()
            .position(|&a| a as char == c)
            .ok_or_else(invalid)?;
        bits = (bits << 5) | value as u64;
        pending += 5;
        if pending >= 8 {
            pending -= 8;
            bytes.push((bits >> pending) as u8);
        }
    }
    bytes.try_into().map_err(|_| invalid())
}

/// Keys from an account credential, the password or the recovery key: a token that proves it
/// to the relay (which stores only the token's hash), and a key that encrypts the account key.
pub struct CredentialKeys {
    pub token: String,
    /// Encrypts the account key for storage on the relay. Never leaves the device.
    wrap: XChaCha20Poly1305,
}

impl CredentialKeys {
    /// A recovery key is random and long, so a plain HKDF is enough (no stretching).
    pub fn from_recovery_key(recovery_key: &str) -> Res<Self> {
        let bytes = parse_recovery_key(recovery_key)?;
        let hkdf = Hkdf::<Sha256>::new(Some(b"ezcount recovery keys"), &bytes);
        let mut token = [0u8; 32];
        let mut wrap = [0u8; 32];
        hkdf.expand(b"ezcount/v1/recovery-token", &mut token)
            .and_then(|_| hkdf.expand(b"ezcount/v1/recovery-key-wrap", &mut wrap))
            .map_err(|_| "Could not derive keys from the recovery key".to_string())?;
        Ok(Self {
            token: URL_SAFE_NO_PAD.encode(token),
            wrap: XChaCha20Poly1305::new_from_slice(&wrap)
                .map_err(|_| "Could not derive keys from the recovery key".to_string())?,
        })
    }

    /// Slow on purpose (Argon2id); call it off the async runtime. The salt is derived from the
    /// username, which is unique per relay, so no extra round trip is needed before logging in.
    pub fn from_password(username: &str, password: &str) -> Res<Self> {
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
            token: URL_SAFE_NO_PAD.encode(login),
            wrap: XChaCha20Poly1305::new_from_slice(&wrap)
                .map_err(|_| "Could not derive keys from the password".to_string())?,
        })
    }

    /// Encrypts the account key (a secret in the same format as group secrets).
    pub fn wrap_account_key(&self, account_id: &str, account_key: &Secret) -> Res<Vec<u8>> {
        seal_with(
            &self.wrap,
            account_id.as_bytes(),
            account_key.expose().as_bytes(),
        )
    }

    pub fn unwrap_account_key(&self, account_id: &str, blob: &[u8]) -> Res<Secret> {
        let unreadable = || "Your account key could not be decrypted".to_string();
        let key = open_with(&self.wrap, account_id.as_bytes(), blob).map_err(|_| unreadable())?;
        let key = Secret::new(String::from_utf8(key).map_err(|_| unreadable())?);
        GroupKeys::derive(&key).map_err(|_| unreadable())?;
        Ok(key)
    }
}

/// Keys from the code of a login link (see `sync::create_login_link`): the ticket the relay
/// keeps the account under, and the key that encrypts it. The relay sees the ticket only,
/// which says nothing about the key.
pub struct LinkKeys {
    pub ticket: String,
    cipher: XChaCha20Poly1305,
}

impl LinkKeys {
    pub fn derive(code: &Secret) -> Res<Self> {
        let invalid = || "This is not an ezcount login code".to_string();
        let code = URL_SAFE_NO_PAD
            .decode(code.expose())
            .map_err(|_| invalid())?;
        if code.len() != SECRET_LEN {
            return Err(invalid());
        }
        let hkdf = Hkdf::<Sha256>::new(Some(b"ezcount login links"), &code);
        let mut ticket = [0u8; 32];
        let mut enc = [0u8; 32];
        hkdf.expand(b"ezcount/v1/link-ticket", &mut ticket)
            .and_then(|_| hkdf.expand(b"ezcount/v1/link-encryption", &mut enc))
            .map_err(|_| invalid())?;
        Ok(Self {
            ticket: URL_SAFE_NO_PAD.encode(ticket),
            cipher: XChaCha20Poly1305::new_from_slice(&enc).map_err(|_| invalid())?,
        })
    }

    /// Bound to the ticket, so what one link holds can't be served for another.
    pub fn seal(&self, plaintext: &[u8]) -> Res<Vec<u8>> {
        seal_with(&self.cipher, self.ticket.as_bytes(), plaintext)
    }

    pub fn open(&self, blob: &[u8]) -> Res<Vec<u8>> {
        open_with(&self.cipher, self.ticket.as_bytes(), blob)
            .map_err(|_| "The sync server sent corrupt account data".to_string())
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
}
