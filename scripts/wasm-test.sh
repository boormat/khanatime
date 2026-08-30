#!/usr/bin/env bash
# Run the wasm test suite headless: `cargo test --target wasm32-unknown-unknown`.
#
# Requires wasm-bindgen-test-runner on PATH (provides the `runner` set in
# .cargo/config.toml) plus a headless browser + driver.  On ubuntu-latest the
# GitHub runner ships firefox + geckodriver; locally they're installed via the
# snap packages.  wasm-bindgen-cli is installed on first run and cached by
# swatinem/rust-cache in CI, so `cargo install` is a no-op after that.
set -euo pipefail

WBG_VER="0.2.127"

if ! command -v wasm-bindgen-test-runner >/dev/null 2>&1; then
    echo "wasm-bindgen-test-runner not found — installing wasm-bindgen-cli $WBG_VER"
    # Some environments export CC=zig (e.g. cargo-zigbuild) which breaks ring's
    # build script; fall back to a real C compiler for the install only.
    CC="${CC:-gcc}" cargo install wasm-bindgen-cli --version "$WBG_VER" --locked
fi

# geckodriver can be killed on startup under load; retry once to absorb the flake.
for attempt in 1 2; do
    if cargo test --target wasm32-unknown-unknown; then
        exit 0
    fi
    echo "wasm test attempt $attempt failed; retrying..." >&2
    pkill -9 -f geckodriver 2>/dev/null || true
    pkill -9 -f firefox 2>/dev/null || true
    sleep 2
done
exit 1
