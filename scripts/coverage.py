#!/usr/bin/env python3
"""Line coverage of the test suite: unit tests and integration tests (and the tally binary the
integration tests run), with rustc's instrumentation (`-C instrument-coverage`) and the rustup
component `llvm-tools` (`rustup component add llvm-tools`).

    scripts/coverage.py [--json out.json] [--uncovered src/file.rs ...]

Builds into target/coverage (separate from the usual builds), runs `cargo test`, merges the
profiles and prints a table per source file: lines, lines covered, functions covered.
Standard library only.
"""

import argparse
import glob
import json
import os
import shutil
import subprocess
import sys


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--json")
    p.add_argument("--uncovered", nargs="*", default=[], help="list the lines no test runs, for these files")
    a = p.parse_args()
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    os.chdir(root)
    sysroot = subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip()
    host = next(l.split(": ")[1] for l in subprocess.check_output(["rustc", "-vV"], text=True).splitlines() if l.startswith("host: "))
    tools = os.path.join(sysroot, "lib", "rustlib", host, "bin")
    if not os.path.exists(os.path.join(tools, "llvm-cov")):
        sys.exit("llvm-tools missing: rustup component add llvm-tools")
    out = os.path.join(root, "target", "coverage")
    prof = os.path.join(out, "prof")
    shutil.rmtree(prof, ignore_errors=True)
    os.makedirs(prof)
    env = dict(os.environ, RUSTFLAGS="-C instrument-coverage", CARGO_TARGET_DIR=out,
               LLVM_PROFILE_FILE=os.path.join(prof, "%p-%m.profraw"))

    # The executables: every test binary, and the tally binary they run.
    built = subprocess.run(["cargo", "test", "--no-run", "--message-format=json"], env=env, capture_output=True, text=True)
    objects = []
    for line in built.stdout.splitlines():
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if msg.get("reason") == "compiler-artifact" and msg.get("executable"):
            objects.append(msg["executable"])
    tests = subprocess.run(["cargo", "test", "--quiet"], env=env, capture_output=True, text=True)
    if tests.returncode != 0:
        sys.stderr.write(tests.stdout[-3000:] + tests.stderr[-3000:])
        sys.exit("the tests failed: coverage not measured")

    data = os.path.join(out, "tally.profdata")
    subprocess.check_call([os.path.join(tools, "llvm-profdata"), "merge", "-sparse", *glob.glob(os.path.join(prof, "*.profraw")), "-o", data])
    args = [os.path.join(tools, "llvm-cov"), "export", "-summary-only", "-instr-profile", data, objects[0]]
    for o in objects[1:]:
        args += ["-object", o]
    args += ["-ignore-filename-regex", r"(\.cargo/registry|/rustc/|^/usr|/tests/)"]
    report = json.loads(subprocess.check_output(args, text=True))
    files = []
    for f in report["data"][0]["files"]:
        name = os.path.relpath(f["filename"], root)
        if not name.startswith("src/"):
            continue
        s = f["summary"]
        files.append({"file": name, "lines": s["lines"]["count"], "covered": s["lines"]["covered"],
                      "functions": s["functions"]["count"], "functions_covered": s["functions"]["covered"]})
    files.sort(key=lambda f: f["file"])
    total = report["data"][0]["totals"]
    print("| file | lines | covered | functions covered |\n|---|---|---|---|")
    for f in files:
        pct = 100 * f["covered"] / f["lines"] if f["lines"] else 100
        fpct = 100 * f["functions_covered"] / f["functions"] if f["functions"] else 100
        print(f"| `{f['file']}` | {f['lines']} | {pct:.0f}% | {fpct:.0f}% |")
    print(f"| **total** | {total['lines']['count']} | {total['lines']['percent']:.0f}% | {total['functions']['percent']:.0f}% |")
    for name in a.uncovered:
        args = [os.path.join(tools, "llvm-cov"), "export", "-instr-profile", data, objects[0]]
        for o in objects[1:]:
            args += ["-object", o]
        detail = json.loads(subprocess.check_output(args + [os.path.join(root, name)], text=True))
        for f in detail["data"][0]["files"]:
            # Segments: (line, column, count, has count, region entry, gap); a line is missed when
            # a region with a count of 0 starts on it.
            missed = sorted({seg[0] for seg in f["segments"] if seg[3] and seg[2] == 0 and not seg[5]})
            ranges, start, prev = [], None, None
            for line in missed:
                if start is None:
                    start = prev = line
                elif line == prev + 1:
                    prev = line
                else:
                    ranges.append((start, prev))
                    start = prev = line
            if start is not None:
                ranges.append((start, prev))
            shown = ", ".join(f"{a}" if a == b else f"{a}-{b}" for a, b in ranges)
            print(f"{os.path.relpath(f['filename'], root)}: not run: {shown or 'nothing'}")
    if a.json:
        with open(a.json, "w") as fh:
            json.dump({"files": files, "lines": total["lines"]["percent"], "functions": total["functions"]["percent"]}, fh, indent=1)


if __name__ == "__main__":
    main()
