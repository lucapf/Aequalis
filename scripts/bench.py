#!/usr/bin/env python3
"""Measures tally: how long requests take and how much memory they use (docs/REVIEW.md).

    scripts/bench.py [--bin target/release/tally] [--runs 40] [--archive cities.json] [--json out.json]

Each request runs in a fresh process, `--runs` times; the median wall time is reported, and the
process's peak memory (VmHWM: from `--json` for requests, else watched in /proc). With `--archive` (the JSON written by
scripts/build-cities.py), the city archive is installed from it (`tally --sync` from a file://
address, the other sources behind a proxy nothing answers; timed and measured) and requests
between any cities are measured too.
Every run uses its own config directory: offline, distances in km, nothing refreshed by itself.
Standard library only.
"""

import argparse
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time

OFFLINE_REQUESTS = [
    ["--version"],
    ["10", "km", "mi"],
    ["12+4/2"],
    ["sqrt", "16"],
    ["now", "Tokyo"],
    ["10:00AM", "Paris", "to", "New", "York"],
    ["Rome", "Paris"],
    ["10:00am", "jappan", "romme"],
    ["2026/12/04", "+", "5wd"],
    ["12/04/2026", "unix"],
    ["12", "usd", "eur"],
    ["12", "dollar", "euro"],
    ["--trivia"],
]

ARCHIVE_REQUESTS = [
    ["Springfield,", "Illinois", "Chicago"],
    ["Rome", "Paris"],
    ["distance", "Sprngfield", "to", "Chicago"],
    ["Москва", "Berlin"],
]


def config(settings):
    d = tempfile.mkdtemp(prefix="tally-bench-")
    os.makedirs(os.path.join(d, "tally"))
    with open(os.path.join(d, "tally", "settings.json"), "w") as f:
        json.dump(settings, f)
    return d


def peak_kb(binary, args, env):
    """The process's peak memory (VmHWM): from `--json` for a request and for `--sync` (tally reads
    its own), else by watching /proc while it runs (once read 17.6 MB for a 6.1 MB `--sync`)."""
    if not args[0].startswith("--") or args[0] == "--sync":
        out = subprocess.run([binary, "--json"] + args, env=env, capture_output=True, text=True, stdin=subprocess.DEVNULL).stdout
        for line in out.splitlines():
            if '"peak_kb":' in line:
                return int(line.rsplit('"peak_kb":', 1)[1].rstrip("}"))
    proc = subprocess.Popen([binary] + args, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    peak = 0
    while proc.poll() is None:
        try:
            with open(f"/proc/{proc.pid}/status") as f:
                for line in f:
                    if line.startswith("VmHWM:"):
                        peak = max(peak, int(line.split()[1]))
        except OSError:
            pass
    return peak


def measure(binary, args, env, runs):
    """(median ms of `runs` runs, peak memory in KB)."""
    times = []
    for _ in range(runs):
        s = time.perf_counter()
        subprocess.run([binary] + args, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        times.append((time.perf_counter() - s) * 1000)
    return statistics.median(times), peak_kb(binary, args, env)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--bin", default="target/release/tally")
    p.add_argument("--runs", type=int, default=40)
    p.add_argument("--archive")
    p.add_argument("--json")
    a = p.parse_args()
    binary = os.path.abspath(a.bin)
    results = {"requests": [], "archive": None}

    env = dict(os.environ, LC_ALL="C", TZ="Europe/Rome")
    env["XDG_CONFIG_HOME"] = config({"offline": True, "distance.unit": "km"})
    print(f"| request | median | peak memory |\n|---|---|---|")
    for args in OFFLINE_REQUESTS:
        ms, kb = measure(binary, args, env, a.runs)
        results["requests"].append({"request": " ".join(args), "ms": round(ms, 2), "kb": kb})
        print(f"| `{' '.join(args)}` | {ms:.1f} ms | {kb / 1024:.1f} MB |")

    if a.archive:
        serve = tempfile.mkdtemp(prefix="tally-bench-archive-")
        shutil.copyfile(a.archive, os.path.join(serve, "cities.json"))
        with open(os.path.join(serve, "cities.version"), "w") as f:
            f.write("0 bench\n")
        settings = {"offline": False, "distance.unit": "km", "cities.refresh": "manual", "rates.refresh": "manual",
                    "live.refresh": "manual", "cities.url": "file://" + os.path.join(serve, "cities.json")}
        env["XDG_CONFIG_HOME"] = config(settings)
        # `--sync` downloads everything: the rates and live values fail at once (as in the tests).
        sync_env = dict(env, http_proxy="http://127.0.0.1:9", https_proxy="http://127.0.0.1:9")
        ms, kb = measure(binary, ["--sync"], sync_env, 3)
        results["archive"] = {"ms": round(ms), "kb": kb}
        print(f"| `--sync` (install the archive) | {ms:.0f} ms | {kb / 1024:.1f} MB |")
        for args in ARCHIVE_REQUESTS:
            ms, kb = measure(binary, args, env, a.runs)
            results["requests"].append({"request": " ".join(args) + " (archive)", "ms": round(ms, 2), "kb": kb})
            print(f"| `{' '.join(args)}` (archive) | {ms:.1f} ms | {kb / 1024:.1f} MB |")

    if a.json:
        with open(a.json, "w") as f:
            json.dump(results, f, indent=1)


if __name__ == "__main__":
    main()
