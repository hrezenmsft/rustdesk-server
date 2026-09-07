# Admin Presence Change Log

All notable changes to this custom administrator-presence server extension are recorded here.

Entries are grouped by date, newest first. Each dated section corresponds to one or more commits on that date; the `Unreleased` section at the top holds changes not yet committed.

## Unreleased

### Fixed

- Fixed `docker-compose.example.yml`, which incorrectly ran the **s6-overlay image** (`ghcr.io/hrezenmsft/rustdeskadmin-server-s6`) as two separate host-networked containers, one "for" `hbbs` and one "for" `hbbr`. That image's s6-overlay supervisor always starts **both** `hbbs` and `hbbr` inside every container it runs (confirmed via `docker/rootfs/etc/s6-overlay/s6-rc.d/user/contents.d/{hbbs,hbbr}`, both enabled in the default bundle with no way to disable one via an environment variable) — so the previous two-container example meant each container tried to bind every port itself, and the second container to start would fail with port-bind conflicts. Rewrote the example to use the **classic** minimal image (`ghcr.io/hrezenmsft/rustdeskadmin-server`, `FROM scratch`, just the two binaries, no supervisor) as two separate containers, one running `hbbs` and one running `hbbr` via explicit `command:` overrides, on a dedicated bridge network with explicit port mappings — this exactly matches the official upstream RustDesk deployment pattern already shown in this repo's own unmodified `docker-compose.yml`. Added a prominent warning comment in the example file (and a matching note in `docs/ADMIN_PRESENCE_DEVELOPMENT.md`) explaining why the `-s6` image must only ever be run as a single container.
- Fixed `.github/workflows/build.yaml`'s `docker-manifest` and `docker-manifest-classic` jobs silently never creating the exact-version Docker manifest (e.g. `ghcr.io/hrezenmsft/rustdeskadmin-server:v1.1.2`) on this fork. Root cause: an upstream guard, `if: github.event_name != 'workflow_dispatch'`, skipped that step specifically on `workflow_dispatch`-triggered runs — reasonable upstream (where a real tag push always creates the version manifest, and manual `workflow_dispatch` reruns are only ever used for troubleshooting other tags/architectures) but wrong for this fork, which is **always** dispatched via `workflow_dispatch` due to GitHub's automatic tag-push-trigger restriction on forked repositories (see the `v1.1.0`/`v1.1.1` entries below). This meant every release so far published per-architecture tags (`:v1.1.2-amd64` etc.) and updated the rolling `:v1`/`:latest` manifests, but never published the exact-version manifest itself — `docker pull ghcr.io/hrezenmsft/rustdeskadmin-server:v1.1.2` (or the `-s6` equivalent) would 404. Removed the guard so this fork always creates/pushes the exact-version manifest.

### Changed

- Expanded `docs/ADMIN_PRESENCE_DEVELOPMENT.md`'s "Production release packages" section with much more deploy-from-package detail: how to find the latest tag/release, an architecture-selection table (amd64/arm64/armhf/i386) shared across all three package types, per-path verification commands, per-path upgrade instructions, and a way to generate the admin token bcrypt hash from the published Docker image (`docker run --rm --entrypoint /usr/bin/rustdesk-utils ...`) so a from-package deployment never needs any local Rust toolchain, not even temporarily.
- Added a "Quick start" summary of the recommended Docker Compose no-build deployment path to `README.md`, linking to the expanded development-doc section and the Releases page, ahead of the from-source build instructions.

## 2026-09-07 13:35 (`242dd00`, released `v1.1.2`)

### Fixed

- Fixed the cross-compiled Linux release builds (`amd64`/`arm64v8`/`armv7`/`i386`, all `*-unknown-linux-musl` targets, via the `.github/workflows/build.yaml` release pipeline) failing with `error: could not find system library 'openssl' required by the 'openssl-sys' crate`. Root cause: `tokio-tungstenite` (used for websocket relay/rendezvous connections) pulls in `native-tls` → `openssl-sys` by default on non-macOS/non-Windows targets, and the `cross`-rs musl Docker images used for cross-compilation do not ship system OpenSSL dev headers. Fixed by adding a direct `openssl-sys = { version = "0.9", features = ["vendored"] }` dependency under the existing `cfg(not(any(target_os = "macos", target_os = "windows")))` target section in the root `Cargo.toml`, forcing Cargo's feature unification to statically build OpenSSL from source instead of requiring a system install. This is a build-configuration-only change; no runtime or protocol behavior is affected. (This bug was latent and never previously exercised because the CI pipeline itself had never successfully run — see below.)
- The first `openssl-sys` fix (declared only under `[target.'cfg(not(macos/windows))'.dependencies]`, i.e. Cargo dependency "kind = normal") was not sufficient: job logs for the retried build showed the *same* pkg-config failure, but this time for an `openssl-sys` build script reporting `$TARGET = x86_64-unknown-linux-gnu` (the CI runner's host triple) instead of the musl cross target — meaning a *different* resolution of `openssl-sys` was being pulled in purely as a host-context (build-script) requirement, which Cargo resolves and unifies features for separately from the target-context ("kind = normal") requirement even on the same platform. Fixed by adding the identical `openssl-sys = { version = "0.9", features = ["vendored"] }` entry under a new `[target.'cfg(not(any(target_os = "macos", target_os = "windows")))'.build-dependencies]` table as well, so both the host-context and target-context resolutions of `openssl-sys` get the `vendored` feature.
- Discovered and worked around a GitHub restriction specific to forked repositories: automatic `push`/tag-triggered workflow runs (`on.push.tags` in `build.yaml`) do not fire until the repository owner manually dismisses a one-time "enable workflows" banner on the fork's Actions tab in the GitHub web UI — this setting is not exposed via the REST API or `gh` CLI. `workflow_dispatch` runs are unaffected and were used as a workaround (dispatched against the release tag's ref, which resolves `GITHUB_REF` identically to a real tag-push trigger). Recommended permanent fix: visit the Actions tab once, or detach the fork relationship entirely (Settings > General > Danger Zone) so future `git push --tags` triggers releases automatically without manual intervention.

### Added

- First fully green run of the `.github/workflows/build.yaml` release pipeline for this fork (`workflow_dispatch` run `34127289017` against tag `v1.1.2`): all 5 build jobs (Linux amd64/arm64v8/armv7/i386 + Windows), all 4 `.deb` package jobs, all Docker build/push/manifest jobs, and the GitHub release job completed successfully. Published release `v1.1.2` (previously created as a draft by the pipeline) with all binary zips, `.deb` packages, and multi-arch GHCR images (`ghcr.io/hrezenmsft/rustdeskadmin-server` and `-s6`) now publicly available — this is the first "release package" end users can install without building from source.

## 2026-09-07 09:45 (`b9c4e72`)

### Added

- Enabled and adapted the existing (previously dormant/upstream-configured) `.github/workflows/build.yaml` CI pipeline for this fork: retargeted GHCR image names to `ghcr.io/hrezenmsft/rustdeskadmin-server-s6` and `ghcr.io/hrezenmsft/rustdeskadmin-server`, fixed the hardcoded GHCR login username, granted the `contents: write` permission needed for release creation, and disabled all Docker Hub publishing steps (`if: false`) since this fork does not use Docker Hub — no `DOCKER_IMAGE`/`DOCKER_HUB_USERNAME`/`DOCKER_HUB_PASSWORD` secrets are required. On every `vX.Y.Z` tag push this now automatically produces: Linux binary zips (amd64/arm64v8/armv7/i386), `.deb` packages per architecture, Windows binaries, and multi-arch Docker images (s6-overlay + classic) published to GHCR — all already containing the admin presence API, no separate build step needed.
- Added `docker-compose.example.yml` at the repo root: a ready-to-use Compose file referencing the published GHCR image, documenting the bcrypt `$`-escaping pitfall, bridge-networking alternative, and existing-keypair migration steps inline.
- Documented all three "no local build" production deployment paths (Docker Compose, plain `docker run`, `.deb` + systemd) in `docs/ADMIN_PRESENCE_DEVELOPMENT.md` under a new "Production release packages" section, ahead of the existing from-source build instructions.
- Added `EXPOSE 21114` to `docker/Dockerfile` (the admin API port) for documentation/introspection purposes (does not affect actual port publishing, which is controlled by `-p`/`ports:`).

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
