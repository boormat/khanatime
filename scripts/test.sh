#!/usr/bin/env bash
# Interactive test server: pick a worktree (or main) and serve with trunk.
# Worktrees signal readiness by containing a "test-me-please" marker file.
# Use arrow keys/j/k to navigate, Enter to select, q to quit.
#
# Requires: trunk, openssl (or mkcert for trusted certs)
# Note: Run with 'mise exec --' or ensure mise env is active for RUSTC_WRAPPER

set -euo pipefail

# Ensure mise environment is active (for RUSTC_WRAPPER, tool paths)
if [ -z "${RUSTC_WRAPPER:-}" ] && command -v mise &>/dev/null; then
    eval "$(mise env)"
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MAIN_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
WORK_BASE="$HOME/work"
HOST="khanatime.test"
PORT=8080
MARKER="test-me-please"

# ── helpers ──────────────────────────────────────────────────────────

die() { echo "error: $*" >&2; exit 1; }

separator() {
    printf '\033[1;36m%s\033[0m\n' \
        "═══════════════════════════════════════════════════════════════"
}

# Find worktrees under ~/work/khanatime-* that contain the marker file.
find_ready_worktrees() {
    for dir in "$WORK_BASE"/khanatime-*/; do
        [ -d "$dir" ] || continue
        [ -f "$dir/$MARKER" ] || continue
        echo "$dir"
    done
}

# Copy SSL certs from main repo into a worktree (idempotent).
ensure_ssl_certs() {
    local wt_dir="$1"
    local wt_cert_dir="$wt_dir/scripts/sslcerts"
    local src_cert_dir="$MAIN_DIR/scripts/sslcerts"
    if [ ! -f "$wt_cert_dir/cert.pem" ]; then
        echo "Copying SSL certs into worktree..."
        mkdir -p "$wt_cert_dir"
        cp "$src_cert_dir/cert.pem" "$src_cert_dir/key.pem" "$wt_cert_dir/"
    fi
}

# Run trunk serve from a directory.
run_trunk() {
    local dir="$1"
    exec env \
        TRUNK_SERVE_DISABLE_ADDRESS_LOOKUP=true \
        TRUNK_SERVE_TLS_CERT_PATH="$dir/scripts/sslcerts/cert.pem" \
        TRUNK_SERVE_TLS_KEY_PATH="$dir/scripts/sslcerts/key.pem" \
        TRUNK_SERVE_WS_PROTOCOL=wss \
        trunk serve
}

# ── terminal raw mode for cursor navigation ──────────────────────────

setup_tty() {
    # Save original stty settings and switch to raw mode
    ORIG_STTY=$(stty -g 2>/dev/null || true)
    stty raw -echo -icanne 2>/dev/null || true
}

restore_tty() {
    [ -n "${ORIG_STTY:-}" ] && stty "$ORIG_STTY" 2>/dev/null || true
}

# Read a single keypress (returns escape sequence for arrows)
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
    local total=${#_items[@]}

    # Move cursor to top and clear
    printf '\033[2J\033[H'

    separator
    printf '\033[1m  Khanatime Test Server — pick a directory to serve\033[0m\n'
    separator
    printf '  \033[2mUse j/k or arrows to navigate, Enter to select, q to quit\033[0m\n\n'

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

# ── main ────────────────────────────────────────────────────────────

# Check hosts entry
if ! grep -qE "^\s*127\.0\.0\.1\s+$HOST(\s|$)" /etc/hosts; then
    die "Missing /etc/hosts alias. Run once with sudo:\n  echo '127.0.0.1 $HOST' | sudo tee -a /etc/hosts"
fi

# Ensure SSL certs exist in main repo
mkdir -p "$MAIN_DIR/scripts/sslcerts"
if [ ! -f "$MAIN_DIR/scripts/sslcerts/cert.pem" ]; then
    echo "Generating self-signed cert for $HOST (accept the browser warning once)..."
    openssl req -x509 -newkey rsa:2048 -nodes \
        -keyout "$MAIN_DIR/scripts/sslcerts/key.pem" \
        -out "$MAIN_DIR/scripts/sslcerts/cert.pem" \
        -days 365 \
        -subj "/CN=$HOST" -addext "subjectAltName=DNS:$HOST"
fi

trap 'restore_tty; echo; exit 0' INT TERM

# Build item list (ready worktrees + main)
items=()
worktree_dirs=()

while IFS= read -r d; do
    [ -n "$d" ] || continue
    name=$(basename "$d")
    items+=("$name")
    worktree_dirs+=("$d")
done < <(find_ready_worktrees)

if [ ${#items[@]} -eq 0 ]; then
    items+=("(no ready worktrees)")
fi

items+=("--- Main repo ($(basename "$MAIN_DIR")) ---")
main_idx=$((${#items[@]} - 1))

selected=0

# Main loop
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

            if [ "$selected" -eq "$main_idx" ]; then
                # Main repo
                echo ""
                echo "Pulling latest changes in main repo..."
                if ! git -C "$MAIN_DIR" pull; then
                    echo "Pull failed — resolve conflicts manually, then try again."
                    echo "Press any key to continue..."
                    read -r -s -n1
                    setup_tty
                    continue
                fi
                separator
                printf '\033[1;33m  Now serving MAIN (%s)\033[0m\n' "$MAIN_DIR"
                separator
                run_trunk "$MAIN_DIR"
            elif [ "$selected" -lt "${#worktree_dirs[@]}" ]; then
                # Worktree
                wt_dir="${worktree_dirs[$selected]}"
                wt_name=$(basename "$wt_dir")
                ensure_ssl_certs "$wt_dir"
                separator
                printf '\033[1;33m  Now serving %s\033[0m\n' "$wt_name"
                separator
                run_trunk "$wt_dir"
            fi

            # Trunk exited (Ctrl-C), loop back to menu
            setup_tty
            ;;
        quit)
            restore_tty
            echo ""
            exit 0
            ;;
    esac
done
