#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if [ "${1:-}" != "--no-build" ]; then cargo build --locked --release; fi
DEST="${CCBRIDGE_DIST_DIR:-dist}"
APP="$DEST/CCBridge.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp README.md LICENSE "$APP/Contents/Resources/"
cp SHADCN-EGUI-LICENSE "$APP/Contents/Resources/"
cp -R licenses "$APP/Contents/Resources/"
for arch in x86_64 aarch64; do
  if [ -f "dist/ccbridge-linux-$arch" ]; then cp "dist/ccbridge-linux-$arch" "$APP/Contents/Resources/"; fi
done
if [ -f dist/ccbridge-windows-x86_64.exe ]; then cp dist/ccbridge-windows-x86_64.exe "$APP/Contents/Resources/"; fi
cp "${CARGO_TARGET_DIR:-target}/release/ccbridge" "$APP/Contents/MacOS/ccbridge"
VERSION=$(awk -F '"' '/^version = / {print $2; exit}' Cargo.toml)
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>ccbridge</string>
<key>CFBundleIdentifier</key><string>dev.ccbridge.desktop</string>
<key>CFBundleName</key><string>CCBridge</string>
<key>CFBundleDisplayName</key><string>CCBridge</string>
<key>CFBundleVersion</key><string>$VERSION</string>
<key>CFBundleShortVersionString</key><string>$VERSION</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
codesign --force --sign - "$APP"
ditto -c -k --sequesterRsrc --keepParent "$APP" "$DEST/CCBridge-macos-arm64.zip"
printf '\nBuilt %s\n' "$APP"
