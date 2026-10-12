#!/bin/sh
# Builds Tally.app, the menu bar app, with the tally CLI inside it, and zips it:
#   macos/build-app.sh <version> <path to the tally CLI> [<output dir>]
# → <output dir>/Tally.app and <output dir>/Tally-<version>-macos.zip (default output dir: dist).
# On a Mac (Apple Silicon), with the Xcode command line tools. The app is signed ad hoc (no Apple
# Developer account): the first time, open it from System Settings → Privacy & Security.
set -eu
version=$1
cli=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
out=${3:-dist}
mkdir -p "$out"
out=$(cd "$out" && pwd)
cd "$(dirname "$0")"

swift build -c release --arch arm64
bin=$(swift build -c release --arch arm64 --show-bin-path)

app="$out/Tally.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin/TallyBar" "$app/Contents/MacOS/TallyBar"
cp "$cli" "$app/Contents/Resources/tally"
sed "s/__VERSION__/$version/g" Info.plist > "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"

# Inside out: the CLI, then the app.
codesign --force --sign - "$app/Contents/Resources/tally"
codesign --force --sign - "$app"
codesign --verify --verbose "$app"

rm -f "$out/Tally-$version-macos.zip"
ditto -c -k --keepParent "$app" "$out/Tally-$version-macos.zip"
echo "$out/Tally-$version-macos.zip"
