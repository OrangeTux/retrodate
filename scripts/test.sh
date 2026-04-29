#!/usr/bin/env bash
set -euo pipefail

# Ensure script works when invoked from anywhere in the repo:
cd "$(dirname "$0")/.." || exit 1

rustc --version
cargo --version

echo "RUST_LOG=debug RUST_BACKTRACE=1 cargo test --all-targets --all-features --verbose"
RUST_LOG=debug RUST_BACKTRACE=1 cargo test --all-targets --all-features --verbose
