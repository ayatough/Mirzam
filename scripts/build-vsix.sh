#!/usr/bin/env bash
# VSCode 拡張(.vsix)をビルドする。
#   ./scripts/build-vsix.sh
# WASM をビルドして拡張の media/ に配置し、vsce でパッケージする。
set -euo pipefail

cd "$(dirname "$0")/.."
EXT_DIR="editors/vscode"

echo "==> WASM をビルドして拡張へ配置"
./scripts/build-wasm.sh "$EXT_DIR/media"
# 型定義は配布不要
rm -f "$EXT_DIR/media"/*.d.ts

echo "==> vsce でパッケージ"
cd "$EXT_DIR"
npx --yes @vscode/vsce package --allow-missing-repository --skip-license

VSIX=$(ls -t ./*.vsix | head -1)
echo "✓ $EXT_DIR/$(basename "$VSIX")"
echo
echo "インストール:"
echo "  code --install-extension $(pwd)/$(basename "$VSIX")"
echo "  または VSCode 拡張ビューの … → 「VSIX からのインストール」"
