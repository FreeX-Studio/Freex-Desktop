#!/usr/bin/env bash
#
# Copyright (c) 2026 MirraNET, Niklas Linz. All rights reserved.
#
# This file is part of the MirraNET project and is licensed under the
# GNU Lesser General Public License v3.0 (LGPLv3).
#
# You may use, distribute and modify this code under the terms
# of the LGPLv3 license. You should have received a copy of the
# license along with this file. If not, see <https://www.gnu.org/licenses/lgpl-3.0.html>
# or contact: niklas.linz@mirranet.de
#
# Builds FreeX Desktop for macOS on a Mac (GitHub Actions: macos-latest):
#
#   packaging/macos/build.sh [build number]
#
# Result in dist/: FreeX.app as freex-desktop-<version>-macos.dmg (drag to Applications) and
# freex-desktop-<version>-macos.zip. The app is a universal binary for Apple silicon and Intel
# Macs, macOS 11 or newer. mpv is not bundled; FreeX uses the one from Homebrew (brew install
# mpv) or mpv.app if present, otherwise the FreeX player.
#
# Signing: without settings the app is signed ad hoc (Gatekeeper asks once, see the README).
# With an Apple Developer ID it is signed for real and, optionally, notarized:
#   MACOS_SIGN_IDENTITY    "Developer ID Application: Name (TEAMID)" from the login keychain
#   MACOS_NOTARY_PROFILE   a profile stored with `xcrun notarytool store-credentials`
#
# Needs Xcode or the Command Line Tools (clang, lipo, codesign, hdiutil) and rustup; the two
# Rust targets are added when missing.

set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
BUILD="${1:-0}"
TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
export MACOSX_DEPLOYMENT_TARGET=11.0

[ "$(uname -s)" = Darwin ] || { echo "macos/build: run this on a Mac" >&2; exit 1; }
for tool in cargo rustup lipo codesign hdiutil ditto plutil; do
    command -v "$tool" >/dev/null || { echo "macos/build: $tool is missing" >&2; exit 1; }
done

VERSION="$(sed -n 's/^version *= *"\(.*\)"/\1/p' "$ROOT/src-tauri/Cargo.toml" | head -n 1)"
APP_NAME=FreeX
OUT="$ROOT/dist"
WORK="$ROOT/src-tauri/target/macos-bundle"
APP="$WORK/$APP_NAME.app"

installed="$(rustup target list --installed)"
for target in "${TARGETS[@]}"; do
    grep -qx "$target" <<<"$installed" || rustup target add "$target"
done

# Both architectures, then one universal binary.
binaries=()
for target in "${TARGETS[@]}"; do
    # The TV mode's interface (ui/tv, from FreeX-TV-Apps) is bundled into the app.
    [ -f "$ROOT/ui/tv/index.html" ] || "$ROOT/packaging/tv-ui.sh"
    (cd "$ROOT/src-tauri" && cargo build --release --locked --features custom-protocol --target "$target")
    binaries+=("$ROOT/src-tauri/target/$target/release/freex-desktop")
done

rm -rf "$WORK"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create -output "$APP/Contents/MacOS/freex-desktop" "${binaries[@]}"
cp "$ROOT/src-tauri/icons/icon.icns" "$APP/Contents/Resources/icon.icns"
sed -e "s/\[\[version\]\]/$VERSION/" -e "s/\[\[build\]\]/$BUILD/" "$HERE/Info.plist" > "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist" >/dev/null
printf 'APPL????' > "$APP/Contents/PkgInfo"

if [ -n "${MACOS_SIGN_IDENTITY:-}" ]; then
    codesign --force --timestamp --options runtime --sign "$MACOS_SIGN_IDENTITY" "$APP"
else
    # Ad hoc: required for Apple silicon, and keeps the bundle consistent after lipo.
    codesign --force --sign - "$APP"
fi
codesign --verify --strict "$APP"

mkdir -p "$OUT"
DMG="$OUT/freex-desktop-$VERSION-macos.dmg"
ZIP="$OUT/freex-desktop-$VERSION-macos.zip"
rm -f "$DMG" "$ZIP"

# The disk image shows the app next to a link to Applications.
STAGE="$WORK/dmg"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -quiet -volname "$APP_NAME" -srcfolder "$STAGE" -fs HFS+ -format UDZO "$DMG"
ditto -c -k --keepParent "$APP" "$ZIP"

if [ -n "${MACOS_SIGN_IDENTITY:-}" ]; then
    codesign --force --timestamp --sign "$MACOS_SIGN_IDENTITY" "$DMG"
    if [ -n "${MACOS_NOTARY_PROFILE:-}" ]; then
        xcrun notarytool submit "$DMG" --keychain-profile "$MACOS_NOTARY_PROFILE" --wait
        xcrun stapler staple "$DMG"
    fi
fi

echo "macos/build: $DMG"
echo "macos/build: $ZIP"
