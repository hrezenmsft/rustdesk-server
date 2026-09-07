mod rendezvous_server;
pub use rendezvous_server::*;
pub mod common;
// Admin-presence customization for this Windows client version: authenticated
// HTTP API backing the custom client admin online-device view.
mod admin_api;
// Admin-presence customization (v2.0.0): persistent store of ed25519 public
// keys authorized to use the admin presence API. Public so `rustdesk-utils`
// (src/utils.rs, a separate bin target) can implement enrollment commands
// (genadminkey/listadminkeys/revokeadminkey) against the same store the
// running server reads, without duplicating its file format/logic.
pub mod admin_keys;
// Admin-presence customization (v2.0.0): ed25519 challenge-response
// authentication built on top of `admin_keys`.
mod admin_auth;
mod database;
mod peer;
mod version;
