## 🛠️ RustDeskAdmin — Custom Admin-Presence Fork

This repository is a **public fork of the official [rustdesk/rustdesk-server](https://github.com/rustdesk/rustdesk-server)**, part of the `RustDeskAdmin` project (paired with [`rustdeskadmin-client`](https://github.com/hrezenmsft/rustdeskadmin-client)). This README documents only the administrator-presence additions in this fork.

### Release v2.0.1

Release **v2.0.1** packages the **2.0.1** server. Check the [v2.0.1 release page](https://github.com/hrezenmsft/rustdeskadmin-server/releases/tag/v2.0.1) for published asset availability. The designated build commit is `961d0886ee48ffc884d0186b25574070daca7fcb`; the final tag may add documentation-only changes without changing runtime/build/package source.

- Product name: **RustDeskAdmin Server - RustDesk Fork**. Preserve upstream copyright notices and add Henrique Rezende's fork attribution.
- Linux amd64 asset set: `rustdeskadmin-server-2.0.1-linux-amd64.zip`, `rustdesk-server-hbbs_2.0.1_amd64.deb`, `rustdesk-server-hbbr_2.0.1_amd64.deb`, and `rustdesk-server-utils_2.0.1_amd64.deb`. The ZIP contains `hbbs`, `hbbr`, `rustdesk-utils`, and `RELEASE-NOTICE.txt`.
- Keep `hbbs`, `hbbr`, `rustdesk-utils`, package/service names, and the existing admin API contract for compatibility. No Windows/ARM/32-bit assets or `RustDeskDeploy.exe` wrapper are part of this patch release.
- This package release does **not** upgrade the existing local Docker image/deployment (`rustdeskadmin-server:2.0.0`) or promise new GHCR images.
- Retirement of the 17 binary/package assets from old **v2.0.0**, including ARM, 32-bit, and Windows assets, remains pending publication as Latest and verification of all four new downloads. Keep the old tag and source archives.

### What this fork adds

- A new **authenticated, versioned admin API** exposed by `hbbs` on a separate port (default `21114`, override with `ADMIN_API_PORT`):
  - `POST /admin/v1/auth/challenge`
  - `POST /admin/v1/auth/verify`
  - `GET /admin/v1/devices?status=online` — returns devices currently registered online with this rendezvous server (ID, optional hostname, last-seen seconds), reading only the existing in-memory peer map and reusing the server's normal 30-second registration heartbeat/timeout — **no new database, file, or log access is exposed to clients**.
- **v2.0.0 is ed25519-only.** Each admin client enrolls its own keypair with `rustdesk-utils genadminkey <label>` instead of sharing one secret.
- `rustdesk-utils genadminkey <label>` / `listadminkeys` / `revokeadminkey <fingerprint>` manage the per-client key lifecycle.
- The authorized-key store (`admin_authorized_keys.json`, or `ADMIN_API_KEYS_FILE`) **hot-reloads** on change. Once the admin API is already running with at least one key, enrolling or revoking keys takes effect live with no service restart.
- The API is **fail-closed**: if `hbbs` starts with no enrolled admin keys, it does not bind the admin API port at all.
- No existing RustDesk protocol messages, wire formats, or database schema were changed — this fork is strictly additive.
- Audit log entries are written for every admin API auth attempt and device-list query.

### Documentation

- **[docs/DEPLOYMENT.md](docs/DEPLOYMENT.md)** — generic Linux deployment guide for systemd, plain `docker run`, and Docker Compose.
- **[docs/ADMIN_PRESENCE_DEVELOPMENT.md](docs/ADMIN_PRESENCE_DEVELOPMENT.md)** — implementation notes, source layout, build workflow, and development guidance for this fork.
- **[docs/ADMIN_PRESENCE_CHANGELOG.md](docs/ADMIN_PRESENCE_CHANGELOG.md)** — dated changelog of every change made in this fork, newest first.
- **[docs/ADMIN_PRESENCE_AI_HANDOFF.md](docs/ADMIN_PRESENCE_AI_HANDOFF.md)** — a public-safe context primer for AI coding agents picking up the server-side fork.

### Quick start

**Recommended: deploy from a prebuilt release package — no local build required.** Select a published Linux amd64 asset from the [v2.0.1 release page](https://github.com/hrezenmsft/rustdeskadmin-server/releases/tag/v2.0.1), following the [deployment guide](docs/DEPLOYMENT.md). Docker Compose remains an alternative using previously published GHCR images, not a v2.0.1 image upgrade. Existing-image quick start:
```bash
curl -O https://raw.githubusercontent.com/hrezenmsft/rustdeskadmin-server/master/docker-compose.example.yml
mv docker-compose.example.yml docker-compose.yml
# edit docker-compose.yml: set hbbs -r <your-domain-or-ip>:21117
# (optional) set ADMIN_API_JWT_SECRET to a long random secret so admin sessions
# survive a restart — the admin API works fine without it too.
docker compose pull
docker compose run --rm --no-deps hbbs rustdesk-utils genadminkey "<label>"
# Save the printed private key now and paste it into rustdeskadmin-client.
docker compose up -d
```
Use `docker compose exec hbbs rustdesk-utils listadminkeys` to verify enrollment, and `docker compose exec hbbs rustdesk-utils revokeadminkey <fingerprint>` to revoke one admin client. If `hbbs` was already started with zero keys, enroll a key and restart `hbbs` once so it begins listening on `21114`; after that, key-store changes hot-reload live with no restart needed.

See **[docs/DEPLOYMENT.md](docs/DEPLOYMENT.md)** for the full Linux deployment walkthroughs and **[docs/ADMIN_PRESENCE_DEVELOPMENT.md](docs/ADMIN_PRESENCE_DEVELOPMENT.md)** for source-level details — or grab packages directly from the **[Releases page](https://github.com/hrezenmsft/rustdeskadmin-server/releases)**.

Security note: the admin API is authenticated but administrative. If it is reachable beyond a trusted private network, deploy it behind HTTPS termination or restrict `ADMIN_API_PORT` to trusted VPN/source IPs. Direct HTTP on the public internet is not recommended because short-lived bearer tokens and device-presence metadata can be observed or tampered with. See **[docs/DEPLOYMENT.md#admin-api-transport-security](docs/DEPLOYMENT.md#admin-api-transport-security)**.

Building from source instead:

```bash
git clone https://github.com/hrezenmsft/rustdeskadmin-server.git
cd rustdeskadmin-server
git remote add upstream https://github.com/rustdesk/rustdesk-server.git
git remote set-url --push upstream DISABLED
git submodule update --init --recursive
cargo build --release
./target/release/rustdesk-utils genadminkey "<label>"
# Save the printed private key now and paste it into rustdeskadmin-client.
# The server stores the public key in ./admin_authorized_keys.json by default.
# Start hbbs/hbbr from the same working directory. ADMIN_API_JWT_SECRET and
# ADMIN_API_PORT are both optional (sane defaults apply, see below).
```

See **[docs/environment-variables.md](docs/environment-variables.md)** for the full list of variables, the file/flag/env precedence rules, database and relay bandwidth tuning, Docker image variables, and examples inherited from upstream RustDesk.

---

This project is a fork of [rustdesk/rustdesk-server](https://github.com/rustdesk/rustdesk-server), the official self-hosted RustDesk server. See the upstream project for the full RustDesk server feature set, licensing, and general documentation.
