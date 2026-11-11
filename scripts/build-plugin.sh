#!/usr/bin/env bash
# Regenerate the Claude Code plugin in plugins/mirzam/ from the binary.
#
#   ./scripts/build-plugin.sh              build the CLI, then write the plugin
#   ./scripts/build-plugin.sh <mirzam>     use that binary instead
#
# The plugin's skill is not written by hand: it is exactly what
# `mirzam skill install` writes - SKILL.md stamped with the version, and the
# syntax card compiled in from docs/llms.md - so the plugin and the binary it
# tells people to install cannot describe different markup. The marketplace
# (.claude-plugin/marketplace.json) serves whatever is committed here, so this
# runs at release time from scripts/release.sh, not on every edit to the card:
# between releases the plugin keeps describing the release `install.sh` fetches.
set -euo pipefail

cd "$(dirname "$0")/.."

BIN="${1:-}"
if [ -z "$BIN" ]; then
  cargo build --quiet --release --bin mirzam
  BIN=./target/release/mirzam
fi

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
PLUGIN=plugins/mirzam
SKILL="$PLUGIN/skills/mirzam"

# `--user` writes under $HOME, so a scratch HOME is a scratch install.
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
HOME="$tmp" "$BIN" skill install --user --force >/dev/null

rm -rf "$SKILL"
mkdir -p "$SKILL"
cp -R "$tmp/.claude/skills/mirzam/." "$SKILL/"

# The plugin's own version moves with the workspace.
awk -v ver="$VERSION" '
  !done && /"version": "/ { sub(/: "[^"]*"/, ": \"" ver "\""); done = 1 } { print }' \
  "$PLUGIN/.claude-plugin/plugin.json" > "$tmp/plugin.json"
mv "$tmp/plugin.json" "$PLUGIN/.claude-plugin/plugin.json"

echo "✓ wrote $PLUGIN (mirzam $VERSION)"
