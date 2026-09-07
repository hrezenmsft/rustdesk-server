# RustDesk Server Program

[![build](https://github.com/rustdesk/rustdesk-server/actions/workflows/build.yaml/badge.svg)](https://github.com/rustdesk/rustdesk-server/actions/workflows/build.yaml)

---

## 🛠️ RustDeskAdmin — Custom Admin-Presence Fork

This repository is a **private fork of the official [rustdesk/rustdesk-server](https://github.com/rustdesk/rustdesk-server)**, part of the `RustDeskAdmin` project (paired with [`rustdeskadmin-client`](https://github.com/hrezenmsft/rustdeskadmin-client)). Everything below the divider is the unmodified upstream README; this section describes what is different in this fork.

### What this fork adds

- A new **authenticated, versioned admin API** exposed by `hbbs` on a separate port (default `21114`, override with `ADMIN_API_PORT`):
  - **v2.0.0, primary:** `POST /admin/v1/auth/challenge` + `POST /admin/v1/auth/verify` — per-client ed25519 challenge-response. Each admin client enrolls its own keypair (`rustdesk-utils genadminkey <label>`) instead of sharing one secret, and a single compromised client can be revoked (`rustdesk-utils revokeadminkey <fingerprint>`) without affecting any other admin.
  - `POST /admin/v1/auth/login` — legacy v1.x path, kept only for migration: exchanges a shared high-entropy admin token (stored server-side only as a bcrypt hash, `ADMIN_API_TOKEN_HASH`) for the same short-lived (15-minute) JWT the key-based path issues. Deprecated; logs a warning on every use.
  - `GET /admin/v1/devices?status=online` — returns devices currently registered online with this rendezvous server (ID, optional hostname, last-seen seconds), reading only the existing in-memory peer map and reusing the server's normal 30-second registration heartbeat/timeout — **no new database, file, or log access is exposed to clients**. Unchanged since v1.x — either auth path above issues the same bearer token this endpoint consumes.
- The API is **fail-closed**: it stays disabled until at least one key is enrolled or `ADMIN_API_TOKEN_HASH` is configured, so an unconfigured server exposes nothing new.
- `rustdesk-utils genadminkey <label>` / `listadminkeys` / `revokeadminkey <fingerprint>` (v2.0.0) manage the per-client key lifecycle. `rustdesk-utils initadmin [env-file] [--force]` / `hashtoken <token>` (legacy) remain available for v1.x migration.
- No existing RustDesk protocol messages, wire formats, or database schema were changed — this fork is strictly additive.
- Audit log entries are written for every admin API auth attempt and device-list query.

### Documentation


- **[docs/ADMIN_PRESENCE_DEVELOPMENT.md](docs/ADMIN_PRESENCE_DEVELOPMENT.md)** — full development-environment setup, build, and deployment instructions for this fork, including both a systemd (bare-metal/VM) deployment path and a Docker/Docker Compose path.
- **[docs/ADMIN_PRESENCE_CHANGELOG.md](docs/ADMIN_PRESENCE_CHANGELOG.md)** — dated changelog of every change made in this fork, newest first.
- **[docs/ADMIN_PRESENCE_AI_HANDOFF.md](docs/ADMIN_PRESENCE_AI_HANDOFF.md)** — a public-safe context primer for AI coding agents picking up this fork with no prior history.

### Quick start (see the development doc for full detail, including Docker Compose)

**Recommended: deploy from a prebuilt release package — no build required.** Every `vX.Y.Z` tag publishes Linux binaries, `.deb` packages, and multi-arch Docker images (GHCR) with the admin API already built in. Fastest path (Docker Compose):
```bash
curl -O https://raw.githubusercontent.com/hrezenmsft/rustdeskadmin-server/master/docker-compose.example.yml
mv docker-compose.example.yml docker-compose.yml
# edit docker-compose.yml: set the hbbs command's -r <host> address
docker compose up -d
docker compose run --rm --no-deps hbbs rustdesk-utils initadmin   # generates & prints the admin token
docker compose restart hbbs hbbr
```
See **[docs/ADMIN_PRESENCE_DEVELOPMENT.md § Production release packages](docs/ADMIN_PRESENCE_DEVELOPMENT.md#production-release-packages-recommended--no-local-build-required)** for the full walkthrough of all three package types (Docker Compose, plain `docker run`, `.deb` + systemd) including architecture selection, admin-token setup without any local Rust install, verification, and upgrade steps — or grab packages directly from the **[Releases page](https://github.com/hrezenmsft/rustdeskadmin-server/releases)**.

Building from source instead:

```bash
git clone https://github.com/hrezenmsft/rustdeskadmin-server.git
cd rustdeskadmin-server
git remote add upstream https://github.com/rustdesk/rustdesk-server.git
git remote set-url --push upstream DISABLED
git submodule update --init --recursive
cargo build --release
./target/release/rustdesk-utils initadmin
# Prints the plaintext admin token once and writes ADMIN_API_TOKEN_HASH / ADMIN_API_JWT_SECRET
# into ./.env (loaded automatically by hbbs/hbbr on startup), then run hbbs/hbbr as usual.
```

Upstream project: **[rustdesk/rustdesk-server](https://github.com/rustdesk/rustdesk-server)** — this fork tracks it read-only via the `upstream` remote (push disabled) and only adds the administrator presence API described above; it does not otherwise change RustDesk's rendezvous/relay protocol or security model.

---

[**Download**](https://github.com/rustdesk/rustdesk-server/releases)

[**Manual**](https://rustdesk.com/docs/en/self-host/)

[**Configuration & environment variables**](docs/environment-variables.md)

[**FAQ**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**How to migrate OSS to Pro**](https://rustdesk.com/docs/en/self-host/rustdesk-server-pro/installscript/#convert-from-open-source)

Self-host your own RustDesk server, it is free and open source.

> [!IMPORTANT]
> **Need more features?** [RustDesk Server Pro](https://rustdesk.com/pricing.html) might suit you better.
>
> **Want to develop your own server?** Start with [rustdesk-server-demo](https://github.com/rustdesk/rustdesk-server-demo), a simpler starting point than this repository.

## How to build manually

```bash
cargo build --release
```

Three executables will be generated in target/release.

- hbbs - RustDesk ID/Rendezvous server
- hbbr - RustDesk relay server
- rustdesk-utils - RustDesk CLI utilities

You can find updated binaries on the [Releases](https://github.com/rustdesk/rustdesk-server/releases) page.

## Configuration

`hbbs` and `hbbr` can be configured with command-line flags, environment
variables, or an `.env` / config file. Run `hbbs --help` or `hbbr --help` to see
the available flags.

The most common options:

| Option | Flag | Env var | Applies to | Purpose |
| --- | --- | --- | --- | --- |
| Key | `-k` | `KEY` | hbbs, hbbr | `hbbs` loads/generates one by default |
| Bind address | `-b` | `BIND` | hbbs, hbbr | Local IP address to listen on (default: all interfaces; requires 1.1.17+) |
| Port | `-p` | `PORT` | hbbs, hbbr | Listening port (hbbs `21116`, hbbr `21117`) |
| Relay servers | `-r` | `RELAY-SERVERS` | hbbs | Override when the relay uses a different address or a non-standard port |
| Force relay | — | `ALWAYS_USE_RELAY` | hbbs | `Y` disables direct connections |
| Log level | — | `RUST_LOG` | hbbs, hbbr | e.g. `debug` (default `info`) |

See **[docs/environment-variables.md](docs/environment-variables.md)** for the
full list of variables, the file/flag/env precedence rules, database and relay
bandwidth tuning, Docker image variables, and examples.

## Installation

Please follow this [doc](https://rustdesk.com/docs/en/self-host/rustdesk-server-oss/)
