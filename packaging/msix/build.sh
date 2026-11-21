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
# Builds the MSIX package of FreeX Desktop for the Microsoft Store on Linux:
# dist/freex-desktop-<version>-x64.msix, unsigned (the Store signs it when it is published).
#
#   packaging/msix/build.sh
#
# Takes the Windows app as packaging/windows/build.sh lays it out for the installer
# (dist/windows: freex-desktop.exe, WebView2Loader.dll, mpv\, licences); run that first.
#
# The package identity comes from Partner Center (the app's page > Product identity) and must
# match it, or the Store refuses the upload. Set in the Jenkins job (Global properties) or here:
#   MSIX_IDENTITY        Package/Identity/Name, e.g. 12345MirraNET.FreeXDesktop
#   MSIX_PUBLISHER       Package/Identity/Publisher, e.g. CN=1A2B3C4D-...
#   MSIX_PUBLISHER_NAME  Package/Properties/PublisherDisplayName, e.g. MirraNET
#
# The package version is the app version with a fourth number 0 (1.2.0 -> 1.2.0.0); the Store
# wants the fourth number to be 0 and every upload newer than the last one, so raise the version
# in src-tauri/Cargo.toml for every Store release.
#
# Needs: rsvg-convert (librsvg2-bin), and for makemsix (Microsoft's MSIX packaging tool, built
# once from source into FREEX_DOWNLOAD_CACHE, default ~/.cache/freex-build): git, cmake 3.29+,
# clang, make. Set MAKEMSIX to use a makemsix built elsewhere.

set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
DIST="$ROOT/dist"
STAGE="$DIST/windows"

MSIX_IDENTITY="${MSIX_IDENTITY:-MirraNET.FreeXDesktop}"
MSIX_PUBLISHER="${MSIX_PUBLISHER:-CN=MirraNET}"
MSIX_PUBLISHER_NAME="${MSIX_PUBLISHER_NAME:-MirraNET}"

# makemsix from github.com/microsoft/msix-packaging; to update it, change the commit.
MSIX_PACKAGING_COMMIT="25a65f5c1690930813bcc10cdf1d59fa865f2bb1"
CACHE="${FREEX_DOWNLOAD_CACHE:-$HOME/.cache/freex-build}"

need() {
    for tool in "$@"; do
        command -v "$tool" >/dev/null || { echo "msix: $tool is missing" >&2; exit 1; }
    done
}

[ -f "$STAGE/freex-desktop.exe" ] || {
    echo "msix: $STAGE/freex-desktop.exe is missing, run packaging/windows/build.sh first" >&2
    exit 1
}
need rsvg-convert

# ------------------------------------------------------------------ makemsix

if [ -z "${MAKEMSIX:-}" ]; then
    src="$CACHE/msix-packaging-$MSIX_PACKAGING_COMMIT"
    MAKEMSIX="$src/.vs/bin/makemsix"
    if [ ! -x "$MAKEMSIX" ]; then
        need git cmake clang clang++ make
        echo "msix: building makemsix (once)" >&2
        rm -rf "$src"
        mkdir -p "$src"
        git -C "$src" init -q
        git -C "$src" fetch -q --depth 1 https://github.com/microsoft/msix-packaging.git "$MSIX_PACKAGING_COMMIT"
        git -C "$src" checkout -q FETCH_HEAD
        # makemsix finds its libmsix.so by an absolute path inside this folder.
        (cd "$src" && ./makelinux.sh --pack --skip-samples --skip-tests >"$src/build.log" 2>&1) || {
            tail -n 30 "$src/build.log" >&2
            echo "msix: building makemsix failed (see $src/build.log)" >&2
            exit 1
        }
    fi
fi

# ------------------------------------------------------------------ the package

VERSION="$(sed -n 's/^version *= *"\(.*\)"/\1/p' "$ROOT/src-tauri/Cargo.toml" | head -1)"
numeric="${VERSION%%-*}"
# Three numbers and a fourth 0: 1.2 -> 1.2.0.0, 1.2.3 -> 1.2.3.0
while [ "$(tr -cd . <<<"$numeric" | wc -c)" -lt 2 ]; do numeric="$numeric.0"; done
numeric="$numeric.0"

LAYOUT="$DIST/msix"
rm -rf "$LAYOUT"
mkdir -p "$LAYOUT/Assets"
cp -r "$STAGE"/. "$LAYOUT/"

# Tiles and icons from the app icon (exact sizes, no scale variants: there is no resources.pri).
svg="$ROOT/src-tauri/icons/app-icon.svg"
icon() { rsvg-convert -w "$2" -h "$2" "$svg" -o "$LAYOUT/Assets/$1"; }
icon Square44x44Logo.png 44
icon Square150x150Logo.png 150
icon StoreLogo.png 50
# The wide tile: the icon in the middle of a transparent 310x150 tile.
rsvg-convert -w 140 -h 140 --page-width 310 --page-height 150 --left 85 --top 5 \
    "$svg" -o "$LAYOUT/Assets/Wide310x150Logo.png" 2>/dev/null || {
    # Older rsvg-convert without page options: a square icon on a wide canvas via ImageMagick.
    need convert
    rsvg-convert -w 140 -h 140 "$svg" -o "$LAYOUT/Assets/wide-icon.png"
    convert -size 310x150 xc:none "$LAYOUT/Assets/wide-icon.png" -gravity center -composite \
        "$LAYOUT/Assets/Wide310x150Logo.png"
    rm "$LAYOUT/Assets/wide-icon.png"
}

xml() { sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g' -e 's/"/\&quot;/g' <<<"$1"; }
sedrepl() { sed -e 's/[\/&|]/\\&/g' <<<"$1"; }
sed -e "s|\[\[identity\]\]|$(sedrepl "$(xml "$MSIX_IDENTITY")")|" \
    -e "s|\[\[publisher\]\]|$(sedrepl "$(xml "$MSIX_PUBLISHER")")|" \
    -e "s|\[\[publisher_name\]\]|$(sedrepl "$(xml "$MSIX_PUBLISHER_NAME")")|" \
    -e "s|\[\[version\]\]|$numeric|" \
    "$HERE/AppxManifest.xml" >"$LAYOUT/AppxManifest.xml"

OUT="$DIST/freex-desktop-$VERSION-x64.msix"
rm -f "$OUT"
"$MAKEMSIX" pack -d "$LAYOUT" -p "$OUT"
echo "msix: $OUT ($MSIX_IDENTITY $numeric, publisher $MSIX_PUBLISHER)"
