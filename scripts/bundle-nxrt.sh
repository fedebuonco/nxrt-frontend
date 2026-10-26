#!/bin/sh
# Stages the nxrt source into the app bundle (Contents/Resources/nxrt) for a release build. The app then needs no
# checkout; NXRT_DIR at run time still overrides it while developing. No games, keys or build output go in.
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$here/.." && pwd)
src="$root/nxrt"
dst="$root/src-tauri/nxrt"
[ -f "$src/nxrt.mk" ] || { echo "bundle-nxrt: $src is not an nxrt checkout (no nxrt.mk)" >&2; exit 1; }
rm -rf "$dst"
mkdir -p "$dst"
rsync -a \
  --exclude '.git' --exclude '.DS_Store' --exclude '__pycache__' --exclude '*.pyc' \
  --exclude 'build' --exclude 'target' --exclude 'games' --exclude 'venv' \
  "$src/" "$dst/"
if rev=$(git -C "$src" rev-parse --short HEAD 2>/dev/null); then
  printf '%s\n' "$rev" > "$dst/.nxrt-revision"
fi
echo "bundle-nxrt: $(du -sh "$dst" | cut -f1) from $src -> $dst"
