#!/usr/bin/env bash
# Releasable check: fmt + clippy (warnings as errors) + tests.
# Run locally before pushing; CI (test + deploy gate) calls the same script.
set -euo pipefail

cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
