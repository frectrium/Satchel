#!/usr/bin/env bash
# Builds the calculator page's WASM bundle (#54) into web/calculator/pkg and
# checks it against the CLI. Needs the wasm32 target and wasm-bindgen-cli at
# the version pinned in Cargo.toml:
#
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.130 --locked
#
# satchel-api builds the page into its binary, so rebuild the bundle and
# commit web/calculator/pkg whenever crates/satchel-cost or satchel-calc change.
set -euo pipefail

cd "$(dirname "$0")/.."
cargo build -p satchel-calc --target wasm32-unknown-unknown --release --locked
target_dir="${CARGO_TARGET_DIR:-target}"
wasm-bindgen --target web --no-typescript --out-dir web/calculator/pkg \
    "$target_dir/wasm32-unknown-unknown/release/satchel_calc.wasm"
node web/calculator/smoke.mjs
