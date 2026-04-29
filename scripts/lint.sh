#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.." || exit 1

rustc --version
cargo --version

echo '$ cargo fmt --verbose --check'
cargo fmt --verbose --check

echo '$ cargo clippy --all-targets --all-features -- --deny=warnings'
cargo clippy --all-targets --all-features -- --deny=warnings

echo '$ RUSTDOCFLAGS='-D warnings' cargo doc --all-features'
RUSTDOCFLAGS='-D warnings' cargo doc --all-features
