#!/usr/bin/env bash
#
# Regenerate every shipped application icon from the master SVG.
#
# Requires rsvg-convert (librsvg), ImageMagick (magick) and, for the macOS
# bundle, iconutil. Run from the repository root:
#
#   tools/build-icons.sh
#
set -euo pipefail

cd "$(dirname "$0")/.."

SRC="docs/art-sources/aegisub-icon-papirus.svg"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

if [ ! -f "$SRC" ]; then
    echo "missing master artwork: $SRC" >&2
    exit 1
fi

render() {
    rsvg-convert -w "$1" -h "$1" "$SRC" -o "$2"
}

echo "==> Linux hicolor icons"
for size in 16 22 24 32 48 64; do
    render "$size" "packages/desktop/${size}x${size}/aegisub.png"
done
cp "$SRC" packages/desktop/scalable/aegisub.svg

echo "==> macOS bundle icon"
iconset="$TMP/Aegisub.iconset"
mkdir -p "$iconset"
render 16 "$iconset/icon_16x16.png"
render 32 "$iconset/icon_16x16@2x.png"
render 32 "$iconset/icon_32x32.png"
render 64 "$iconset/icon_32x32@2x.png"
render 128 "$iconset/icon_128x128.png"
render 256 "$iconset/icon_128x128@2x.png"
render 256 "$iconset/icon_256x256.png"
render 512 "$iconset/icon_256x256@2x.png"
render 512 "$iconset/icon_512x512.png"
render 1024 "$iconset/icon_512x512@2x.png"
iconutil -c icns "$iconset" -o packages/osx_bundle/Contents/Resources/Aegisub.icns

echo "==> Windows icon"
for size in 16 24 32 48 64 128 256; do
    render "$size" "$TMP/win-$size.png"
done
magick "$TMP"/win-256.png "$TMP"/win-128.png "$TMP"/win-64.png "$TMP"/win-48.png \
    "$TMP"/win-32.png "$TMP"/win-24.png "$TMP"/win-16.png \
    src/bitmaps/windows/icon.ico
cp src/bitmaps/windows/icon.ico packages/win_installer/portable/icon.ico

echo "done"
