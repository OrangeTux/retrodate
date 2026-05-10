#!/usr/bin/env bash
set -e

git cliff \
  --unreleased \
  --bump \
  --prepend CHANGELOG.md

version=$(git cliff --bumped-version)

sed  -i 's/^version = ".*"/version = "'${version//[v]}'"/g' Cargo.toml
cargo build
