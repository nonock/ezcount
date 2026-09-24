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

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use sha2::Sha256;

type Res<T> = Result<T, String>;

const FORMAT_V1: u8 = 1;
const NONCE_LEN: usize = 24;
const SECRET_LEN: usize = 32;
const SALT: &[u8] = b"ezcount group keys";

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
        let mut nonce = [0u8; NONCE_LEN];
        getrandom::fill(&mut nonce).map_err(|e| format!("Could not generate a nonce: {e}"))?;
        let ciphertext = self
            .cipher
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: plaintext,
                    aad: group_id.as_bytes(),
                },
            )
            .map_err(|_| "Could not encrypt changes".to_string())?;

        let mut blob = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
        blob.push(FORMAT_V1);
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&ciphertext);
        Ok(blob)
    }

    /// Decrypts an update downloaded from the relay.
    pub fn open(&self, group_id: &str, blob: &[u8]) -> Res<Vec<u8>> {
        let unreadable = || {
            "A change from the server could not be decrypted (wrong key or altered data)"
                .to_string()
        };
        let (&version, rest) = blob.split_first().ok_or_else(unreadable)?;
        if version != FORMAT_V1 {
            return Err(format!(
                "A change from the server uses an unknown format ({version}); update ezcount"
            ));
        }
        if rest.len() < NONCE_LEN {
            return Err(unreadable());
        }
        let (nonce, ciphertext) = rest.split_at(NONCE_LEN);
        let nonce = XNonce::try_from(nonce).map_err(|_| unreadable())?;
        self.cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: ciphertext,
                    aad: group_id.as_bytes(),
                },
            )
            .map_err(|_| unreadable())
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
    fn rejects_malformed_secrets() {
        assert!(GroupKeys::derive("short").is_err());
        assert!(GroupKeys::derive("not base64 !!").is_err());
    }
}
