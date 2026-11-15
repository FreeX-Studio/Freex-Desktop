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
# Puts the interface of the FreeX TV apps (web/ of FreeX-TV-Apps) into ui/tv/, where the build
# bundles it for the TV mode (src-tauri/src/tv.rs). Run before cargo build:
#
#   packaging/tv-ui.sh [<FreeX-TV-Apps checkout or its web/ directory>]
#
# Without an argument it clones FreeX-TV-Apps (branch TV_APPS_REF, default main). Without ui/tv the
# app builds as well; it then has no TV mode.

set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/ui/tv"

src="${1:-}"
tmp=""
if [ -z "$src" ]; then
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    git clone -q --depth 1 --branch "${TV_APPS_REF:-main}" https://github.com/FreeX-Studio/FreeX-TV-Apps.git "$tmp/tv"
    echo "tv-ui: FreeX-TV-Apps $(git -C "$tmp/tv" log -1 --format='%h %s')"
    src="$tmp/tv"
fi
[ -f "$src/web/index.html" ] && src="$src/web"
[ -f "$src/index.html" ] && [ -f "$src/js/backends.js" ] || { echo "tv-ui: $src is not the FreeX TV interface" >&2; exit 1; }
grep -q '"freexht"' "$src/js/backends.js" || { echo "tv-ui: $src has no FreeXHT platform (FreeX-TV-Apps too old)" >&2; exit 1; }

rm -rf "$DEST"
mkdir -p "$DEST"
cp -r "$src/." "$DEST/"
echo "tv-ui: $DEST"
