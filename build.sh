#!/usr/bin/env bash
# Builds gears-0.8.0.zip, ready for SolidCraft's Preferences > Plugins > Install plugin…
# Needs: rustup target add wasm32-unknown-unknown; cargo install wasm-tools
set -euo pipefail
cd "$(dirname "$0")"
cargo test --quiet
cargo build --release --target wasm32-unknown-unknown
wasm-tools component new target/wasm32-unknown-unknown/release/gears.wasm -o package/gears.wasm
wasm-tools validate package/gears.wasm
rm -f gears-0.8.0.zip
python3 - <<'PY'
import zipfile
with zipfile.ZipFile("gears-0.8.0.zip", "w", zipfile.ZIP_DEFLATED) as z:
    for f in ("plugin.json", "gears.wasm"):
        z.write(f"package/{f}", f"gears-0.8.0/{f}")
PY
echo "built gears-0.8.0.zip"
