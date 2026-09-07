# Admin Presence AI Handoff

This document gives a new AI agent enough context to understand and continue the custom RustDesk admin-presence project from zero prior knowledge. It is written for a public fork and intentionally uses example hostnames, example addresses, and generated placeholder secrets only.

Do not replace the placeholders in this file with private lab values, credentials, peer IDs, private keys, generated server keys, or production endpoints.

## 1. Project overview

This project customizes the official RustDesk client and server to support an administrator-only online-device view for a self-hosted RustDesk deployment.

The feature has two coordinated parts:

1. A RustDesk server extension that exposes an authenticated, versioned admin API.
2. A RustDesk Windows Flutter client extension that displays online devices in an embedded admin pane.

The admin view is only a discovery and navigation surface. Selecting a device starts RustDesk's existing connection flow with that device ID. It must not bypass target passwords, consent dialogs, permission controls, ACLs, unattended-access settings, or any other normal RustDesk security behavior.

## 2. Repository layout

Use two forks:

```text
rustdesk-client/
  origin:   https://github.com/<your-user>/rustdesk.git
  upstream: https://github.com/rustdesk/rustdesk.git

rustdesk-server/
  origin:   https://github.com/<your-user>/rustdesk-server.git
  upstream: https://github.com/rustdesk/rustdesk-server.git
```

Push only to your forks. Do not push to the official upstream repositories. Keep upstream remotes fetch-only or set their push URL to a disabled value.

## 3. Example lab topology

Use a private test lab before production:

| Example name | Role | Example OS | Example address |
|---|---|---|---|
| `dev-admin-pc` | Development workstation and Windows admin client | Windows 11 | `10.10.0.10` |
| `example-admin-api-01` | RustDesk rendezvous/relay/admin API server | Ubuntu Server | `10.10.0.20` |
| `example-managed-endpoint-01` | Test managed endpoint | Windows 11 | `10.10.0.30` |

The addresses above are documentation examples only. Use addresses from your own private network.

All RustDesk clients used in a test must point to the same self-hosted rendezvous/relay server. The admin API tells the client which devices are online, but the actual connection attempt still uses the normal RustDesk server settings.

## 4. Development environment setup

### Windows client build machine

Install:

- Git.
- GitHub CLI.
- Rustup.
- Rust toolchain pinned by the client repository, currently `1.75.0-x86_64-pc-windows-msvc`.
- `rustfmt` for the pinned Rust toolchain.
- Visual Studio 2022 Build Tools with the C++ workload.
- LLVM.
- Python 3.12.
- CMake.
- Ninja.
- NASM.
- Flutter 3.24.5.
- vcpkg pinned to the repository-required revision.

Set:

```powershell
$env:VCPKG_ROOT = "C:\dev\vcpkg"
```

The client `hwcodec` dependency expects static FFmpeg in the `x64-windows-static` vcpkg triplet. Install it using the RustDesk client overlay ports/triplets:

```powershell
$env:VCPKG_MAX_CONCURRENCY = "1"
$env:CL = "/MP1"
& $env:ComSpec /c 'call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat" && cd /d C:\dev\vcpkg && vcpkg install --classic "ffmpeg[amf,core,nvcodec,qsv]:x64-windows-static" --overlay-ports="C:\dev\rustdesk-client\res\vcpkg" --overlay-triplets="C:\dev\rustdesk-client\res\vcpkg-triplets" --x-install-root="C:\dev\vcpkg\installed"'
```

Expected files:

```text
C:\dev\vcpkg\installed\x64-windows-static\include\libavutil\attributes.h
C:\dev\vcpkg\installed\x64-windows-static\lib\avcodec.lib
```

Install the Flutter Rust bridge generator version expected by the client repo:

```powershell
cargo install flutter_rust_bridge_codegen --version 1.80.1 --features "uuid" --locked
```

Baseline client checks:

```powershell
cd C:\dev\rustdesk-client
cargo build --locked --features flutter,hwcodec --lib
cargo build --release --locked --features flutter,hwcodec --lib
cd flutter
C:\dev\flutter\3.24.5\bin\flutter.bat build windows --release
```

### Linux server build machine

On Ubuntu:

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev protobuf-compiler git curl
curl https://sh.rustup.rs -sSf | sh
source "$HOME/.cargo/env"
cd ~/rustdesk-server
git submodule update --init --recursive
cargo build --release
```

Expected binaries:

```text
target/release/hbbs
target/release/hbbr
target/release/rustdesk-utils
```

Install `hbbs` and `hbbr` as services using the repository's systemd templates or your deployment standard. Run them as an unprivileged service account.

### Deployment

Server (systemd):

1. `cargo build --release` on the Linux build machine.
2. Copy `target/release/{hbbs,hbbr,rustdesk-utils}` to the target host.
3. Install as `rustdesk-hbbs`/`rustdesk-hbbr` systemd services using the repo's `systemd/` templates.
4. Set the admin token/JWT secret: `rustdesk-utils initadmin /var/lib/rustdesk-server/.env` (writes `ADMIN_API_TOKEN_HASH`/`ADMIN_API_JWT_SECRET` into the `.env` file both systemd units already load from their `WorkingDirectory`, printing the plaintext token once) — or set `ADMIN_API_TOKEN_HASH`/`ADMIN_API_JWT_SECRET`/`ADMIN_API_PORT` as service environment variables by hand instead.
5. Open firewall ports for the rendezvous/relay ports plus the admin API port (`21114` by default).

Server (Docker): this repo's shipped `docker/Dockerfile` and `docker-classic/Dockerfile` both expect prebuilt `hbbs`/`hbbr` binaries already present in the build context (normally populated by CI). To build a self-contained image from source instead, use a small multi-stage Dockerfile:

```dockerfile
# syntax=docker/dockerfile:1
FROM rust:1-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev protobuf-compiler && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/hbbs /usr/local/bin/hbbs
COPY --from=build /src/target/release/hbbr /usr/local/bin/hbbr
COPY --from=build /src/target/release/rustdesk-utils /usr/local/bin/rustdesk-utils
WORKDIR /data
VOLUME /data
EXPOSE 21115 21116 21116/udp 21117 21118 21119 21114
ENTRYPOINT ["/usr/local/bin/hbbs"]
```

```bash
docker build -t rustdesk-hbbs-admin .
docker volume create rustdesk-data
docker run -d --name rustdesk-hbbs --network host \
  -v rustdesk-data:/data \
  rustdesk-hbbs-admin

docker run -d --name rustdesk-hbbr --network host \
  -v rustdesk-data:/data \
  --entrypoint /usr/local/bin/hbbr \
  rustdesk-hbbs-admin

docker run --rm -v rustdesk-data:/data --entrypoint /usr/local/bin/rustdesk-utils \
  rustdesk-hbbs-admin initadmin /data/.env   # generates & prints the admin token, writes ADMIN_API_TOKEN_HASH/ADMIN_API_JWT_SECRET into /data/.env
docker restart rustdesk-hbbs rustdesk-hbbr
```

Prefer `--network host` (Linux Docker hosts only) over published/bridge ports: RustDesk's UDP hole-punching relies on `hbbs` seeing each client's real source port, and Docker's bridge NAT can rewrite it, degrading traversal reliability. If host networking isn't available (e.g. Docker Desktop on Windows/macOS), fall back to `-p 21115-21119:21115-21119 -p 21116:21116/udp -p 21114:21114` on a bridge network and validate connectivity between two real clients before relying on it in production. Persist the `/data` volume across restarts so the server keypair isn't regenerated.

Client: copy the built `flutter/build/windows/x64/runner/Release/` folder (the `.exe` plus sibling DLLs — there is no separate installer) to each admin workstation or managed endpoint. Stop any running `rustdesk.exe` on the target before overwriting. Configure Settings > Network (rendezvous/relay + key) and Settings > Network > Admin Presence (admin API `host:port` + token) to point at the **same** self-hosted server deployment.

## 5. Server implementation details

The server change is additive. It adds an authenticated HTTP API beside the normal rendezvous service and reuses the existing in-memory peer registration state.

Important files:

| File | Purpose |
|---|---|
| `src/admin_api.rs` | Defines auth (ed25519 challenge/verify, primary; legacy shared-token login, deprecated), JWT validation, online-device listing, and audit logging. |
| `src/admin_keys.rs` | `AdminKeyStore`: JSON-file-backed store of authorized per-client ed25519 public keys (add/revoke/list/find-active/touch-last-used) plus `fingerprint_of()`. |
| `src/admin_auth.rs` | `ChallengeStore`: ed25519 challenge-response state machine (per-key-scoped, single-use, 30s TTL nonces). |
| `src/peer.rs` | Adds `OnlineDevice` and `PeerMap::list_online()`. |
| `src/rendezvous_server.rs` | Starts the admin API task beside the existing rendezvous server. |
| `src/lib.rs` | Registers the admin API, `admin_keys`, and `admin_auth` modules. |
| `src/utils.rs` | Adds `rustdesk-utils genadminkey <label>` / `listadminkeys` / `revokeadminkey <fingerprint>` (v2.0.0, primary key lifecycle) and `hashtoken <token>` / `initadmin` (legacy, still available for v1.x migration). |

### Server API (v2.0.0: per-client ed25519 challenge-response, primary)

Step 1 — request a challenge for this client's public key:

```http
POST /admin/v1/auth/challenge
Content-Type: application/json

{"public_key":"<base64 raw ed25519 public key>"}
```

Response:

```json
{"nonce": "<hex nonce>", "expires_in": 30}
```

Step 2 — sign the nonce with the matching private key and verify:

```http
POST /admin/v1/auth/verify
Content-Type: application/json

{
  "public_key": "<base64 raw ed25519 public key>",
  "nonce": "<hex nonce from step 1>",
  "signature": "<base64 detached ed25519 signature over the nonce's UTF-8 bytes>"
}
```

Response (same shape the legacy login issues):

```json
{
  "access_token": "<jwt>",
  "token_type": "Bearer",
  "expires_in": 900
}
```

Both endpoints fail closed with the same error shape for unknown/revoked keys (`401 unknown_or_revoked_key`), so neither can be used to enumerate which keys are registered. A nonce is scoped to the exact key it was issued for, is single-use (consumed on first verify attempt, success or failure), and expires after 30 seconds.

Legacy login (v1.x, deprecated — only for migration, requires `ADMIN_API_TOKEN_HASH` to still be configured or returns `404 not_supported`):

```http
POST /admin/v1/auth/login
Content-Type: application/json

{"token":"<admin-token>"}
```

Device list (unchanged since v1.x — both auth paths issue the same JWT bearer token this endpoint consumes):

```http
GET /admin/v1/devices?status=online
Authorization: Bearer <jwt>
```

Response:

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

### Server security model

Configuration:

| Setting | Required | Purpose |
|---|---|---|
| `ADMIN_API_KEYS_FILE` | Optional | Path to the authorized-keys JSON file managed by `rustdesk-utils genadminkey`/`listadminkeys`/`revokeadminkey`. Defaults to a file in the current working directory. |
| `ADMIN_API_TOKEN_HASH` | Only for v1.x migration | bcrypt hash of the legacy shared admin token. If neither this nor at least one enrolled key exists, the API is disabled fail-closed. |
| `ADMIN_API_JWT_SECRET` | Strongly recommended | Stable random secret for signing 15-minute JWTs. |
| `ADMIN_API_PORT` | Optional | API port, default `21114`. |

Implemented protections:

- No unauthenticated device enumeration.
- Primary auth (v2.0.0): per-client ed25519 challenge-response — a client must hold the private key matching one of the server's authorized public keys, and can be individually revoked (`revokeadminkey`) without affecting any other enrolled client.
- Legacy auth (v1.x, deprecated): shared bcrypt-verified token, all-or-nothing revocation (rotate the one shared secret).
- Device list requires a valid short-lived JWT (issued identically by either auth path).
- Unsupported status filters are rejected.
- Device list reads only in-memory rendezvous state.
- No direct database, file, or log access is exposed.
- Response omits IP addresses and returns least-privilege data only.
- Every auth attempt (challenge issuance, verify, legacy login) and device-list query is audit-logged.
- Existing RustDesk protocol messages are not changed. (An earlier design considered tunnelling the admin API over the rendezvous connection via the protocol's dormant `HttpProxyRequest`/`KeyExchange` messages; this was rejected because `KeyExchange` handling doesn't exist server-side in this fork and implementing it would mean pushing new messages onto the main rendezvous port, risking interference with stock clients. The admin API keeps its own separate port instead.)

Enroll the first per-client key (recommended, v2.0.0):

```bash
./rustdesk-utils genadminkey "nina-laptop"
# Prints the 64-byte private key ONCE — copy it into the client's
# Settings > Network > Admin Presence > "Enroll key" field immediately.
./rustdesk-utils listadminkeys
./rustdesk-utils revokeadminkey <fingerprint>   # to revoke a single client
```

Or, only for migrating an existing v1.x deployment, generate and configure the legacy shared token in one step:

```bash
./rustdesk-utils initadmin
```

Or generate just the legacy bcrypt hash for a token you already chose yourself:

```bash
./rustdesk-utils hashtoken '<long-random-admin-token>'
```

Example generated-looking values for documentation only:

```text
ADMIN_API_TOKEN_HASH=$2b$12$t8sUjHoDIv3QcWnu0wF0weJvwmjGo7G8Gm7jD4iowdOYOxbV4XzpO
ADMIN_API_JWT_SECRET=example-9f0e3d2c1b0a4e5f8a7b6c5d4e3f2a1b
ADMIN_API_PORT=21114
```

Do not use those example values in a real deployment.

### Presence semantics

The server considers a peer online if its latest rendezvous registration/heartbeat is within the existing rendezvous timeout. The implementation uses `PeerMap::list_online(REG_TIMEOUT)`.

If safe hostname metadata exists in `PeerInfo`, the server includes it as optional `name`. If not, the client falls back to its local peer caches or the raw RustDesk ID.

## 6. Client implementation details

The client change is additive and Flutter-focused. It adds a desktop admin pane and settings UI without changing RustDesk's target connection authorization behavior. All admin-presence logic — including the v2.0.0 keypair handling — is pure Dart; no Rust source file or `flutter_rust_bridge` binding was touched.

Important files:

| File | Purpose |
|---|---|
| `flutter/lib/models/admin_presence_model.dart` | Admin API auth (v2.0.0 key-based `loginWithKeyPair()`, primary; legacy `login(token)`, deprecated), JWT handling, refresh, stale cache, local-ID filtering. |
| `flutter/lib/models/admin_presence_keypair.dart` | v2.0.0: keypair load/import/clear, fingerprint derivation, DPAPI-encrypted local storage of the private key seed. |
| `flutter/lib/common/widgets/admin_presence_dialog.dart` | Embedded `AdminPresencePane` and device cards. |
| `flutter/lib/common/widgets/peer_tab_page.dart` | Adds the first-left admin icon and renders the admin pane. |
| `flutter/lib/models/peer_tab_model.dart` | Adds `PeerTabIndex.admin` and excludes it from the draggable normal tab strip. |
| `flutter/lib/common/widgets/peers_view.dart` | Handles the admin logical tab in shared view wiring. |
| `flutter/lib/common/widgets/peer_card.dart` | Handles the admin logical tab in shared deletion switch logic. |
| `flutter/lib/consts.dart` | Adds admin-presence option keys. |
| `flutter/lib/desktop/pages/desktop_setting_page.dart` | Adds Settings > Network > Admin Presence, including the v2.0.0 key enrollment UI. |

### Client settings

The Windows admin client stores:

```dart
const String kOptionAdminPresenceServer = "admin-presence-server";
const String kOptionAdminPresenceToken = "admin-presence-token";           // legacy, deprecated
const String kOptionAdminPresenceDevices = "admin-presence-devices";
const String kOptionAdminPresencePrivateKeyEnc = "admin-presence-privkey-enc"; // v2.0.0
const String kOptionAdminPresencePublicKey = "admin-presence-pubkey";         // v2.0.0
```

Settings location:

```text
Settings > Network > Admin Presence
```

Fields:

- Admin server address: `host:port`, `http://host:port`, or `https://host`.
- Admin key (v2.0.0, primary): paste the 64-byte base64 private key printed once by `rustdesk-utils genadminkey` and click "Enroll key". The enrolled key's fingerprint is shown (matches the server's `admin_keys::fingerprint_of` exactly) so it can be cross-checked against `rustdesk-utils listadminkeys`. "Remove key from this device" clears local enrollment only — revoke server-side separately with `rustdesk-utils revokeadminkey`.
- Legacy shared token (v1.x, deprecated, collapsed by default): plaintext token matching the server-side bcrypt hash. Only consulted if no admin key is enrolled.

The v2.0.0 private key seed is DPAPI-encrypted at rest (`CryptProtectData`/`CryptUnprotectData`, current-user scope) via the `win32` package before being stored in the RustDesk local options store — decryption only succeeds on the same Windows user profile that enrolled it. The legacy token remains stored in plaintext in local options; migrate to a key when possible.

### Admin pane behavior

`AdminPresencePane`:

- Is embedded in the peer-tab content area.
- Is selected by the first icon on the left of the tab row.
- Auto-logins using saved settings.
- Auto-refreshes every 5 seconds.
- Shows server reachable/unreachable state.
- Lists online devices.
- Keeps missing devices as greyed-out stale/offline rows after successful refreshes.
- Shows offline duration for stale rows.
- Allows deleting stale rows with an `X`.
- Filters out the local admin client's own RustDesk ID.
- Shows a friendly name above the ID when the API or local peer caches provide one.

Clicking an online row calls the existing `connect(context, id)` path.

### ID normalization

RustDesk IDs may appear formatted with spaces in UI/cache contexts, for example:

```text
1 234 567 890
```

The API may return compact IDs:

```text
1234567890
```

Normalize before comparisons:

```dart
String _normalizeId(String id) => id.replaceAll(' ', '').trim();
```

Use normalized comparisons for:

- Filtering out the admin client's own ID.
- Matching API devices against local peer caches for friendly names.
- Deleting stale cached entries.

## 7. Validation workflow

### Server

```bash
cargo check --tests
cargo test --lib peer::tests::list_online_filters_by_timeout -- --nocapture
cargo test --lib admin_api::tests -- --nocapture
cargo test --lib admin_keys::tests -- --nocapture
cargo test --lib admin_auth::tests -- --nocapture
cargo build --release --bin hbbs
```

### Client

```powershell
cd C:\dev\rustdesk-client\flutter
$env:Path += ';C:\Program Files\Git\cmd'
C:\dev\flutter\3.24.5\bin\flutter.bat analyze `
  lib\models\admin_presence_model.dart `
  lib\models\admin_presence_keypair.dart `
  lib\common\widgets\admin_presence_dialog.dart `
  lib\common\widgets\peer_tab_page.dart `
  lib\common\widgets\peers_view.dart `
  lib\common\widgets\peer_card.dart `
  lib\models\peer_tab_model.dart `
  lib\desktop\pages\desktop_setting_page.dart `
  lib\consts.dart
C:\dev\flutter\3.24.5\bin\flutter.bat test test\admin_presence_keypair_test.dart
C:\dev\flutter\3.24.5\bin\flutter.bat build windows --release
```

Existing upstream/deprecation analyzer info messages may appear. New analyzer errors are blockers.

### API smoke tests

```bash
# v2.0.0 primary path: challenge/verify with an enrolled key
NONCE=$(curl -s -X POST https://admin-api.example.com/admin/v1/auth/challenge \
  -H 'Content-Type: application/json' \
  -d '{"public_key":"<base64 public key>"}' | jq -r .nonce)
SIG=$(sign_with_private_key "$NONCE")   # produce a base64 detached ed25519 signature
curl -X POST https://admin-api.example.com/admin/v1/auth/verify \
  -H 'Content-Type: application/json' \
  -d "{\"public_key\":\"<base64 public key>\",\"nonce\":\"$NONCE\",\"signature\":\"$SIG\"}"

# legacy path (only if ADMIN_API_TOKEN_HASH is still configured)
curl -X POST https://admin-api.example.com/admin/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"token":"<admin-token>"}'

curl https://admin-api.example.com/admin/v1/devices?status=online \
  -H "Authorization: Bearer <jwt>"
```

Expected:

- Valid challenge+verify returns a JWT; an unknown/revoked public key returns `401 unknown_or_revoked_key` from `/auth/challenge` (fails closed, does not reveal whether the key is unknown vs. revoked).
- A stale/replayed/wrong-key nonce returns `401` from `/auth/verify` (`no_such_challenge`/`challenge_expired`).
- Valid legacy login returns a JWT (only if `ADMIN_API_TOKEN_HASH` is configured; otherwise `404 not_supported`).
- Missing/invalid legacy login token returns `401`.
- Missing/invalid bearer token returns `401`.
- Unsupported status filter returns `400`.
- Online endpoint appears while registered.
- Endpoint disappears from the online API response after the rendezvous timeout.
- Client pane retains disappeared devices as stale/offline entries.
- Clicking an online row starts the normal RustDesk connection flow.

## 8. Internet exposure guidance

The admin API is authenticated and least-privilege, but it is still an administrative endpoint.

Do not expose it directly over plaintext HTTP on the internet.

Before internet exposure:

1. Put the admin API behind HTTPS/TLS.
2. Restrict access with firewall allow-lists, VPN, WireGuard, Tailscale, Zero Trust access, or equivalent controls.
3. Add rate limiting to `/admin/v1/auth/challenge`, `/admin/v1/auth/verify`, and (if still enabled) `/admin/v1/auth/login`.
4. Prefer per-client ed25519 keys (`rustdesk-utils genadminkey`) over the legacy shared token — revoke a single compromised client with `rustdesk-utils revokeadminkey <fingerprint>` instead of rotating one secret for every admin.
5. If the legacy shared token is still in use during migration, use a long random value and store only the bcrypt hash in `ADMIN_API_TOKEN_HASH`.
6. Set a stable random `ADMIN_API_JWT_SECRET`.
7. Rotate/revoke keys (or the legacy token/JWT secret) if disclosure is suspected.
8. Monitor audit logs for failed login attempts and unusual list activity.
9. Consider mTLS/OIDC, scoped admin identities, revocation, and OS-protected client token storage for production.

Current security posture: the query is authenticated, versioned, audited, and least-privilege, but it should still be protected by HTTPS and network-layer access controls before being opened to the public internet.

## 9. Change discipline

When continuing this work:

- Keep changes additive and localized.
- Do not bypass target-side RustDesk security.
- Do not expose unauthenticated enumeration.
- Do not expose direct database/file/log access to clients.
- Avoid changing existing RustDesk protocol messages unless unavoidable.
- Update public docs and changelogs with sanitized information.
- Do not commit passwords, tokens, private keys, generated server keys, real peer IDs, private IPs, public IPs, or lab-specific hostnames.
- Validate with the narrowest relevant tests/builds.
- Push only to the fork remotes.
