#!/bin/sh
set -eu

APP="Sparrow.app"
ROOT="packaging/macos/build"
APP_DIR="$ROOT/$APP"
BIN="${1:-target/release/sparrow}"
DMG="${2:-Sparrow.dmg}"

rm -rf "$ROOT" "$DMG"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"
cp "$BIN" "$APP_DIR/Contents/MacOS/sparrow"
chmod +x "$APP_DIR/Contents/MacOS/sparrow"
cat > "$APP_DIR/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>sparrow</string>
  <key>CFBundleIdentifier</key><string>com.ucav.sparrow</string>
  <key>CFBundleName</key><string>Sparrow</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.9.0</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
</dict>
</plist>
PLIST
hdiutil create -volname "Sparrow" -srcfolder "$ROOT" -ov -format UDZO "$DMG"
