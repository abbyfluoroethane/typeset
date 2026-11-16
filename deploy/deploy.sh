#!/usr/bin/env bash
# Builds the server for the target host and installs it as a systemd service.
#
#   deploy/deploy.sh [ssh-host]        (default: typeset@bento.foid.space)
#
# Needs: zig, cargo-zigbuild, `rustup target add aarch64-unknown-linux-musl`.
# The host needs passwordless sudo. /etc/typeset/typeset.env is created from
# .env.example on first deploy and never overwritten afterwards. A new server
# starts in setup mode: open it in a browser with the link from its log.
#
# PULL_FROM=<this machine's LAN IP> makes the host download the files from a
# temporary HTTP server here instead of receiving them over scp (useful when
# uploads over the SSH path stall, e.g. behind hairpin NAT).
set -euo pipefail

HOST="${1:-typeset@bento.foid.space}"
TARGET="${TARGET:-aarch64-unknown-linux-musl}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/$TARGET/release/typeset-server"

echo "==> building typeset-server for $TARGET"
(cd "$ROOT" && cargo zigbuild --release -p typeset-server --target "$TARGET")

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"; [ -n "${SERVER_PID:-}" ] && kill "$SERVER_PID" 2>/dev/null || true' EXIT
cp "$BIN" "$ROOT/deploy/typeset.service" "$STAGE/"
cp "$ROOT/.env.example" "$STAGE/env.example"

if [ -n "${PULL_FROM:-}" ]; then
  echo "==> serving files on $PULL_FROM:8765 for $HOST to pull"
  (cd "$STAGE" && sha256sum typeset-server typeset.service env.example 2>/dev/null \
    || shasum -a 256 typeset-server typeset.service env.example) > "$STAGE/SHA256SUMS"
  python3 -m http.server 8765 --bind "$PULL_FROM" --directory "$STAGE" >/dev/null 2>&1 &
  SERVER_PID=$!
  sleep 1
  ssh "$HOST" "set -euo pipefail; cd /tmp
    for f in typeset-server typeset.service env.example SHA256SUMS; do
      curl -fsS -m 60 -o \$f http://$PULL_FROM:8765/\$f
    done
    sha256sum -c --quiet SHA256SUMS && rm SHA256SUMS"
else
  echo "==> uploading to $HOST"
  scp -q "$STAGE/typeset-server" "$STAGE/typeset.service" "$STAGE/env.example" "$HOST:/tmp/"
fi

echo "==> installing"
ssh "$HOST" 'set -euo pipefail
  sudo install -m 0755 /tmp/typeset-server /usr/local/bin/typeset-server
  sudo install -m 0644 /tmp/typeset.service /etc/systemd/system/typeset.service
  sudo restorecon -F /usr/local/bin/typeset-server /etc/systemd/system/typeset.service 2>/dev/null || true
  if ! sudo test -f /etc/typeset/typeset.env; then
    sudo install -d -m 0750 /etc/typeset
    sudo install -m 0600 /tmp/env.example /etc/typeset/typeset.env
    echo "    created /etc/typeset/typeset.env from .env.example"
    echo "    finish setup in a browser: sudo journalctl -u typeset | grep setup"
  fi
  rm -f /tmp/typeset-server /tmp/typeset.service /tmp/env.example
  sudo systemctl daemon-reload
  sudo systemctl enable --quiet typeset
  sudo systemctl restart typeset
  sleep 1
  port=$(sudo sed -n "s/^TYPESET_BIND=.*://p" /etc/typeset/typeset.env)
  systemctl is-active typeset && curl -fsS "http://127.0.0.1:${port:-8080}/health" >/dev/null && echo "    healthy"
'
