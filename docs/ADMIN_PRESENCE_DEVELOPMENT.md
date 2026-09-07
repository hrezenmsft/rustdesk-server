# Admin Presence Server Development

## Purpose

This fork will provide the authenticated server-side presence API used by the paired administrator-only RustDesk Windows client. It reports devices currently online with the local rendezvous service.

## Contract

Expose `GET /admin/v1/devices?status=online` only to authorized administrators. Presence records are produced by the rendezvous server, expire using its heartbeat/timeout semantics, and are not exposed through logs, files, or direct database access to clients.

## Development Environment

- Server VM: `rd-admin-server` (Ubuntu Server 24.04, 1 vCPU / 2 GB RAM, Hyper-V "Default Switch"), current IP `172.27.17.85`
- Admin client development laptop: `NINA-LAPTOP`
- Windows endpoint VM: `rd-endpoint-01` (Windows 11 Pro, 2 vCPU / 4 GB RAM, same switch), current IP `172.27.26.223`

### Networking Notes (Hyper-V lab)

- Both VMs run on Hyper-V's built-in "Default Switch" (NAT) rather than the external Wi-Fi-bridged switch, which produced a DHCP address collision with the host laptop.
- The switch's real NAT subnet is `172.27.16.1/20`; Hyper-V's DHCP handed the Windows guest a `/24` mask, which broke direct VM-to-VM routing until corrected to `/20`.
- DHCP leases on this switch may not be stable across reboots; static addressing should be considered if this recurs.

### Baseline Server Build (unmodified upstream)

- Toolchain: `rustup`/`cargo`/`rustc` 1.98.1 (stable, no version pin in this repo), system `libssl-dev`, `pkg-config`, `protobuf-compiler`, `build-essential`.
- `git submodule update --init --recursive` required for `libs/hbb_common`.
- `cargo build --release` succeeds (~18 min on 1 vCPU), producing `hbbs`, `hbbr`, `rustdesk-utils` with warnings only, no errors.
- Installed as systemd services (`rustdesk-hbbs`, `rustdesk-hbbr`) from the repo's `systemd/*.service` templates, running under the `lab` user, data in `/var/lib/rustdesk-server`, logs in `/var/log/rustdesk-server`.
- Validated end-to-end reachability: the unmodified Windows client baseline built from `rustdesk-client`, deployed to `rd-endpoint-01` and pointed at this server via its generated key, successfully registered its peer ID (`update_pk` observed in `hbbs.log`). This confirms rendezvous registration/heartbeat works before any admin-presence API code is added.

## Change Discipline

Update this document when the API contract, stored presence data, authorization model, persistence, or deployment workflow changes. Add every externally observable or operational change to `docs/ADMIN_PRESENCE_CHANGELOG.md` in the same change set.
