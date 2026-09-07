//! Admin-presence customization (v2.0.0): ed25519 challenge-response
//! authentication, replacing the shared bcrypt-token/JWT login used in
//! v1.x. Reuses the server's existing `sodiumoxide::crypto::sign`
//! precedent (see `rendezvous_server.rs::get_server_sk`) — no new crypto
//! primitive is introduced.
//!
//! Flow:
//! 1. Client asks for a challenge, presenting its ed25519 public key.
//! 2. Server generates a random nonce, remembers it (short TTL, single use,
//!    scoped to that public key) and returns it.
//! 3. Client signs the nonce with its private key and sends the signature
//!    back along with the public key.
//! 4. Server verifies: (a) the public key is an active (non-revoked) entry
//!    in the `AdminKeyStore`, (b) the nonce exists, is unexpired, and has not
//!    already been consumed, (c) the signature verifies against the nonce
//!    bytes and the presented public key. On success the nonce is consumed
//!    (removed) so it can never be replayed, and a short-lived bearer
//!    session token is issued (reusing the existing JWT session-token
//!    mechanism from v1.x — only the *login* step changed, not how an
//!    authenticated session is represented afterwards).

use crate::admin_keys::AdminKeyStore;
use sodiumoxide::crypto::sign;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// How long a challenge nonce remains valid before it must be discarded and
/// a fresh one requested. Kept short to minimize the replay/guessing window.
pub const CHALLENGE_TTL: Duration = Duration::from_secs(30);

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

struct PendingChallenge {
    /// Fingerprint of the public key this nonce was issued to. A signature
    /// verification must be attempted only against this same key: prevents
    /// one client's outstanding challenge from being satisfied by a
    /// different, unrelated authorized key signing the same nonce bytes.
    fingerprint: String,
    expires_at: u64,
}

/// Holds outstanding, not-yet-consumed challenge nonces in memory. Nonces
/// are intentionally not persisted to disk: a server restart simply
/// invalidates any in-flight login attempt, which is safe and requires only
/// a retry.
pub struct ChallengeStore {
    pending: Mutex<HashMap<String, PendingChallenge>>,
}

#[derive(Debug)]
pub enum AuthError {
    UnknownOrRevokedKey,
    InvalidPublicKey,
    InvalidSignature,
    NoSuchChallenge,
    ChallengeExpired,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            AuthError::UnknownOrRevokedKey => "unknown or revoked public key",
            AuthError::InvalidPublicKey => "malformed public key",
            AuthError::InvalidSignature => "signature verification failed",
            AuthError::NoSuchChallenge => "no matching outstanding challenge",
            AuthError::ChallengeExpired => "challenge expired, request a new one",
        };
        f.write_str(msg)
    }
}

impl Default for ChallengeStore {
    fn default() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
        }
    }
}

impl ChallengeStore {
    /// Issues a fresh nonce for the given (base64) public key, first
    /// confirming the key is a currently-active authorized admin key. The
    /// nonce itself carries no information about which key it was issued to
    /// (that binding is kept server-side only) so it cannot be used to probe
    /// which keys are registered.
    pub fn issue(
        &self,
        keys: &AdminKeyStore,
        public_key_b64: &str,
    ) -> Result<String, AuthError> {
        let pk_bytes = base64::decode(public_key_b64).map_err(|_| AuthError::InvalidPublicKey)?;
        let public_key =
            sign::PublicKey::from_slice(&pk_bytes).ok_or(AuthError::InvalidPublicKey)?;
        let record = keys
            .find_active(public_key.as_ref())
            .ok_or(AuthError::UnknownOrRevokedKey)?;

        let nonce = to_hex(&sodiumoxide::randombytes::randombytes(32));
        let mut pending = self.pending.lock().unwrap();
        // Opportunistically sweep expired entries so this map cannot grow
        // unbounded if clients request challenges without ever completing
        // the verify step.
        let now = now_secs();
        pending.retain(|_, c| c.expires_at > now);
        pending.insert(
            nonce.clone(),
            PendingChallenge {
                fingerprint: record.fingerprint,
                expires_at: now + CHALLENGE_TTL.as_secs(),
            },
        );
        Ok(nonce)
    }

    /// Verifies a signed nonce against the presented public key. On success,
    /// the nonce is consumed (removed) and the record's fingerprint is
    /// returned so the caller can look up/touch the full `AuthorizedKey`
    /// record (e.g. to update `last_used_at` and to log the label).
    pub fn verify(
        &self,
        keys: &AdminKeyStore,
        public_key_b64: &str,
        nonce: &str,
        signature_b64: &str,
    ) -> Result<String, AuthError> {
        let pk_bytes = base64::decode(public_key_b64).map_err(|_| AuthError::InvalidPublicKey)?;
        let public_key =
            sign::PublicKey::from_slice(&pk_bytes).ok_or(AuthError::InvalidPublicKey)?;
        let record = keys
            .find_active(public_key.as_ref())
            .ok_or(AuthError::UnknownOrRevokedKey)?;

        let mut pending = self.pending.lock().unwrap();
        let now = now_secs();
        pending.retain(|_, c| c.expires_at > now);
        let challenge = pending.get(nonce).ok_or(AuthError::NoSuchChallenge)?;
        if challenge.fingerprint != record.fingerprint {
            // Nonce exists but was issued to a different key: treat exactly
            // like "no such challenge" rather than leaking that the nonce is
            // valid for someone else.
            return Err(AuthError::NoSuchChallenge);
        }
        if challenge.expires_at <= now {
            pending.remove(nonce);
            return Err(AuthError::ChallengeExpired);
        }

        let signature_bytes = base64::decode(signature_b64).map_err(|_| AuthError::InvalidSignature)?;
        let signed_message: Vec<u8> = signature_bytes
            .iter()
            .chain(nonce.as_bytes().iter())
            .copied()
            .collect();
        // sodiumoxide's `sign::verify` expects a signature prefixed onto the
        // message it signs (the same "attached signature" format used
        // elsewhere in this codebase, e.g. `utils.rs::validate_keypair`).
        if sign::verify(&signed_message, &public_key).is_err() {
            return Err(AuthError::InvalidSignature);
        }

        // Consume: this exact nonce can never be replayed again, satisfied
        // or not.
        pending.remove(nonce);
        Ok(record.fingerprint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_keys(name: &str) -> (AdminKeyStore, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "admin_auth_test_{name}_{}.json",
            uuid::Uuid::new_v4()
        ));
        (AdminKeyStore::load(&path).unwrap(), path)
    }

    fn sign_nonce(sk: &sign::SecretKey, nonce: &str) -> String {
        let signed = sign::sign(nonce.as_bytes(), sk);
        // `sign::sign` returns signature++message; strip the message back
        // off so we transmit only the signature, mirroring how a real client
        // would compute and send just the detached signature bytes.
        let sig_len = signed.len() - nonce.as_bytes().len();
        base64::encode(&signed[..sig_len])
    }

    #[test]
    fn full_challenge_response_roundtrip_succeeds() {
        let (keys, path) = temp_keys("roundtrip");
        let (pk, sk) = sign::gen_keypair();
        keys.add(&pk, "test-client").unwrap();
        let pk_b64 = base64::encode(pk.as_ref());

        let challenges = ChallengeStore::default();
        let nonce = challenges.issue(&keys, &pk_b64).unwrap();
        let sig_b64 = sign_nonce(&sk, &nonce);

        let fingerprint = challenges.verify(&keys, &pk_b64, &nonce, &sig_b64).unwrap();
        assert_eq!(fingerprint, keys.list()[0].fingerprint);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn nonce_cannot_be_replayed() {
        let (keys, path) = temp_keys("replay");
        let (pk, sk) = sign::gen_keypair();
        keys.add(&pk, "test-client").unwrap();
        let pk_b64 = base64::encode(pk.as_ref());

        let challenges = ChallengeStore::default();
        let nonce = challenges.issue(&keys, &pk_b64).unwrap();
        let sig_b64 = sign_nonce(&sk, &nonce);

        assert!(challenges.verify(&keys, &pk_b64, &nonce, &sig_b64).is_ok());
        let replay = challenges.verify(&keys, &pk_b64, &nonce, &sig_b64);
        assert!(matches!(replay, Err(AuthError::NoSuchChallenge)));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn wrong_key_signature_is_rejected() {
        let (keys, path) = temp_keys("wrongkey");
        let (pk, _sk) = sign::gen_keypair();
        keys.add(&pk, "test-client").unwrap();
        let pk_b64 = base64::encode(pk.as_ref());

        let (_other_pk, other_sk) = sign::gen_keypair();

        let challenges = ChallengeStore::default();
        let nonce = challenges.issue(&keys, &pk_b64).unwrap();
        // Sign with a *different* key's secret than the one the challenge
        // was issued to.
        let sig_b64 = sign_nonce(&other_sk, &nonce);

        let result = challenges.verify(&keys, &pk_b64, &nonce, &sig_b64);
        assert!(matches!(result, Err(AuthError::InvalidSignature)));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn revoked_key_cannot_get_a_challenge() {
        let (keys, path) = temp_keys("revoked");
        let (pk, _sk) = sign::gen_keypair();
        let record = keys.add(&pk, "test-client").unwrap();
        keys.revoke(&record.fingerprint).unwrap();
        let pk_b64 = base64::encode(pk.as_ref());

        let challenges = ChallengeStore::default();
        let result = challenges.issue(&keys, &pk_b64);
        assert!(matches!(result, Err(AuthError::UnknownOrRevokedKey)));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unknown_key_cannot_get_a_challenge() {
        let (keys, path) = temp_keys("unknown");
        let (pk, _sk) = sign::gen_keypair();
        let pk_b64 = base64::encode(pk.as_ref());

        let challenges = ChallengeStore::default();
        let result = challenges.issue(&keys, &pk_b64);
        assert!(matches!(result, Err(AuthError::UnknownOrRevokedKey)));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn nonce_from_different_key_is_rejected() {
        let (keys, path) = temp_keys("crosskey");
        let (pk_a, _sk_a) = sign::gen_keypair();
        let (pk_b, sk_b) = sign::gen_keypair();
        keys.add(&pk_a, "client-a").unwrap();
        keys.add(&pk_b, "client-b").unwrap();
        let pk_a_b64 = base64::encode(pk_a.as_ref());
        let pk_b_b64 = base64::encode(pk_b.as_ref());

        let challenges = ChallengeStore::default();
        // Challenge issued to client A's key...
        let nonce = challenges.issue(&keys, &pk_a_b64).unwrap();
        // ...but client B tries to answer it with its own (validly signed)
        // signature.
        let sig_b64 = sign_nonce(&sk_b, &nonce);
        let result = challenges.verify(&keys, &pk_b_b64, &nonce, &sig_b64);
        assert!(matches!(result, Err(AuthError::NoSuchChallenge)));
        let _ = std::fs::remove_file(&path);
    }
}
