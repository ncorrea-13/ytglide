#!/usr/bin/env bash
# Builds release binaries for glibc and musl and packages each into a
# tar.gz under dist/ (gitignored), bundling the binary with LICENSE.
#
# Requires the musl target: rustup target add x86_64-unknown-linux-musl
# (no musl C toolchain needed: every dependency is pure Rust, so rustup's
# bundled musl libc.a is enough to link statically.)
#
# Usage: scripts/package.sh

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

version="v$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
targets=(x86_64-unknown-linux-gnu x86_64-unknown-linux-musl)

mkdir -p dist

for target in "${targets[@]}"; do
  echo "==> building $target"
  cargo build --release --target "$target"
  name="ytglide-$version-$target"
  stage="dist/$name"
  mkdir -p "$stage"
  cp "target/$target/release/ytglide" "$stage/"
  cp LICENSE "$stage/"
  tar -C dist -czf "dist/$name.tar.gz" "$name"
  rm -rf "$stage"
  echo "==> dist/$name.tar.gz"
done
