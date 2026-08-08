#!/usr/bin/env bash
# WASM コアをブラウザで試す。
#   ./scripts/serve-wasm-demo.sh [ポート]
# ビルド済みでなければ自動でビルドし、ローカルサーバを立てて案内する。
# (.wasm は file:// では読み込めないため HTTP 配信が必要)
set -euo pipefail

cd "$(dirname "$0")/.."
PORT="${1:-8080}"
DEMO_DIR="web/wasm-demo"

if [ ! -f "$DEMO_DIR/pkg/mirzam_wasm.js" ]; then
  echo "==> WASM が未ビルドのためビルドします"
  ./scripts/build-wasm.sh "$DEMO_DIR/pkg"
fi

echo
echo "▶ http://localhost:$PORT/ を開いてください(Ctrl-C で終了)"
echo
cd "$DEMO_DIR"
python3 -m http.server "$PORT"
