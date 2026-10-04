#!/usr/bin/env bash
# Removes a FileViewer install created by install.sh.
#
#   ./uninstall.sh [--prefix DIR]
#
# Default prefix: $HOME/.local. Only touches this prefix, never the system one.
set -euo pipefail

PREFIX="${HOME}/.local"

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix) PREFIX="$2"; shift 2 ;;
        -h|--help)
            sed -n '2,7p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
done

APPDIR="$PREFIX/share/applications"
ICONDIR="$PREFIX/share/icons/hicolor"

echo "==> Removing FileViewer from $PREFIX"
rm -fv "$PREFIX/bin/file-viewer"
rm -fv "$APPDIR/file-viewer.desktop"
for size in 16 32 48 64 128 256 512; do
    rm -fv "$ICONDIR/${size}x${size}/apps/file-viewer.png"
done

echo "==> Refreshing desktop caches"
command -v update-desktop-database >/dev/null 2>&1 && \
    update-desktop-database -q "$APPDIR" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && \
    gtk-update-icon-cache -q -f -t "$ICONDIR" 2>/dev/null || true

echo "==> Done."
