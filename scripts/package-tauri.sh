#!/bin/sh
# Ad-hoc macOS signing + independently signed, architecture-specific updater artifact.
set -eu
cd "$(dirname "$0")/.."
: "${TAURI_SIGNING_PRIVATE_KEY:?Set the updater signing key path or secret}"
test -f dist/ccbridge-linux-x86_64 || { echo 'Missing bundled Linux Runner' >&2; exit 1; }
TARGET_DIR="${CARGO_TARGET_DIR:-src-tauri/target}"
case "${CCBRIDGE_TARGET:-}" in
  x86_64-apple-darwin) ARCH=x86_64; BUILD_SUBDIR=x86_64-apple-darwin/ ;;
  aarch64-apple-darwin) ARCH=arm64; BUILD_SUBDIR=aarch64-apple-darwin/ ;;
  '') BUILD_SUBDIR=; case "$(uname -m)" in arm64) ARCH=arm64;; x86_64) ARCH=x86_64;; *) echo 'Unsupported Mac architecture' >&2; exit 1;; esac ;;
  *) echo 'CCBRIDGE_TARGET must be x86_64-apple-darwin or aarch64-apple-darwin' >&2; exit 1 ;;
esac
pnpm --dir ui install --frozen-lockfile
if [ -n "${CCBRIDGE_TARGET:-}" ]; then
  (cd src-tauri && ../ui/node_modules/.bin/tauri build --bundles app --target "$CCBRIDGE_TARGET")
else
  (cd src-tauri && ../ui/node_modules/.bin/tauri build --bundles app)
fi
APP="$TARGET_DIR/${BUILD_SUBDIR}release/bundle/macos/CCBridge.app"
codesign --verify --deep --strict "$APP"
test "$(lipo -archs "$APP/Contents/MacOS/ccbridge")" = "$ARCH"
DEST="${CCBRIDGE_DIST_DIR:-dist-tauri}"
mkdir -p "$DEST"
ditto "$APP" "$DEST/CCBridge.app"
ditto -c -k --sequesterRsrc --keepParent "$APP" "$DEST/CCBridge-macos-$ARCH.zip"
# Preserve the original ARM asset URL for already-installed clients.
ASSET=CCBridge.app.tar.gz
[ "$ARCH" = arm64 ] || ASSET="CCBridge-macos-$ARCH.app.tar.gz"
cp "$TARGET_DIR/${BUILD_SUBDIR}release/bundle/macos/CCBridge.app.tar.gz" "$DEST/$ASSET"
cp "$TARGET_DIR/${BUILD_SUBDIR}release/bundle/macos/CCBridge.app.tar.gz.sig" "$DEST/$ASSET.sig"
python3 scripts/update-manifest.py "$DEST" "$ARCH" "$ASSET"

# Standard first-install experience; updater archives retain their existing URLs.
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT HUP INT TERM
ditto "$APP" "$STAGE/CCBridge.app"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname CCBridge -srcfolder "$STAGE" -format UDZO -ov "$DEST/CCBridge-macos-$ARCH.dmg"
