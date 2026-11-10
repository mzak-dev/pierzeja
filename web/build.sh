#!/usr/bin/env bash
# Build przeglądarkowego portu edytora (crates/app → wasm32) do katalogu dist/ (GitHub Pages).
# Wymaga: rustup target add wasm32-unknown-unknown; wasm-bindgen-cli w wersji z Cargo.lock;
# opcjonalnie wasm-opt (binaryen) — zmniejsza plik .wasm.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:-dist}"

cargo build -p pierzeja-app --target wasm32-unknown-unknown --profile web
rm -rf "$OUT" && mkdir -p "$OUT"
wasm-bindgen --target web --no-typescript --out-dir "$OUT" target/wasm32-unknown-unknown/web/pierzeja-app.wasm
if command -v wasm-opt >/dev/null; then
  # stara wersja binaryen może nie znać nowszych rozszerzeń wasm — wtedy zostaje plik bez optymalizacji
  if wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals --enable-reference-types --enable-multivalue -o "$OUT/opt.wasm" "$OUT/pierzeja-app_bg.wasm"; then
    mv "$OUT/opt.wasm" "$OUT/pierzeja-app_bg.wasm"
  else
    echo "wasm-opt nie powiódł się — pomijam optymalizację" >&2
    rm -f "$OUT/opt.wasm"
  fi
fi
cp web/index.html "$OUT/"
cp crates/app/assets/icon.png "$OUT/"
touch "$OUT/.nojekyll"
ls -la "$OUT"
