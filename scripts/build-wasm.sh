#!/usr/bin/env bash
# Builds the crate (Rust + the Qualetize C library) for wasm32-unknown-unknown,
# generates the wasm-bindgen JS glue and runs the smoke test under node.
#
# Works on macOS arm64 and on ubuntu-latest x86_64.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

WASI_SDK_VERSION="${WASI_SDK_VERSION:-34.0}"
WASI_SDK_MAJOR="${WASI_SDK_VERSION%%.*}"
TOOLS_DIR="${TOOLS_DIR:-$REPO_ROOT/.wasm-tools}"
# The wasm-bindgen CLI must match the wasm-bindgen crate version in Cargo.lock.
WASM_BINDGEN_VERSION="$(awk '/^name = "wasm-bindgen"$/{f=1;next} f&&/^version = /{gsub(/"/,"",$3);print $3;exit}' Cargo.lock)"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)  WASI_SDK_ASSET="wasi-sdk-${WASI_SDK_VERSION}-arm64-macos" ;;
  Darwin-x86_64) WASI_SDK_ASSET="wasi-sdk-${WASI_SDK_VERSION}-x86_64-macos" ;;
  Linux-x86_64)  WASI_SDK_ASSET="wasi-sdk-${WASI_SDK_VERSION}-x86_64-linux" ;;
  Linux-aarch64) WASI_SDK_ASSET="wasi-sdk-${WASI_SDK_VERSION}-arm64-linux" ;;
  *) echo "unsupported host $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac

export WASI_SDK_PATH="${WASI_SDK_PATH:-$TOOLS_DIR/$WASI_SDK_ASSET}"
if [ ! -x "$WASI_SDK_PATH/bin/clang" ]; then
  mkdir -p "$TOOLS_DIR"
  URL="https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-${WASI_SDK_MAJOR}/${WASI_SDK_ASSET}.tar.gz"
  echo "downloading $URL"
  curl -fsSL "$URL" | tar xz -C "$TOOLS_DIR"
fi

rustup target add wasm32-unknown-unknown
if ! command -v wasm-bindgen >/dev/null || \
   [ "$(wasm-bindgen --version | awk '{print $2}')" != "$WASM_BINDGEN_VERSION" ]; then
  cargo install wasm-bindgen-cli --version "$WASM_BINDGEN_VERSION" --locked
fi

# The Qualetize C sources are compiled by build.rs with the wasi-sdk clang; the
# wasi sysroot also provides the malloc/qsort/math the objects reference.
export CC_wasm32_unknown_unknown="$WASI_SDK_PATH/bin/clang"
export AR_wasm32_unknown_unknown="$WASI_SDK_PATH/bin/llvm-ar"
export CFLAGS_wasm32_unknown_unknown="--target=wasm32-unknown-unknown"

cargo build --target wasm32-unknown-unknown --lib --release

WASM="target/wasm32-unknown-unknown/release/qualetize_gui.wasm"
OUT="target/wasm-spike"
rm -rf "$OUT"
wasm-bindgen --target nodejs --out-dir "$OUT" "$WASM"

echo "== imports of $OUT/qualetize_gui_bg.wasm =="
"$WASI_SDK_PATH/bin/llvm-objdump" -h "$OUT/qualetize_gui_bg.wasm" >/dev/null
if command -v wasm-dis >/dev/null; then
  wasm-dis "$OUT/qualetize_gui_bg.wasm" | grep -E '^\s*\(import' || echo "(none)"
fi

echo "== sizes =="
ls -l "$WASM" "$OUT/qualetize_gui_bg.wasm" | awk '{print $5, $NF}'
if command -v wasm-opt >/dev/null; then
  wasm-opt -Oz "$OUT/qualetize_gui_bg.wasm" -o "$OUT/qualetize_gui_bg.opt.wasm"
  ls -l "$OUT/qualetize_gui_bg.opt.wasm" | awk '{print $5, $NF}'
fi

echo "== smoke test (node) =="
node -e "console.log(require('./$OUT/qualetize_gui.js').smoke())"
