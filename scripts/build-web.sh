#!/usr/bin/env bash
# Builds the web version into web/dist: the wasm module, its JS glue, the
# page and the worker script. Serve web/dist with any static file server.
#
# Needs the wasi-sdk (downloaded by scripts/build-wasm.sh into .wasm-tools)
# and wasm-bindgen-cli matching Cargo.lock; wasm-opt is used when present.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

WASI_SDK_VERSION="${WASI_SDK_VERSION:-34.0}"
TOOLS_DIR="${TOOLS_DIR:-$REPO_ROOT/.wasm-tools}"
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)  WASI_SDK_ASSET="wasi-sdk-${WASI_SDK_VERSION}-arm64-macos" ;;
  Darwin-x86_64) WASI_SDK_ASSET="wasi-sdk-${WASI_SDK_VERSION}-x86_64-macos" ;;
  Linux-x86_64)  WASI_SDK_ASSET="wasi-sdk-${WASI_SDK_VERSION}-x86_64-linux" ;;
  Linux-aarch64) WASI_SDK_ASSET="wasi-sdk-${WASI_SDK_VERSION}-arm64-linux" ;;
  *) echo "unsupported host $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac
export WASI_SDK_PATH="${WASI_SDK_PATH:-$TOOLS_DIR/$WASI_SDK_ASSET}"
if [ ! -x "$WASI_SDK_PATH/bin/clang" ]; then
  echo "wasi-sdk not found at $WASI_SDK_PATH; run scripts/build-wasm.sh once to download it" >&2
  exit 1
fi

export CC_wasm32_unknown_unknown="$WASI_SDK_PATH/bin/clang"
export AR_wasm32_unknown_unknown="$WASI_SDK_PATH/bin/llvm-ar"
export CFLAGS_wasm32_unknown_unknown="--target=wasm32-unknown-unknown"

PROFILE="${PROFILE:-release}"
cargo build --target wasm32-unknown-unknown --lib --profile "$PROFILE"

WASM="target/wasm32-unknown-unknown/$PROFILE/qualetize_gui.wasm"
DIST="web/dist"
rm -rf "$DIST"
mkdir -p "$DIST"
wasm-bindgen --target web --no-typescript --out-dir "$DIST" "$WASM"

if command -v wasm-opt >/dev/null; then
  wasm-opt -O3 "$DIST/qualetize_gui_bg.wasm" -o "$DIST/qualetize_gui_bg.wasm"
fi

cp web/index.html web/worker.js "$DIST/"
ls -l "$DIST" | awk 'NR>1 {print $5, $NF}'
