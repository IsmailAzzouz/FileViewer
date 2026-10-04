#!/usr/bin/env bash
# Builds the Linux release artifacts for FileViewer.
#
#   packaging/linux/build_release.sh [version]
#
# Produces, under dist/:
#   file-viewer_<version>_amd64.deb              dpkg installer
#   file-viewer_<version>_linux-x86_64.tar.xz    portable bundle
#   file-viewer_<version>_amd64.AppImage         self-contained single file
#   SHA256SUMS                                  checksums for all of the above
#
# Requires: cargo, dpkg-deb, xorriso, and (for the AppImage) the appimagetool
# runtime from https://github.com/continuousintegration/releases/releases.
# AppImage is skipped with a warning when appimagetool is unavailable.
set -euo pipefail

VERSION="${1:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)}"
ARCH="amd64"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DIST="$ROOT/dist"
STAGE="$ROOT/target/packaging"

cd "$ROOT"

echo "==> Building release binary (version $VERSION)"
# LIBRARY_PATH, not RUSTFLAGS: keeps the crate fingerprint stable and avoids a
# full rebuild of the dependency tree.
LIBRARY_PATH="${LIBRARY_PATH:-${HOME}/.local/sysroot/usr/lib/x86_64-linux-gnu}" \
    cargo build --release --locked

BIN="$ROOT/target/release/file-viewer"
test -x "$BIN"

echo "==> Staging package tree"
rm -rf "$STAGE"
mkdir -p "$STAGE/deb/DEBIAN" \
         "$STAGE/deb/usr/bin" \
         "$STAGE/deb/usr/share/applications" \
         "$STAGE/deb/usr/share/doc/file-viewer" \
         "$STAGE/deb/usr/share/icons/hicolor/16x16/apps" \
         "$STAGE/deb/usr/share/icons/hicolor/32x32/apps" \
         "$STAGE/deb/usr/share/icons/hicolor/48x48/apps" \
         "$STAGE/deb/usr/share/icons/hicolor/64x64/apps" \
         "$STAGE/deb/usr/share/icons/hicolor/128x128/apps" \
         "$STAGE/deb/usr/share/icons/hicolor/256x256/apps" \
         "$STAGE/deb/usr/share/icons/hicolor/512x512/apps" \
         "$STAGE/bundle"

install -m 0755 "$BIN" "$STAGE/deb/usr/bin/file-viewer"
install -m 0644 packaging/linux/file-viewer.desktop \
    "$STAGE/deb/usr/share/applications/file-viewer.desktop"
install -m 0644 LICENSE "$STAGE/deb/usr/share/doc/file-viewer/copyright"
install -m 0644 README.md "$STAGE/deb/usr/share/doc/file-viewer/README.md"

for size in 16 32 48 64 128 256 512; do
    src="$ROOT/assets/icons/$size.png"
    if [ ! -f "$src" ]; then
        python3 packaging/linux/make_icon.py "$src" "$size"
    fi
    install -m 0644 "$src" "$STAGE/deb/usr/share/icons/hicolor/${size}x${size}/apps/file-viewer.png"
done

INSTALLED_SIZE="$(du -ks "$STAGE/deb/usr" | cut -f1)"

cat > "$STAGE/deb/DEBIAN/control" <<EOF
Package: file-viewer
Version: $VERSION
Section: devel
Priority: optional
Architecture: $ARCH
Maintainer: Ismail Azzouz <124143187+IsmailAzzouz@users.noreply.github.com>
Homepage: https://github.com/IsmailAzzouz/FileViewer
Installed-Size: $INSTALLED_SIZE
Depends: libc6 (>= 2.31), libgcc-s1, libxcb1, libxkbcommon0, libxkbcommon-x11-0
Description: GPUI document viewer with a synchronized tree
 FileViewer opens JSON, JSONC, JSONL, TOML, and YAML with an editor, a
 hierarchical tree view, and two-way cursor synchronization between them.
 .
 Parsers are hand-written so every node carries an exact source span, which
 drives syntax highlighting, validation diagnostics with jump-to-error, and
 click-to-locate in the tree. Formatting and minification never reserialize the
 value tree, so comments and blank lines survive a round trip.
EOF

cat > "$STAGE/deb/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -f -t /usr/share/icons/hicolor || true
fi
exit 0
EOF
chmod 0755 "$STAGE/deb/DEBIAN/postinst"

cat > "$STAGE/deb/DEBIAN/postrm" <<'EOF'
#!/bin/sh
set -e
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -f -t /usr/share/icons/hicolor || true
fi
exit 0
EOF
chmod 0755 "$STAGE/deb/DEBIAN/postrm"

echo "==> Building .deb"
mkdir -p "$DIST"
DEB="$DIST/file-viewer_${VERSION}_${ARCH}.deb"
rm -f "$DEB"
fakeroot dpkg-deb --root-owner-group --build "$STAGE/deb" "$DEB" >/dev/null
echo "    $DEB"

echo "==> Building portable bundle"
BUNDLE="$STAGE/bundle"
install -m 0755 "$BIN" "$BUNDLE/file-viewer"
install -m 0644 LICENSE "$BUNDLE/LICENSE"
install -m 0644 README.md "$BUNDLE/README.md"
install -m 0644 packaging/linux/file-viewer.desktop "$BUNDLE/file-viewer.desktop"
mkdir -p "$BUNDLE/icons"
for size in 16 32 48 64 128 256 512; do
    install -m 0644 "$ROOT/assets/icons/$size.png" "$BUNDLE/icons/$size.png"
done

install -m 0755 packaging/linux/install.sh "$BUNDLE/install.sh"
install -m 0755 packaging/linux/uninstall.sh "$BUNDLE/uninstall.sh"

TARBALL="$DIST/file-viewer_${VERSION}_linux-x86_64.tar.xz"
rm -f "$TARBALL"
tar -C "$STAGE" -cJf "$TARBALL" --owner=0 --group=0 bundle
echo "    $TARBALL"

APPIMAGE="$DIST/file-viewer_${VERSION}_${ARCH}.AppImage"
if command -v appimagetool >/dev/null 2>&1; then
    echo "==> Building AppImage"
    APPR="$STAGE/appimage"
    rm -rf "$APPR"
    mkdir -p "$APPR/usr/bin" "$APPR/usr/share/applications" "$APPR/usr/share/icons/hicolor/256x256/apps"
    install -m 0755 "$BIN" "$APPR/usr/bin/file-viewer"
    install -m 0644 packaging/linux/file-viewer.desktop "$APPR/usr/share/applications/file-viewer.desktop"
    install -m 0644 "$ROOT/assets/icons/256.png" "$APPR/usr/share/icons/hicolor/256x256/apps/file-viewer.png"
    sed -e 's|^Exec=.*|Exec=AppRun|' -e 's|^Icon=.*|Icon=file-viewer|' \
        packaging/linux/file-viewer.desktop > "$APPR/file-viewer.desktop"
    cat > "$APPR/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/file-viewer" "$@"
EOF
    chmod 0755 "$APPR/AppRun"
    rm -f "$APPIMAGE"
    APPIMAGE_EXTRACT_AND_RUN=1 appimagetool "$APPR" "$APPIMAGE" >/dev/null
    echo "    $APPIMAGE"
else
    echo "==> Skipping AppImage (appimagetool not installed)"
fi

echo "==> Checksums"
cd "$DIST"
rm -f SHA256SUMS
sha256sum ./*.deb ./*.tar.xz >> SHA256SUMS
if [ -f "$APPIMAGE" ]; then
    sha256sum ./*.AppImage >> SHA256SUMS
fi
cat SHA256SUMS

echo "==> Done. Artifacts in $DIST"
ls -lh "$DIST"
