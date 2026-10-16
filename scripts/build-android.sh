#!/bin/sh
# Builds tally's core for the Android app: libtally.so (the C interface and the JNI bridge,
# src/ffi.rs and src/jni.rs) for the emulator (x86_64) and for phones (arm64, and the 32-bit ones such as the Redmi 9A), into
# apps/android/app/src/main/jniLibs/<abi>/, where Gradle packs it into the app. Needs the NDK
# (Android Studio: SDK Manager › SDK Tools › NDK) and the Rust targets:
#   rustup target add x86_64-linux-android aarch64-linux-android armv7-linux-androideabi
#   scripts/build-android.sh
# Run it again after changing the Rust code, then Run in Android Studio.
#
# It also ships the online data as valid now, in app/src/main/assets/ (not in git): the exchange
# rates (ExchangeRate-API's answer, refreshed after 12 hours) and the city archive (refreshed
# after 30 days); the app installs them at its first start, so currencies and distances between
# any cities work before its first download. Offline, the ones already there are shipped.
set -eu
cd "$(dirname "$0")/.."
sdk=${ANDROID_HOME:-$HOME/Android/Sdk}
ndk=${ANDROID_NDK_HOME:-$(ls -d "$sdk"/ndk/* | sort -V | tail -1)}
bin=$(ls -d "$ndk"/toolchains/llvm/prebuilt/*/bin | head -1)
# The oldest Android the app runs on (app/build.gradle.kts: minSdk).
api=26
# Rust's target, the NDK's clang for it (the same name, but for 32-bit ARM), Android's ABI.
for triple in x86_64-linux-android:x86_64-linux-android:x86_64 \
    aarch64-linux-android:aarch64-linux-android:arm64-v8a \
    armv7-linux-androideabi:armv7a-linux-androideabi:armeabi-v7a; do
    target=${triple%%:*}
    abi=${triple##*:}
    clang=${triple#*:}
    clang=${clang%%:*}
    # Cargo links with the NDK's clang for that target and Android version.
    linker_variable=CARGO_TARGET_$(echo "$target" | tr 'a-z-' 'A-Z_')_LINKER
    env "$linker_variable=$bin/$clang$api-clang" \
        cargo rustc --release --lib --no-default-features --features ffi --target "$target" --crate-type cdylib
    mkdir -p "apps/android/app/src/main/jniLibs/$abi"
    cp "target/$target/release/libtally.so" "apps/android/app/src/main/jniLibs/$abi/"
    echo "apps/android/app/src/main/jniLibs/$abi/libtally.so"
done

# The online data, as valid now (a download that fails keeps the copy there, if any).
assets=apps/android/app/src/main/assets
mkdir -p "$assets"
fresh() { [ -s "$1" ] && [ -n "$(find "$1" -mmin -"$2" 2>/dev/null)" ]; }
if ! fresh "$assets/rates.json" 720; then
    curl -sfL --max-time 30 https://open.er-api.com/v6/latest/USD -o "$assets/rates.json.new" \
        && mv "$assets/rates.json.new" "$assets/rates.json" || echo "rates: not downloaded (offline?)" >&2
fi
if ! fresh "$assets/cities.json" 43200; then
    release=https://github.com/lucapf/tally/releases/download/cities
    # The plain archive; while the release still has only the compressed one, that one unpacked.
    { curl -sfL --max-time 600 "$release/cities.json" -o "$assets/cities.json.new" \
        || curl -sfL --max-time 600 "$release/cities.json.gz" | gzip -dc > "$assets/cities.json.new"; } \
        && [ -s "$assets/cities.json.new" ] && mv "$assets/cities.json.new" "$assets/cities.json" \
        || { rm -f "$assets/cities.json.new"; echo "cities: not downloaded (offline?)" >&2; }
fi
ls -la "$assets"
