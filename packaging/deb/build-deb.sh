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
# Builds the Debian package (freex-desktop_<version>_amd64.deb):
#   build-deb.sh <package version> <freex-desktop binary> <output directory>
#
# The package version may carry a build suffix (1.1.1+202611142030.gitabc1234, see
# .github/workflows/build.yml); the AppStream data names the plain app version. The minimum libc6 version is
# taken from the binary itself, so apt refuses the package on systems whose glibc is too old
# instead of the app failing to start. The APT repository is meant for Debian 12+ and Ubuntu
# 22.04+, so the build machine should not have a newer glibc than 2.35.
#
# The package installs the program to /usr/bin, the menu entry, the icons (hicolor theme) and
# the AppStream data. mpv and WebKitGTK come from the distribution (Depends). The icon caches
# and the menu are updated by the triggers of the icon theme and desktop-file-utils, so the
# package needs no maintainer scripts. Needs dpkg-deb (package dpkg), gzip and rsvg-convert
# (librsvg2-bin) and objdump (binutils); no root.

set -euo pipefail

VERSION="$1"
BINARY="$2"
OUT="$3"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
PKG=freex-desktop
ICON="$ROOT/src-tauri/icons/app-icon.svg"

for tool in dpkg-deb gzip rsvg-convert objdump; do
    command -v "$tool" >/dev/null || { echo "build-deb: $tool is missing" >&2; exit 1; }
done
[ -f "$BINARY" ] || { echo "build-deb: $BINARY not found (build the app first)" >&2; exit 1; }

APP_VERSION="${VERSION%%+*}"
GLIBC="$(objdump -T "$BINARY" | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/GLIBC_//' | sort -uV | tail -n 1)"
[ -n "$GLIBC" ] || { echo "build-deb: could not read the glibc version $BINARY needs" >&2; exit 1; }
if [ "$(printf '%s\n' "$GLIBC" 2.35 | sort -V | tail -n 1)" != 2.35 ]; then
    echo "build-deb: warning: the app needs glibc $GLIBC; older systems cannot install it (Ubuntu 22.04 has 2.35, Debian 12 2.36)." >&2
    echo "build-deb: build on Debian 12 or Ubuntu 22.04 for a package that runs everywhere." >&2
fi

STAGE="$(mktemp -d)"
chmod 0755 "$STAGE"
trap 'rm -rf "$STAGE"' EXIT

install -D -m 0755 "$BINARY" "$STAGE/usr/bin/$PKG"
install -D -m 0644 "$HERE/$PKG.desktop" "$STAGE/usr/share/applications/$PKG.desktop"
mkdir -p "$STAGE/usr/share/metainfo"
# The release date is the build date (or SOURCE_DATE_EPOCH for reproducible builds).
DATE="$(date -u -d "@${SOURCE_DATE_EPOCH:-$(date +%s)}" +%Y-%m-%d)"
sed -e "s/\[\[version\]\]/$APP_VERSION/" -e "s/\[\[date\]\]/$DATE/" "$HERE/de.mirranet.freex.desktop.metainfo.xml" \
    > "$STAGE/usr/share/metainfo/de.mirranet.freex.desktop.metainfo.xml"
chmod 0644 "$STAGE/usr/share/metainfo/de.mirranet.freex.desktop.metainfo.xml"
for size in 16 24 32 48 64 128 256 512; do
    dir="$STAGE/usr/share/icons/hicolor/${size}x${size}/apps"
    mkdir -p "$dir"
    rsvg-convert -w "$size" -h "$size" "$ICON" -o "$dir/$PKG.png"
    chmod 0644 "$dir/$PKG.png"
done
install -D -m 0644 "$ICON" "$STAGE/usr/share/icons/hicolor/scalable/apps/$PKG.svg"
install -D -m 0644 "$ROOT/LICENSE" "$STAGE/usr/share/doc/$PKG/copyright"
gzip -9 -n -c "$ROOT/README.md" > "$STAGE/usr/share/doc/$PKG/README.md.gz"
chmod 0644 "$STAGE/usr/share/doc/$PKG/README.md.gz"

mkdir -p "$STAGE/DEBIAN"
sed -e "s/\[\[version\]\]/$VERSION/" -e "s/\[\[glibc\]\]/$GLIBC/" "$HERE/control/control" > "$STAGE/DEBIAN/control"
size=$(du -sk --exclude=DEBIAN "$STAGE" | cut -f1)
echo "Installed-Size: $size" >> "$STAGE/DEBIAN/control"

mkdir -p "$OUT"
DEB="$OUT/${PKG}_${VERSION}_amd64.deb"
dpkg-deb --root-owner-group --build "$STAGE" "$DEB" >/dev/null
echo "build-deb: $DEB (needs glibc $GLIBC)"
