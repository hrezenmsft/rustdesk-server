# Deploying RustDeskAdmin server

This guide covers three Linux deployment modes for this fork:

1. native `systemd`
2. plain `docker run`
3. `docker compose`

All examples use placeholders such as `<your-domain-or-ip>`, `<label>`, `<fingerprint>`, and `<generate-a-long-random-secret>`. Replace them with your own values before use.

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
  - `21114/tcp` for the admin API
- A long random value for `ADMIN_API_JWT_SECRET`.
- The paired Windows client fork (`rustdeskadmin-client`) ready to receive the private admin key printed by `genadminkey`.

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

```bash
TAG=<tag>
ARCH=<arch-zip-suffix>
mkdir -p "$HOME/rustdeskadmin-release"
cd "$HOME/rustdeskadmin-release"
curl -LO "https://github.com/hrezenmsft/rustdeskadmin-server/releases/download/${TAG}/rustdesk-server-linux-${ARCH}.zip"
unzip "rustdesk-server-linux-${ARCH}.zip"
```

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

### Create the environment file

```bash
sudo tee /etc/rustdeskadmin/rustdesk.env > /dev/null <<'EOF'
ADMIN_API_JWT_SECRET=<generate-a-long-random-secret>
ADMIN_API_PORT=21114
EOF
```

Add other standard RustDesk server variables to the same file if needed.

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

After the client authenticates, `journalctl` should show the `admin_api audit` lines for challenge, verify, and device listing.

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

Use the printed private key in the client fork. Successful client activity should create `admin_api audit` log lines with `auth_challenge`, `auth_verify`, and `list_devices` actions.

### Revoke a key

```bash
docker exec rustdeskadmin-hbbs rustdesk-utils revokeadminkey <fingerprint>
docker exec rustdeskadmin-hbbs rustdesk-utils listadminkeys
```

---

## 3. Docker Compose

A ready-to-copy example is kept at the repo root as **`docker-compose.example.yml`**. It uses a named volume, the published classic image, and one container per binary.

### Example `docker-compose.yml`

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

Once the client uses the printed private key, `hbbs` logs should include the `admin_api audit` actions for challenge, verify, and device listing.

---

## Operational notes

- Keep the admin API behind HTTPS termination and appropriate network restrictions before exposing it beyond a trusted network.
- `GET /admin/v1/devices?status=online` only reports presence from the same rendezvous server instance; point the admin client at the same RustDesk server the endpoints use.
- The admin API does not expose direct DB, file, or log browsing to clients.
- Revoke one compromised client with `revokeadminkey <fingerprint>` instead of redistributing shared secrets; there are no shared admin tokens in v2.0.0.
