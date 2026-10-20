#!/bin/sh
# Free ad-hoc macOS signing + independently signed updater artifact.
set -eu
cd "$(dirname "$0")/.."
: "${TAURI_SIGNING_PRIVATE_KEY:?Set the updater signing key path or secret}"
test -f dist/ccbridge-linux-x86_64 || { echo 'Missing bundled Linux Runner' >&2; exit 1; }
pnpm --dir ui install --frozen-lockfile
(cd src-tauri && ../ui/node_modules/.bin/tauri build --bundles app)
TARGET="${CARGO_TARGET_DIR:-src-tauri/target}"
APP="$TARGET/release/bundle/macos/CCBridge.app"
codesign --verify --deep --strict "$APP"
DEST="${CCBRIDGE_DIST_DIR:-dist-tauri}"
mkdir -p "$DEST"
ditto "$APP" "$DEST/CCBridge.app"
ditto -c -k --sequesterRsrc --keepParent "$APP" "$DEST/CCBridge-macos-arm64.zip"
cp "$TARGET/release/bundle/macos/CCBridge.app.tar.gz" "$DEST/"
cp "$TARGET/release/bundle/macos/CCBridge.app.tar.gz.sig" "$DEST/"
python3 scripts/update-manifest.py "$DEST"
