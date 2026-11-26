#!/usr/bin/env bash
# builds the engine to wasm and publishes it to the frontend
#
#  build-wasm.sh        release build
#  build-wasm.sh --dev  for `npm run dev`
set -euo pipefail

cd "$(dirname "$0")"
OUT="../frontend/src/wasm"
WASM="$OUT/engine_bg.wasm"

if [[ "${1:-}" == "--dev" ]]; then
    if [[ -f "$WASM" ]] && [[ -z "$(find src Cargo.toml Cargo.lock build-wasm.sh -newer "$WASM" -print -quit)" ]]; then
        echo "wasm up to date"
        exit 0
    fi
    BUILD=(--profile wasm-dev --no-opt)
else
    BUILD=(--release)
fi

rm -rf pkg
wasm-pack build --target web "${BUILD[@]}"

mkdir -p "$OUT"
cp pkg/engine_bg.wasm pkg/engine.js pkg/engine.d.ts pkg/engine_bg.wasm.d.ts pkg/package.json "$OUT/"
echo "wasm published to $OUT"
