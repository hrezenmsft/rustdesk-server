# Admin Presence Server Development

## Purpose

This fork will provide the authenticated server-side presence API used by the paired administrator-only RustDesk Windows client. It reports devices currently online with the local rendezvous service.

## Contract

Expose `GET /admin/v1/devices?status=online` only to authorized administrators. Presence records are produced by the rendezvous server, expire using its heartbeat/timeout semantics, and are not exposed through logs, files, or direct database access to clients.

## Development Environment

- Recommended lab topology: one RustDesk rendezvous/relay server, one Windows administrator client, and at least one Windows endpoint client on a private test network.
- Keep concrete hostnames, IP addresses, credentials, generated keys, peer IDs, and local deployment paths out of this public documentation.

### Networking Notes

- Validate the API in a private lab before exposing any service publicly.
- Ensure the administrator client and endpoint clients use the same self-hosted RustDesk rendezvous/relay server; the admin API reports presence from the rendezvous process, while RustDesk connection attempts still use the normal client server settings.
- Do not document or commit lab-specific DHCP leases, public IPs, private IPs, peer IDs, passwords, tokens, or server keys.

### Baseline Server Build (unmodified upstream)

- Toolchain: `rustup`/`cargo`/`rustc` 1.98.1 (stable, no version pin in this repo), system `libssl-dev`, `pkg-config`, `protobuf-compiler`, `build-essential`.
- `git submodule update --init --recursive` required for `libs/hbb_common`.
- `cargo build --release` succeeds (~18 min on 1 vCPU), producing `hbbs`, `hbbr`, `rustdesk-utils` with warnings only, no errors.
- Installed as systemd services (`rustdesk-hbbs`, `rustdesk-hbbr`) from the repo's `systemd/*.service` templates in a private lab.
- Validated end-to-end reachability: the unmodified Windows client baseline built from `rustdesk-client`, deployed to a private endpoint VM and pointed at this server via its generated key, successfully registered (`update_pk` observed in `hbbs.log`). This confirms rendezvous registration/heartbeat works before any admin-presence API code is added.

### Implemented Admin Presence API

- Added `GET /admin/v1/devices?status=online` and `POST /admin/v1/auth/login` on the separate API port `21114` (override with `ADMIN_API_PORT`).
- The API is disabled unless `ADMIN_API_TOKEN_HASH` is configured. Login verifies the bcrypt hash and returns a 15-minute JWT signed with `ADMIN_API_JWT_SECRET` (an ephemeral secret is used if the optional setting is omitted).
- Device enumeration reads only the live in-memory `PeerMap`, applies the existing 30-second rendezvous registration timeout, returns device ID, optional device name when known, and last-seen seconds, and writes audit events to the server log.
- `rustdesk-utils hashtoken <token>` generates the bcrypt value needed for `ADMIN_API_TOKEN_HASH` for a token you already chose yourself. `rustdesk-utils initadmin [env-file] [--force]` is the recommended path for new deployments: it generates a fresh random admin token *and* `ADMIN_API_JWT_SECRET`, bcrypt-hashes the token, and writes both into a `.env` file (default `.env` in the current directory — the same file `hbbs`/`hbbr` already load from their working directory on startup, see `src/common.rs::init_args`), printing the plaintext token once. No existing RustDesk protocol messages or database schema were changed.
- Deployment validation installed the new `hbbs` binary in the private lab, enabled the API on port `21114`, verified missing/invalid tokens are rejected, verified unsupported status filters are rejected, and confirmed a test endpoint appears/disappears with the rendezvous registration timeout.

### Internet Exposure Guidance

- Treat the admin API as an administrative endpoint. Do not expose it over plaintext HTTP on the public internet.
- Put the API behind HTTPS termination, firewall or VPN restrictions, and operational rate limiting before internet exposure.
- Use a high-entropy admin token, store only its bcrypt hash in `ADMIN_API_TOKEN_HASH`, set a stable random `ADMIN_API_JWT_SECRET`, and rotate both if disclosure is suspected.
- The endpoint is fail-closed when `ADMIN_API_TOKEN_HASH` is unset and returns least-privilege data, but network-layer protections are still required for public deployments.

## How to Set Up the Development Environment

1. Install a Linux build host (a small VM is enough): `git`, `curl`, `build-essential`, `pkg-config`, `libssl-dev`, `protobuf-compiler`.
2. Install Rust via `rustup` (stable channel; this repo does not pin a specific version): `curl https://sh.rustup.rs -sSf | sh`, then `rustup default stable`.
3. Clone your fork and add the upstream remote read-only, so you can pull upstream fixes without accidentally pushing to it:
   ```bash
   git clone https://github.com/<your-fork>/rustdeskadmin-server.git
   cd rustdeskadmin-server
   git remote add upstream https://github.com/rustdesk/rustdesk-server.git
   git remote set-url --push upstream DISABLED
   git submodule update --init --recursive   # required for libs/hbb_common
   ```
4. (Optional, recommended) Provision a second small VM as a Windows or Linux RustDesk endpoint client so you can validate rendezvous registration end-to-end without touching a production device.

## How to Build

- Debug build (fast iteration): `cargo build`
- Release build (what you deploy): `cargo build --release`
  - Produces `target/release/hbbs`, `target/release/hbbr`, and `target/release/rustdesk-utils`.
  - A clean release build takes roughly 15–20 minutes on a single vCPU; expect longer on constrained hardware. If you hit memory-pressure build failures, retry with `CARGO_BUILD_JOBS=1`.
- Run the test suite (no admin-presence-specific test crate yet; validate via `cargo check --tests` plus the manual API checks below): `cargo check --tests`
- Configure the admin token + JWT secret (needed before the admin API will start): `./target/release/rustdesk-utils initadmin` — generates both, bcrypt-hashes the token, and writes `ADMIN_API_TOKEN_HASH`/`ADMIN_API_JWT_SECRET` into `./.env`, printing the plaintext token once. Or `./target/release/rustdesk-utils hashtoken <your-own-token>` if you'd rather choose the plaintext token yourself (you then set `ADMIN_API_TOKEN_HASH` manually).

## How to Deploy

### Production release packages (recommended — no local build required)

Starting with `v1.1.0`, `.github/workflows/build.yaml` (adapted from upstream's own CI, retargeted to this fork's GHCR namespace and with Docker Hub publishing disabled since this fork does not use Docker Hub) automatically builds and publishes a full set of installable artifacts on every `vX.Y.Z` tag push. **End users deploying to a server they administer should use these prebuilt artifacts instead of building from source.** They already contain the admin presence feature — no separate "admin" build step or extra Dockerfile is needed.

Each tagged release publishes:
- **Linux binaries** (`hbbs`, `hbbr`, `rustdesk-utils`), zipped per architecture: `rustdesk-server-linux-{amd64,arm64v8,armv7,i386}.zip` — attached to the GitHub release.
- **Debian packages** (`.deb`) per architecture — `rustdesk-server-hbbs_*.deb`, `rustdesk-server-hbbr_*.deb`, `rustdesk-server-utils_*.deb` — also attached to the release, installable via `apt install ./<file>.deb` and wired up with the repo's own `systemd/` unit files automatically.
- **Docker images**, published to GHCR (public, no login needed to pull):
  - `ghcr.io/hrezenmsft/rustdeskadmin-server:<version>` / `:latest` — "classic" minimal `FROM scratch` image, just the `hbbs`/`hbbr` binaries with no process supervisor. **This is the image the Docker Compose/`docker run` instructions below use** — one container per binary, exactly like the official upstream RustDesk deployment pattern (see the plain `docker-compose.yml` at the repo root, which uses the same pattern against the official `rustdesk/rustdesk-server:latest` image). Multi-arch manifest covers amd64/arm64/armv7.
  - `ghcr.io/hrezenmsft/rustdeskadmin-server-s6:<version>` / `:latest` — s6-overlay based image that runs **both** `hbbs` and `hbbr` inside a **single** container (an internal process supervisor starts both automatically — there is no way to run just one of them via this image). Only use this image as **one** container per server, never as two containers each expecting to own one binary — doing so causes both containers to each try to bind every port (rendezvous *and* relay), since each one starts a full internal copy of both services. Multi-arch manifest covers amd64/arm64/armv7/i386.

Releases are created as **drafts** — after a tag push, go to the repo's Releases page and publish (or `gh release edit <tag> --draft=false`) once you've confirmed all artifacts uploaded successfully.

> **Fork-specific CI note:** because this fork must be dispatched via `workflow_dispatch` rather than a real tag push (see the `ADMIN_PRESENCE_CHANGELOG.md` entry on the fork's disabled automatic tag-push trigger), an upstream guard that skipped creating the exact-version (`:vX.Y.Z`) Docker manifest specifically on `workflow_dispatch` runs was removed from `.github/workflows/build.yaml` (fixed after `v1.1.2`, where this caused the bare `:v1.1.2` manifest to be silently missing from GHCR even though the per-arch `:v1.1.2-amd64` etc. tags existed). If you ever see a version tag missing from `ghcr.io/hrezenmsft/rustdeskadmin-server[-s6]` while its per-arch-suffixed tags do exist, re-check this guard first.

#### Finding the current release/tag and picking the right architecture

```bash
# Latest published (non-draft) tag:
gh release list --repo hrezenmsft/rustdeskadmin-server --limit 1
# or, without gh CLI:
curl -s https://api.github.com/repos/hrezenmsft/rustdeskadmin-server/releases/latest | grep '"tag_name"'
```
Architecture suffixes used consistently across all three package types below:
| Host CPU | `.deb`/zip suffix | Docker manifest arch |
| --- | --- | --- |
| Intel/AMD 64-bit (most VMs/cloud instances) | `amd64` | `linux/amd64` |
| ARM64 (e.g. Raspberry Pi 4/5 64-bit OS, AWS Graviton, Oracle Ampere) | `arm64` (`.deb`) / `arm64v8` (zip) | `linux/arm64` |
| ARMv7 32-bit (older Raspberry Pi) | `armhf` (`.deb`) / `armv7` (zip) | `linux/arm/v7` |
| 32-bit x86 (rare, legacy hardware) | `i386` | `linux/386` |

Check with `dpkg --print-architecture` (Debian/Ubuntu) or `uname -m` if unsure. Docker automatically pulls the matching arch from the multi-arch manifest, so architecture selection only matters for the zip/`.deb` paths.

#### Deploy via Docker Compose (fastest path, recommended for most users)

This runs `hbbs` and `hbbr` as **two separate containers** on a dedicated bridge network — the same topology as the official upstream RustDesk deployment (see this repo's own unmodified `docker-compose.yml` at the repo root, which does the same thing against `rustdesk/rustdesk-server:latest`) — using this fork's "classic" GHCR image (`ghcr.io/hrezenmsft/rustdeskadmin-server`), **not** the `-s6` image (see the important note above about why the `-s6` image must not be split across two containers this way).
```bash
mkdir -p /opt/rustdeskadmin && cd /opt/rustdeskadmin
curl -O https://raw.githubusercontent.com/hrezenmsft/rustdeskadmin-server/master/docker-compose.example.yml
mv docker-compose.example.yml docker-compose.yml
```
Edit `docker-compose.yml` before starting it:
1. Set the `-r` argument in the `hbbs` service's `command:` (e.g. `hbbs -r your-server-public-ip-or-hostname:21117`) to your server's public IP or hostname — this is what clients are told to use for the relay connection, and it must be reachable by every client, not just the server itself.
2. If you are replacing an existing server real clients already trust, follow the "migrating an existing keypair" comment block inside the file **before** the first `docker compose up` — otherwise a fresh keypair is generated and every client will show a "server key changed" warning on next connect.

```bash
docker compose pull
docker compose up -d
docker compose ps                 # both hbbs and hbbr should show "running"/"healthy"
```
Then generate the admin token and JWT secret (a one-time step, or repeat with `--force` to rotate them later):
```bash
docker compose run --rm --no-deps hbbs rustdesk-utils initadmin
docker compose restart hbbs hbbr
```
This writes `ADMIN_API_TOKEN_HASH`/`ADMIN_API_JWT_SECRET` into `./data/.env` (which `hbbs`/`hbbr` already load from their working directory on every start — see `src/common.rs::init_args`) and prints the plaintext admin token to the terminal **once** — save it now, it cannot be recovered later, only its bcrypt hash is stored. No hand-editing of `docker-compose.yml`, and no `$`-escaping of the bcrypt hash, is needed.
```bash
docker compose logs -f hbbs        # watch for client registrations (update_pk) and admin API startup log line
curl -X POST http://localhost:21114/admin/v1/auth/login -H 'Content-Type: application/json' \
  -d '{"token":"<the plaintext admin token initadmin printed>"}'   # should return a JWT
```
See the comments inside `docker-compose.example.yml` for the bridge-networking alternative (host networking, used by default, is strongly recommended for NAT traversal — see Option B below for why).

**Upgrading:** edit the image tag in `docker-compose.yml` (or keep `:latest` and just re-pull), then:
```bash
docker compose pull
docker compose up -d
```
The named volume (and the keypair/database inside it) is untouched by an image upgrade.

#### Deploy via plain `docker run` (classic image)
```bash
docker volume create rustdesk-data
docker run -d --name rustdeskadmin-hbbs --network host --restart unless-stopped \
  -v rustdesk-data:/root \
  ghcr.io/hrezenmsft/rustdeskadmin-server:latest /usr/bin/hbbs -r your-server-hostname
docker run -d --name rustdeskadmin-hbbr --network host --restart unless-stopped \
  -v rustdesk-data:/root \
  ghcr.io/hrezenmsft/rustdeskadmin-server:latest /usr/bin/hbbr
```
> The classic image's `WORKDIR`/`HOME` is `/root` (see `docker-classic/Dockerfile`), so the named
> volume must be mounted at `/root`, not `/data` — mounting anywhere else means the keypair/database
> (and the `.env` file used below) won't actually persist across container recreation.

Generate the admin token and JWT secret (one-time, or repeat with `--force` to rotate them later):
```bash
docker run --rm -v rustdesk-data:/root ghcr.io/hrezenmsft/rustdeskadmin-server:latest \
  /usr/bin/rustdesk-utils initadmin
docker restart rustdeskadmin-hbbs rustdeskadmin-hbbr
```
This writes `ADMIN_API_TOKEN_HASH`/`ADMIN_API_JWT_SECRET` into `/root/.env` inside the volume (loaded
automatically by `hbbs`/`hbbr` on startup — see `src/common.rs::init_args`) and prints the plaintext
admin token to the terminal **once** — save it now, it cannot be recovered later.

Verify both containers stay up and check logs the same way as Compose above:
```bash
docker ps --filter name=rustdeskadmin
docker logs -f rustdeskadmin-hbbs
```
**Upgrading:**
```bash
docker pull ghcr.io/hrezenmsft/rustdeskadmin-server:latest
docker stop rustdeskadmin-hbbs rustdeskadmin-hbbr && docker rm rustdeskadmin-hbbs rustdeskadmin-hbbr
# re-run the two `docker run` commands above — the named volume persists the keypair/database/.env
```

#### Deploy via `.deb` package on a VM (systemd, no Rust toolchain needed)
```bash
# replace <tag> with the value from "Finding the current release" above, e.g. v1.1.2, and <version> with the
# numeric package version shown in that release's asset list (Cargo package version, e.g. 1.1.17 — not the git tag)
TAG=v1.1.2
VERSION=1.1.17
ARCH=amd64   # or arm64 / armhf / i386, see architecture table above
wget "https://github.com/hrezenmsft/rustdeskadmin-server/releases/download/${TAG}/rustdesk-server-hbbs_${VERSION}_${ARCH}.deb"
wget "https://github.com/hrezenmsft/rustdeskadmin-server/releases/download/${TAG}/rustdesk-server-hbbr_${VERSION}_${ARCH}.deb"
wget "https://github.com/hrezenmsft/rustdeskadmin-server/releases/download/${TAG}/rustdesk-server-utils_${VERSION}_${ARCH}.deb"
sudo apt install ./rustdesk-server-hbbs_*_${ARCH}.deb ./rustdesk-server-hbbr_*_${ARCH}.deb ./rustdesk-server-utils_*_${ARCH}.deb
```
Generate the admin token and JWT secret using the just-installed `rustdesk-utils` binary (no build required — the `.deb` already put it on `PATH`), writing directly into the systemd unit's working directory so `hbbs`/`hbbr` pick it up automatically on next start:
```bash
sudo -u rustdesk rustdesk-utils initadmin /var/lib/rustdesk-server/.env
```
This prints the plaintext admin token to the terminal **once** — save it now, it cannot be recovered later, only its bcrypt hash is stored (in `/var/lib/rustdesk-server/.env`, which both units already load from their `WorkingDirectory` on startup — see `src/common.rs::init_args`). Re-running it later refuses to overwrite an already-configured token unless you add `--force` (which invalidates the old token for every already-configured admin client). No systemd drop-in or hand-edited environment variables are needed:
```bash
sudo ufw allow 21115:21119/tcp
sudo ufw allow 21116/udp
sudo ufw allow 21114/tcp   # admin API — restrict to trusted source IPs/VPN if possible
sudo systemctl daemon-reload
sudo systemctl enable --now rustdesk-hbbs rustdesk-hbbr
sudo systemctl status rustdesk-hbbs rustdesk-hbbr
curl -X POST http://localhost:21114/admin/v1/auth/login -H 'Content-Type: application/json' \
  -d '{"token":"<the plaintext admin token initadmin printed>"}'   # should return a JWT
```
**Upgrading:** download the new `.deb` files for the newer tag and re-run `sudo apt install ./rustdesk-server-*_${ARCH}.deb` (apt upgrades in place); `/var/lib/rustdesk-server/.env` (and the rest of `/var/lib/rustdesk-server/`) is untouched. Restart both services afterward: `sudo systemctl restart rustdesk-hbbs rustdesk-hbbr`.

This `.deb` path is functionally equivalent to Option A below, but skips the Rust toolchain install and the ~15–20 minute local build entirely.

#### Generating the admin token and JWT secret from a container, without installing Rust
If you are deploying entirely from prebuilt packages and don't want to install any Rust toolchain even temporarily, run `rustdesk-utils initadmin` using the already-built binary inside the Docker image itself instead of building it locally:
```bash
docker run --rm -v rustdesk-data:/root --entrypoint /usr/bin/rustdesk-utils \
  ghcr.io/hrezenmsft/rustdeskadmin-server:latest initadmin
```
This writes `ADMIN_API_TOKEN_HASH`/`ADMIN_API_JWT_SECRET` straight into `/root/.env` inside the same
named volume `hbbs`/`hbbr` use, and prints the plaintext admin token once — no separate `hashtoken` +
hand-edited config step needed. (`rustdesk-utils hashtoken '<token>'` still exists if you'd rather
choose your own plaintext token instead of a randomly generated one; you'd need to append the
resulting `ADMIN_API_TOKEN_HASH=...` line into the target `.env` file yourself in that case.)

### Option A: Ubuntu Server VM with systemd (build from source)

This is the recommended path for a private lab or a small production deployment (for example a 1–2 vCPU / 2 GB RAM Ubuntu Server VM under Hyper-V, KVM, or any hypervisor/cloud provider).

1. **Provision the VM.** Ubuntu Server 22.04/24.04 LTS, minimum 1 vCPU / 2 GB RAM / 20 GB disk. Give it a static IP or DHCP reservation on the network your RustDesk clients can reach, and open inbound access on the ports listed in step 7 (either directly or via your hypervisor's virtual switch/NAT rules).
2. **Install build/runtime prerequisites** on the VM:
   ```bash
   sudo apt-get update
   sudo apt-get install -y build-essential pkg-config libssl-dev protobuf-compiler git curl ufw
   curl https://sh.rustup.rs -sSf | sh -s -- -y
   source "$HOME/.cargo/env"
   ```
3. **Clone the fork and build**, either directly on the VM or by building elsewhere and copying the binaries over:
   ```bash
   git clone https://github.com/<your-fork>/rustdeskadmin-server.git
   cd rustdeskadmin-server
   git submodule update --init --recursive
   cargo build --release
   ```
   This produces `target/release/hbbs`, `target/release/hbbr`, and `target/release/rustdesk-utils`. Expect ~15–20 minutes on a single vCPU; if the build OOMs on a low-RAM VM, retry with `CARGO_BUILD_JOBS=1`.
4. **Install the binaries and create a dedicated service account and directories:**
   ```bash
   sudo useradd --system --no-create-home --shell /usr/sbin/nologin rustdesk
   sudo install -m 755 target/release/hbbs target/release/hbbr target/release/rustdesk-utils /usr/bin/
   sudo mkdir -p /var/lib/rustdesk-server /var/log/rustdesk-server
   sudo chown -R rustdesk:rustdesk /var/lib/rustdesk-server /var/log/rustdesk-server
   ```
5. **Generate the admin token and JWT secret** before enabling the API:
   ```bash
   sudo -u rustdesk /usr/bin/rustdesk-utils initadmin /var/lib/rustdesk-server/.env
   ```
   This writes `ADMIN_API_TOKEN_HASH`/`ADMIN_API_JWT_SECRET` directly into `/var/lib/rustdesk-server/.env`
   (loaded automatically by `hbbs`/`hbbr` from their systemd `WorkingDirectory` on every start — see
   `src/common.rs::init_args`), and prints the plaintext admin token to the terminal **once**. Save it
   now — only its bcrypt hash is stored, it cannot be recovered later. Keep it in your password
   manager/client settings, never in the repo. Re-running this command later refuses to overwrite an
   already-configured token unless you add `--force`.
6. **Install the systemd units** from this repo's `systemd/rustdesk-hbbs.service` and `systemd/rustdesk-hbbr.service`, filling in the blank `User=`/`Group=` fields:
   ```bash
   sudo cp systemd/rustdesk-hbbs.service /etc/systemd/system/rustdesk-hbbs.service
   sudo cp systemd/rustdesk-hbbr.service /etc/systemd/system/rustdesk-hbbr.service
   sudo sed -i 's/^User=$/User=rustdesk/; s/^Group=$/Group=rustdesk/' \
     /etc/systemd/system/rustdesk-hbbs.service /etc/systemd/system/rustdesk-hbbr.service
   ```
   No `systemctl edit` drop-in or hand-typed environment variables are needed — both units already run
   with `WorkingDirectory=/var/lib/rustdesk-server/`, so the `.env` file written in step 5 is picked up
   automatically. See `docs/environment-variables.md` for the full list of non-admin-presence server
   environment variables (relay/key settings, etc.) if you need to set any of those too.
7. **Open firewall ports** for the rendezvous/relay service plus the admin API:
   ```bash
   sudo ufw allow 21115:21119/tcp
   sudo ufw allow 21116/udp
   sudo ufw allow 21114/tcp   # admin API — restrict this to trusted source IPs/VPN if possible, see guidance above
   sudo ufw enable
   ```
8. **Start and enable both services:**
   ```bash
   sudo systemctl daemon-reload
   sudo systemctl enable --now rustdesk-hbbs rustdesk-hbbr
   ```
9. **Verify the deployment:**
   ```bash
   sudo systemctl status rustdesk-hbbs rustdesk-hbbr
   sudo tail -f /var/log/rustdesk-server/hbbs.log   # watch for client registrations (update_pk)
   curl -X POST http://localhost:21114/admin/v1/auth/login \
     -H 'Content-Type: application/json' -d '{"token":"<your-plaintext-admin-token>"}'
   ```
   A successful login returns a JWT; point a RustDesk client's ID/Relay server settings at this VM's address and key to confirm end-to-end registration, and point the Windows admin client's Settings > Network > Admin Presence at `http://<vm-address>:21114` with the same token.
10. **Upgrading:** rebuild, `sudo systemctl stop rustdesk-hbbs rustdesk-hbbr`, replace the binaries in `/usr/bin/`, then `sudo systemctl start rustdesk-hbbs rustdesk-hbbr`. The keypair and any database live under `/var/lib/rustdesk-server/` and are untouched by a binary swap.

### Option B: Docker

This repository ships two upstream Dockerfiles (`docker/Dockerfile` and `docker-classic/Dockerfile`) that both expect prebuilt `hbbs`/`hbbr` binaries to already exist in the build context (they are normally populated by CI from a prior `cargo build --release` step, not built inside the Dockerfile). To build and run a container image that includes the admin-presence API from source, use a small multi-stage Dockerfile of your own:

1. **Create the Dockerfile** at the repo root (e.g. `docker-admin/Dockerfile`, alongside the existing `docker/` and `docker-classic/` folders). If your build/runtime host's clock is not closely synced to real-world time (observed on some lab VMs), add the `Check-Valid-Until "false"` apt workaround shown below in **both** stages — otherwise `apt-get update` can fail with `At least one invalid signature was encountered` because Debian's `InRelease` signature validity window no longer overlaps the container's clock:
   ```dockerfile
   # syntax=docker/dockerfile:1
   FROM rust:1-bookworm AS build
   RUN echo 'Acquire::Check-Valid-Until "false";' > /etc/apt/apt.conf.d/99no-check-valid-until
   RUN apt-get update && apt-get install -y --no-install-recommends \
       pkg-config libssl-dev protobuf-compiler && rm -rf /var/lib/apt/lists/*
   WORKDIR /src
   COPY . .
   RUN cargo build --release

   FROM debian:bookworm-slim
   RUN echo 'Acquire::Check-Valid-Until "false";' > /etc/apt/apt.conf.d/99no-check-valid-until
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
2. **Build the image** from the repo root (run from the same directory as step 1's Dockerfile, adjusting `-f` if you placed it in a subfolder):
   ```bash
   docker build -t rustdesk-hbbs-admin -f docker-admin/Dockerfile .
   ```
3. **Run `hbbs` and `hbbr` using host networking (recommended).** RustDesk's NAT traversal (UDP hole-punching between two clients) depends on `hbbs` seeing each client's real source port. Docker's default bridge networking rewrites/DNATs that port, which can break traversal for some client pairs; `--network host` avoids this entirely and matches how the systemd deployment behaves. Host networking requires a native Linux Docker engine (this is what the Ubuntu Server VM in Option A provides) — it is not available through Docker Desktop on Windows/macOS.
   ```bash
   docker volume create rustdesk-data
   docker run -d --name rustdesk-hbbs --network host \
     -v rustdesk-data:/data \
     --restart unless-stopped \
     rustdesk-hbbs-admin

   docker run -d --name rustdesk-hbbr --network host \
     -v rustdesk-data:/data \
     --restart unless-stopped \
     --entrypoint /usr/local/bin/hbbr \
     rustdesk-hbbs-admin
   ```
   With `--network host` there is no `-p`/port mapping — the containers bind directly to the VM's ports (`21115-21119` TCP/UDP, `21114` for the admin API), so open those ports in the VM's firewall exactly as in Option A step 7.

   **Alternative: bridge networking with published ports**, if you must run on a non-Linux Docker host or need network-namespace isolation between containers. This is simpler to reason about but can be less reliable for UDP hole-punching in some environments — test connectivity between two real clients (not just client-to-server) before relying on it in production:
   ```bash
   docker network create rustdesk-net
   docker run -d --name rustdesk-hbbs --network rustdesk-net \
     -p 21115-21119:21115-21119 -p 21116:21116/udp -p 21114:21114 \
     -v rustdesk-data:/data \
     --restart unless-stopped \
     rustdesk-hbbs-admin

   docker run -d --name rustdesk-hbbr --network rustdesk-net \
     -v rustdesk-data:/data \
     --restart unless-stopped \
     --entrypoint /usr/local/bin/hbbr \
     rustdesk-hbbs-admin
   ```
4. **Or use `docker-compose.yml`.** Host networking (recommended):
   ```yaml
   services:
     hbbs:
       image: rustdesk-hbbs-admin
       build:
         context: .
         dockerfile: docker-admin/Dockerfile
       network_mode: host
       volumes:
         - rustdesk-data:/data
       restart: unless-stopped
     hbbr:
       image: rustdesk-hbbs-admin
       network_mode: host
       entrypoint: ["/usr/local/bin/hbbr"]
       volumes:
         - rustdesk-data:/data
       restart: unless-stopped
   volumes:
     rustdesk-data:
   ```
   `network_mode: host` and `ports:` are mutually exclusive in Compose; if you fall back to bridge networking, remove `network_mode: host` and add the `ports:` list shown in the bridge example above instead.

5. **Generate the admin token and JWT secret**, after the containers exist (image built, volume created), instead of hand-editing `ADMIN_API_TOKEN_HASH`/`ADMIN_API_JWT_SECRET` env vars into the `docker run`/Compose commands above (which would otherwise need every literal `$` in the bcrypt hash escaped as `$$` in Compose — `initadmin` avoids that entirely by writing straight into the same volume as a `.env` file):
   ```bash
   docker run --rm -v rustdesk-data:/data --entrypoint /usr/local/bin/rustdesk-utils \
     rustdesk-hbbs-admin initadmin /data/.env
   docker restart rustdesk-hbbs rustdesk-hbbr   # or: docker compose restart hbbs hbbr
   ```
   This writes `ADMIN_API_TOKEN_HASH`/`ADMIN_API_JWT_SECRET` into `/data/.env` inside the volume (loaded
   automatically by `hbbs`/`hbbr` from their `/data` working directory on every start — see
   `src/common.rs::init_args`) and prints the plaintext admin token to the terminal **once** — save it
   now, it cannot be recovered later. Re-running it later refuses to overwrite an already-configured
   token unless you add `--force`.

6. **Verify the deployment:**
   ```bash
   docker ps                       # confirm both containers are Up
   docker logs -f rustdesk-hbbs    # watch for client registrations
   curl -X POST http://localhost:21114/admin/v1/auth/login \
     -H 'Content-Type: application/json' -d '{"token":"<the plaintext admin token initadmin printed>"}'
   ```
7. **Upgrading:** rebuild the image (`docker build`/`docker compose build`) and recreate the containers (`docker compose up -d` or `docker stop && docker rm && docker run` again) — the named `rustdesk-data` volume persists the keypair/database/`.env` across recreation.

Persist the `/data` volume across restarts so the server keypair and any embedded SQLite database are not regenerated/lost. As with the systemd path, keep the admin API behind HTTPS termination and network-layer restrictions (firewall/VPN, avoid publishing port `21114` directly to `0.0.0.0` on an internet-facing host) before exposing it publicly — see "Internet Exposure Guidance" above.

**Preserving an existing server keypair when moving to Docker:** `hbbs` auto-generates a new `id_ed25519`/`id_ed25519.pub` keypair the first time it starts against an empty `/data`, which silently changes the server's public key fingerprint and breaks every client that already trusted the old one. If you are migrating an existing deployment (e.g. from the systemd path above) into Docker, copy the existing keypair files into the named volume *before* the first `docker compose up`, or immediately after (stop the containers, `docker cp` the two files into the volume's mount point, restart):
```bash
docker compose stop hbbs hbbr
docker run --rm -v rustdeskadmin-server_rustdesk-data:/data -v /path/to/old/data:/old alpine \
  cp /old/id_ed25519 /old/id_ed25519.pub /data/
docker compose start hbbs hbbr
```
Verify the fingerprint matches the original by checking `id_ed25519.pub` before and after, and confirm existing clients reconnect without a "server key changed" warning.

## Change Discipline

Update this document when the API contract, stored presence data, authorization model, persistence, or deployment workflow changes. Add every externally observable or operational change to `docs/ADMIN_PRESENCE_CHANGELOG.md` in the same change set.
