#!/bin/sh
# Builds tally's core for the iPhone app: the static library (src/ffi.rs) for devices and for the
# simulator, packed with include/ (tally.h and its module map, `import TallyFFI`) into
# ios/TallyCore.xcframework. Run on a Mac with Xcode:
#   rustup target add aarch64-apple-ios aarch64-apple-ios-sim
#   scripts/build-ios.sh
set -eu
cd "$(dirname "$0")/.."
for target in aarch64-apple-ios aarch64-apple-ios-sim; do
    cargo rustc --release --lib --no-default-features --features ffi --target "$target" --crate-type staticlib
done
rm -rf ios/TallyCore.xcframework
xcodebuild -create-xcframework \
    -library target/aarch64-apple-ios/release/libtally.a -headers include \
    -library target/aarch64-apple-ios-sim/release/libtally.a -headers include \
    -output ios/TallyCore.xcframework
echo "ios/TallyCore.xcframework"
