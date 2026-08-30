#!/usr/bin/env bash
# Releasable check: fmt + clippy (warnings as errors) + tests.
# Run locally before pushing; CI (test + deploy gate) calls the same script.
# The wasm target is built + linted too, so cfg(target_arch = "wasm32") code
# (the browser transport) can't drift out of compile without failing here.
# The wasm test suite runs the browser-only paths (localStorage, DOM, Matrix
# transport) headless via wasm-bindgen-test-runner + geckodriver.
set -euo pipefail

cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
rustup target add wasm32-unknown-unknown
cargo clippy --target wasm32-unknown-unknown -- -D warnings
cargo build --target wasm32-unknown-unknown
./scripts/wasm-test.sh
