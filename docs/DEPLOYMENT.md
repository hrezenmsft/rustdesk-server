# Deploying RustDeskAdmin server

This guide covers three Linux deployment modes for this fork:

1. native `systemd`
2. plain `docker run`
3. `docker compose`

All examples use placeholders such as `<your-domain-or-ip>`, `<label>`, `<fingerprint>`, and `<generate-a-long-random-secret>`. Replace them with your own values before use.

## Release availability — PREPARING v2.0.1

GitHub Latest is still **v2.0.0**. The approved **2.0.1** patch is not yet published and must ship from source matching tag **`v2.0.1`**, with product name **RustDeskAdmin Server - RustDesk Fork**, upstream copyright retained, and Henrique Rezende's attribution added.

The planned assets are `rustdeskadmin-server-2.0.1-linux-amd64.zip`, `rustdesk-server-hbbs_2.0.1_amd64.deb`, `rustdesk-server-hbbr_2.0.1_amd64.deb`, and `rustdesk-server-utils_2.0.1_amd64.deb`. Only Linux amd64 is in this patch release; no Windows, ARM, 32-bit, or `RustDeskDeploy.exe` wrapper assets are planned. Internal executable/package/service names and enrollment behavior stay compatible.

The Docker examples below describe existing-image deployment, **not** a v2.0.1 image release. The current local image/deployment stays **`rustdeskadmin-server:2.0.0`** unchanged. Do not infer an image upgrade from the package version.

After publication as Latest, verify all four new downloads before removing the 17 old v2.0.0 assets, including ARM/32-bit/Windows assets. Keep the old tag/source archives. Until then use the currently published asset list, not the pending URLs below.

> The admin API is **fail-closed**. `hbbs` only listens on `ADMIN_API_PORT` after it starts with at least one enrolled admin key. If you enroll the first key after `hbbs` already started empty, restart `hbbs` once. After that, later enroll/revoke operations hot-reload live with no restart.

## Common prerequisites

- A Linux host reachable by the RustDesk clients that should use it.
- Open inbound ports for RustDesk server traffic:
  - `21115/tcp`
  - `21116/tcp`
  - `21116/udp`
  - `21117/tcp`
  - `21118/tcp`
  - `21119/tcp`
  - `21114/tcp` for the admin API, after choosing a secure exposure model below
- (Optional) A long random value for `ADMIN_API_JWT_SECRET` — see below for why you may still want to set it.
- The paired Windows client fork (`rustdeskadmin-client`) ready to receive the private admin key printed by `genadminkey`.

## Admin API transport security

The admin API is authenticated, but it is still an administrative interface. Choose the transport and firewall model deliberately before exposing `ADMIN_API_PORT` outside the host.

**Client behavior (rustdeskadmin-client v2.2.0+):** every admin API request from the Windows client automatically tries `https://` first and only falls back to plain `http://` when the HTTPS attempt fails at the transport level (TLS handshake error, connection refused, timeout) — a real HTTP response (4xx/5xx) is never retried, since that means the scheme itself worked. There is no user-facing scheme setting. The client shows a padlock icon next to the online-device count: locked/green means the last successful request used HTTPS; open/orange means it fell back to plain HTTP. **The client does not implement any custom certificate pinning or trust-prompt UI** — a self-signed or otherwise untrusted certificate simply fails the TLS handshake silently and falls back to HTTP (visible via the open padlock), it is not rejected/blocked outright. To make the client actually use HTTPS with a self-signed certificate, you must import that certificate into each Windows admin machine's OS trust store (see below) so the client's underlying TLS stack accepts it during the handshake.

By default, `hbbs` listens for the admin API directly on HTTP. The ed25519 admin-key flow does **not** send the admin private key to the server, but plain HTTP still exposes sensitive traffic to anyone who can observe or modify the network path:

- the short-lived bearer/JWT token issued after a successful challenge/verify login can be captured and replayed until it expires;
- online device IDs, optional hostnames, and last-seen metadata can be read from responses;
- responses can be tampered with, which can hide real devices or inject misleading device data;
- the admin client cannot cryptographically verify that it is talking to the intended server;
- an internet-exposed admin port receives more scanning, brute-force attempts, log noise, and attack-surface pressure.

Recommended exposure models, from strongest to weakest:

| Model | Recommendation |
| --- | --- |
| DNS name + HTTPS reverse proxy | Preferred for internet access. Put Nginx, Caddy, another TLS terminator, or a trusted load balancer in front of the admin API using a normal public CA certificate. Bind `hbbs` to a private/admin backend port if your network layout supports it, or firewall the backend so only the proxy can reach it. |
| VPN or trusted source allowlist | Good when admins can connect through WireGuard, Tailscale, a bastion, or fixed trusted IPs. Expose `ADMIN_API_PORT` only to those sources with host firewall and cloud firewall rules. |
| Self-signed IP certificate behind a reverse proxy | Acceptable for direct-IP deployments when no DNS name exists. Generate a certificate with `subjectAltName=IP:<server-ip>`, configure the admin client/OS to trust it, and protect the private key on the server. |
| Direct HTTP on a trusted private network | Usable only when the network path is already trusted and isolated. Do not use this for general internet exposure. |
| Direct HTTP on the public internet | Not recommended. Authentication still works, but bearer tokens and device-presence metadata are exposed to passive observers and active network attackers. |

### Example: HTTPS with a reverse proxy

One common pattern is to keep the public admin port encrypted while forwarding to a local-only backend:

```text
https://<admin-host-or-ip>:21114 -> TLS reverse proxy -> http://127.0.0.1:21124 -> hbbs admin API
```

Set `hbbs` to use the backend port:

```bash
ADMIN_API_PORT=21124
```

Then configure your reverse proxy to listen on the public `21114/tcp` endpoint and proxy only `/admin/` to `http://127.0.0.1:21124`. Keep `21124/tcp` blocked from untrusted networks.

If you use a self-signed certificate for a bare IP address, the certificate must include the IP address as a Subject Alternative Name:

```bash
openssl req -x509 -nodes -newkey rsa:2048 -days 825 \
  -keyout rustdesk-admin.key \
  -out rustdesk-admin.crt \
  -subj "/CN=<server-ip>" \
  -addext "subjectAltName=IP:<server-ip>" \
  -addext "keyUsage=digitalSignature,keyEncipherment" \
  -addext "extendedKeyUsage=serverAuth"
```

Import that certificate into each Windows admin client machine's local machine trust store — this is required for the client to actually use HTTPS, since it has no custom trust logic of its own:

```powershell
# Copy rustdesk-admin.crt to the client machine first, then run as Administrator:
Import-Certificate -FilePath ".\rustdesk-admin.crt" -CertStoreLocation Cert:\LocalMachine\Root
# or, using certutil:
certutil -addstore Root ".\rustdesk-admin.crt"
```

Restart `rustdesk.exe` on the client afterward and confirm the padlock icon next to the online-device count in the Admin online devices pane shows locked/green (HTTPS) rather than open/orange (HTTP fallback). Without this import step, the client's HTTPS attempt will fail the TLS handshake and it will silently fall back to plain HTTP — it will not show an error, just the open padlock — so always check the padlock after any certificate change.

## Two separate keys you will generate: don't confuse them

A fresh deployment involves **two unrelated keys**. Mixing them up is the most common setup mistake:

| | Rendezvous/relay identity key (`id_ed25519`) | Admin API key (`genadminkey`) |
| --- | --- | --- |
| Used for | Identifying **this server** to every ordinary RustDesk client (unrelated to the admin-presence fork; unmodified upstream behavior) | Authenticating **one admin operator/admin client** to the fork's admin API |
| Generated by | `hbbs` itself, automatically, on first start (or explicitly with `rustdesk-utils genkeypair`) | You, explicitly, once per admin client, with `rustdesk-utils genadminkey <label>` |
| Where it lives | `id_ed25519` / `id_ed25519.pub` in `hbbs`'s working directory | `admin_authorized_keys.json` (public keys + metadata only; `ADMIN_API_KEYS_FILE`) |
| Configured on the client at | **Settings > Network > ID/Relay Server > Key** | **Settings > Network > Admin Presence > Enroll key** |
| Revoked/rotated by | Deleting `id_ed25519*` and restarting `hbbs` (forces **every** client to update its Key field) | `rustdesk-utils revokeadminkey <fingerprint>` (hot, no restart, affects only that one admin client) |

### Fresh install: generating and distributing the rendezvous identity key

1. Start `hbbs` for the very first time with no `-k`/`KEY` value set (or `-k -`). It auto-generates `id_ed25519` and `id_ed25519.pub` in its working directory (for the systemd layout below, that's `/var/lib/rustdeskadmin`) and logs the resulting **public** key once, e.g. `Key: <base64-public-key>`.
   ```bash
   sudo journalctl -u rustdesk-hbbs | grep -m1 '^.*Key: '
   ```
   or, for Docker, `docker logs rustdeskadmin-hbbs 2>&1 | grep -m1 'Key: '`.
2. **Recommended instead of relying on auto-generation:** generate the keypair explicitly up front so it is reproducible and you can back it up before first start:
   ```bash
   rustdesk-utils genkeypair
   ```
   This prints a `Public Key` and `Secret Key` (both base64). Save the secret key as `id_ed25519` (base64 text, single line) in `hbbs`'s working directory before starting it, or pass it directly with `hbbs -k <secret-key-base64>` / the `KEY` environment variable.
3. Distribute the **public** key to every ordinary RustDesk client that should trust this server, by setting **Settings > Network > ID/Relay Server > Key** to that base64 public key value (in addition to the ID/Relay Server host/port).
4. This key has nothing to do with the admin-presence feature — it is the same mechanism every self-hosted RustDesk deployment uses, unmodified by this fork. Losing/rotating it invalidates the trust relationship for **all** connected clients (not just admin clients), so back up `id_ed25519` alongside your other server state.

### `ADMIN_API_JWT_SECRET` and `ADMIN_API_PORT` are both optional

Neither variable is required to run the admin API — both have sane defaults and the server works correctly without setting either:

- `ADMIN_API_PORT` defaults to `21114` if unset.
- `ADMIN_API_JWT_SECRET` signs and verifies the short-lived (15-minute) bearer session token issued **after** a successful ed25519 challenge/verify login (see `rustdesk-utils genadminkey`). If it is unset or empty, `hbbs` logs a warning and falls back to a random secret generated at process start. The only practical effect is that admin clients must redo the challenge/verify handshake (a few seconds) after an `hbbs` restart — the admin API itself keeps working normally either way.

Set `ADMIN_API_JWT_SECRET` explicitly only if you want admin sessions to survive routine `hbbs` restarts without a re-login:
  ```bash
  openssl rand -hex 32
  ```
  Use the same value across `hbbs` restarts (and across a fail-over pair, if you run one), but it does **not** need to be shared with `hbbr`, other services, or any client — the client never sees or needs this secret, only the bearer token `hbbs` issues after a successful ed25519 challenge/verify exchange.

## Enrollment workflow used in every deployment mode

### Enroll a key

```bash
rustdesk-utils genadminkey "<label>" [keys-file]
```

- The command prints the private key **once**.
- Paste that private key into the client fork's admin-presence enrollment UI immediately.
- The server stores only the public key, fingerprint, label, and timestamps.

### Verify enrolled keys

```bash
rustdesk-utils listadminkeys [keys-file]
```

This lists the label, fingerprint, enrolled time, last-used time, and revoked status.

### Revoke one key

```bash
rustdesk-utils revokeadminkey <fingerprint> [keys-file]
```

If the admin API is already running, revocation takes effect automatically with no restart.

### Verify the API is responding

Once `hbbs` has started with at least one key:

```bash
curl -i http://127.0.0.1:21114/admin/v1/devices?status=online
```

Expected result: JSON `401 missing_token` instead of a connection failure.

When a real admin client authenticates and refreshes the device list, `hbbs` logs audit lines containing:

- `action=auth_challenge`
- `action=auth_verify`
- `action=list_devices`

---

## 1. Native systemd deployment

### Install prerequisites

```bash
sudo apt-get update
sudo apt-get install -y curl unzip ca-certificates
```

If you want to build from source on this machine instead of using a release package:

```bash
sudo apt-get install -y build-essential pkg-config libssl-dev protobuf-compiler git
curl https://sh.rustup.rs -sSf | sh -s -- -y
. "$HOME/.cargo/env"
```

### Obtain the binaries

Choose **one** of the following.

#### Option A: download a release package

The following is the **v2.0.1 postpublication** command; do not run it until the release is published and its downloads verified. For current v2.0.0 packages, select the exact asset name from that release rather than assuming the new naming convention.

```bash
TAG=v2.0.1
VERSION=2.0.1
mkdir -p "$HOME/rustdeskadmin-release"
cd "$HOME/rustdeskadmin-release"
curl -fLO "https://github.com/hrezenmsft/rustdeskadmin-server/releases/download/${TAG}/rustdeskadmin-server-${VERSION}-linux-amd64.zip"
unzip "rustdeskadmin-server-${VERSION}-linux-amd64.zip"
```

Alternatively, after publication download the three matching `rustdesk-server-{hbbs,hbbr,utils}_2.0.1_amd64.deb` assets for a Debian-based amd64 host. Back up existing configuration and keys before any package upgrade. The steps below describe the manual ZIP/systemd layout; do not mix that layout with a package-managed installation.

Expected extracted binaries:

```text
hbbs
hbbr
rustdesk-utils
```

#### Option B: build from source

```bash
git clone https://github.com/hrezenmsft/rustdeskadmin-server.git
cd rustdeskadmin-server
git submodule update --init --recursive
cargo build --release
```

### Install binaries and directories

The following commands work for either source-built or downloaded binaries. Run them from the directory that contains `hbbs`, `hbbr`, and `rustdesk-utils`.

```bash
sudo useradd --system --home /var/lib/rustdeskadmin --create-home --shell /usr/sbin/nologin rustdesk || true
sudo install -d -m 0750 -o rustdesk -g rustdesk /var/lib/rustdeskadmin
sudo install -d -m 0755 /etc/rustdeskadmin
sudo install -m 0755 hbbs /usr/local/bin/hbbs
sudo install -m 0755 hbbr /usr/local/bin/hbbr
sudo install -m 0755 rustdesk-utils /usr/local/bin/rustdesk-utils
```

### Create the environment file (optional)

Both variables below are optional — `hbbs` runs fine with sane defaults if you skip this file entirely (`ADMIN_API_PORT` defaults to `21114`; `ADMIN_API_JWT_SECRET` falls back to an ephemeral secret, meaning admin clients just re-login after a restart). Set them only if you want a fixed port override or admin sessions that survive restarts:

```bash
sudo tee /etc/rustdeskadmin/rustdesk.env > /dev/null <<'EOF'
ADMIN_API_JWT_SECRET=<generate-a-long-random-secret>
ADMIN_API_PORT=21114
EOF
```

Add other standard RustDesk server variables to the same file if needed. If you don't create this file, remove the `EnvironmentFile=` line from the systemd units below.

### Create the systemd units

`/etc/systemd/system/rustdesk-hbbs.service`

```ini
[Unit]
Description=RustDeskAdmin hbbs
After=network.target

[Service]
Type=simple
User=rustdesk
Group=rustdesk
WorkingDirectory=/var/lib/rustdeskadmin
EnvironmentFile=/etc/rustdeskadmin/rustdesk.env
ExecStart=/usr/local/bin/hbbs -r <your-domain-or-ip>:21117
Restart=on-failure
RestartSec=5
LimitNOFILE=1000000

[Install]
WantedBy=multi-user.target
```

`/etc/systemd/system/rustdesk-hbbr.service`

```ini
[Unit]
Description=RustDeskAdmin hbbr
After=network.target

[Service]
Type=simple
User=rustdesk
Group=rustdesk
WorkingDirectory=/var/lib/rustdeskadmin
EnvironmentFile=/etc/rustdeskadmin/rustdesk.env
ExecStart=/usr/local/bin/hbbr
Restart=on-failure
RestartSec=5
LimitNOFILE=1000000

[Install]
WantedBy=multi-user.target
```

### Enroll the first admin key

Use the same data directory that `hbbs` will use:

```bash
sudo -u rustdesk /usr/local/bin/rustdesk-utils genadminkey "<label>" /var/lib/rustdeskadmin/admin_authorized_keys.json
sudo -u rustdesk /usr/local/bin/rustdesk-utils listadminkeys /var/lib/rustdeskadmin/admin_authorized_keys.json
```

Copy the printed private key into the client fork before continuing.

### Enable services

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now rustdesk-hbbs rustdesk-hbbr
```

### Open firewall ports

```bash
sudo ufw allow 21114/tcp
sudo ufw allow 21115/tcp
sudo ufw allow 21116/tcp
sudo ufw allow 21116/udp
sudo ufw allow 21117/tcp
sudo ufw allow 21118/tcp
sudo ufw allow 21119/tcp
```

### Verify

```bash
sudo systemctl status rustdesk-hbbs rustdesk-hbbr --no-pager
curl -i http://127.0.0.1:21114/admin/v1/devices?status=online
sudo journalctl -u rustdesk-hbbs -f
```

The `curl` command above is a local HTTP health check. Use HTTPS or a trusted-network allowlist for remote admin clients as described in [Admin API transport security](#admin-api-transport-security). After the client authenticates, `journalctl` should show the `admin_api audit` lines for challenge, verify, and device listing.

### Revoke a key

```bash
sudo -u rustdesk /usr/local/bin/rustdesk-utils revokeadminkey <fingerprint> /var/lib/rustdeskadmin/admin_authorized_keys.json
sudo -u rustdesk /usr/local/bin/rustdesk-utils listadminkeys /var/lib/rustdeskadmin/admin_authorized_keys.json
```

No restart is needed if the API was already running.

---

## 2. Plain docker run

These examples use the published classic image and a named Docker volume mounted at `/root`, which is the image's working directory.

### Create persistent storage

```bash
docker volume create rustdesk-data
```

### Start the containers

`ADMIN_API_JWT_SECRET` and `ADMIN_API_PORT` are both optional — omit either `-e` flag to use the defaults (`21114` for the port; an ephemeral JWT secret regenerated on every restart, which only means admin clients re-login after a restart). Set `ADMIN_API_JWT_SECRET` explicitly if you want admin sessions to survive container restarts:

```bash
docker run -d --name rustdeskadmin-hbbs \
  --restart unless-stopped \
  -e ADMIN_API_JWT_SECRET=<generate-a-long-random-secret> \
  -e ADMIN_API_PORT=21114 \
  -p 21114:21114 \
  -p 21115:21115 \
  -p 21116:21116 \
  -p 21116:21116/udp \
  -p 21118:21118 \
  -v rustdesk-data:/root \
  ghcr.io/hrezenmsft/rustdeskadmin-server:latest \
  hbbs -r <your-domain-or-ip>:21117

docker run -d --name rustdeskadmin-hbbr \
  --restart unless-stopped \
  -p 21117:21117 \
  -p 21119:21119 \
  -v rustdesk-data:/root \
  ghcr.io/hrezenmsft/rustdeskadmin-server:latest \
  hbbr
```

### Enroll an admin key

```bash
docker exec rustdeskadmin-hbbs rustdesk-utils genadminkey "<label>"
docker exec rustdeskadmin-hbbs rustdesk-utils listadminkeys
```

If this is the **first** enrolled key and `hbbs` was already started with zero keys, bind the admin API by restarting `hbbs` once:

```bash
docker restart rustdeskadmin-hbbs
```

After that first enablement, later enroll/revoke changes hot-reload live.

### Verify

```bash
docker ps --filter name=rustdeskadmin
curl -i http://127.0.0.1:21114/admin/v1/devices?status=online
docker logs -f rustdeskadmin-hbbs
```

The `curl` command above is a local HTTP health check. Use HTTPS or a trusted-network allowlist for remote admin clients as described in [Admin API transport security](#admin-api-transport-security). Use the printed private key in the client fork. Successful client activity should create `admin_api audit` log lines with `auth_challenge`, `auth_verify`, and `list_devices` actions.

### Revoke a key

```bash
docker exec rustdeskadmin-hbbs rustdesk-utils revokeadminkey <fingerprint>
docker exec rustdeskadmin-hbbs rustdesk-utils listadminkeys
```

---

## 3. Docker Compose

A ready-to-copy example is kept at the repo root as **`docker-compose.example.yml`**. It uses a named volume, the published classic image, and one container per binary.

### Example `docker-compose.yml`

`ADMIN_API_JWT_SECRET` and `ADMIN_API_PORT` under `environment:` are both optional — remove either line to use the defaults (port `21114`; an ephemeral JWT secret regenerated on every restart, which only means admin clients re-login after a restart). Set `ADMIN_API_JWT_SECRET` explicitly if you want admin sessions to survive container restarts.

```yaml
services:
  hbbs:
    image: ghcr.io/hrezenmsft/rustdeskadmin-server:latest
    container_name: rustdeskadmin-hbbs
    command: hbbs -r <your-domain-or-ip>:21117
    environment:
      ADMIN_API_JWT_SECRET: <generate-a-long-random-secret>
      ADMIN_API_PORT: 21114
    ports:
      - "21114:21114"
      - "21115:21115"
      - "21116:21116"
      - "21116:21116/udp"
      - "21118:21118"
    volumes:
      - rustdesk-data:/root
    restart: unless-stopped

  hbbr:
    image: ghcr.io/hrezenmsft/rustdeskadmin-server:latest
    container_name: rustdeskadmin-hbbr
    command: hbbr
    ports:
      - "21117:21117"
      - "21119:21119"
    volumes:
      - rustdesk-data:/root
    restart: unless-stopped

volumes:
  rustdesk-data:
```

### Start with the first key already enrolled

```bash
mkdir -p /opt/rustdeskadmin
cd /opt/rustdeskadmin
curl -O https://raw.githubusercontent.com/hrezenmsft/rustdeskadmin-server/master/docker-compose.example.yml
mv docker-compose.example.yml docker-compose.yml
docker compose pull
docker compose run --rm --no-deps hbbs rustdesk-utils genadminkey "<label>"
docker compose run --rm --no-deps hbbs rustdesk-utils listadminkeys
docker compose up -d
```

This avoids the one-time restart that would otherwise be needed if `hbbs` first started with zero enrolled keys.

### Enroll or revoke later

```bash
docker compose exec hbbs rustdesk-utils genadminkey "<label>"
docker compose exec hbbs rustdesk-utils listadminkeys
docker compose exec hbbs rustdesk-utils revokeadminkey <fingerprint>
```

If `hbbs` was already running with at least one key, these changes take effect live.

### Verify

```bash
docker compose ps
curl -i http://127.0.0.1:21114/admin/v1/devices?status=online
docker compose logs -f hbbs
```

The `curl` command above is a local HTTP health check. Use HTTPS or a trusted-network allowlist for remote admin clients as described in [Admin API transport security](#admin-api-transport-security). Once the client uses the printed private key, `hbbs` logs should include the `admin_api audit` actions for challenge, verify, and device listing.

---

## Operational notes

- Keep the admin API behind HTTPS termination or strict source/network restrictions before exposing it beyond a trusted network.
- The ordinary RustDesk ports (`21115`-`21119`) are raw RustDesk server ports and are not normally routed through an HTTP reverse proxy. Apply the admin API TLS/proxy guidance specifically to `ADMIN_API_PORT`.
- `GET /admin/v1/devices?status=online` only reports presence from the same rendezvous server instance; point the admin client at the same RustDesk server the endpoints use.
- The admin API does not expose direct DB, file, or log browsing to clients.
- Revoke one compromised client with `revokeadminkey <fingerprint>` instead of redistributing shared secrets; there are no shared admin tokens in v2.0.0.
