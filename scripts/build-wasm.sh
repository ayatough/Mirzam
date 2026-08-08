#!/usr/bin/env bash
# Mirzam コアを WebAssembly にビルドする。
# 出力: pkg/(ESM の JS グルー + .wasm + 型定義)
#
# 必要なツールは自動で導入する:
#   - rustup target add wasm32-unknown-unknown
#   - cargo install wasm-bindgen-cli --version <Cargo.lock の解決バージョン>
#
# 自動導入させたくない場合は MIRZAM_NO_INSTALL=1 を設定する。
set -euo pipefail

cd "$(dirname "$0")/.."
OUT_DIR="${1:-pkg}"

# wasm-bindgen CLI は crate と*完全に同じ*バージョンでなければならない。
# Cargo.toml のキャレット指定ではなく、実際に解決された Cargo.lock の値を使う。
WANT=$(awk '/^name = "wasm-bindgen"$/{getline; gsub(/version = |"/, ""); print; exit}' Cargo.lock)
if [ -z "$WANT" ]; then
  echo "エラー: Cargo.lock から wasm-bindgen のバージョンを取得できません" >&2
  echo "       先に cargo build を実行してください" >&2
  exit 1
fi

install_hint() {
  echo "  cargo install wasm-bindgen-cli --version $WANT" >&2
}

# wasm32 ターゲット
if ! rustup target list --installed 2>/dev/null | grep -q wasm32-unknown-unknown; then
  if [ -n "${MIRZAM_NO_INSTALL:-}" ]; then
    echo "エラー: wasm32-unknown-unknown ターゲットがありません" >&2
    echo "  rustup target add wasm32-unknown-unknown" >&2
    exit 1
  fi
  echo "==> wasm32-unknown-unknown ターゲットを追加します"
  rustup target add wasm32-unknown-unknown
fi

# wasm-bindgen CLI(未導入・バージョン不一致のどちらも入れ直す)
HAVE=""
if command -v wasm-bindgen >/dev/null 2>&1; then
  HAVE=$(wasm-bindgen --version | awk '{print $2}')
fi
if [ "$HAVE" != "$WANT" ]; then
  if [ -n "${MIRZAM_NO_INSTALL:-}" ]; then
    echo "エラー: wasm-bindgen CLI ${HAVE:-未導入} が必要な $WANT と一致しません" >&2
    install_hint
    exit 1
  fi
  echo "==> wasm-bindgen-cli $WANT を導入します(${HAVE:-未導入} → $WANT、数分かかります)"
  cargo install wasm-bindgen-cli --version "$WANT" --locked
fi

echo "==> cargo build (wasm32-unknown-unknown, release)"
cargo build --release --target wasm32-unknown-unknown -p mirzam-wasm

echo "==> wasm-bindgen"
wasm-bindgen --target web --out-dir "$OUT_DIR" \
  target/wasm32-unknown-unknown/release/mirzam_wasm.wasm

SIZE=$(du -h "$OUT_DIR"/mirzam_wasm_bg.wasm | cut -f1)
echo "✓ $OUT_DIR に出力しました (wasm: $SIZE)"
echo "  ブラウザで試す: scripts/serve-wasm-demo.sh"
