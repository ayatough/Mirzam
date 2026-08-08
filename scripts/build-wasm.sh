#!/usr/bin/env bash
# Mirzam コアを WebAssembly にビルドする。
# 出力: pkg/(ESM の JS グルー + .wasm + 型定義)
#
# 前提:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli   # バージョンは Cargo.toml の wasm-bindgen と揃える
set -euo pipefail

cd "$(dirname "$0")/.."
OUT_DIR="${1:-pkg}"

# wasm-bindgen CLI と crate のバージョン不一致はよくある失敗なので先に検査する
CRATE_VER=$(grep -oP 'wasm-bindgen = "\K[0-9.]+' crates/mirzam-wasm/Cargo.toml)
CLI_VER=$(wasm-bindgen --version | awk '{print $2}')
if [ "$CRATE_VER" != "$CLI_VER" ]; then
  echo "警告: wasm-bindgen CLI ($CLI_VER) と crate ($CRATE_VER) のバージョンが異なります" >&2
  echo "      cargo install wasm-bindgen-cli --version $CRATE_VER" >&2
fi

echo "==> cargo build (wasm32-unknown-unknown, release)"
cargo build --release --target wasm32-unknown-unknown -p mirzam-wasm

echo "==> wasm-bindgen"
wasm-bindgen --target web --out-dir "$OUT_DIR" \
  target/wasm32-unknown-unknown/release/mirzam_wasm.wasm

SIZE=$(du -h "$OUT_DIR"/mirzam_wasm_bg.wasm | cut -f1)
echo "✓ $OUT_DIR に出力しました (wasm: $SIZE)"
echo "  wasm-opt -Oz があればさらに縮小できます:"
echo "    wasm-opt -Oz -o $OUT_DIR/mirzam_wasm_bg.wasm $OUT_DIR/mirzam_wasm_bg.wasm"
