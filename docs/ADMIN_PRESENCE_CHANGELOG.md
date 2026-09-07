# Admin Presence Change Log

All notable changes to this custom administrator-presence server extension are recorded here.

## Unreleased

### Added

- Initial development environment and versioned, authenticated online-device API contract documentation.
- Stood up `rd-admin-server` (Ubuntu 24.04 Hyper-V VM) on the "Default Switch" and resolved a DHCP conflict / subnet-mask mismatch that had blocked lab VM networking.
- Built the unmodified upstream server baseline (`hbbs`, `hbbr`, `rustdesk-utils` via `cargo build --release`) and installed it as systemd services (`rustdesk-hbbs`, `rustdesk-hbbr`).
- Validated baseline rendezvous registration end-to-end: an unmodified Windows client (built from the paired `rustdesk-client` fork) running on `rd-endpoint-01` successfully registered with this server, confirming the environment is ready for admin-presence API development.

### Changed

- Updated documented lab IP for `rd-admin-server` to its actual Hyper-V "Default Switch" address (172.27.17.85) instead of the originally planned external-switch static address.
