#!/usr/bin/env bash
# Packages release binaries for one target into dist/satchel-<target>.tar.gz
# plus its .sha256, the files scripts/install.sh downloads.
#
#   scripts/package-release.sh <target> <version> [cargo target dir]
#
# Builds with `cargo build --release --target <target>` unless the binaries
# are already in <cargo target dir>/<target>/release.
set -euo pipefail

target=$1
version=$2
target_dir="${3:-${CARGO_TARGET_DIR:-target}}"
bins=(satchel-api satchel-index satchel-node satchel-verifier satchel-files)

cd "$(dirname "$0")/.."
release="$target_dir/$target/release"
if [[ ! -x "$release/satchel-files" ]]; then
    args=()
    for b in "${bins[@]}"; do args+=(-p "$b"); done
    cargo build --release --locked --target "$target" "${args[@]}"
fi

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/bin"
for b in "${bins[@]}"; do cp "$release/$b" "$stage/bin/"; done
cp scripts/satchel "$stage/bin/satchel"
chmod +x "$stage/bin/"*
echo "$version" >"$stage/VERSION"
cp LICENSE "$stage/" 2>/dev/null || true

mkdir -p dist
asset="dist/satchel-$target.tar.gz"
tar -czf "$asset" -C "$stage" .
if command -v sha256sum >/dev/null; then
    (cd dist && sha256sum "satchel-$target.tar.gz" >"satchel-$target.tar.gz.sha256")
else
    (cd dist && shasum -a 256 "satchel-$target.tar.gz" >"satchel-$target.tar.gz.sha256")
fi
echo "$asset"
