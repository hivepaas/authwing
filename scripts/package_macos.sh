#!/bin/bash
#
# Packages a release build of authwing for macOS into dist/: authwing.app,
# signed ad hoc, tarred for the updater, and in a disk image for people (see
# docs/releasing.md).
#
#   cargo build --release -p authwing
#   scripts/package_macos.sh
#
#   dist/authwing_0.1.0_macos_aarch64.app.tar.gz
#   dist/authwing_0.1.0_macos_aarch64.dmg
#
# The signature is ad hoc (no Developer ID yet): Gatekeeper asks the first time
# the app opens (Control-click > Open), and the updater accepts a new .app whose
# signature verifies, as it does for any app not signed by a team.
#
# Environment: BIN (default target/release/authwing), ARCH (default the
# machine's: aarch64 or x86_64), DIST (default dist). APP_ONLY=1 makes only
# authwing.app, into DIST: what `make build` gives, to open as any app (the
# bare binary, opened from the Finder, runs in Terminal).

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

VERSION="$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)"
ARCH="${ARCH:-$(uname -m | sed 's/arm64/aarch64/')}"
BIN="${BIN:-target/release/authwing}"
DIST="${DIST:-dist}"
WORK="target/package-macos"

[ -x "$BIN" ] || { echo "no build at $BIN: cargo build --release -p authwing" >&2; exit 1; }
rm -rf "$WORK"
mkdir -p "$WORK" "$DIST"

## The icon, from the PNGs
ICONSET="$WORK/authwing.iconset"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
	cp "packaging/icons/png/authwing-$size.png" "$ICONSET/icon_${size}x${size}.png"
	cp "packaging/icons/png/authwing-$((size * 2)).png" "$ICONSET/icon_${size}x${size}@2x.png"
done
iconutil -c icns -o "$WORK/authwing.icns" "$ICONSET"

## authwing.app
APP="$WORK/authwing.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/authwing"
strip -x "$APP/Contents/MacOS/authwing"
cp "$WORK/authwing.icns" "$APP/Contents/Resources/authwing.icns"
sed "s/@VERSION@/$VERSION/g" packaging/macos/Info.plist >"$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist"
codesign --force --sign - --timestamp=none "$APP"
codesign --verify --deep --strict "$APP"

if [ "${APP_ONLY:-}" = 1 ]; then
	rm -rf "$DIST/authwing.app"
	ditto "$APP" "$DIST/authwing.app"
	echo "built $DIST/authwing.app"
	exit 0
fi

## For the updater: the .app, tarred
tar -C "$WORK" -czf "$DIST/authwing_${VERSION}_macos_${ARCH}.app.tar.gz" authwing.app

## For people: a disk image with the app and a link to /Applications
STAGE="$WORK/dmg"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "authwing $VERSION" -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov \
	"$DIST/authwing_${VERSION}_macos_${ARCH}.dmg"

ls -l "$DIST"
