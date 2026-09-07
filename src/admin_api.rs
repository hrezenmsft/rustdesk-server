//! Authenticated, versioned HTTP API used only by the custom admin-presence
//! Windows client to list devices currently online with this rendezvous
//! server. See docs/ADMIN_PRESENCE_DEVELOPMENT.md for the full contract.
//!
//! Design constraints (see docs/ADMIN_PRESENCE_DEVELOPMENT.md):
//! - No direct database/file access is exposed to clients; presence data
//!   comes only from `PeerMap::list_online`, which reads in-memory state
//!   populated by the existing rendezvous registration/heartbeat path.
//! - No unauthenticated enumeration: every route other than login requires a
//!   valid, short-lived bearer token issued by `/admin/v1/auth/login`.
//! - Least-privilege data: the device list exposes only the device id and
//!   how recently it was last seen; no IP address or other peer detail.
//! - Every login attempt and device-list query is audit-logged.
//! - This API is additive: it does not alter any existing RustDesk protocol
//!   message or the main rendezvous port; it listens on its own port.

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
    token_hash: Arc<str>,
    jwt_secret: Arc<str>,
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

#[derive(Debug, Serialize)]
struct DeviceView {
    id: String,
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
    let valid = bcrypt::verify(&req.token, &state.token_hash).unwrap_or(false);
    if !valid {
        audit("login", remote, "denied", "invalid admin token");
        return error_response(StatusCode::UNAUTHORIZED, "invalid_token");
    }
    match issue_token(&state.jwt_secret) {
        Ok((access_token, expires_in)) => {
            audit("login", remote, "granted", "");
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
/// binding any socket when `ADMIN_API_TOKEN_HASH` is not set, so the API is
/// disabled (fail-closed) by default rather than exposing an unauthenticated
/// or weakly-authenticated endpoint.
pub(crate) async fn serve(pm: PeerMap, bind_addr: Option<IpAddr>) -> ResultType<()> {
    let token_hash = match get_arg_opt("ADMIN_API_TOKEN_HASH") {
        Some(v) if !v.is_empty() => v,
        _ => {
            log::warn!(
                "ADMIN_API_TOKEN_HASH not set; admin presence API is disabled. \
                 Generate a hash with `rustdesk-utils hashtoken <token>` and set it \
                 via ADMIN_API_TOKEN_HASH to enable the API."
            );
            return Ok(());
        }
    };
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
        token_hash: Arc::from(token_hash.as_str()),
        jwt_secret: Arc::from(jwt_secret.as_str()),
    };

    let app = Router::new()
        .route(&format!("{ADMIN_API_PATH_PREFIX}/auth/login"), post(login))
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
}
