#!/usr/bin/env bash
# Installs FileViewer from the extracted portable bundle into a user-local
# prefix. No root, no package manager. Run from inside the bundle directory.
#
#   ./install.sh [--prefix DIR] [--no-desktop]
#
# Default prefix: $HOME/.local
set -euo pipefail

BUNDLE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${HOME}/.local"
MAKE_DESKTOP=1

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix) PREFIX="$2"; shift 2 ;;
        --no-desktop) MAKE_DESKTOP=0; shift ;;
        -h|--help)
            sed -n '2,9p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
done

BINDIR="$PREFIX/bin"
APPDIR="$PREFIX/share/applications"
ICONDIR="$PREFIX/share/icons/hicolor"

echo "==> Installing FileViewer into $PREFIX"

mkdir -p "$BINDIR"
install -m 0755 "$BUNDLE/file-viewer" "$BINDIR/file-viewer"
echo "    $BINDIR/file-viewer"

if [ "$MAKE_DESKTOP" -eq 1 ]; then
    mkdir -p "$APPDIR"
    # Point Exec at the installed absolute path so the launcher works from
    # anywhere, and use the largest bundled icon for the menu entry.
    sed "s|^Exec=.*|Exec=$BINDIR/file-viewer %f|" "$BUNDLE/file-viewer.desktop" \
        > "$APPDIR/file-viewer.desktop"
    chmod 0644 "$APPDIR/file-viewer.desktop"
    echo "    $APPDIR/file-viewer.desktop"

    BEST=256
    for size in 16 32 48 64 128 256 512; do
        if [ -f "$BUNDLE/icons/$size.png" ]; then
            mkdir -p "$ICONDIR/${size}x${size}/apps"
            install -m 0644 "$BUNDLE/icons/$size.png" "$ICONDIR/${size}x${size}/apps/file-viewer.png"
            BEST="$size"
        fi
    done
    echo "    $ICONDIR/${BEST}x${BEST}/apps/file-viewer.png"
fi

case ":$PATH:" in
    *":$BINDIR:"*) ;;
    *) echo "==> Add to PATH: export PATH=\"$BINDIR:\$PATH\"" ;;
esac

echo "==> Refreshing desktop caches"
command -v update-desktop-database >/dev/null 2>&1 && \
    update-desktop-database -q "$APPDIR" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && \
    gtk-update-icon-cache -q -f -t "$ICONDIR" 2>/dev/null || true

echo "==> Done. Run: file-viewer <path>"
