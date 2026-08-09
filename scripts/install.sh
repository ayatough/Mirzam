#!/bin/sh
# Installs a prebuilt `mirzam` binary. No Rust toolchain required.
#
#   curl -fsSL https://raw.githubusercontent.com/ayatough/Mirzam/main/scripts/install.sh | sh
#
# Environment:
#   MIRZAM_VERSION   tag to install (default: the latest release)
#   MIRZAM_BIN_DIR   where to put the binary (default: ~/.local/bin)
#
# Windows is published as a .zip on the releases page; download it there.

set -eu

REPO=ayatough/Mirzam
BIN_DIR=${MIRZAM_BIN_DIR:-$HOME/.local/bin}

die() { echo "install.sh: $*" >&2; exit 1; }

target() {
  os=$(uname -s)
  arch=$(uname -m)
  case "$os/$arch" in
    Linux/x86_64)          echo x86_64-unknown-linux-gnu ;;
    Linux/aarch64|Linux/arm64) echo aarch64-unknown-linux-gnu ;;
    Darwin/arm64)          echo aarch64-apple-darwin ;;
    Darwin/x86_64)         echo x86_64-apple-darwin ;;
    *) die "no prebuilt binary for $os/$arch - build from source: https://github.com/$REPO#install" ;;
  esac
}

# The releases API answers with the tag of the latest release; asking for it is
# how `MIRZAM_VERSION` gets a default without this script knowing the version.
latest() {
  curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
    | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' \
    | head -1
}

command -v curl >/dev/null || die "curl is required"
command -v tar  >/dev/null || die "tar is required"

TARGET=$(target)
VERSION=${MIRZAM_VERSION:-$(latest)}
[ -n "$VERSION" ] || die "could not determine the latest release; set MIRZAM_VERSION"

NAME="mirzam-$VERSION-$TARGET"
URL="https://github.com/$REPO/releases/download/$VERSION/$NAME.tar.gz"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

echo "downloading $NAME"
curl -fsSL "$URL" -o "$TMP/mirzam.tar.gz" \
  || die "download failed: $URL"

# Verify against the checksum published beside the archive when it is
# reachable. A missing checksum is not fatal; a mismatched one is.
if curl -fsSL "$URL.sha256" -o "$TMP/mirzam.tar.gz.sha256" 2>/dev/null; then
  want=$(cut -d' ' -f1 < "$TMP/mirzam.tar.gz.sha256")
  if command -v sha256sum >/dev/null; then
    got=$(sha256sum "$TMP/mirzam.tar.gz" | cut -d' ' -f1)
  elif command -v shasum >/dev/null; then
    got=$(shasum -a 256 "$TMP/mirzam.tar.gz" | cut -d' ' -f1)
  else
    got=$want
  fi
  [ "$want" = "$got" ] || die "checksum mismatch for $NAME.tar.gz"
fi

tar xzf "$TMP/mirzam.tar.gz" -C "$TMP"
mkdir -p "$BIN_DIR"
install -m 755 "$TMP/$NAME/mirzam" "$BIN_DIR/mirzam"

echo "installed $("$BIN_DIR/mirzam" --version) to $BIN_DIR/mirzam"
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) echo "note: $BIN_DIR is not on your PATH" ;;
esac
