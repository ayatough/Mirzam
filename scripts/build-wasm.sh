#!/usr/bin/env bash
# Build the Mirzam core to WebAssembly.
# Output: pkg/ (ESM glue JS, the .wasm module, and type definitions)
#
# Missing tooling is installed automatically:
#   - rustup target add wasm32-unknown-unknown
#   - cargo install wasm-bindgen-cli --version <version resolved in Cargo.lock>
#
# Set MIRZAM_NO_INSTALL=1 to opt out of automatic installation.
set -euo pipefail

cd "$(dirname "$0")/.."
OUT_DIR="${1:-pkg}"

# The wasm-bindgen CLI must match the crate version *exactly*, so read the
# resolved value from Cargo.lock rather than the caret range in Cargo.toml.
WANT=$(awk '/^name = "wasm-bindgen"$/{getline; gsub(/version = |"/, ""); print; exit}' Cargo.lock)
if [ -z "$WANT" ]; then
  echo "error: cannot read the wasm-bindgen version from Cargo.lock" >&2
  echo "       run cargo build first" >&2
  exit 1
fi

install_hint() {
  echo "  cargo install wasm-bindgen-cli --version $WANT" >&2
}

# wasm32 target
if ! rustup target list --installed 2>/dev/null | grep -q wasm32-unknown-unknown; then
  if [ -n "${MIRZAM_NO_INSTALL:-}" ]; then
    echo "error: the wasm32-unknown-unknown target is not installed" >&2
    echo "  rustup target add wasm32-unknown-unknown" >&2
    exit 1
  fi
  echo "==> adding the wasm32-unknown-unknown target"
  rustup target add wasm32-unknown-unknown
fi

# wasm-bindgen CLI: install when missing or when the version differs.
HAVE=""
if command -v wasm-bindgen >/dev/null 2>&1; then
  HAVE=$(wasm-bindgen --version | awk '{print $2}')
fi
if [ "$HAVE" != "$WANT" ]; then
  if [ -n "${MIRZAM_NO_INSTALL:-}" ]; then
    echo "error: wasm-bindgen CLI ${HAVE:-not installed} does not match the required $WANT" >&2
    install_hint
    exit 1
  fi
  echo "==> installing wasm-bindgen-cli $WANT (from ${HAVE:-none}; this takes a few minutes)"
  cargo install wasm-bindgen-cli --version "$WANT" --locked
fi

echo "==> cargo build (wasm32-unknown-unknown, release)"
cargo build --release --target wasm32-unknown-unknown -p mirzam-wasm

echo "==> wasm-bindgen"
wasm-bindgen --target web --out-dir "$OUT_DIR" \
  target/wasm32-unknown-unknown/release/mirzam_wasm.wasm

SIZE=$(du -h "$OUT_DIR"/mirzam_wasm_bg.wasm | cut -f1)
echo "✓ wrote $OUT_DIR (wasm: $SIZE)"
echo "  Try it in a browser: scripts/serve-wasm-demo.sh"
