//! Admin-presence customization (v2.0.0): persistent store of ed25519 public
//! keys authorized to use the admin presence API, replacing the shared
//! bcrypt-token/JWT login from v1.x.
//!
//! Design constraints (see docs/ADMIN_PRESENCE_DEVELOPMENT.md):
//! - Reuses the server's existing ed25519 signing precedent
//!   (`sodiumoxide::crypto::sign`, see `rendezvous_server.rs`'s
//!   `get_server_sk`) rather than introducing a new crypto primitive.
//! - No direct database/file access is exposed to clients; this store is
//!   only ever read/written from server-side code (`admin_api.rs`,
//!   `rustdesk-utils` enrollment commands).
//! - Least-privilege: a key's record carries no capability beyond "may call
//!   the admin presence API"; revocation is immediate and persisted.
//! - Every add/revoke is meant to be audit-logged by the caller (this module
//!   only manages the on-disk state).

use crate::common::get_arg_opt;
use serde::{Deserialize, Serialize};
use sodiumoxide::crypto::{hash::sha256, sign};
use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    sync::RwLock,
    time::{SystemTime, UNIX_EPOCH},
};

/// Default filename for the authorized-keys store, resolved relative to the
/// same working directory hbbs/hbbr already use for `db_v2.sqlite3` and the
/// `.env` file (see `peer.rs::PeerMap::new` and `utils.rs::init_admin`).
/// Overridable via `ADMIN_API_KEYS_FILE`.
const DEFAULT_KEYS_FILE: &str = "admin_authorized_keys.json";

/// A single ed25519 public key authorized to use the admin presence API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizedKey {
    /// Short, stable identifier derived from the public key (sha256, hex,
    /// first 16 chars) — safe to display/log/reference without leaking the
    /// full key material.
    pub fingerprint: String,
    /// Base64-encoded raw ed25519 public key bytes.
    pub public_key: String,
    /// Administrator-assigned label (e.g. device/user name), purely
    /// descriptive.
    pub label: String,
    pub added_at: u64,
    pub last_used_at: Option<u64>,
    #[serde(default)]
    pub revoked: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct KeysFile {
    keys: Vec<AuthorizedKey>,
}

pub struct AdminKeyStore {
    path: PathBuf,
    keys: RwLock<HashMap<String, AuthorizedKey>>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// Admin-presence customization: derive a short, stable fingerprint from raw
/// ed25519 public key bytes. Not secret; used only to reference/display a
/// key without needing to show the full base64 value.
pub fn fingerprint_of(pk_bytes: &[u8]) -> String {
    let digest = sha256::hash(pk_bytes);
    digest.0.iter().map(|b| format!("{b:02x}")).take(16).collect()
}

impl AdminKeyStore {
    /// Resolves the store path from `ADMIN_API_KEYS_FILE`, falling back to
    /// `DEFAULT_KEYS_FILE` in the current working directory (the same
    /// directory convention used by `db_v2.sqlite3`/`.env`).
    pub fn resolve_default_path() -> PathBuf {
        PathBuf::from(get_arg_opt("ADMIN_API_KEYS_FILE").unwrap_or_else(|| DEFAULT_KEYS_FILE.to_owned()))
    }

    /// Loads the store from `path`, creating an empty in-memory store (not
    /// yet written to disk) if the file does not exist.
    pub fn load(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let keys = match std::fs::read_to_string(&path) {
            Ok(content) => {
                let parsed: KeysFile = serde_json::from_str(&content).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("failed to parse {}: {e}", path.display()),
                    )
                })?;
                parsed
                    .keys
                    .into_iter()
                    .map(|k| (k.fingerprint.clone(), k))
                    .collect()
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => HashMap::new(),
            Err(e) => return Err(e),
        };
        Ok(Self {
            path,
            keys: RwLock::new(keys),
        })
    }

    /// Atomically persists the current in-memory state to disk (write to a
    /// temp file in the same directory, then rename) so a crash mid-write
    /// can never leave a corrupt/partial store on disk.
    fn persist(&self) -> io::Result<()> {
        let keys: Vec<AuthorizedKey> = self.keys.read().unwrap().values().cloned().collect();
        let content = serde_json::to_string_pretty(&KeysFile { keys })
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let tmp_path = self.path.with_extension("json.tmp");
        std::fs::write(&tmp_path, content)?;
        std::fs::rename(&tmp_path, &self.path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&self.path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o600);
                let _ = std::fs::set_permissions(&self.path, perms);
            }
        }
        Ok(())
    }

    /// Adds a new authorized key. Returns an error if a (non-revoked or
    /// revoked) record already exists for this key's fingerprint, so
    /// re-adding a previously revoked key requires an explicit un-revoke
    /// rather than silently reactivating it.
    pub fn add(&self, public_key: &sign::PublicKey, label: &str) -> io::Result<AuthorizedKey> {
        let fingerprint = fingerprint_of(public_key.as_ref());
        let mut keys = self.keys.write().unwrap();
        if keys.contains_key(&fingerprint) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("a key with fingerprint {fingerprint} is already registered"),
            ));
        }
        let record = AuthorizedKey {
            fingerprint: fingerprint.clone(),
            public_key: base64::encode(public_key.as_ref()),
            label: label.to_owned(),
            added_at: now_secs(),
            last_used_at: None,
            revoked: false,
        };
        keys.insert(fingerprint, record.clone());
        drop(keys);
        self.persist()?;
        Ok(record)
    }

    /// Marks a key revoked (soft delete, kept for audit history). Returns
    /// `true` if a matching, not-already-revoked key was found.
    pub fn revoke(&self, fingerprint: &str) -> io::Result<bool> {
        let mut keys = self.keys.write().unwrap();
        let found = match keys.get_mut(fingerprint) {
            Some(k) if !k.revoked => {
                k.revoked = true;
                true
            }
            _ => false,
        };
        drop(keys);
        if found {
            self.persist()?;
        }
        Ok(found)
    }

    /// Lists all known keys (including revoked ones, for audit visibility).
    pub fn list(&self) -> Vec<AuthorizedKey> {
        let mut keys: Vec<AuthorizedKey> = self.keys.read().unwrap().values().cloned().collect();
        keys.sort_by(|a, b| a.added_at.cmp(&b.added_at));
        keys
    }

    /// Looks up a non-revoked key by its raw public key bytes, used during
    /// challenge-response verification. Returns `None` for unknown or
    /// revoked keys (both fail closed the same way).
    pub fn find_active(&self, pk_bytes: &[u8]) -> Option<AuthorizedKey> {
        let fingerprint = fingerprint_of(pk_bytes);
        let keys = self.keys.read().unwrap();
        keys.get(&fingerprint)
            .filter(|k| !k.revoked)
            .cloned()
    }

    /// Records that a key was just used for a successful authentication.
    pub fn touch_last_used(&self, fingerprint: &str) {
        let mut keys = self.keys.write().unwrap();
        if let Some(k) = keys.get_mut(fingerprint) {
            k.last_used_at = Some(now_secs());
        }
        drop(keys);
        let _ = self.persist();
    }

    pub fn is_empty(&self) -> bool {
        self.keys.read().unwrap().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("admin_keys_test_{name}_{}.json", uuid::Uuid::new_v4()))
    }

    #[test]
    fn add_list_revoke_roundtrip() {
        let path = temp_store_path("roundtrip");
        let store = AdminKeyStore::load(&path).unwrap();
        assert!(store.is_empty());

        let (pk, _sk) = sign::gen_keypair();
        let record = store.add(&pk, "nina-laptop admin").unwrap();
        assert_eq!(record.label, "nina-laptop admin");
        assert!(!record.revoked);

        assert!(store.find_active(pk.as_ref()).is_some());
        assert_eq!(store.list().len(), 1);

        assert!(store.revoke(&record.fingerprint).unwrap());
        assert!(store.find_active(pk.as_ref()).is_none());
        // Still listed (soft delete), just marked revoked.
        assert_eq!(store.list().len(), 1);
        assert!(store.list()[0].revoked);

        // Revoking again reports "nothing to do" rather than erroring.
        assert!(!store.revoke(&record.fingerprint).unwrap());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn add_duplicate_fingerprint_is_rejected() {
        let path = temp_store_path("dup");
        let store = AdminKeyStore::load(&path).unwrap();
        let (pk, _sk) = sign::gen_keypair();
        store.add(&pk, "first").unwrap();
        let err = store.add(&pk, "second").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn persists_and_reloads_from_disk() {
        let path = temp_store_path("persist");
        let (pk, _sk) = sign::gen_keypair();
        {
            let store = AdminKeyStore::load(&path).unwrap();
            store.add(&pk, "reload-me").unwrap();
        }
        let reloaded = AdminKeyStore::load(&path).unwrap();
        let found = reloaded.find_active(pk.as_ref()).unwrap();
        assert_eq!(found.label, "reload-me");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unknown_key_is_not_active() {
        let path = temp_store_path("unknown");
        let store = AdminKeyStore::load(&path).unwrap();
        let (pk, _sk) = sign::gen_keypair();
        assert!(store.find_active(pk.as_ref()).is_none());
        let _ = std::fs::remove_file(&path);
    }
}
