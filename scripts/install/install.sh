#!/bin/sh
# Install octorun from a GitHub release.
# Usage: sh install.sh
#   OCTORUN_VERSION       release tag to install (defaults to the latest)
#   OCTORUN_INSTALL_DIR   directory to install into (defaults to ~/.local/bin)

set -eu

REPO="CHE3MZ/octorun"
VERSION="${OCTORUN_VERSION:-}"
INSTALL_DIR="${OCTORUN_INSTALL_DIR:-$HOME/.local/bin}"

detect_target() {
  os=$(uname -s)
  arch=$(uname -m)

  case "$os" in
    Linux) os_part="unknown-linux-gnu" ;;
    Darwin) os_part="apple-darwin" ;;
    *)
      echo "unsupported operating system: $os" >&2
      exit 1
      ;;
  esac

  case "$arch" in
    x86_64 | amd64) arch_part="x86_64" ;;
    arm64 | aarch64) arch_part="aarch64" ;;
    *)
      echo "unsupported architecture: $arch" >&2
      exit 1
      ;;
  esac

  printf '%s-%s' "$arch_part" "$os_part"
}

if [ -z "$VERSION" ]; then
  VERSION=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" |
    sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)
  if [ -z "$VERSION" ]; then
    echo "could not determine the latest release" >&2
    exit 1
  fi
fi

case "$VERSION" in
  v*) ;;
  *) VERSION="v$VERSION" ;;
esac

TARGET=$(detect_target)
ASSET="octorun-${TARGET}.tar.gz"
URL="https://github.com/$REPO/releases/download/$VERSION/$ASSET"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

echo "Downloading $URL"
curl -fsSL -o "$TMP/$ASSET" "$URL"
curl -fsSL -o "$TMP/$ASSET.sha256" "$URL.sha256"

cd "$TMP"
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum -c "$ASSET.sha256"
elif command -v shasum >/dev/null 2>&1; then
  shasum -a 256 -c "$ASSET.sha256"
else
  echo "warning: no sha256 tool found; skipping checksum verification" >&2
fi

tar -xzf "$ASSET"
mkdir -p "$INSTALL_DIR"
if command -v install >/dev/null 2>&1; then
  install -m 0755 octorun "$INSTALL_DIR/octorun"
else
  cp octorun "$INSTALL_DIR/octorun"
  chmod 0755 "$INSTALL_DIR/octorun"
fi

echo "Installed octorun to $INSTALL_DIR/octorun"
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) echo "Add $INSTALL_DIR to PATH to use octorun." ;;
esac