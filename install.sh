#!/bin/sh
# Herduck installer: downloads the prebuilt binary for this platform from GitHub Releases.
#
#   curl -fsSL https://raw.githubusercontent.com/wenhanweime/herduck/main/install.sh | sh
#
# Environment overrides:
#   HERDUCK_VERSION      tag to install (default: latest release, including prereleases)
#   HERDUCK_INSTALL_DIR  destination directory (default: ~/.local/bin)
set -eu

repo="wenhanweime/herduck"
install_dir="${HERDUCK_INSTALL_DIR:-$HOME/.local/bin}"
version="${HERDUCK_VERSION:-}"

say() { printf '%s\n' "$*" >&2; }
die() { say "herduck install: $*"; exit 1; }

need() { command -v "$1" >/dev/null 2>&1 || die "missing required tool: $1"; }
need curl
need tar
need uname

os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Darwin) os_slug=macos ;;
  Linux) os_slug=linux ;;
  *) die "unsupported OS: $os (macOS and Linux only)" ;;
esac
case "$arch" in
  x86_64|amd64) arch_slug=x86_64 ;;
  aarch64|arm64) arch_slug=aarch64 ;;
  *) die "unsupported architecture: $arch" ;;
esac
asset="herduck-${os_slug}-${arch_slug}"

if [ -z "$version" ]; then
  version="$(curl -fsSL "https://api.github.com/repos/${repo}/releases" \
    | grep -m1 '"tag_name"' | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')"
  [ -n "$version" ] || die "could not determine the latest release tag"
fi

base="https://github.com/${repo}/releases/download/${version}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

say "Downloading herduck ${version} (${asset})..."
curl -fsSL "${base}/${asset}.tar.gz" -o "$tmp/${asset}.tar.gz" \
  || die "download failed: ${base}/${asset}.tar.gz"

if curl -fsSL "${base}/SHA256SUMS" -o "$tmp/SHA256SUMS" 2>/dev/null; then
  expected="$(grep " ${asset}.tar.gz\$" "$tmp/SHA256SUMS" | awk '{print $1}')"
  if [ -n "$expected" ]; then
    if command -v sha256sum >/dev/null 2>&1; then
      actual="$(sha256sum "$tmp/${asset}.tar.gz" | awk '{print $1}')"
    else
      actual="$(shasum -a 256 "$tmp/${asset}.tar.gz" | awk '{print $1}')"
    fi
    [ "$expected" = "$actual" ] || die "checksum mismatch for ${asset}.tar.gz"
  fi
fi

tar -xzf "$tmp/${asset}.tar.gz" -C "$tmp"
mkdir -p "$install_dir"
install -m 755 "$tmp/${asset}" "$install_dir/herduck"

if [ "$os_slug" = macos ] && command -v codesign >/dev/null 2>&1; then
  codesign --force --sign - "$install_dir/herduck" >/dev/null 2>&1 || true
fi

say "Installed: $install_dir/herduck"
"$install_dir/herduck" --version >&2 || true

case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) say ""; say "Add to PATH:  export PATH=\"$install_dir:\$PATH\"" ;;
esac
say "Run: herduck"
