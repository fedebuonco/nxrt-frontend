#!/bin/sh
# Packages the built app into a DMG and writes dist/SHA256SUMS. Run `npm run build` first.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

APP="src-tauri/target/release/bundle/macos/nxrt.app"
if [ ! -d "$APP" ]; then
    echo "Build the app first: npm run build" >&2
    exit 1
fi

VERSION=$(node -p "require('./package.json').version")
ARCH=$(uname -m)
OUT="dist/nxrt-${VERSION}-${ARCH}.dmg"
STAGE=$(mktemp -d "${TMPDIR:-/tmp}/nxrt-dmg.XXXXXX")
trap 'rm -rf "$STAGE"' EXIT INT TERM

mkdir -p dist
cp -R "$APP" "$STAGE/nxrt.app"
ln -s /Applications "$STAGE/Applications"

hdiutil create -volname "nxrt $VERSION" -srcfolder "$STAGE" -ov -format UDZO "$OUT"
hdiutil verify "$OUT"
shasum -a 256 "$OUT" > dist/SHA256SUMS
cat dist/SHA256SUMS
