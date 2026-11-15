#!/usr/bin/env bash
# Installs Satchel on this computer and starts it.
#
#   curl -fsSL https://raw.githubusercontent.com/frectrium/Satchel/main/scripts/install.sh | bash
#
# Everything goes in ~/satchel (binaries, data, logs). Nothing needs root,
# nothing outside ~/satchel is touched, and every service listens on
# 127.0.0.1 only. Remove it all with: ~/satchel/bin/satchel uninstall
#
# Settings, all optional:
#   SATCHEL_VERSION   release tag to install, for example v1.0.0 (default: latest)
#   SATCHEL_HOME      install directory (default: ~/satchel)
#   SATCHEL_NO_START  set to 1 to install without starting
#   SATCHEL_ALLOWED_ORIGINS  website origins allowed to call the local API
#   SATCHEL_RELEASE_BASE  where to download from (default: GitHub Releases)
set -euo pipefail

REPO="frectrium/Satchel"
SATCHEL_HOME="${SATCHEL_HOME:-$HOME/satchel}"
VERSION="${SATCHEL_VERSION:-latest}"

say() { printf '%s\n' "$*"; }
die() { printf 'install: %s\n' "$*" >&2; exit 1; }

command -v curl >/dev/null || die "curl is required"
command -v tar >/dev/null || die "tar is required"

case "$(uname -s)" in
    Darwin) os=apple-darwin ;;
    Linux) os=unknown-linux-gnu ;;
    *) die "unsupported OS $(uname -s). Satchel ships for macOS and Linux." ;;
esac
case "$(uname -m)" in
    arm64 | aarch64) arch=aarch64 ;;
    x86_64 | amd64) arch=x86_64 ;;
    *) die "unsupported CPU $(uname -m)" ;;
esac
target="$arch-$os"
asset="satchel-$target.tar.gz"

if [[ -n "${SATCHEL_RELEASE_BASE:-}" ]]; then
    base="$SATCHEL_RELEASE_BASE"
elif [[ "$VERSION" == latest ]]; then
    base="https://github.com/$REPO/releases/latest/download"
else
    base="https://github.com/$REPO/releases/download/$VERSION"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

say "Downloading Satchel ($VERSION) for $target"
curl -fsSL "$base/$asset" -o "$tmp/$asset" || die "could not download $base/$asset"
curl -fsSL "$base/$asset.sha256" -o "$tmp/$asset.sha256" || die "could not download the checksum"

want="$(awk '{print $1}' "$tmp/$asset.sha256")"
if command -v sha256sum >/dev/null; then
    got="$(sha256sum "$tmp/$asset" | awk '{print $1}')"
else
    got="$(shasum -a 256 "$tmp/$asset" | awk '{print $1}')"
fi
[[ "$want" == "$got" ]] || die "checksum mismatch for $asset"

if [[ -x "$SATCHEL_HOME/bin/satchel" ]]; then
    "$SATCHEL_HOME/bin/satchel" stop >/dev/null 2>&1 || true
fi
mkdir -p "$SATCHEL_HOME"
tar -xzf "$tmp/$asset" -C "$SATCHEL_HOME"
chmod +x "$SATCHEL_HOME"/bin/*

# The website passes its own address, so the browser may call this install.
origins="${SATCHEL_ALLOWED_ORIGINS:-}"
[[ "$origins" =~ ^[A-Za-z0-9:/.,_-]*$ ]] || die "SATCHEL_ALLOWED_ORIGINS has unexpected characters"
config="$SATCHEL_HOME/config"
if [[ ! -f "$config" ]]; then
    {
        echo "# Sourced by 'satchel start'. Origins of the Satchel website, which may call"
        echo "# the files API on 127.0.0.1:47800 from the browser, separated by commas."
        echo "# Local pages are always allowed."
        echo "SATCHEL_ALLOWED_ORIGINS=\"$origins\""
    } >"$config"
elif [[ -n "$origins" ]]; then
    grep -v '^SATCHEL_ALLOWED_ORIGINS=' "$config" >"$tmp/config" || true
    echo "SATCHEL_ALLOWED_ORIGINS=\"$origins\"" >>"$tmp/config"
    cp "$tmp/config" "$config"
fi

say "Installed Satchel $(cat "$SATCHEL_HOME/VERSION") in $SATCHEL_HOME"
if [[ "${SATCHEL_NO_START:-0}" != 1 ]]; then
    "$SATCHEL_HOME/bin/satchel" start
fi
say ""
say "To use the satchel command from anywhere, add it to your PATH:"
say "  export PATH=\"$SATCHEL_HOME/bin:\$PATH\""
