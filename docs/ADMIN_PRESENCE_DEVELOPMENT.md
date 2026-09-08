# Admin Presence Server Development

## Purpose

This fork adds an authenticated, administrator-only presence API to `hbbs` so the paired Windows client fork can list devices that are currently online with the same self-hosted RustDesk rendezvous server.

The admin pane is a discovery surface only. Selecting a device still uses RustDesk's normal, unmodified connection flow and does **not** bypass passwords, consent dialogs, permissions, ACLs, or unattended-access rules.

## Current server contract (v2.0.0)

- Auth model: **per-client ed25519 challenge-response only**.
- Routes:
  - `POST /admin/v1/auth/challenge`
  - `POST /admin/v1/auth/verify`
  - `GET /admin/v1/devices?status=online`
- Enrollment CLI:
  - `rustdesk-utils genadminkey <label> [keys-file]`
  - `rustdesk-utils listadminkeys [keys-file]`
  - `rustdesk-utils revokeadminkey <fingerprint> [keys-file]`
- Key store: `admin_authorized_keys.json` by default, or `ADMIN_API_KEYS_FILE` if set.
- API port: `21114` by default, or `ADMIN_API_PORT` if set.
- JWT signing: `ADMIN_API_JWT_SECRET` is strongly recommended for stable sessions.
- Fail-closed behavior: if `hbbs` starts with no enrolled keys, the admin API does not bind a socket.
- Hot-reload behavior: after the API is already running with at least one enrolled key, later enroll/revoke operations are picked up automatically from disk with no restart.

## Security boundaries

- No unauthenticated device enumeration.
- No direct database, file, or log access is exposed to clients.
- Device listing reuses the existing in-memory rendezvous peer map and normal registration timeout.
- Responses omit peer IP addresses and return least-privilege data only.
- Every auth attempt and device-list request is audit-logged by `hbbs`.
- The fork keeps the admin API on its own HTTP port; it does not change RustDesk's normal rendezvous or relay protocol.

## Important source files

| File | Purpose |
| --- | --- |
| `src/admin_api.rs` | Axum routes, JWT issuance/validation, fail-closed startup, audit logging, and online-device listing. |
| `src/admin_auth.rs` | Challenge issuance and verification state machine, including TTL and single-use nonce handling. |
| `src/admin_keys.rs` | Authorized-key store, fingerprinting, persistence, and mtime-based hot reload. |
| `src/peer.rs` | Online-device projection from the existing rendezvous peer map. |
| `src/rendezvous_server.rs` | Starts the admin API beside the normal rendezvous server. |
| `src/utils.rs` | `genadminkey`, `listadminkeys`, and `revokeadminkey` CLI commands. |

## Building from source

### Prerequisites

On a Linux build host:

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev protobuf-compiler git curl
curl https://sh.rustup.rs -sSf | sh -s -- -y
. "$HOME/.cargo/env"
```

### Build

```bash
git clone https://github.com/hrezenmsft/rustdeskadmin-server.git
cd rustdeskadmin-server
git submodule update --init --recursive
cargo build --release
```

Expected binaries:

```text
target/release/hbbs
target/release/hbbr
target/release/rustdesk-utils
```

### First key enrollment

Enroll the first admin client key **before** starting `hbbs`, or restart `hbbs` once after enrolling if it was already started with zero keys:

```bash
./target/release/rustdesk-utils genadminkey "<label>"
./target/release/rustdesk-utils listadminkeys
```

`genadminkey` prints the private key **once**. Copy it into the paired client fork immediately. The server stores only the public key and metadata.

To use a non-default key-store path, either pass it as the optional CLI argument or set `ADMIN_API_KEYS_FILE` consistently for both the CLI and `hbbs`.

### Running locally

Both env vars below are optional (defaults: port `21114`, ephemeral JWT secret). Set them explicitly if you want a fixed port or admin sessions that survive restarts:

```bash
export ADMIN_API_JWT_SECRET='<generate-a-long-random-secret>'
export ADMIN_API_PORT=21114
./target/release/hbbs -r <your-domain-or-ip>:21117
./target/release/hbbr
```

Run both binaries from the same working directory if you want the defaults for `.env`, `admin_authorized_keys.json`, server keys, and `db_v2.sqlite3` to stay together.

## Verifying behavior

Once `hbbs` has started with at least one enrolled key:

```bash
curl -i http://127.0.0.1:21114/admin/v1/devices?status=online
```

A listening admin API should respond with JSON `401 missing_token`; `curl` should no longer fail with connection refused.

Useful checks:

- `./target/release/rustdesk-utils listadminkeys`
- `RUST_LOG=info ./target/release/hbbs ...` and look for `admin_api audit action=auth_challenge`, `auth_verify`, and `list_devices`
- Connect with `rustdeskadmin-client` using the printed private key and the same server address

To revoke one client:

```bash
./target/release/rustdesk-utils revokeadminkey <fingerprint>
./target/release/rustdesk-utils listadminkeys
```

If the API was already running, revocation is picked up automatically.

## Deployment guidance

For copy-pasteable Linux deployment instructions, see **[docs/DEPLOYMENT.md](DEPLOYMENT.md)**. That guide covers:

- native `systemd` deployment
- plain `docker run`
- `docker compose`
- firewall ports
- enrollment, verification, and revocation workflows

## Documentation discipline

When this fork's externally visible behavior changes, update these files in the same change set:

- `README.md`
- `docs/DEPLOYMENT.md`
- `docs/ADMIN_PRESENCE_DEVELOPMENT.md`
- `docs/ADMIN_PRESENCE_CHANGELOG.md`
- `docs/ADMIN_PRESENCE_AI_HANDOFF.md`

Keep all examples generic. Do not commit real hostnames, IP addresses, credentials, private keys, peer IDs, or environment-specific file paths.
