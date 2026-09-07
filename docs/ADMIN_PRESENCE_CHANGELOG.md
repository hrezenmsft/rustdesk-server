# Admin Presence Change Log

All notable changes to this custom administrator-presence server extension are recorded here.

Entries are grouped by date, newest first. Each dated section corresponds to one or more commits on that date; the `Unreleased` section at the top holds changes not yet committed.

## Unreleased

### Added

- Deployed the admin-presence-enabled server as Docker Compose–managed containers (`hbbs`/`hbbr`, host networking) on the lab server VM, replacing the earlier systemd-managed baseline for day-to-day testing.

### Changed

- Repository renamed on GitHub from `rustdesk-server` to `rustdeskadmin-server` (origin remote updated to match; `upstream` remote unchanged, still points read-only at `rustdesk/rustdesk-server`).
- Fixed the Docker image build failing under `apt-get update` inside `debian:bookworm-slim` with "At least one invalid signature was encountered" — root cause was the build host's clock being far enough ahead of real-world time to fall outside the Debian repo's signed `Valid-Until` window; fixed by adding `Acquire::Check-Valid-Until "false";` to the apt config in every Dockerfile stage that runs `apt-get update`.
- Fixed a Docker Compose–specific bug where literal `$` characters in the bcrypt `ADMIN_API_TOKEN_HASH` value (for example `$2b$12$...`) were being interpreted as Compose environment-variable interpolation syntax; fixed by escaping every `$` as `$$` in `docker-compose.yml`. This does not affect the systemd/`docker run` deployment paths, only Compose.
- Fixed the Docker container generating a brand-new server keypair on first start instead of reusing the one already known to existing endpoints; the original keypair (`id_ed25519`/`id_ed25519.pub`) must be copied into the named Compose volume before starting the containers for the first time, otherwise every previously-registered endpoint will need to re-pair.
- Disabled (and stopped) the previously-installed systemd `rustdesk-hbbs`/`rustdesk-hbbr` services on the lab server VM after switching to the Docker Compose deployment, to avoid a port-bind conflict with the containers' `network_mode: host` networking on VM reboot.
- Diagnosed and fixed a disk-full condition on the lab server VM (`/` at 100% used, causing writes to silently truncate to 0 bytes with no error) by expanding the LVM logical volume into previously-unallocated physical volume space (`lvextend -l +100%FREE` + `resize2fs`) and clearing an obsolete 6.5&nbsp;GB build directory and package caches.

## 2026-09-07 01:23 (`ce054cd` — Add code comments, sanitize docs, add AI handoff and build/deploy guides)

### Added

- Added a public-safe AI handoff document (`docs/ADMIN_PRESENCE_AI_HANDOFF.md`) with generated/example values for future agents with no prior context on this fork.
- Added consolidated "How to Build", "How to Set Up the Development Environment", and "How to Deploy" (systemd and Docker) sections to `docs/ADMIN_PRESENCE_DEVELOPMENT.md` and the AI handoff document.
- Added a Docker deployment path for the admin-presence-enabled server, including required environment variables and port exposure for the admin API alongside the existing rendezvous/relay ports.

### Changed

- Added inline code comments at every admin-presence integration point (`src/lib.rs`, `src/peer.rs`, `src/rendezvous_server.rs`, `src/utils.rs`) to make the customizations easy to locate and review.
- Added public deployment guidance requiring HTTPS, firewall/VPN restrictions, rate limiting, high-entropy admin tokens, and a stable JWT secret before exposing the admin API to the internet.
- Removed lab-specific hostnames, IP addresses, peer IDs, paths, generated keys, and credentials from public documentation.
- Reorganized this changelog into dated sections (newest first) matching actual commit history instead of a single flat "Unreleased" list.

## 2026-09-07 00:25 (`4cee4d5` — Include optional names in admin presence API)

### Added

- Added an optional `name` field to admin presence device responses so clients can show a friendly name above the RustDesk ID when safe metadata is available.

## 2026-09-06 23:51 (`d649b2a` — Document admin presence deployment validation)

### Added

- Deployed the API in a private lab and validated authenticated listing, missing/invalid-token rejection, unsupported-filter rejection, and online/offline timeout behavior with a test endpoint.

## 2026-09-06 23:31 (`73b311d` — Add authenticated admin presence API)

### Added

- Implemented the authenticated, versioned admin presence API with bcrypt login, short-lived JWT bearer tokens, least-privilege online-device responses, audit logging, fail-closed configuration, and `rustdesk-utils hashtoken`.

### Changed

- Online status reuses the rendezvous server's existing 30-second heartbeat timeout and reads only in-memory registration state; no client-facing database/file access was added.

## 2026-09-06 21:28 (`2614320` — Record completed dev environment setup and baseline validation)

### Added

- Stood up a private Ubuntu Hyper-V server VM and resolved a DHCP conflict / subnet-mask mismatch that had blocked lab VM networking.
- Built the unmodified upstream server baseline (`hbbs`, `hbbr`, `rustdesk-utils` via `cargo build --release`) and installed it as systemd services (`rustdesk-hbbs`, `rustdesk-hbbr`).
- Validated baseline rendezvous registration end-to-end: an unmodified Windows client built from the paired `rustdesk-client` fork successfully registered with this server, confirming the environment is ready for admin-presence API development.

## 2026-09-06 18:06 (`e30374d` — Add admin-presence development environment and changelog)

### Added

- Initial development environment and versioned, authenticated online-device API contract documentation.
