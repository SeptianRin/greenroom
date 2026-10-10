#!/usr/bin/env bash
# Build Greenroom.app + Greenroom.dmg into dist/. macOS only.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
BIN=target/release/greenroom
APP=dist/Greenroom.app

cargo build --release
[ -x "$BIN" ] || { echo "build produced no binary"; exit 1; }

rm -rf dist
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$BIN" "$APP/Contents/MacOS/greenroom"
if [ -f assets/AppIcon.icns ]; then
  cp assets/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
fi

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Greenroom</string>
  <key>CFBundleDisplayName</key><string>Greenroom</string>
  <key>CFBundleIdentifier</key><string>io.github.septianrin.greenroom</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleExecutable</key><string>greenroom</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$APP" 2>/dev/null || echo "note: ad-hoc codesign skipped"

STAGE=dist/dmg-stage
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "Greenroom" -srcfolder "$STAGE" -ov -format UDZO -quiet dist/Greenroom.dmg
rm -rf "$STAGE"

echo "OK: $APP"
echo "OK: dist/Greenroom.dmg"
