#!/usr/bin/env bash
# T20 packaging: builds the `light` and/or `bundled` flavor for the current
# OS (ARCHITECTURE §9 — build flavor is a compile-time constant, so each
# flavor is a separate `tauri build` invocation, not a runtime switch).
#
# Usage: scripts/build-flavors.sh [light|bundled|all]   (default: all)
#
# The four release artifacts (linux/windows × bundled/light) come from
# running this script once per OS in CI (a Linux runner produces the two
# linux artifacts, a Windows runner the two windows ones) — cross-compiling
# a Tauri/WebView2 GUI app is out of scope here, same not-cross-built-in-
# this-sandbox gap CONVENTIONS already accepts for T1/T16's PATH-search and
# download-URL code (Windows verified by inspection, not CI, in this repo).
set -euo pipefail
cd "$(dirname "$0")/.."   # src-tauri/

flavor="${1:-all}"

build_light() {
  echo "==> building light flavor"
  npm run --prefix .. tauri build
}

build_bundled() {
  if [ ! -f binaries/bin/yt-dlp ] && [ ! -f binaries/bin/yt-dlp.exe ]; then
    echo "error: binaries/bin/ is empty — see binaries/bin/README.md" >&2
    exit 1
  fi
  echo "==> building bundled flavor"
  npm run --prefix .. tauri build -- --features bundled --config tauri.bundled.conf.json
}

case "$flavor" in
  light) build_light ;;
  bundled) build_bundled ;;
  all) build_light; build_bundled ;;
  *) echo "usage: $0 [light|bundled|all]" >&2; exit 1 ;;
esac
