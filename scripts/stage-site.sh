#!/usr/bin/env bash
# Assembles the website as it's deployed into <out> (default dist/site): the
# site at the root and the calculator under calculator/.
set -euo pipefail
cd "$(dirname "$0")/.."
out="${1:-dist/site}"
rm -rf "$out"
mkdir -p "$out/calculator/pkg"
cp web/site/index.html web/site/app.js web/site/style.css web/site/config.js "$out/"
cp web/calculator/index.html web/calculator/app.js web/calculator/style.css "$out/calculator/"
cp web/calculator/pkg/satchel_calc.js web/calculator/pkg/satchel_calc_bg.wasm "$out/calculator/pkg/"
echo "$out"
