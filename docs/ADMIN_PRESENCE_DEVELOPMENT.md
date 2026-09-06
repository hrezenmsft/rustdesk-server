# Admin Presence Server Development

## Purpose

This fork will provide the authenticated server-side presence API used by the paired administrator-only RustDesk Windows client. It reports devices currently online with the local rendezvous service.

## Contract

Expose `GET /admin/v1/devices?status=online` only to authorized administrators. Presence records are produced by the rendezvous server, expire using its heartbeat/timeout semantics, and are not exposed through logs, files, or direct database access to clients.

## Development Environment

- Server VM: `rd-admin-server` at `192.168.0.119`
- Admin client development laptop: `NINA-LAPTOP`
- Windows endpoint VM: `rd-endpoint-01` at `192.168.0.120`

## Change Discipline

Update this document when the API contract, stored presence data, authorization model, persistence, or deployment workflow changes. Add every externally observable or operational change to `docs/ADMIN_PRESENCE_CHANGELOG.md` in the same change set.
