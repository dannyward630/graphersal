#!/usr/bin/env bash
# Builds the Graphersal engine as WebAssembly for the playground: playground/pkg/ (git-ignored).
#
# Prerequisites (one-time):
#   rustup target add wasm32-unknown-unknown
#   cargo install --locked wasm-bindgen-cli --version 0.2.129   # must equal the wasm-bindgen crate
#   wasm-opt (binaryen), optional: shrinks the module further  # e.g. brew install binaryen
#
# Usage: playground/build.sh [--dev]
#   --dev  unoptimized debug build (fast to compile, large, with debug info); no wasm-opt.
# Then:  python3 -m http.server -d playground 8000   and open http://localhost:8000/
set -euo pipefail

WASM_BINDGEN_VERSION="0.2.129"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/playground/pkg"
PROFILE="wasm-release"
if [[ "${1:-}" == "--dev" ]]; then
  PROFILE="dev"
fi

if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "error: wasm-bindgen CLI not found." >&2
  echo "       cargo install --locked wasm-bindgen-cli --version $WASM_BINDGEN_VERSION" >&2
  exit 1
fi
found="$(wasm-bindgen --version | awk '{print $2}')"
if [[ "$found" != "$WASM_BINDGEN_VERSION" ]]; then
  echo "error: wasm-bindgen CLI $found found, $WASM_BINDGEN_VERSION required (it must match the crate)." >&2
  echo "       cargo install --locked --force wasm-bindgen-cli --version $WASM_BINDGEN_VERSION" >&2
  exit 1
fi

cd "$ROOT"
cargo build -p graphersal-wasm --target wasm32-unknown-unknown --profile "$PROFILE"
target_dir="$PROFILE"
[[ "$PROFILE" == "dev" ]] && target_dir="debug"

rm -rf "$OUT"
wasm-bindgen --target web --no-typescript --out-dir "$OUT" \
  "target/wasm32-unknown-unknown/$target_dir/graphersal_wasm.wasm"

wasm="$OUT/graphersal_wasm_bg.wasm"
if [[ "$PROFILE" != "dev" ]]; then
  if command -v wasm-opt >/dev/null 2>&1; then
    # The features rustc enables by default for wasm32-unknown-unknown.
    wasm-opt -Oz \
      --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
      --enable-mutable-globals --enable-reference-types --enable-multivalue \
      -o "$wasm.opt" "$wasm"
    mv "$wasm.opt" "$wasm"
  else
    echo "note: wasm-opt not found; the module is not size-optimized further (optional)." >&2
  fi
fi

# What the page shows next to the engine version in its header (js/build-info.js): the commit the
# module was built from. A tree with uncommitted changes is marked dirty.
commit=$(git -C "$ROOT" rev-parse --short=12 HEAD 2>/dev/null || echo "")
describe=$(git -C "$ROOT" describe --tags --always --dirty 2>/dev/null || echo "")
printf '{"commit": "%s", "describe": "%s"}\n' "$commit" "$describe" > "$OUT/build-info.json"

raw=$(wc -c < "$wasm" | tr -d ' ')
gz=$(gzip -9 -c "$wasm" | wc -c | tr -d ' ')
echo "built $wasm: $raw bytes ($gz bytes gzipped)"
