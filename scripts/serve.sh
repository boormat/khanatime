#!/usr/bin/env bash
# Unified HTTPS dev server with interactive worktree picker.
# Serves the app over https://dev.localhost (resolves to 127.0.0.1 natively).
#
# Usage:
#   scripts/serve.sh              # interactive worktree picker
#   scripts/serve.sh <directory>  # serve a specific directory
#
# Certs are stored in ~/.local/share/ssl-certs/dev.localhost/ and generated
# automatically on first use (requires mkcert — run `mise run bootstrap` first).

set -euo pipefail

# Clear screen immediately for clean startup
printf '\033[2J\033[H'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MAIN_DIR="$(git worktree list --porcelain | head -n 1 | cut -d' ' -f 2)"
HOST="dev.localhost"
PORT=8080
CERT_DIR="$HOME/.local/share/ssl-certs/$HOST"
CERT="$CERT_DIR/cert.pem"
KEY="$CERT_DIR/key.pem"

# ── helpers ──────────────────────────────────────────────────────────

die() { echo "error: $*" >&2; exit 1; }

separator() {
    printf '\033[1;36m%s\033[0m\n' \
        "═══════════════════════════════════════════════════════════════"
}

# Generate TLS certs with mkcert if they don't exist.
ensure_certs() {
    if [ ! -f "$CERT" ] || [ ! -f "$KEY" ]; then
        echo "Generating TLS cert for $HOST..."
        mkdir -p "$CERT_DIR"
        mkcert -cert-file "$CERT" -key-file "$KEY" "$HOST"
        echo "Certs created at $CERT_DIR"
    fi
}

# Find worktrees under ~/work/khanatime-* that contain the marker file.
find_ready_worktrees() {
    echo $(git worktree list |cut -d' ' -f 1)
}

# Run trunk serve from a directory with TLS (must be called in a subshell).
run_trunk() {
    cd "$1"
    env \
        TRUNK_SERVE_DISABLE_ADDRESS_LOOKUP=true \
        TRUNK_SERVE_TLS_CERT_PATH="$CERT" \
        TRUNK_SERVE_TLS_KEY_PATH="$KEY" \
        TRUNK_SERVE_WS_PROTOCOL=wss \
        trunk serve --port "$PORT"
}

# ── terminal raw mode for cursor navigation ──────────────────────────

setup_tty() {
    ORIG_STTY=$(stty -g 2>/dev/null || true)
    stty raw -echo -icanne 2>/dev/null || true
}

restore_tty() {
    [ -n "${ORIG_STTY:-}" ] && stty "$ORIG_STTY" 2>/dev/null || true
}

read_key() {
    local key=""
    IFS= read -r -s -n1 key 2>/dev/null || true
    if [ "$key" = $'\x1b' ]; then
        local seq2=""
        IFS= read -r -s -n1 -t0.1 seq2 2>/dev/null || true
        if [ "$seq2" = "[" ]; then
            local seq3=""
            IFS= read -r -s -n1 -t0.1 seq3 2>/dev/null || true
            case "$seq3" in
                A) echo "up" ;;
                B) echo "down" ;;
                *) echo "escape" ;;
            esac
        else
            echo "escape"
        fi
    elif [ "$key" = "" ]; then
        echo "enter"
    elif [ "$key" = "q" ] || [ "$key" = "Q" ]; then
        echo "quit"
    elif [ "$key" = "j" ]; then
        echo "down"
    elif [ "$key" = "k" ]; then
        echo "up"
    else
        echo "$key"
    fi
}

# ── menu rendering ──────────────────────────────────────────────────

render_menu() {
    local -n _items=$1
    local selected=$2

    printf '\033[2J\033[H'

    separator
    printf '\033[1m  Khanatime Dev Server — pick a directory to serve\033[0m\n'
    separator
    printf '  \033[2mUse j/k or arrows to navigate, Enter to select, q to quit\033[0m\n'
    printf '  \033[2mCtrl-C while serving returns to this menu\033[0m\n\n'

    for i in "${!_items[@]}"; do
        local label="${_items[$i]}"
        if [ "$i" -eq "$selected" ]; then
            printf '  \033[1;33m▶ %s\033[0m\n' "$label"
        else
            printf '    %s\n' "$label"
        fi
    done

    separator
}

# ── interactive mode ────────────────────────────────────────────────

do_interactive() {
    trap 'restore_tty; echo; exit 0' INT TERM

    items=()
    worktree_dirs=()

    for d in $(find_ready_worktrees) ; do
        [ -n "$d" ] || continue
        name=$(basename "$d")
        items+=("$name")
        worktree_dirs+=("$d")
    done

    cwd="$(pwd -P)"
    selected=0
    for i in "${!worktree_dirs[@]}"; do
        if [ "$(cd "${worktree_dirs[$i]}" && pwd -P)" = "$cwd" ]; then
            selected=$i
            break
        fi
    done

    while true; do
        render_menu items "$selected"
        key=$(read_key)

        case "$key" in
            up)
                selected=$((selected - 1))
                [ "$selected" -lt 0 ] && selected=$((${#items[@]} - 1))
                ;;
            down)
                selected=$((selected + 1))
                [ "$selected" -ge "${#items[@]}" ] && selected=0
                ;;
            enter)
                restore_tty

                wt_dir="${worktree_dirs[$selected]}"
                wt_name=$(basename "$wt_dir")
                separator
                printf '\033[1;33m  Now serving %s\033[0m\n' "$wt_dir"
                separator
                trap 'kill $TRUNK_PID 2>/dev/null' INT TERM
                run_trunk "$wt_dir" &
                TRUNK_PID=$!
                wait $TRUNK_PID 2>/dev/null || true
                kill $TRUNK_PID 2>/dev/null || true
                wait $TRUNK_PID 2>/dev/null || true
                TRUNK_PID=
                trap 'restore_tty; echo; exit 0' INT TERM
                setup_tty
                ;;
            quit)
                restore_tty
                echo ""
                exit 0
                ;;
        esac
    done
}

# ── direct mode ─────────────────────────────────────────────────────

do_direct() {
    local dir="$1"
    [ -d "$dir" ] || die "Directory not found: $dir"
    separator
    printf '\033[1;33m  Now serving %s\033[0m\n' "$(realpath "$dir")"
    separator
    run_trunk "$dir"
}

# ── main ────────────────────────────────────────────────────────────

ensure_certs

if [ $# -ge 1 ]; then
    do_direct "$1"
else
    do_interactive
fi
