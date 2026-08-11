#!/usr/bin/env bash
# Element Web (browser Matrix client) for the local Synapse.
#
# Serves Element at http://localhost:8085, defaulting to the Khanatime
# homeserver (http://localhost:8008, see element-config.json). `start` also
# boots the local Synapse matrix homeserver (`synapse` container).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG="$SCRIPT_DIR/element-config.json"
NAME="element-web"
SYNAPSE="synapse"
PORT=8085
IMAGE="docker.io/vectorim/element-web:latest"

start() {
    start_synapse
    if ! podman image exists "$IMAGE" 2>/dev/null; then
        echo "Pulling $IMAGE (first run, may take a minute)..."
        podman pull "$IMAGE"
    fi
    if podman container exists "$NAME" 2>/dev/null; then
        podman start "$NAME"
    else
        podman run -d \
            --name "$NAME" \
            -e ELEMENT_WEB_PORT=8080 \
            -p "127.0.0.1:$PORT:8080" \
            -v "$CONFIG:/app/config.json:ro" \
            "$IMAGE"
    fi
    echo "Element Web: http://localhost:$PORT (homeserver http://localhost:8008)"
}

start_synapse() {
    if ! podman container exists "$SYNAPSE" 2>/dev/null; then
        echo "No '$SYNAPSE' container — create the Synapse homeserver first (see AGENTS.md)." >&2
        return 1
    fi
    podman start "$SYNAPSE"
    echo "Synapse homeserver: http://localhost:8008"
}

stop() {
    podman stop "$NAME"
}

restart() {
    podman restart "$NAME"
    echo "Element Web: http://localhost:$PORT"
}

status() {
    if ! podman container exists "$NAME" 2>/dev/null; then
        echo "stopped (not created)"
        return 1
    fi
    podman ps --filter "name=^$NAME$" --format "{{.Names}}: {{.Status}}"
    if podman ps --filter "name=^$NAME$" --format "{{.Status}}" | grep -q "Up"; then
        code=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:$PORT" || true)
        echo "http://localhost:$PORT -> $code"
    fi
}

log() {
    podman logs -f "$NAME"
}

case "${1:-status}" in
    start) start ;;
    stop) stop ;;
    restart) restart ;;
    status) status ;;
    log) log ;;
    *) echo "usage: $0 {start|stop|restart|status|log}" >&2; exit 1 ;;
esac
