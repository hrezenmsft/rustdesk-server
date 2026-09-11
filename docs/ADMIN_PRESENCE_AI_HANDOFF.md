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

## 3. Release v2.0.1

Release **v2.0.1** was published as **Latest** on **2026-09-10 UTC** (release ID `386695124`, neither draft nor prerelease). The [release notes](https://github.com/hrezenmsft/rustdeskadmin-server/releases/tag/v2.0.1) provide downloads and SHA-256 checksums. Tag commit `b19843d296d7b5697c3f593e9f9c76a843338767` adds **documentation only** to binary build commit `961d0886ee48ffc884d0186b25574070daca7fcb`; the comparison verified no runtime/build/package-source changes. Later documentation commits must not move the release tag.

- Display product: **RustDeskAdmin Server - RustDesk Fork**; retain upstream copyright and add Henrique Rezende.
- Exactly four published assets: `rustdeskadmin-server-2.0.1-linux-amd64.zip`, `rustdesk-server-hbbs_2.0.1_amd64.deb`, `rustdesk-server-hbbr_2.0.1_amd64.deb`, and `rustdesk-server-utils_2.0.1_amd64.deb`.
- The single static-musl build completed in 4m31s with `CARGO_BUILD_JOBS=2 cargo build --release --locked --target x86_64-unknown-linux-musl`. All three binaries are x86-64 static PIE and already stripped. `hbbs` and `hbbr` report `2.0.1`; `rustdesk-utils` does **not** support `--version`.
- The ZIP contains exactly `hbbs`, `hbbr`, `rustdesk-utils`, and `RELEASE-NOTICE.txt`. Ubuntu 22.04 packaging used `DEB_BUILD_OPTIONS=nostrip debuild -i -us -uc -b -aamd64` to prevent debhelper rewriting stripped binaries. All three DEB binaries are byte-identical to the build and ZIP. Debian `2.0.1`/`amd64` metadata, maintainer, homepage, copyright, and services were checked. Keep this recipe and run server/client builds sequentially, not once per format.
- Keep `hbbs`, `hbbr`, `rustdesk-utils`, package/service names, and the v2.0.0 admin API contract. No Windows, ARM, or 32-bit release assets and no `RustDeskDeploy.exe` wrapper are approved for this patch.
- The initial 2026-09-10 package release made no Docker deployment changes and published no new GHCR images; the separately completed Docker follow-up is recorded below.
- All four packages were downloaded at draft stage and again via public HTTPS after publication; every hash matched the local original and GitHub SHA-256 digest. Validation covered runtime version and package/static/hash checks, not installation smoke tests.
- Retirement completed after verification: all 17 old server `v2.0.0` assets (including ARM/32-bit/Windows packages) and all three paired client `v2.2.0` assets were removed after verified backups. Old release pages, automatic source archives, and tags remain; descriptions link to the replacements. Server old tag object `c84ee56170f65c5325ee687195e06497cf2dc1cb` is unchanged. The paired client `v2.2.1` is also published as Latest with exactly three assets.
- All temporarily paused GitHub workflow states were restored to their originals. No unwanted CI rebuilds, server images, or ARM/32-bit/Windows server packages were triggered.

### Docker follow-up (2026-09-11 UTC)

- Published classic image **`ghcr.io/hrezenmsft/rustdeskadmin-server:v2.0.1`**, **Linux amd64 only**. Tags `v2.0.1`, `v2.0.1-amd64`, `latest`, `latest-amd64`, `v2`, and `v2-amd64` all resolve to the single OCI image manifest **`sha256:5020adfdfb60de969a2e2a62712f72ddf179906cde90f9a71816cdb9db06c5c1`** (GHCR package version `1235297545`).
- Docker packaging source and OCI revision: **`8cbc59c8950e24645cdbd81cc5499f2105907330`**. The three executables are byte-identical to the existing v2.0.1 Linux ZIP; **no Rust rebuild** occurred. Binary build commit remains `961d0886ee48ffc884d0186b25574070daca7fcb`, and Git release tag commit remains `b19843d296d7b5697c3f593e9f9c76a843338767`. Do not confuse packaging provenance with the binary build or move the release tag for later documentation.
- Published image was pulled and checked with isolated, network-disabled, read-only containers: `hbbs --version` and `hbbr --version` report `2.0.1`; all three binary bytes match the ZIP. Labels identify **RustDeskAdmin Server - RustDesk Fork**, author **Henrique Rezende**, version `v2.0.1`, license `AGPL-3.0-only`, and repository `https://github.com/hrezenmsft/rustdeskadmin-server`. `RELEASE-NOTICE.txt` and `LICENSE` are included at `/usr/share/doc/rustdeskadmin-server/`, preserving upstream attribution.
- Classic `FROM scratch`, `/usr/bin` binaries, `HOME`/`WORKDIR /root`, and the two-container topology are unchanged. Both services in `docker-compose.example.yml` pin `:v2.0.1` and document amd64-only support.
- Future classic CI scope is amd64 only: `docker-classic` has the reduced matrix, adds license/notices to the build context and `VERSION`/`VCS_REF` build arguments for labels; `docker-manifest-classic` aliases only amd64 and now depends on `docker-classic` rather than `docker`. **S6, binary, and DEB release matrices are unchanged.**
- Before retirement, all **16** original classic GHCR versions (manifests, configs, and layers) received a complete digest-verified OCI backup. Retired **8** historical ARM image versions and **4** obsolete multi-platform index versions. **5** package versions remain: the **4 original amd64 images** plus 2.0.1. Historical tags `v2.0.0`, `v1.1.4`, `v1`, and `v1.1.3` now select their **original amd64 images**, not relabeled 2.0.1 binaries; `v1.1.2-amd64` is preserved.
- Retired ARM tags/digests and old multi-platform index digests no longer work. Restoring a reference by an old manifest digest requires the backup; unchanged **Git release tags** do not imply unchanged Docker index digests. **No changes were made to the `rustdeskadmin-server-s6` GHCR package.**
- The running local server remains **`rustdeskadmin-server:2.0.0`**: no restart, recreation, upgrade, or deployment-configuration change. Do not apply the new Compose example to that stack without separate authorization. The four server and three client release assets and their completed old-asset retirements remain unchanged.

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
