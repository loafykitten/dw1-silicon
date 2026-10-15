#!/bin/sh
# Builds the Mac app from this repo: build/Digimon World.app (release build, the icon from
# packaging/icon-1024.png, signed ad hoc for this Mac). The app needs nothing but macOS; it
# finds your disc image on its first start.
#
#   tools/make-app.sh             build it
#   tools/make-app.sh --install   and copy it to ~/Applications (replacing an earlier copy)
#
# The app holds code and text built from the game, so it's for your own Mac: don't share it.
set -eu
cd "$(dirname "$0")/.." || exit 1

install=0
case "${1:-}" in
    --install) install=1 ;;
    "") ;;
    *) echo "usage: tools/make-app.sh [--install]" >&2; exit 2 ;;
esac

cargo build --release -p dw1
bin=target/release/dw1

# Only macOS's own libraries (SDL is built in): anything else wouldn't be on another Mac.
others=$(otool -L "$bin" | tail -n +2 | awk '{print $1}' | grep -v -E '^(/System/Library/|/usr/lib/)' || true)
if [ -n "$others" ]; then
    echo "make-app: the game links libraries outside macOS:" >&2
    echo "$others" >&2
    exit 1
fi

version=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/dw1/Cargo.toml | head -n 1)
minos=$(otool -l "$bin" | awk '/LC_BUILD_VERSION/ { found = 1 } found && $1 == "minos" { print $2; exit }')
[ -n "$version" ] && [ -n "$minos" ] || { echo "make-app: no version or minimum macOS" >&2; exit 1; }

app="build/Digimon World.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/dw1"
sed -e "s/@VERSION@/$version/g" -e "s/@MINOS@/$minos/g" packaging/Info.plist.in > "$app/Contents/Info.plist"
plutil -lint -s "$app/Contents/Info.plist"

iconset=build/dw1.iconset
rm -rf "$iconset"
mkdir -p "$iconset"
for s in 16 32 128 256 512; do
    sips -z "$s" "$s" packaging/icon-1024.png --out "$iconset/icon_${s}x${s}.png" > /dev/null
    d=$((s * 2))
    sips -z "$d" "$d" packaging/icon-1024.png --out "$iconset/icon_${s}x${s}@2x.png" > /dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/dw1.icns"
rm -rf "$iconset"

codesign --force --sign - "$app"
echo "make-app: built $app ($version, macOS $minos or later)"

if [ "$install" = 1 ]; then
    dest="$HOME/Applications/Digimon World.app"
    if [ -e "$dest" ]; then
        # only ever replace this app, not something else with the same name
        id=$(defaults read "$dest/Contents/Info" CFBundleIdentifier 2> /dev/null || true)
        if [ "$id" != "cafe.loafykitten.dw1-silicon" ]; then
            echo "make-app: $dest is another app (${id:-no bundle id}); not replacing it" >&2
            exit 1
        fi
        rm -rf "$dest"
    fi
    mkdir -p "$HOME/Applications"
    cp -R "$app" "$dest"
    echo "make-app: installed $dest"
fi
