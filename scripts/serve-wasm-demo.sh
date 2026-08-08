#!/usr/bin/env bash
# Try the WASM core in a browser.
#   ./scripts/serve-wasm-demo.sh [port]
# Builds the WASM package if needed, then serves the demo locally.
# (.wasm cannot be loaded over file://, so HTTP is required)
set -euo pipefail

cd "$(dirname "$0")/.."
PORT="${1:-8080}"
DEMO_DIR="web/wasm-demo"

if [ ! -f "$DEMO_DIR/pkg/mirzam_wasm.js" ]; then
  echo "==> WASM package not built yet; building it"
  ./scripts/build-wasm.sh "$DEMO_DIR/pkg"
fi

echo
echo "▶ open http://localhost:$PORT/ (Ctrl-C to stop)"
echo
cd "$DEMO_DIR"
python3 -m http.server "$PORT"
