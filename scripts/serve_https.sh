#!/usr/bin/env bash
# HTTPS dev server for OIDC/SSO testing against matrix.org.
#
# matrix.org MAS rejects redirect URIs on http or localhost hosts, so this
# serves the app over https on a non-localhost hostname (khanatime.test) that
# still resolves to this machine via /etc/hosts. `start` generates a self-signed
# cert on first run; accept the browser warning once per host.
#
# One-time setup (needs sudo):
#   echo "127.0.0.1 khanatime.test" | sudo tee -a /etc/hosts
#
# Then:
#   scripts/serve_https.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HOST="khanatime.test"
PORT=8080
CERT_DIR="$SCRIPT_DIR/sslcerts"
CERT="$CERT_DIR/cert.pem"
KEY="$CERT_DIR/key.pem"
URL="https://$HOST:$PORT"

hosts_ok() {
    grep -qE "^\s*127\.0\.0\.1\s+$HOST(\s|$)" /etc/hosts
}

gen_cert() {
    mkdir -p "$CERT_DIR"
    if [ ! -f "$CERT" ] || [ ! -f "$KEY" ]; then
        echo "Generating self-signed cert for $HOST (accept the browser warning once)..."
        openssl req -x509 -newkey rsa:2048 -nodes \
            -keyout "$KEY" -out "$CERT" -days 365 \
            -subj "/CN=$HOST" -addext "subjectAltName=DNS:$HOST"
    fi
}

start() {
    if ! hosts_ok; then
        echo "Missing /etc/hosts alias. One-time setup (needs sudo):" >&2
        echo "  echo '127.0.0.1 $HOST' | sudo tee -a /etc/hosts" >&2
        exit 1
    fi
    gen_cert
    echo "Serving $URL (homeserver https://matrix.org for SSO)"
    # disable_address_lookup: trunk reverse-resolves 127.0.0.1 via /etc/hosts,
    # printing the hostname with a trailing FQDN dot (khanatime.test.) — noise
    TRUNK_SERVE_DISABLE_ADDRESS_LOOKUP=true \
    TRUNK_SERVE_TLS_CERT_PATH="$CERT" \
    TRUNK_SERVE_TLS_KEY_PATH="$KEY" \
    TRUNK_SERVE_WS_PROTOCOL=wss \
    trunk serve
}

case "${1:-start}" in
    start) start ;;
    cert) gen_cert ;;
    *) echo "usage: $0 {start|cert}" >&2; exit 1 ;;
esac