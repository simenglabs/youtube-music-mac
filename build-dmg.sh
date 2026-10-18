#!/bin/sh
# Build a universal (Apple Silicon + Intel) "YT Music Bar.app" and pack it into
# dist/YT-Music-Bar-<version>.dmg with an Applications shortcut and INSTALL.md.
set -eu
cd "$(dirname "$0")"

NAME="YT Music Bar"
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
APP="dist/$NAME.app"
DMG="dist/YT-Music-Bar-$VERSION.dmg"

for target in aarch64-apple-darwin x86_64-apple-darwin; do
  cargo build --release --target "$target"
done

rm -rf dist
mkdir -p "$APP/Contents/MacOS"
lipo -create -output "$APP/Contents/MacOS/yt-music" \
  target/aarch64-apple-darwin/release/yt-music \
  target/x86_64-apple-darwin/release/yt-music

# LSUIElement: menubar only, no Dock icon.
# NSAppleEventsUsageDescription: text shown in the "control Vivaldi" permission prompt.
cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$NAME</string>
  <key>CFBundleDisplayName</key><string>$NAME</string>
  <key>CFBundleIdentifier</key><string>com.menglabs.yt-music</string>
  <key>CFBundleExecutable</key><string>yt-music</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSAppleEventsUsageDescription</key><string>$NAME controls the YouTube Music tab in Vivaldi (play, pause, skip, volume).</string>
</dict>
</plist>
EOF

# Sign with SIGN_IDENTITY, else the best certificate in the keychain (Developer ID,
# then Apple Development), else ad-hoc. A real certificate keeps the app's identity
# stable across builds, so the "control Vivaldi" permission survives updates.
# ponytail: only Developer ID + notarization removes the Gatekeeper "Open Anyway"
# step; that needs a paid Apple Developer Program account.
find_identity() {
  security find-identity -v -p codesigning | awk -F'"' -v kind="$1" 'index($2, kind) == 1 { print $2; exit }'
}
IDENTITY=${SIGN_IDENTITY:-$(find_identity "Developer ID Application")}
IDENTITY=${IDENTITY:-$(find_identity "Apple Development")}
IDENTITY=${IDENTITY:--}
echo "Signing with: $IDENTITY"
codesign --force --deep --sign "$IDENTITY" "$APP"

mkdir dist/dmg
cp -R "$APP" dist/dmg/
ln -s /Applications dist/dmg/Applications
cp INSTALL.md "dist/dmg/Panduan Install.md"
hdiutil create -volname "$NAME" -srcfolder dist/dmg -ov -format UDZO "$DMG" >/dev/null
rm -rf dist/dmg

echo "Built $DMG"
