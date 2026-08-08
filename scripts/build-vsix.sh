#!/usr/bin/env bash
# Build the VS Code extension (.vsix).
#   ./scripts/build-vsix.sh
# Builds the WASM package into the extension's media/ directory, then runs vsce.
set -euo pipefail

cd "$(dirname "$0")/.."
EXT_DIR="editors/vscode"

echo "==> building WASM into the extension"
./scripts/build-wasm.sh "$EXT_DIR/media"
# Type definitions are not needed in the package.
rm -f "$EXT_DIR/media"/*.d.ts

echo "==> packaging with vsce"
cd "$EXT_DIR"
npx --yes @vscode/vsce package --allow-missing-repository --skip-license

VSIX=$(ls -t ./*.vsix | head -1)
echo "✓ $EXT_DIR/$(basename "$VSIX")"
echo
echo "Install with:"
echo "  code --install-extension $(pwd)/$(basename "$VSIX")"
echo "  or: Extensions view -> ... -> Install from VSIX"
