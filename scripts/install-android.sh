#!/bin/sh
# Builds Tally's fast build (release: optimized by R8, with its Baseline Profile) and installs it on
# a device, over the debug build or an older one: the app's data is kept. As the Play Store does,
# Android compiles it at install with the profile (the .dm file next to the APK, the one for the
# device's Android version), so even the first start doesn't run in the interpreter.
#   scripts/install-android.sh <serial>        (the serials: adb devices)
# After changing the Rust code, run scripts/build-android.sh first: it builds libtally.so.
# Android Studio's Run installs the debug build instead, which is slow on purpose (a debugger can
# stop it anywhere); scripts/android-startup.sh measures either.
set -eu
cd "$(dirname "$0")/.."
serial=${1:?usage: scripts/install-android.sh <serial>}
adb="${ANDROID_HOME:-$HOME/Android/Sdk}/platform-tools/adb"
# Android Studio's Java, unless JAVA_HOME says otherwise.
export JAVA_HOME="${JAVA_HOME:-$HOME/.local/share/android-studio/jbr}"
(cd apps/android && ./gradlew --console=plain -q assembleRelease)

out=apps/android/app/build/outputs/apk/release
api=$("$adb" -s "$serial" shell getprop ro.build.version.sdk | tr -d '\r')
# The profile for that Android version, as output-metadata.json lists them (by minApi and maxApi).
# Before Android 9 there is none: the app installs its profile at its first start (profileinstaller),
# and Android compiles it when the phone is idle.
dm=$(awk -v api="$api" '
    /"minApi"/ { gsub(/[^0-9]/, ""); min = $0 }
    /"maxApi"/ { gsub(/[^0-9]/, ""); max = $0 }
    /\.dm"/ { gsub(/[ ",]/, ""); if (api + 0 >= min + 0 && api + 0 <= max + 0) print }
' "$out/output-metadata.json")
if [ -n "$dm" ]; then
    "$adb" -s "$serial" install-multiple -r "$out/app-release.apk" "$out/$dm"
else
    "$adb" -s "$serial" install -r "$out/app-release.apk"
fi
# How Android compiled it: `speed-profile` with the profile, `verify` without.
"$adb" -s "$serial" shell dumpsys package dexopt | grep -A2 '\[io.lucapf.tally\]' | grep -o 'status=[^]]*' | head -1
