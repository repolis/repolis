#!/usr/bin/env bash
# Builds the engine to WebAssembly and publishes it into the frontend.
#
# Always into a clean ./pkg, never straight into frontend/src/wasm:
# wasm-pack re-reads the package.json it wrote on a previous run and fails
# with "invalid type: sequence, expected a string" when the output directory
# is not empty, leaving an unoptimised binary behind.
set -euo pipefail

cd "$(dirname "$0")"
OUT="../frontend/src/wasm"

rm -rf pkg
wasm-pack build --target web --release

mkdir -p "$OUT"
cp pkg/engine_bg.wasm pkg/engine.js pkg/engine.d.ts pkg/engine_bg.wasm.d.ts pkg/package.json "$OUT/"
echo "wasm published to $OUT"
