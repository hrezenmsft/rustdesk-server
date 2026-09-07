//! Authenticated, versioned HTTP API used only by the custom admin-presence
//! Windows client to list devices currently online with this rendezvous
//! server. See docs/ADMIN_PRESENCE_DEVELOPMENT.md for the full contract.
//!
//! Design constraints (see docs/ADMIN_PRESENCE_DEVELOPMENT.md):
//! - No direct database/file access is exposed to clients; presence data
//!   comes only from `PeerMap::list_online`, which reads in-memory state
//!   populated by the existing rendezvous registration/heartbeat path.
//! - No unauthenticated enumeration: every route other than login/challenge
//!   requires a valid, short-lived bearer token issued after authentication.
//! - Least-privilege data: the device list exposes only the device id and
//!   how recently it was last seen; no IP address or other peer detail.
//! - Every auth attempt and device-list query is audit-logged.
//! - This API is additive: it does not alter any existing RustDesk protocol
//!   message or the main rendezvous port; it listens on its own port.
//!
//! v2.0.0: primary authentication is now per-client ed25519 challenge-response
//! (`admin_auth::ChallengeStore` + `admin_keys::AdminKeyStore`) instead of a
//! single shared bcrypt-hashed token. The legacy `POST /admin/v1/auth/login`
//! (shared token) path is kept, disabled by default, only for migration from
//! v1.x and is logged as deprecated on every use. Both paths issue the same
//! short-lived JWT bearer session token consumed by `/admin/v1/devices`, so
//! nothing downstream of login changed.

use crate::admin_auth::{AuthError, ChallengeStore};
use crate::admin_keys::AdminKeyStore;
use crate::common::get_arg_opt;
use crate::peer::PeerMap;
use crate::rendezvous_server::REG_TIMEOUT;
use axum::{
    extract::{ConnectInfo, Extension, Query, TypedHeader},
    headers::{authorization::Bearer, Authorization},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use hbb_common::{log, ResultType};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

const ADMIN_API_PATH_PREFIX: &str = "/admin/v1";
const DEFAULT_ADMIN_API_PORT: u16 = 21114;
const JWT_TTL_SECS: u64 = 15 * 60;
const JWT_ISSUER: &str = "rustdesk-admin-presence";

#[derive(Clone)]
struct AdminApiState {
    pm: PeerMap,
    /// `None` once the legacy shared-token login is fully retired for this
    /// deployment (ADMIN_API_TOKEN_HASH not set).
    token_hash: Option<Arc<str>>,
    jwt_secret: Arc<str>,
    keys: Arc<AdminKeyStore>,
    challenges: Arc<ChallengeStore>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    iss: String,
    iat: u64,
    exp: u64,
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    token: String,
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    access_token: String,
    token_type: &'static str,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct ChallengeRequest {
    /// Base64-encoded raw ed25519 public key of the requesting admin client.
    public_key: String,
}

#[derive(Debug, Serialize)]
struct ChallengeResponse {
    /// Hex-encoded random nonce the client must sign and echo back via
    /// `/admin/v1/auth/verify` within `CHALLENGE_TTL`.
    nonce: String,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct VerifyRequest {
    public_key: String,
    nonce: String,
    /// Base64-encoded detached ed25519 signature over the nonce's UTF-8
    /// bytes, produced with the private key matching `public_key`.
    signature: String,
}

#[derive(Debug, Serialize)]
struct DeviceView {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    last_seen_secs: f64,
}

#[derive(Debug, Serialize)]
struct DevicesResponse {
    devices: Vec<DeviceView>,
}

#[derive(Debug, Deserialize)]
struct DevicesQuery {
    status: Option<String>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: &'static str,
}

fn error_response(status: StatusCode, error: &'static str) -> axum::response::Response {
    (status, Json(ErrorResponse { error })).into_response()
}

/// Records an admin API access attempt in the standard server log, so every
/// call (successful or not) is auditable without a separate storage system.
fn audit(action: &str, remote: SocketAddr, outcome: &str, detail: &str) {
    log::info!(
        "admin_api audit action={action} remote={remote} outcome={outcome} detail={detail}"
    );
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

fn issue_token(secret: &str) -> ResultType<(String, u64)> {
    let iat = now_secs();
    let exp = iat + JWT_TTL_SECS;
    let claims = Claims {
        sub: "admin".to_owned(),
        iss: JWT_ISSUER.to_owned(),
        iat,
        exp,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    Ok((token, JWT_TTL_SECS))
}

fn verify_token(secret: &str, token: &str) -> bool {
    let mut validation = Validation::default();
    validation.set_issuer(&[JWT_ISSUER]);
    decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &validation).is_ok()
}

/// Legacy v1.x login: a single shared bcrypt-hashed token grants a session.
/// Deprecated in favor of per-client ed25519 challenge-response (see
/// `challenge`/`verify` below); kept only so a v1.x deployment can migrate
/// without a hard cutover, and disabled entirely unless
/// `ADMIN_API_TOKEN_HASH` is configured.
async fn login(
    Extension(state): Extension<AdminApiState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    body: Result<Json<LoginRequest>, axum::extract::rejection::JsonRejection>,
) -> axum::response::Response {
    let Json(req) = match body {
        Ok(v) => v,
        Err(_) => {
            audit("login", remote, "denied", "malformed request body");
            return error_response(StatusCode::BAD_REQUEST, "malformed_request");
        }
    };
    let Some(token_hash) = state.token_hash.as_deref() else {
        audit("login", remote, "denied", "legacy token login not configured");
        return error_response(StatusCode::NOT_FOUND, "not_supported");
    };
    let valid = bcrypt::verify(&req.token, token_hash).unwrap_or(false);
    if !valid {
        audit("login", remote, "denied", "invalid admin token");
        return error_response(StatusCode::UNAUTHORIZED, "invalid_token");
    }
    log::warn!(
        "admin_api: deprecated shared-token login used by {remote}; migrate this client to \
         key-based enrollment (see docs/ADMIN_PRESENCE_DEVELOPMENT.md)"
    );
    match issue_token(&state.jwt_secret) {
        Ok((access_token, expires_in)) => {
            audit("login", remote, "granted", "legacy shared-token path");
            (
                StatusCode::OK,
                Json(LoginResponse {
                    access_token,
                    token_type: "Bearer",
                    expires_in,
                }),
            )
                .into_response()
        }
        Err(err) => {
            log::error!("admin_api: failed to issue token: {err}");
            audit("login", remote, "error", "token issuance failed");
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "server_error")
        }
    }
}

fn auth_error_status(err: &AuthError) -> StatusCode {
    match err {
        AuthError::UnknownOrRevokedKey => StatusCode::UNAUTHORIZED,
        AuthError::InvalidPublicKey => StatusCode::BAD_REQUEST,
        AuthError::InvalidSignature => StatusCode::UNAUTHORIZED,
        AuthError::NoSuchChallenge => StatusCode::UNAUTHORIZED,
        AuthError::ChallengeExpired => StatusCode::UNAUTHORIZED,
    }
}

fn auth_error_code(err: &AuthError) -> &'static str {
    match err {
        AuthError::UnknownOrRevokedKey => "unknown_or_revoked_key",
        AuthError::InvalidPublicKey => "invalid_public_key",
        AuthError::InvalidSignature => "invalid_signature",
        AuthError::NoSuchChallenge => "no_such_challenge",
        AuthError::ChallengeExpired => "challenge_expired",
    }
}

/// v2.0.0 step 1 of 2: the client presents its ed25519 public key and
/// receives a short-lived, single-use nonce to sign. Fails closed for
/// unknown/revoked keys with the same error shape either way, so this
/// endpoint cannot be used to enumerate which keys are registered.
async fn auth_challenge(
    Extension(state): Extension<AdminApiState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    body: Result<Json<ChallengeRequest>, axum::extract::rejection::JsonRejection>,
) -> axum::response::Response {
    let Json(req) = match body {
        Ok(v) => v,
        Err(_) => {
            audit("auth_challenge", remote, "denied", "malformed request body");
            return error_response(StatusCode::BAD_REQUEST, "malformed_request");
        }
    };
    match state.challenges.issue(&state.keys, &req.public_key) {
        Ok(nonce) => {
            audit("auth_challenge", remote, "issued", "");
            (
                StatusCode::OK,
                Json(ChallengeResponse {
                    nonce,
                    expires_in: crate::admin_auth::CHALLENGE_TTL.as_secs(),
                }),
            )
                .into_response()
        }
        Err(err) => {
            audit(
                "auth_challenge",
                remote,
                "denied",
                &format!("{err}"),
            );
            error_response(auth_error_status(&err), auth_error_code(&err))
        }
    }
}

/// v2.0.0 step 2 of 2: the client returns the signed nonce; on success this
/// issues the same short-lived JWT session token the legacy login path
/// issues, so `/admin/v1/devices` needs no changes.
async fn auth_verify(
    Extension(state): Extension<AdminApiState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    body: Result<Json<VerifyRequest>, axum::extract::rejection::JsonRejection>,
) -> axum::response::Response {
    let Json(req) = match body {
        Ok(v) => v,
        Err(_) => {
            audit("auth_verify", remote, "denied", "malformed request body");
            return error_response(StatusCode::BAD_REQUEST, "malformed_request");
        }
    };
    let fingerprint = match state
        .challenges
        .verify(&state.keys, &req.public_key, &req.nonce, &req.signature)
    {
        Ok(fp) => fp,
        Err(err) => {
            audit("auth_verify", remote, "denied", &format!("{err}"));
            return error_response(auth_error_status(&err), auth_error_code(&err));
        }
    };
    state.keys.touch_last_used(&fingerprint);
    match issue_token(&state.jwt_secret) {
        Ok((access_token, expires_in)) => {
            audit(
                "auth_verify",
                remote,
                "granted",
                &format!("fingerprint={fingerprint}"),
            );
            (
                StatusCode::OK,
                Json(LoginResponse {
                    access_token,
                    token_type: "Bearer",
                    expires_in,
                }),
            )
                .into_response()
        }
        Err(err) => {
            log::error!("admin_api: failed to issue token: {err}");
            audit("auth_verify", remote, "error", "token issuance failed");
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "server_error")
        }
    }
}

async fn list_devices(
    Extension(state): Extension<AdminApiState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    Query(query): Query<DevicesQuery>,
    auth: Option<TypedHeader<Authorization<Bearer>>>,
) -> axum::response::Response {
    let token = match &auth {
        Some(TypedHeader(bearer)) => bearer.token(),
        None => {
            audit("list_devices", remote, "denied", "missing bearer token");
            return error_response(StatusCode::UNAUTHORIZED, "missing_token");
        }
    };
    if !verify_token(&state.jwt_secret, token) {
        audit("list_devices", remote, "denied", "invalid or expired token");
        return error_response(StatusCode::UNAUTHORIZED, "invalid_token");
    }
    // The API contract only supports `status=online` (see docs/ADMIN_PRESENCE_DEVELOPMENT.md);
    // reject anything else explicitly rather than silently ignoring the filter.
    match query.status.as_deref() {
        Some("online") => {}
        other => {
            audit(
                "list_devices",
                remote,
                "denied",
                &format!("unsupported status filter: {other:?}"),
            );
            return error_response(StatusCode::BAD_REQUEST, "unsupported_status_filter");
        }
    }
    let devices: Vec<DeviceView> = state
        .pm
        .list_online(REG_TIMEOUT)
        .await
        .into_iter()
        .map(|d| DeviceView {
            id: d.id,
            name: d.name,
            last_seen_secs: d.last_seen_ms as f64 / 1000.0,
        })
        .collect();
    audit(
        "list_devices",
        remote,
        "granted",
        &format!("count={}", devices.len()),
    );
    (StatusCode::OK, Json(DevicesResponse { devices })).into_response()
}

/// Starts the admin presence API, if configured. Returns `Ok(())` without
/// binding any socket when neither auth method is configured (no
/// authorized keys and no legacy `ADMIN_API_TOKEN_HASH`), so the API is
/// disabled (fail-closed) by default rather than exposing an unauthenticated
/// or weakly-authenticated endpoint.
pub(crate) async fn serve(pm: PeerMap, bind_addr: Option<IpAddr>) -> ResultType<()> {
    let keys_path = AdminKeyStore::resolve_default_path();
    let keys = AdminKeyStore::load(&keys_path).map_err(|e| {
        hbb_common::anyhow::anyhow!("failed to load admin key store {}: {e}", keys_path.display())
    })?;

    let token_hash = get_arg_opt("ADMIN_API_TOKEN_HASH").filter(|v| !v.is_empty());
    if token_hash.is_some() {
        log::warn!(
            "ADMIN_API_TOKEN_HASH is set; the legacy shared-token admin login is enabled for \
             migration. Prefer enrolling clients with `rustdesk-utils genadminkey` (ed25519 \
             challenge-response) and unset ADMIN_API_TOKEN_HASH once all clients are migrated."
        );
    }
    if token_hash.is_none() && keys.is_empty() {
        log::warn!(
            "Admin presence API is disabled: no authorized admin keys and no \
             ADMIN_API_TOKEN_HASH configured. Enroll a client with \
             `rustdesk-utils genadminkey <label>` to enable the API."
        );
        return Ok(());
    }

    let jwt_secret = match get_arg_opt("ADMIN_API_JWT_SECRET") {
        Some(v) if !v.is_empty() => v,
        _ => {
            log::warn!(
                "ADMIN_API_JWT_SECRET not set; using a random ephemeral secret. \
                 Admin sessions will not survive a server restart. Set \
                 ADMIN_API_JWT_SECRET explicitly for stable sessions."
            );
            uuid::Uuid::new_v4().to_string()
        }
    };
    let port: u16 = get_arg_opt("ADMIN_API_PORT")
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_ADMIN_API_PORT);
    let ip = bind_addr.unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    let addr = SocketAddr::new(ip, port);

    let state = AdminApiState {
        pm,
        token_hash: token_hash.map(|v| Arc::from(v.as_str())),
        jwt_secret: Arc::from(jwt_secret.as_str()),
        keys: Arc::new(keys),
        challenges: Arc::new(ChallengeStore::default()),
    };

    let app = Router::new()
        .route(&format!("{ADMIN_API_PATH_PREFIX}/auth/login"), post(login))
        .route(
            &format!("{ADMIN_API_PATH_PREFIX}/auth/challenge"),
            post(auth_challenge),
        )
        .route(
            &format!("{ADMIN_API_PATH_PREFIX}/auth/verify"),
            post(auth_verify),
        )
        .route(&format!("{ADMIN_API_PATH_PREFIX}/devices"), get(list_devices))
        .layer(Extension(state));

    log::info!("Admin presence API listening on {addr}");
    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issued_token_verifies_and_rejects_wrong_secret() {
        let (token, ttl) = issue_token("test-secret").unwrap();
        assert_eq!(ttl, JWT_TTL_SECS);
        assert!(verify_token("test-secret", &token));
        assert!(!verify_token("wrong-secret", &token));
    }

    #[test]
    fn verify_token_rejects_garbage() {
        assert!(!verify_token("test-secret", "not-a-jwt"));
    }

    /// End-to-end coverage of the v2.0.0 key-based auth path at the handler
    /// level (challenge -> sign -> verify -> bearer-gated device list),
    /// exercising the same code the HTTP routes call.
    #[tokio::test]
    async fn key_based_auth_issues_a_session_that_gates_devices() {
        use sodiumoxide::crypto::sign;

        let keys_path = std::env::temp_dir().join(format!(
            "admin_api_test_keys_{}.json",
            uuid::Uuid::new_v4()
        ));
        let keys = AdminKeyStore::load(&keys_path).unwrap();
        let (pk, sk) = sign::gen_keypair();
        keys.add(&pk, "test-admin").unwrap();
        let pk_b64 = base64::encode(pk.as_ref());

        let challenges = ChallengeStore::default();
        let nonce = challenges.issue(&keys, &pk_b64).unwrap();
        let signed = sign::sign(nonce.as_bytes(), &sk);
        let sig_len = signed.len() - nonce.as_bytes().len();
        let sig_b64 = base64::encode(&signed[..sig_len]);

        let fingerprint = challenges.verify(&keys, &pk_b64, &nonce, &sig_b64).unwrap();
        assert!(!fingerprint.is_empty());

        let (token, _ttl) = issue_token("test-secret").unwrap();
        assert!(verify_token("test-secret", &token));

        let _ = std::fs::remove_file(&keys_path);
    }
}
