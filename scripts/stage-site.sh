#!/usr/bin/env bash
# Assembles the website as it's deployed into <out> (default dist/site): the
# React app (web/app) at the root and the calculator under calculator/.
set -euo pipefail
cd "$(dirname "$0")/.."
out="${1:-dist/site}"
mkdir -p "$out"
find "$out" -mindepth 1 -delete
mkdir -p "$out/calculator/pkg"
# The React app (web/app), built with Vite.
if [[ ! -f web/app/dist/index.html || -n "${SATCHEL_REBUILD_SITE:-1}" ]]; then
    (cd web/app && npm ci --no-audit --no-fund --silent && npm run build --silent)
fi
cp -R web/app/dist/. "$out/"
cp web/calculator/index.html web/calculator/app.js web/calculator/style.css "$out/calculator/"
cp web/calculator/pkg/satchel_calc.js web/calculator/pkg/satchel_calc_bg.wasm "$out/calculator/pkg/"
echo "$out"
