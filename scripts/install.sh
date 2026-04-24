#!/usr/bin/env bash
set -euo pipefail

REPO="beelol/overmind"
BIN="ovmd"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

platform="$(uname -s | tr '[:upper:]' '[:lower:]')"
arch="$(uname -m)"

case "$arch" in
  x86_64|amd64) arch="x86_64" ;;
  arm64|aarch64) arch="aarch64" ;;
  *) echo "Unsupported architecture: $arch" >&2; exit 1 ;;
esac

case "$platform" in
  darwin) target="${arch}-apple-darwin" ;;
  linux) target="${arch}-unknown-linux-gnu" ;;
  *) echo "Unsupported platform: $platform" >&2; exit 1 ;;
esac

url="https://github.com/${REPO}/releases/latest/download/${BIN}-${target}.tar.gz"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

mkdir -p "$INSTALL_DIR"
curl -fsSL "$url" -o "$tmp/${BIN}.tar.gz"
tar -xzf "$tmp/${BIN}.tar.gz" -C "$tmp"
install "$tmp/$BIN" "$INSTALL_DIR/$BIN"

echo "Installed $BIN to $INSTALL_DIR/$BIN"
