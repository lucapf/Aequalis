#!/bin/bash
# Builds Aequalis.app (Apple Silicon and Intel in one) and its zip, on macOS:
#
#   apps/macos/build.sh <version>
#
# The app is Swift (apps/macos/Sources) over the Rust core's C interface (apps/macos/core, a static
# library). The command line, `aequalis`, goes inside it (Contents/Resources) for the Homebrew cask.
# Written to dist/: Aequalis.app and aequalis-<version>-macos.zip with its .sha256.
set -euo pipefail

version=${1:?usage: build.sh <version>}
cd "$(dirname "$0")/../.."

export MACOSX_DEPLOYMENT_TARGET=11.0
targets=(aarch64-apple-darwin x86_64-apple-darwin)
rustup target add "${targets[@]}"

work=target/macos-app
rm -rf "$work" dist/Aequalis.app
mkdir -p "$work" dist

for target in "${targets[@]}"; do
  arch=${target%%-*}
  cargo build --release --target "$target"
  cargo build --release --manifest-path apps/macos/core/Cargo.toml --target "$target"
  swiftc -O -swift-version 5 -target "$arch-apple-macos11.0" \
    -import-objc-header include/aequalis.h \
    apps/macos/Sources/*.swift \
    "apps/macos/core/target/$target/release/libaequalis_core.a" \
    -lresolv -liconv -framework Carbon -framework ServiceManagement \
    -o "$work/Aequalis-$arch"
done

app=dist/Aequalis.app
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
lipo -create "$work"/Aequalis-aarch64 "$work"/Aequalis-x86_64 -output "$app/Contents/MacOS/Aequalis"
lipo -create target/aarch64-apple-darwin/release/aequalis target/x86_64-apple-darwin/release/aequalis \
  -output "$app/Contents/Resources/aequalis"
sed "s/@VERSION@/$version/g" apps/macos/Info.plist > "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"

# Ad-hoc signed: no developer account behind it, but Apple Silicon refuses code with no signature.
codesign --force --sign - "$app/Contents/Resources/aequalis"
codesign --force --sign - "$app"
codesign --verify --strict "$app"

# The checks the release stands on.
file "$app/Contents/MacOS/Aequalis" "$app/Contents/Resources/aequalis"
test "$("$app/Contents/Resources/aequalis" --version)" = "aequalis $version"
test "$("$app/Contents/MacOS/Aequalis" --selftest 10 km mi)" = "6.2137 mi"

zip=aequalis-$version-macos.zip
rm -f "dist/$zip" "dist/$zip.sha256"
ditto -c -k --keepParent "$app" "dist/$zip"
(cd dist && shasum -a 256 "$zip" > "$zip.sha256")
ls -l dist
