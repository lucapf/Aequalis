#!/bin/sh
# Measures how long Aequalis takes to start on a device, as Android counts it: from the launch to the
# app's first frame (`am start -W`, TotalTime, in milliseconds).
#   cold: the app's process killed first, as after a reboot or when Android needed the memory;
#   hot:  from the home screen, the app still in memory.
# Each `runs` times (default 10): every time, then the median, the fastest and the slowest. The
# app's data is never touched. The device's screen must be on and unlocked.
#   scripts/android-startup.sh <serial> [runs]        (the serials: adb devices)
set -eu
serial=${1:?usage: scripts/android-startup.sh <serial> [runs]}
runs=${2:-10}
adb="${ANDROID_HOME:-$HOME/Android/Sdk}/platform-tools/adb"
app=io.lucapf.aequalis/.MainActivity
device() { "$adb" -s "$serial" shell "$@" | tr -d '\r'; }

# One launch's TotalTime; `-S`: the process killed first.
launch() { device am start "$@" -W -n "$app" | sed -n 's/^TotalTime: *//p'; }
home() { device am start -W -a android.intent.action.MAIN -c android.intent.category.HOME >/dev/null; }

# The times given, then their median (of the two middle ones, for an even count), min and max.
summary() {
    sorted=$(printf '%s\n' "$@" | sort -n)
    n=$#
    middle() { printf '%s\n' "$sorted" | sed -n "${1}p"; }
    if [ $((n % 2)) -eq 1 ]; then
        median=$(middle $(((n + 1) / 2)))
    else
        median=$((($(middle $((n / 2))) + $(middle $((n / 2 + 1)))) / 2))
    fi
    echo "  $*"
    echo "  median $median ms, fastest $(middle 1) ms, slowest $(middle "$n") ms"
}

# What is measured: the device, and the build (debug or release, how Android compiled it).
echo "$(device getprop ro.product.manufacturer) $(device getprop ro.product.model), Android $(device getprop ro.build.version.release)"
case $(device dumpsys package io.lucapf.aequalis | grep -c DEBUGGABLE) in
    0) echo "build: release" ;;
    *) echo "build: debug" ;;
esac
device dumpsys package dexopt | grep -A2 '\[io.lucapf.aequalis\]' | grep -o 'status=[^]]*' | head -1 | sed 's/^/compiled: /'

times=""
for _ in $(seq "$runs"); do
    times="$times $(launch -S)"
    # The app's start finished (its background work too) before the next one.
    sleep 3
done
echo "cold start:"
# shellcheck disable=SC2086
summary $times

times=""
for _ in $(seq "$runs"); do
    home
    sleep 2
    times="$times $(launch)"
    sleep 1
done
echo "hot start:"
# shellcheck disable=SC2086
summary $times
