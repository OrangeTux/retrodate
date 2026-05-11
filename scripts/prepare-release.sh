#!/usr/bin/env bash
set -euo pipefail

# Ensure script works when invoked from anywhere in the repo:
cd "$(dirname "$0")/.." || exit 1

git cliff \
  --unreleased \
  --bump \
  --prepend CHANGELOG.md

version=$(git cliff --bumped-version)

sed  -i 's/^version = ".*"/version = "'${version#v}'"/g' Cargo.toml
cargo build
