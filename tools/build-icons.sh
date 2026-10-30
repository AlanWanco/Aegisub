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
# Legacy loose copies next to the hicolor directories.
cp packages/desktop/16x16/aegisub.png packages/desktop/16x16.png
cp packages/desktop/32x32/aegisub.png packages/desktop/32x32.png

echo "==> wx About dialog splash and Inno Setup branding"
logo="$TMP/wordmark.png"
rsvg-convert -w 300 docs/art-sources/aegisub-logo.svg -o "$logo"
render 1024 "$TMP/icon.png"
magick -size 420x240 xc:white \( "$TMP/icon.png" -resize 380x380 \) \
    -gravity northwest -geometry +20+55 -composite "$logo" \
    -gravity center -geometry +0-8 -composite -depth 8 -strip \
    PNG24:src/bitmaps/misc/splash.png

# The installer's wizard images: wordmark on top, icon bleeding off the bottom.
brand() { # name width height icon-size x y
    rsvg-convert -w $(($2 - 24)) docs/art-sources/aegisub-logo.svg -o "$TMP/wm-$1.png"
    magick -size "$2x$3" xc:white "$TMP/wm-$1.png" -gravity northwest \
        -geometry +10+8 -composite \( "$TMP/icon.png" -resize "$4x$4" \) \
        -gravity northwest -geometry "$5+$6" -composite -depth 8 \
        -type TrueColor BMP3:packages/win_installer/$1.bmp
}
brand welcome-large 219 386 400 -90 130
brand welcome 164 314 300 -68 100

badge() { # name width height
    side=$(( $2 < $3 ? $2 : $3 ))
    magick -size "$2x$3" xc:white \( "$TMP/icon.png" -resize $((side-4))x$((side-4)) \) \
        -gravity center -composite -depth 8 -type TrueColor BMP3:packages/win_installer/$1.bmp
}
badge aegisub-large 73 71
badge aegisub 55 58

echo "done"
