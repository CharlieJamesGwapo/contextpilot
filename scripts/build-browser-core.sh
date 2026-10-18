#!/bin/sh
# Build the genuine Rust core for browser use. No hosted services or paid tools.
# Prerequisites: rustup target add wasm32-unknown-unknown
#               cargo install wasm-bindgen-cli --version 0.2.129 --locked
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CARGO=${CARGO:-cargo}
WASM_BINDGEN=${WASM_BINDGEN:-wasm-bindgen}
# Strip local build-machine paths from panic strings and generated binaries.
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$HOME=/build-home --remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}=/cargo --remap-path-prefix=$ROOT=/contextpilot"
"$CARGO" build --manifest-path "$ROOT/Cargo.toml" --locked --release \
  --target wasm32-unknown-unknown -p contextpilot-browser
TARGET_DIR=$("$CARGO" metadata --manifest-path "$ROOT/Cargo.toml" --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
"$WASM_BINDGEN" "$TARGET_DIR/wasm32-unknown-unknown/release/contextpilot_browser.wasm" \
  --target web --out-dir "$ROOT/apps/dashboard/app/browser-core" --out-name contextpilot_browser

mkdir -p "$ROOT/apps/dashboard/public/browser-core"
mv "$ROOT/apps/dashboard/app/browser-core/contextpilot_browser_bg.wasm" "$ROOT/apps/dashboard/public/browser-core/"
rm -f "$ROOT/apps/dashboard/public/browser-core/contextpilot_browser.js" "$ROOT/apps/dashboard/public/browser-core/"*.d.ts
python3 - "$ROOT/apps/dashboard/app/browser-core/contextpilot_browser.js" <<'PYTHON'
from pathlib import Path
import sys
p = Path(sys.argv[1])
s = p.read_text()
old = "new URL('contextpilot_browser_bg.wasm', import.meta.url)"
assert s.count(old) == 1, 'wasm-bindgen loader format changed; review before publication'
p.write_text(s.replace(old, "'/browser-core/contextpilot_browser_bg.wasm'"))
PYTHON
