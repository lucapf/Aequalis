#!/usr/bin/env python3
"""Aequalis's fuzzer: runs the engine (the library alone, as the Android app calls it) for hours on
generated requests, in several configurations at once, and keeps what goes wrong under
`fuzz/findings/`. See `docs/fuzzy.md`.

    fuzz/fuzz.py build                       the fuzz build of the engine, and the vocabulary
    fuzz/fuzz.py run [--hours H | --minutes M] [--workers N] [--seed S]
                                             starts clean (no findings, no work files), and
                                             writes fuzz/findings/report.md at the end
    fuzz/fuzz.py minimize [FINDING_DIR ...]  the smallest input that still crashes the same way
    fuzz/fuzz.py replay                      fuzz/corpus/regressions.txt, in every configuration

Standard library only. `run` puts itself in a network namespace (`unshare -rn`): the engine can
reach no network, and its downloads come from the fuzzer (`worker.Fetcher`).
"""

import argparse
import hashlib
import json
import os
import re
import random
import selectors
import shlex
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import unicodedata

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
WORK = os.path.join(HERE, "work")
FINDINGS = os.path.join(HERE, "findings")
LIB = os.path.join(ROOT, "target", "fuzz", "libaequalis.so")
BIN = os.path.join(ROOT, "target", "fuzz", "aequalis")
WORKER = os.path.join(HERE, "worker.py")
HANG = 2.0

sys.path.insert(0, HERE)
import worker as worker_module  # noqa: E402

NOW = 1792222391  # 2026-10-17, frozen: the same request always gets the same answer.
PLAIN = {"decimal": ".", "thousands": "", "grouping": [3]}
CONFIGS = [
    {"id": "rome-plain", "zone": "Europe/Rome", "zoneinfo": "system", **PLAIN, "data": "sample", "settings": {"offline": True, "precision": 15}},
    {"id": "newyork-imperial-archive", "zone": "America/New_York", "zoneinfo": "system", **PLAIN, "imperial": True, "data": "shipped", "settings": {"offline": True, "precision": 15}},
    {"id": "kolkata-lakh", "zone": "Asia/Kolkata", "zoneinfo": "system", "decimal": ".", "thousands": ",", "grouping": [3, 2], "data": "sample", "settings": {"offline": True}},
    {"id": "no-zone-files", "zone": "UTC", "zoneinfo": "none", **PLAIN, "data": "none", "settings": {"offline": True, "precision": 15}},
    {"id": "chatham-damaged-zones", "zone": "Pacific/Chatham", "zoneinfo": "mutated", "decimal": ",", "thousands": ".", "grouping": [3], "data": "sample", "settings": {"offline": True}},
    {"id": "lordhowe-android-zones", "zone": "Australia/Lord_Howe", "zoneinfo": "bundle", "decimal": ",", "thousands": " ", "grouping": [3], "data": "shipped", "settings": {"offline": True, "precision": 0}},
    {"id": "rome-damaged-android-zones", "zone": "Europe/Rome", "zoneinfo": "bundle-mutated", "decimal": ".", "thousands": "'", "grouping": [3], "data": "sample", "settings": {"offline": True}},
    {"id": "downloads", "zone": "Europe/Rome", "zoneinfo": "system", **PLAIN, "data": "none", "ingest": True, "settings": {"offline": False, "precision": 15}},
    {"id": "newyork-miles-formats", "zone": "America/New_York", "zoneinfo": "system", **PLAIN, "data": "sample",
     "settings": {"offline": True, "precision": 15, "distance.unit": "mi", "format.date": "dddd D MMMM YYYY", "format.datetime": "ddd DD/MM [at] H12:mm A ZZ"}},
]
for c in CONFIGS:
    c["now"] = NOW


def config(cid):
    return next(c for c in CONFIGS if c["id"] == cid)


# ---------------------------------------------------------------- build


def build():
    cargo = ["cargo", "--quiet"]
    subprocess.check_call(cargo + ["rustc", "--profile", "fuzz", "--lib", "--no-default-features", "--features", "ffi", "--crate-type", "cdylib"], cwd=ROOT)
    subprocess.check_call(cargo + ["build", "--profile", "fuzz", "--bin", "aequalis"], cwd=ROOT)
    os.makedirs(WORK, exist_ok=True)
    env = dict(os.environ, XDG_CONFIG_HOME=os.path.join(ROOT, "tests", "config"))
    listing = subprocess.check_output([BIN, "--list"], text=True, env=env)
    units, currencies = parse_list(listing)
    data = lambda name: json.load(open(os.path.join(ROOT, "src", "data", name)))
    with open(os.path.join(ROOT, "src", "data", "currencies.json")) as f:
        currency_data = json.load(f)
    by_code = {c["code"]: c for c in (currency_data if isinstance(currency_data, list) else currency_data.get("currencies", []))}
    for c in currencies:
        c.update({k: by_code.get(c["code"], {}).get(k, []) for k in ("words", "symbols")})
    archive_file = os.path.join(ROOT, "apps", "android", "app", "src", "main", "assets", "cities.json")
    archive = []
    if os.path.exists(archive_file):
        cities = json.load(open(archive_file))["cities"]
        r = random.Random(0)
        for city in r.sample(cities, min(4000, len(cities))):
            archive.append(city["name"])
            archive += city.get("aliases", [])[:3]
            archive.append(f"{city['name']}, {city.get('region') or city.get('country')}")
    vocab = {
        "units": units,
        "currencies": currencies,
        "countries": data("countries.json")["countries"],
        "cities": sorted(data("places.json")["places"]),
        "archive": archive,
        "archive_file": archive_file if os.path.exists(archive_file) else None,
        "samples": {
            "rates": os.path.join(ROOT, "apps", "android", "app", "src", "main", "assets", "rates.json"),
            "cities": os.path.join(ROOT, "tests", "fixtures", "cities-sample.json"),
        },
        "seeds": seeds(),
    }
    vocab["samples"] = {k: v for k, v in vocab["samples"].items() if os.path.exists(v)}
    with open(os.path.join(WORK, "vocab.json"), "w") as f:
        json.dump(vocab, f)
    print(f"built {LIB}; vocabulary: {sum(len(u['units']) for u in units)} units, {len(currencies)} currencies, "
          f"{len(vocab['countries'])} countries, {len(vocab['cities'])} cities, {len(archive)} archive names, {len(vocab['seeds'])} seed requests")


def parse_list(text):
    """`aequalis --list`: the categories and their units, then the currencies."""
    units, currencies, part = [], [], "units"
    for line in text.splitlines():
        if line.startswith("Compound units"):
            continue
        if line.startswith("Currencies"):
            part = "currencies"
            continue
        if not line.strip():
            continue
        if part == "units":
            if not line.startswith(" "):
                name = line.split(" (base:")[0]
                units.append({"name": name, "case_sensitive": name.startswith("Bits"), "units": []})
            else:
                symbol, _, aliases = line.strip().partition(" ")
                units[-1]["units"].append({"symbol": symbol, "aliases": [a.strip() for a in aliases.split(",") if a.strip()]})
        elif line.startswith("  "):
            code, _, rest = line.strip().partition(" ")
            name = rest.strip().split(":")[0]
            currencies.append({"code": code, "name": name})
    return units, currencies


def seeds():
    """Requests written in the docs (in backticks) and in the tests (string literals)."""
    found = set()
    for doc in ["README.md", "docs/Accepted_Syntax.md", "docs/SPEC.md"]:
        path = os.path.join(ROOT, doc)
        if os.path.exists(path):
            for s in re.findall(r"`([^`\n]{2,80})`", open(path).read()):
                if not s.startswith(("-", "~", "/", "$", "aequalis --", "cargo")):
                    found.add(s.removeprefix("aequalis "))
    for name in os.listdir(os.path.join(ROOT, "tests")):
        if name.endswith(".rs"):
            for s in re.findall(r'"((?:[^"\\\n]|\\.){2,80})"', open(os.path.join(ROOT, "tests", name)).read()):
                # Escapes (`\u{200b}`, `\n`) left as written: they are characters to fuzz with too.
                found.add(s)
    return sorted(found)


# ---------------------------------------------------------------- findings


def slug(text):
    clean = re.sub(r"[^A-Za-z0-9._-]+", "-", text).strip("-")[:60]
    return f"{clean}-{hashlib.sha1(text.encode()).hexdigest()[:8]}"


def record(kind, key, fields):
    """A finding's folder: `report.json` (counts, first and last seen), `input.txt`."""
    folder = os.path.join(FINDINGS, kind, slug(key))
    os.makedirs(folder, exist_ok=True)
    path = os.path.join(folder, "report.json")
    now = time.strftime("%Y-%m-%d %H:%M:%S")
    report = json.load(open(path)) if os.path.exists(path) else {"kind": kind, "key": key, "count": 0, "first_seen": now}
    new = report["count"] == 0
    report.update(fields)
    # Workers report a key once each: the count is how many times it came up anew.
    report["count"] += 1
    report["last_seen"] = now
    with open(path, "w") as f:
        json.dump(report, f, indent=1, ensure_ascii=False)
    if "input" in fields and (new or not os.path.exists(os.path.join(folder, "input.txt"))):
        with open(os.path.join(folder, "input.txt"), "w") as f:
            f.write(fields["input"])
    return folder, new


PANIC = re.compile(r"panicked at ([^\n]+?):(\d+):(\d+):\n([^\n]*)")


def panic_of(stderr):
    m = PANIC.search(stderr)
    if not m:
        return None, None
    return f"{m.group(1)}:{m.group(2)}", m.group(4)


def clean():
    """Before a run: no findings and no work files left from the one before (the vocabulary
    stays: `build` makes it)."""
    shutil.rmtree(FINDINGS, ignore_errors=True)
    if os.path.isdir(WORK):
        for name in os.listdir(WORK):
            path = os.path.join(WORK, name)
            if name != "vocab.json":
                shutil.rmtree(path) if os.path.isdir(path) else os.unlink(path)


def inspect_hang(pid, started, cpu_at_start):
    """What a stuck worker is doing, before it is killed: wall and CPU time of the call, the
    scheduler's state, the load of the machine, and the stack of every thread."""
    wall = time.time() - started
    info = {"wall_s": round(wall, 2), "load": [round(x, 2) for x in os.getloadavg()]}
    try:
        with open(f"/proc/{pid}/stat") as f:
            fields = f.read().rsplit(")", 1)[1].split()
        tick = os.sysconf("SC_CLK_TCK")
        cpu = (int(fields[11]) + int(fields[12])) / tick - cpu_at_start
        info["cpu_s"] = round(max(cpu, 0.0), 2)
        info["state"] = fields[0]
        with open(f"/proc/{pid}/wchan") as f:
            wchan = f.read().strip()
            info["wchan"] = wchan if wchan not in ("", "0") else "-"
    except (OSError, ValueError, IndexError):
        pass
    if "cpu_s" in info:
        ratio = info["cpu_s"] / max(wall, 1e-9)
        info["verdict"] = ("busy: computing (a loop or a slow path)" if ratio > 0.8 else
                           "waiting: blocked (disk, lock, sleep) or not scheduled" if ratio < 0.2 else "partly busy, partly waiting")
    try:
        p = subprocess.run(["eu-stack", "-s", "-p", str(pid)], capture_output=True, text=True, timeout=20)
        info["stack"] = (p.stdout or p.stderr)[-12000:]
    except (OSError, subprocess.TimeoutExpired) as e:
        info["stack"] = f"(no stack: {e})"
    return info


# ---------------------------------------------------------------- report.md


def shown(text, limit=60):
    """`text` on one line, the invisible characters written as `\\uXXXX`, cut at `limit`."""
    out = []
    for ch in text:
        cat = unicodedata.category(ch)
        if ch == "\\":
            out.append("\\\\")
        elif cat[0] in "C" or cat in ("Zl", "Zp") or (cat == "Zs" and ch != " "):
            out.append(f"\\x{ord(ch):02x}" if ord(ch) < 0x100 else f"\\u{ord(ch):04x}")
        else:
            out.append(ch)
    s = "".join(out)
    return s if len(s) <= limit else s[: limit - 1] + "…"


def plain(text, limit=60):
    """Text for a table cell: escaped, cut, and with what Markdown would take as a table's pipe
    or an HTML tag escaped."""
    return shown(text, limit).replace("|", "\\|").replace("<", "\\<")


def code(text, limit=60):
    """Inline code for a table cell: escaped, cut, and with the pipes that would split the cell."""
    s = shown(text, limit).replace("|", "\\|")
    if not s:
        return "(empty)"
    fence = "``" if "`" in s else "`"
    pad = " " if s.startswith("`") or s.endswith("`") else ""
    return f"{fence}{pad}{s}{pad}{fence}"


def width(s):
    """Columns `s` takes in a monospaced font."""
    n = 0
    for ch in s:
        if unicodedata.combining(ch) or unicodedata.category(ch) == "Mn":
            continue
        n += 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1
    return n


def table(header, rows, right=()):
    """A Markdown table whose columns line up in the file too: every cell padded to its column's
    width. `right`: the columns aligned to the right (numbers)."""
    rows = [[str(c) for c in r] for r in rows]
    widths = [max(width(r[i]) for r in [header] + rows) for i in range(len(header))]
    widths = [max(w, 3) for w in widths]

    def line(cells):
        out = []
        for i, c in enumerate(cells):
            gap = " " * (widths[i] - width(c))
            out.append(gap + c if i in right else c + gap)
        return "| " + " | ".join(out) + " |"

    rule = "| " + " | ".join("-" * (w - 1) + ":" if i in right else "-" * w for i, w in enumerate(widths)) + " |"
    return "\n".join([line(header), rule] + [line(r) for r in rows])


def duration(seconds):
    h, rest = divmod(int(seconds), 3600)
    m, s = divmod(rest, 60)
    return f"{h} h {m:02d} min" if h else f"{m} min {s:02d} s"


def commit():
    try:
        rev = subprocess.check_output(["git", "-c", "safe.directory=*", "rev-parse", "--short", "HEAD"], cwd=ROOT, text=True, stderr=subprocess.DEVNULL).strip()
        dirty = subprocess.run(["git", "-c", "safe.directory=*", "diff", "--quiet", "HEAD"], cwd=ROOT, stderr=subprocess.DEVNULL).returncode != 0
        return rev + (" (with uncommitted changes)" if dirty else "")
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def findings():
    """Every finding of the run, by kind, the most frequent first: `(folder, report)`."""
    out = {}
    if os.path.isdir(FINDINGS):
        for kind in sorted(os.listdir(FINDINGS)):
            folder = os.path.join(FINDINGS, kind)
            if not os.path.isdir(folder):
                continue
            items = [(os.path.join(folder, n), json.load(open(os.path.join(folder, n, "report.json")))) for n in os.listdir(folder)]
            out[kind] = sorted(items, key=lambda x: (-x[1]["count"], x[1]["key"]))
    return out


def rel(path):
    return os.path.relpath(path, ROOT)


def write_report(a, began, ended, calls, totals, by_config):
    """`fuzz/findings/report.md`: the run, then every finding; crashes and hangs in detail, with
    the way to reproduce them."""
    found = findings()
    stamp = lambda t: time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(t))
    out = ["# Aequalis fuzz report", ""]
    verdict = "**No crashes and no hangs.**" if not (found.get("crash") or found.get("hang")) else \
        f"**{len(found.get('crash', []))} distinct crash(es) and {len(found.get('hang', []))} distinct hang(s): see below.**"
    out += [verdict, "", "## Run", ""]
    out.append(table(["Field", "Value"], [
        ["Started", stamp(began)],
        ["Finished", stamp(ended)],
        ["Duration", duration(ended - began)],
        ["Workers", a.workers],
        ["Seed", a.seed],
        ["Commit", commit()],
        ["Library", rel(LIB)],
        ["Calls", f"{calls:,}"],
        ["Calls per second", f"{calls / max(1, ended - began):,.0f}"],
        ["Crashes (all)", totals["crashes"]],
        ["Hangs (all)", totals["hangs"]],
        ["Worker restarts", totals["restarts"]],
        ["Hang limit", f"{HANG:g} s per call"],
    ]))
    out += ["", "## Findings by kind", ""]
    kinds = ["crash", "hang"] + sorted(k for k in found if k not in ("crash", "hang"))
    out.append(table(["Kind", "Distinct", "Times seen"],
                     [[k, len(found.get(k, [])), sum(r["count"] for _, r in found.get(k, []))] for k in kinds], right=(1, 2)))
    out += ["", "## Configurations", ""]
    out.append(table(["Configuration", "Calls", "Crashes", "Hangs"],
                     [[f"`{c}`", f"{v['calls']:,}", v["crashes"], v["hangs"]] for c, v in by_config.items()], right=(1, 2, 3)))

    crashes = found.get("crash", [])
    out += ["", "## Crashes", ""]
    if not crashes:
        out.append("None.")
    else:
        out.append(table(["#", "Location", "Message", "Times", "Configuration", "Input"],
                         [[i, f"`{r.get('location') or r['key']}`", plain(r.get("message") or "-", 50), r["count"], f"`{r['config']}`",
                           code(r.get("minimal") or r.get("input", ""))] for i, (_, r) in enumerate(crashes, 1)], right=(0, 3)))
        for i, (folder, r) in enumerate(crashes, 1):
            out += ["", f"### {i}. {r.get('location') or r['key']}: {r.get('message') or ''}".rstrip(": "), ""]
            out.append(table(["Field", "Value"], [
                ["Function", f"`{r.get('fn')}`"],
                ["Input", code(r.get("input", ""), 120)],
                ["Minimal input", code(r["minimal"], 120) if r.get("minimal") else r.get("minimal_note", "-")],
                ["Configuration", f"`{r['config']}`"],
                ["Seed", r.get("seed", "-")],
                ["Times seen", r["count"]],
                ["First / last seen", f"{r['first_seen']} / {r['last_seen']}"],
                ["Folder", f"`{rel(folder)}`"],
            ]))
            text = r.get("minimal") or r.get("input", "")
            out += ["", "Reproduce (the fuzz build checks every overflow; a release build may not crash):", "", "```sh",
                    f"printf '%s' {shlex.quote(text)} | target/fuzz/aequalis",
                    f"python3 fuzz/fuzz.py minimize {rel(folder)}   # with its configuration and seed", "```"]
            panic = r.get("stderr", "").strip()
            if panic:
                out += ["", "<details><summary>Panic and backtrace</summary>", "", "```", panic[:4000], "```", "", "</details>"]

    hangs = found.get("hang", [])
    out += ["", "## Hangs", ""]
    if not hangs:
        out.append("None.")
    else:
        out.append(table(["#", "Function", "Wall", "CPU", "Load", "Verdict", "Configuration", "Input"],
                         [[i, f"`{r.get('fn')}`", f"{r.get('wall_s', '?')} s", f"{r.get('cpu_s', '?')} s", (r.get("load") or ["?"])[0],
                           (r.get("verdict") or "-").split(":")[0], f"`{r['config']}`", code(r.get("input", ""), 40)] for i, (_, r) in enumerate(hangs, 1)],
                         right=(0, 2, 3, 4)))
        for i, (folder, r) in enumerate(hangs, 1):
            out += ["", f"### {i}. `{r.get('fn')}` stuck for {r.get('wall_s', '?')} s", ""]
            out.append(table(["Field", "Value"], [
                ["Input", code(r.get("input", ""), 120)],
                ["Configuration", f"`{r['config']}`"],
                ["Seed", r.get("seed", "-")],
                ["Wall / CPU time", f"{r.get('wall_s', '?')} s / {r.get('cpu_s', '?')} s"],
                ["Verdict", r.get("verdict", "-")],
                ["Process state", f"{r.get('state', '?')} (waiting in `{r.get('wchan', '?')}`)"],
                ["Load (1, 5, 15 min)", ", ".join(map(str, r.get("load", []))) or "?"],
                ["Times seen", r["count"]],
                ["Folder", f"`{rel(folder)}`"],
            ]))
            out += ["", "Calls before it (oldest first):", ""]
            history = r.get("history", [])[-8:]
            out.append(table(["Function", "Input"], [[f"`{h['fn']}`", code(h["input"], 80)] for h in history]) if history else "None.")
            if r.get("stack"):
                out += ["", "<details><summary>Stack of the stuck worker</summary>", "", "```", r["stack"].strip(), "```", "", "</details>"]

    for kind in kinds[2:]:
        items = found[kind]
        out += ["", f"## {kind.capitalize()}", ""]
        out.append(table(["Times", "Key", "Configuration", "Input", "Detail"],
                         [[r["count"], code(r["key"], 40), f"`{r.get('config', '-')}`", code(r.get("input", ""), 40), plain(r.get("detail", "-"), 90)]
                          for _, r in items], right=(0,)))
    out.append("")
    os.makedirs(FINDINGS, exist_ok=True)
    path = os.path.join(FINDINGS, "report.md")
    with open(path, "w") as f:
        f.write("\n".join(out))
    return path


# ---------------------------------------------------------------- run


def sandboxed():
    if os.environ.get("AEQUALIS_FUZZ_SANDBOX") == "1":
        return True
    env = dict(os.environ, AEQUALIS_FUZZ_SANDBOX="1")
    try:
        os.execvpe("unshare", ["unshare", "-rn", sys.executable, "-I", os.path.abspath(__file__), *sys.argv[1:]], env)
    except OSError as e:
        sys.exit(f"fuzz: unshare -rn failed ({e}); --no-sandbox runs with the network reachable")


def worker_env(cfg, work):
    return {"PATH": os.path.join(work, "no-bin"), "HOME": work, "TZ": cfg["zone"], "LC_ALL": "C", "RUST_BACKTRACE": "1", "PYTHONDONTWRITEBYTECODE": "1"}


class Slot:
    """One worker process: its configuration, seed, black box and stderr."""

    def __init__(self, index, cfg, seed):
        self.index, self.cfg, self.seed = index, cfg, seed
        self.work = os.path.join(WORK, f"w{index}", cfg["id"])
        # Every worker starts from nothing: no settings, data or downloads from the one before.
        shutil.rmtree(self.work, ignore_errors=True)
        os.makedirs(os.path.join(self.work, "no-bin"), exist_ok=True)
        self.box = os.path.join(self.work, "blackbox")
        # The previous worker's black box would read as a call still running.
        if os.path.exists(self.box):
            os.unlink(self.box)
        self.config_path = os.path.join(self.work, "config.json")
        with open(self.config_path, "w") as f:
            json.dump(cfg, f)
        self.err_path = os.path.join(self.work, "stderr.txt")
        self.err = open(self.err_path, "w")
        self.proc = subprocess.Popen(
            [sys.executable, "-I", WORKER, "--lib", LIB, "--config", self.config_path, "--seed", str(seed), "--blackbox", self.box, "--work", self.work],
            stdout=subprocess.PIPE, stderr=self.err, env=worker_env(cfg, self.work), text=True, bufsize=1)
        self.started = time.time()
        self.killed = None
        self.calls = 0
        self.found = {}


def run(a):
    if not a.no_sandbox:
        sandboxed()
    if not os.path.exists(LIB) or not os.path.exists(os.path.join(WORK, "vocab.json")):
        sys.exit("fuzz: run `fuzz/fuzz.py build` first")
    clean()
    seconds = a.hours * 3600 + a.minutes * 60
    began = time.time()
    end = began + seconds
    sel = selectors.DefaultSelector()
    rng = random.Random(a.seed)
    slots = {}
    spawned = [0]
    new_crashes = []
    totals = {"crashes": 0, "hangs": 0, "restarts": 0}
    done_calls = 0
    by_config = {c["id"]: {"calls": 0, "crashes": 0, "hangs": 0} for c in CONFIGS}
    suspects = {}

    def spawn(i, same=None):
        # The next configuration in turn (the same one again after a crash: it may crash again).
        cfg = same or CONFIGS[spawned[0] % len(CONFIGS)]
        spawned[0] += same is None
        s = Slot(i, cfg, rng.randrange(1 << 31))
        slots[i] = s
        sel.register(s.proc.stdout, selectors.EVENT_READ, s)

    def finish(s, why=None):
        nonlocal done_calls
        sel.unregister(s.proc.stdout)
        if s.proc.poll() is None:
            s.proc.kill()
        s.proc.wait()
        for line in s.proc.stdout:
            handle(s, line)
        s.err.close()
        done_calls += s.calls
        by_config[s.cfg["id"]]["calls"] += s.calls
        code = s.proc.returncode
        if why is None and code not in (0, None):
            crashed(s, code)

    def crashed(s, code, hang=None):
        in_call, started, count, cpu, calls = worker_module.read_blackbox(s.box)
        stderr = open(s.err_path, errors="replace").read()[-20000:]
        location, message = panic_of(stderr)
        extra = {}
        if hang:
            # One finding per call (function and input), not one per function.
            fn, _, data = calls[-1] if calls else ("?", 0, b"")
            kind, key = "hang", f"{fn}:{hashlib.sha1(data).hexdigest()[:8]}"
            extra = hang
        elif location:
            kind, key = "crash", location
        else:
            kind, key = "crash", f"exit {code}" + ("" if in_call else " (outside a call)")
            if "Traceback" in stderr:
                kind, key = "harness", stderr.strip().splitlines()[-1][:120]
        last = calls[-1] if calls else ("?", 0, b"")
        text = last[2].decode("utf-8", "replace")
        history = [{"fn": fn, "input": data.decode("utf-8", "replace"), "length": n} for fn, n, data in calls]
        folder, new = record(kind, key, {"location": location, "message": message, "fn": last[0], "input": text, "length": last[1],
                                         "config": s.cfg["id"], "seed": s.seed, "history": history, "stderr": stderr[-4000:], **extra})
        totals["hangs" if hang else "crashes"] += 1
        by_config[s.cfg["id"]]["hangs" if hang else "crashes"] += 1
        if new and kind in ("crash", "hang"):
            new_crashes.append(folder)
            print(f"  NEW {kind}: {key} ({s.cfg['id']}): {last[0]}({text[:80]!r}) {message or ''}", flush=True)

    def handle(s, line):
        try:
            msg = json.loads(line)
        except ValueError:
            return
        if msg.get("type") == "stats":
            s.calls = msg["calls"]
            if msg.get("rss_kb", 0) > a.rss_limit_mb * 1024:
                record("memory", f"rss over {a.rss_limit_mb} MB ({s.cfg['id']})", {"rss_kb": msg["rss_kb"], "config": s.cfg["id"]})
        elif msg.get("type") == "finding":
            _, new = record(msg["kind"], msg["key"], {"detail": msg["detail"], "fn": msg["fn"], "input": msg["input"], "config": msg["config"]})
            if new:
                print(f"  new {msg['kind']}: {msg['key']}: {msg['detail'][:140]}", flush=True)

    for i in range(a.workers):
        spawn(i)
    print(f"fuzz: {a.workers} workers, {len(CONFIGS)} configurations, until {time.strftime('%H:%M', time.localtime(end))}; findings in {FINDINGS}", flush=True)
    last_status = time.time()
    try:
        while time.time() < end:
            for key, _ in sel.select(timeout=0.5):
                s = key.data
                line = s.proc.stdout.readline()
                if line:
                    handle(s, line)
            for i, s in list(slots.items()):
                if s.proc.poll() is not None:
                    finish(s)
                    totals["restarts"] += 1
                    spawn(i, s.cfg)
                    continue
                try:
                    in_call, started, count, cpu, calls = worker_module.read_blackbox(s.box)
                except (OSError, ValueError, IndexError, struct.error):
                    # Not written yet: the worker is starting.
                    continue
                # Starting the engine (installing the city archive) takes longer than a request.
                limit = 60 if calls and calls[-1][0] in ("init", "seed", "sync") else HANG
                over = in_call and count > 0 and time.time() - started > limit
                # A hang only when two reads in a row see the same call still running.
                if over and suspects.get(i) == (count, started):
                    suspects.pop(i)
                    hang = inspect_hang(s.proc.pid, started, cpu)
                    s.proc.kill()
                    s.proc.wait()
                    crashed(s, None, hang=hang)
                    finish(s, "hang")
                    spawn(i, s.cfg)
                    continue
                if over:
                    suspects[i] = (count, started)
                    continue
                suspects.pop(i, None)
                if time.time() - s.started > a.rotate * 60:
                    finish(s, "rotate")
                    spawn(i)
            if time.time() - last_status > 30:
                last_status = time.time()
                calls = done_calls + sum(s.calls for s in slots.values())
                rate = calls / max(1, time.time() - (end - seconds))
                kinds = {k: len(v) for k, v in findings().items()}
                print(f"{time.strftime('%H:%M:%S')} {calls:,} calls ({rate:,.0f}/s), crashes {totals['crashes']}, hangs {totals['hangs']}; findings {kinds}", flush=True)
    except KeyboardInterrupt:
        print("stopping", flush=True)
    for s in list(slots.values()):
        finish(s, "end")
    calls = done_calls
    print(f"\n{calls:,} calls in {(time.time() - began) / 60:.1f} min; {totals['crashes']} crashes, {totals['hangs']} hangs, {totals['restarts']} restarts")
    summary()
    if new_crashes and not a.no_minimize:
        print("\nminimizing the new crashes")
        for folder in new_crashes:
            minimize_one(folder)
    path = write_report(a, began, time.time(), calls, totals, by_config)
    print(f"\nreport: {path}")


def summary():
    if not os.path.isdir(FINDINGS):
        print("no findings")
        return
    for kind in sorted(os.listdir(FINDINGS)):
        if not os.path.isdir(os.path.join(FINDINGS, kind)):
            continue
        print(f"\n{kind}:")
        for name in sorted(os.listdir(os.path.join(FINDINGS, kind))):
            r = json.load(open(os.path.join(FINDINGS, kind, name, "report.json")))
            shown = r.get("minimal", r.get("input", ""))
            print(f"  {r['count']:>7} × {r['key'][:70]:<70} {shown[:60]!r}")


# ---------------------------------------------------------------- replay and minimize


def replay_calls(cfg, calls, seed=0):
    """Runs `calls` in a fresh worker; `(location or exit, replies)`: the panic's location when the
    engine took the worker down, else None."""
    work = os.path.join(WORK, "replay", cfg["id"])
    os.makedirs(os.path.join(work, "no-bin"), exist_ok=True)
    with tempfile.NamedTemporaryFile("w", suffix=".jsonl", dir=work, delete=False) as f:
        for c in calls:
            f.write(json.dumps(c) + "\n")
        path = f.name
    with open(os.path.join(work, "config.json"), "w") as f:
        json.dump(cfg, f)
    p = subprocess.run([sys.executable, "-I", WORKER, "--lib", LIB, "--config", os.path.join(work, "config.json"), "--work", work, "--seed", str(seed), "--replay", path],
                       capture_output=True, text=True, env=worker_env(cfg, work), timeout=60)
    os.unlink(path)
    location, _ = panic_of(p.stderr)
    replies = [json.loads(l) for l in p.stdout.splitlines() if l.startswith("{")]
    if p.returncode != 0:
        return location or f"exit {p.returncode}", replies
    return None, replies


def ddmin(text, still_fails, unit):
    """Delta debugging: removes chunks of `text` (characters, or words) while it still fails."""
    parts = list(text) if unit == "char" else text.split(" ")
    join = "".join if unit == "char" else " ".join
    n = 2
    while len(parts) >= 2:
        size = max(1, len(parts) // n)
        removed = False
        for start in range(0, len(parts), size):
            candidate = parts[:start] + parts[start + size:]
            if candidate and still_fails(join(candidate)):
                parts, n, removed = candidate, max(n - 1, 2), True
                break
        if not removed:
            if size == 1:
                break
            n = min(len(parts), n * 2)
    return join(parts)


def minimize_one(folder):
    r = json.load(open(os.path.join(folder, "report.json")))
    if r["kind"] != "crash" or r.get("fn") not in ("eval", "suggest"):
        print(f"  {r['key']}: not minimized (a {r['kind']} in {r.get('fn')})")
        return
    cfg = config(r["config"])
    target = r["key"]
    seed = r.get("seed", 0)
    tests = [0]

    def still_fails(candidate):
        tests[0] += 1
        location, _ = replay_calls(cfg, [{"fn": r["fn"], "input": candidate}], seed)
        return location == target

    if not still_fails(r["input"]):
        r["minimal_note"] = "does not crash alone: see `history` (the calls before it)"
        print(f"  {target}: not reproduced alone ({r['input'][:60]!r})")
    else:
        small = ddmin(ddmin(r["input"], still_fails, "word"), still_fails, "char")
        r["minimal"] = small
        print(f"  {target}: {r['input'][:60]!r} → {small!r} ({tests[0]} runs, {cfg['id']})")
        with open(os.path.join(folder, "minimal.txt"), "w") as f:
            f.write(small)
    with open(os.path.join(folder, "report.json"), "w") as f:
        json.dump(r, f, indent=1, ensure_ascii=False)


def minimize(a):
    crashes = os.path.join(FINDINGS, "crash")
    folders = a.findings or ([os.path.join(crashes, n) for n in sorted(os.listdir(crashes))] if os.path.isdir(crashes) else [])
    for folder in folders:
        minimize_one(folder)


def replay(a):
    """`fuzz/corpus/regressions.txt`: each line (JSON: `input`, optional `config`) typed and
    evaluated; any crash fails."""
    path = os.path.join(HERE, "corpus", "regressions.txt")
    lines = [json.loads(l) for l in open(path) if l.strip() and not l.startswith("#")]
    failed = 0
    for cfg in CONFIGS:
        calls = []
        for line in lines:
            if line.get("config", cfg["id"]) != cfg["id"]:
                continue
            text = line["input"]
            calls += [{"fn": "suggest", "input": text[:i]} for i in range(1, len(text) + 1)]
            calls += [{"fn": "eval", "input": text}]
        if not calls:
            continue
        location, replies = replay_calls(cfg, calls)
        status = f"CRASH at {location}" if location else "ok"
        failed += bool(location)
        print(f"  {cfg['id']:<28} {len(calls):>6} calls: {status}")
    sys.exit(1 if failed else 0)


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="command", required=True)
    sub.add_parser("build")
    r = sub.add_parser("run")
    r.add_argument("--hours", type=float, default=0)
    r.add_argument("--minutes", type=float, default=0)
    r.add_argument("--workers", type=int, default=max(1, (os.cpu_count() or 2) - 2))
    r.add_argument("--seed", type=int, default=int(time.time()))
    r.add_argument("--rotate", type=float, default=20, help="minutes before a worker moves to the next configuration")
    r.add_argument("--rss-limit-mb", type=int, default=300)
    r.add_argument("--no-minimize", action="store_true")
    r.add_argument("--no-sandbox", action="store_true")
    m = sub.add_parser("minimize")
    m.add_argument("findings", nargs="*")
    sub.add_parser("replay")
    a = p.parse_args()
    if a.command == "build":
        build()
    elif a.command == "run":
        if not (a.hours or a.minutes):
            a.minutes = 10
        run(a)
    elif a.command == "minimize":
        minimize(a)
    else:
        replay(a)


if __name__ == "__main__":
    main()
