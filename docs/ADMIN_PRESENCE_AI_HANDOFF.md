# Admin Presence AI Handoff

This document gives a new AI agent enough context to understand and continue the **server-side** RustDeskAdmin work safely from zero prior history.

It is public-safe by design: use placeholders only, and do not replace them with real hostnames, IP addresses, private keys, peer IDs, credentials, or deployment-specific paths.

> Client-side UI, storage, and workflow details live in the paired repository **`rustdeskadmin-client`**, which maintains its own AI handoff document. This file is for the `rustdeskadmin-server` fork only.

## 1. Project overview

This fork adds an authenticated administrator presence API to `hbbs` so an authorized admin client can list devices currently online with the same self-hosted RustDesk rendezvous server.

The feature is intentionally narrow:

- it lists online devices
- it lets the admin click a device ID in the client UI
- the actual connection still uses RustDesk's normal, unchanged connection path
- it must not bypass target passwords, consent, permissions, ACLs, or unattended-access controls

## 2. Repository/remotes model

Typical fork layout:

```text
rustdeskadmin-server/
  origin:   https://github.com/<your-user>/rustdeskadmin-server.git
  upstream: https://github.com/rustdesk/rustdesk-server.git
```

Keep `upstream` fetch-only (or disable its push URL). Push only to the fork.

## 3. Current server state (PREPARING v2.0.1)

GitHub Latest remains **v2.0.0**. The approved **2.0.1** branding/version patch is not published; source metadata, binary/package versions, and tag **`v2.0.1`** must agree before release.

- Display product: **RustDeskAdmin Server - RustDesk Fork**; retain upstream copyright and add Henrique Rezende.
- Expected assets: `rustdeskadmin-server-2.0.1-linux-amd64.zip`, `rustdesk-server-hbbs_2.0.1_amd64.deb`, `rustdesk-server-hbbr_2.0.1_amd64.deb`, and `rustdesk-server-utils_2.0.1_amd64.deb`.
- Build the Linux amd64 server binaries once and reuse that exact output for ZIP and all three Debian packages. Run server and client builds sequentially.
- Keep `hbbs`, `hbbr`, `rustdesk-utils`, package/service names, and the v2.0.0 admin API contract. No Windows, ARM, or 32-bit release assets and no `RustDeskDeploy.exe` wrapper are approved for this patch.
- The existing local Docker deployment/image remains **`rustdeskadmin-server:2.0.0`**; do not rebuild/redeploy it or claim a v2.0.1 image upgrade. New GHCR image publication is not part of this package release.
- Publish as Latest only after verifying source/artifact identity. Verify all four new downloads **before** removing the 17 old v2.0.0 assets, including ARM/32-bit/Windows packages; retain its tag and source archives. The client independently prepares v2.2.1 and a three-asset cleanup of v2.2.0.
- After publication/download verification, finalize README, deployment/development guides, changelog, this handoff, and shared local memory. Until then keep the PREPARING state.

### Authentication model

The server is now **ed25519-only** for admin authentication.

Supported admin routes:

- `POST /admin/v1/auth/challenge`
- `POST /admin/v1/auth/verify`
- `GET /admin/v1/devices?status=online`

Removed in v2.0.0:

- `ADMIN_API_TOKEN_HASH`
- `rustdesk-utils hashtoken`
- `rustdesk-utils initadmin`
- `POST /admin/v1/auth/login`

This means pre-v2 admin clients that only know the shared-token flow are no longer compatible with the server.

### Key management model

Per-client admin keys are managed with:

```bash
rustdesk-utils genadminkey "<label>" [keys-file]
rustdesk-utils listadminkeys [keys-file]
rustdesk-utils revokeadminkey <fingerprint> [keys-file]
```

Behavior:

- `genadminkey` generates a new ed25519 keypair.
- It stores only the public key plus metadata server-side.
- It prints the private key **once** for the admin client to import.
- `listadminkeys` shows label, fingerprint, enrolled time, last-used time, and revoked status.
- `revokeadminkey` disables exactly one enrolled client key.

Default key-store path:

```text
admin_authorized_keys.json
```

Override via:

```text
ADMIN_API_KEYS_FILE
```

### Fail-closed behavior

`src/admin_api.rs::serve()` loads the key store at startup.

- If there are **no enrolled keys**, `hbbs` logs a warning and does **not** bind the admin API port.
- If `hbbs` starts with at least one key, the API listens on `ADMIN_API_PORT` (default `21114`).

Operational implication:

- First key should ideally be enrolled before starting `hbbs`.
- If the first key is added after `hbbs` already started empty, restart `hbbs` once.

### Hot-reload behavior

`src/admin_keys.rs` implements mtime-based lazy reload via `AdminKeyStore::refresh_if_stale()`, called from `find_active()`.

Practical effect:

- once the API is already running with at least one key
- later `genadminkey` and `revokeadminkey` changes are picked up automatically
- no restart is required for those subsequent changes

### Device-list behavior

`GET /admin/v1/devices?status=online`:

- reads only the existing in-memory rendezvous `PeerMap`
- uses the existing registration timeout semantics
- returns least-privilege device information (`id`, optional `name`, `last_seen_secs`)
- does not expose peer IP addresses, DB contents, file contents, or logs to the client

### JWT behavior

Successful `auth/verify` returns a short-lived bearer token used by `/admin/v1/devices`.

`ADMIN_API_JWT_SECRET`:

- should be set explicitly for stable sessions across restarts
- falls back to an ephemeral random value if omitted

## 4. Important server files

| File | Purpose |
| --- | --- |
| `src/admin_api.rs` | HTTP route definitions, bearer-token issuance/validation, audit logging, fail-closed startup, and device listing. |
| `src/admin_auth.rs` | Challenge issuance and verification; nonce TTL, single-use semantics, and signature checks. |
| `src/admin_keys.rs` | Authorized-key persistence, fingerprinting, revocation, last-used tracking, and hot reload. |
| `src/peer.rs` | Online-device view projected from the existing rendezvous peer state. |
| `src/rendezvous_server.rs` | Starts the admin API alongside the rendezvous server. |
| `src/utils.rs` | Admin key lifecycle CLI commands. |
| `docs/DEPLOYMENT.md` | Generic Linux deployment instructions for systemd, `docker run`, and Compose. |

## 5. HTTP contract summary

### Step 1: challenge request

```http
POST /admin/v1/auth/challenge
Content-Type: application/json

{"public_key":"<base64 raw ed25519 public key>"}
```

Success response:

```json
{"nonce":"<hex nonce>","expires_in":30}
```

### Step 2: verify signed challenge

```http
POST /admin/v1/auth/verify
Content-Type: application/json

{
  "public_key":"<base64 raw ed25519 public key>",
  "nonce":"<hex nonce>",
  "signature":"<base64 detached ed25519 signature over the nonce bytes>"
}
```

Success response:

```json
{
  "access_token":"<jwt>",
  "token_type":"Bearer",
  "expires_in":900
}
```

### Step 3: list devices

```http
GET /admin/v1/devices?status=online
Authorization: Bearer <jwt>
```

Response shape:

```json
{
  "devices": [
    {
      "id": "123456789",
      "name": "example-endpoint",
      "last_seen_secs": 1.2
    }
  ]
}
```

Important behavior details:

- unknown and revoked keys fail closed
- nonces are key-scoped
- nonces are single-use
- nonces expire quickly
- unsupported status filters are rejected
- auth and device-list requests are audit-logged

## 6. Security constraints

Do not regress any of these:

- no unauthenticated enumeration
- no bypass of target-side RustDesk security
- no direct DB/file/log browsing from clients
- no exposure of peer IP addresses through this API
- no protocol changes to ordinary RustDesk clients unless absolutely necessary
- no real secrets or environment-specific values in docs or commits

The current design deliberately keeps the admin API on a separate HTTP port instead of trying to reuse dormant RustDesk protocol messages.

## 7. Build and validation notes

Typical server build:

```bash
git submodule update --init --recursive
cargo build --release
```

Useful validation commands:

```bash
cargo test --lib admin_api::tests -- --nocapture
cargo test --lib admin_keys::tests -- --nocapture
cargo test --lib admin_auth::tests -- --nocapture
curl -i http://127.0.0.1:21114/admin/v1/devices?status=online
```

Expected API-port behavior after startup with at least one key:

- `curl` gets a JSON `401 missing_token`
- `hbbs` logs `admin_api audit` entries when the real client performs auth/list operations

## 8. Deployment summary

Use **`docs/DEPLOYMENT.md`** for exact Linux commands.

Key operational points:

- enroll the first admin key before starting `hbbs`, or restart once after first enrollment if `hbbs` started empty
- later key enrollments/revocations hot-reload automatically
- keep `ADMIN_API_JWT_SECRET` stable
- expose the admin API only behind HTTPS and network-layer controls appropriate for an administrative endpoint

## 9. Change discipline

When changing externally visible server behavior, update together:

- `README.md`
- `docs/DEPLOYMENT.md`
- `docs/ADMIN_PRESENCE_DEVELOPMENT.md`
- `docs/ADMIN_PRESENCE_CHANGELOG.md`
- this handoff file

Keep all examples generic and public-safe.
