"""One fuzzing process with one configuration: loads the fuzz build of the engine (`libaequalis.so`)
with ctypes and calls it as the Android app does. Before every call it writes the call into the
black box (a file mapped in memory), so when the engine takes the process down the supervisor
(`fuzz.py`) knows exactly what did it. Findings and figures go to stdout, one JSON per line.

    worker.py --lib LIB --config CONFIG.json --seed N --blackbox FILE --work DIR
    worker.py --lib LIB --config CONFIG.json --work DIR --replay CALLS.jsonl

Standard library only.
"""

import argparse
import ctypes
import json
import math
import mmap
import os
import random
import shutil
import struct
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import generate  # noqa: E402
import oracles  # noqa: E402

FUNCTIONS = ["init", "eval", "suggest", "set", "list", "sync", "seed"]
SLOTS, SLOT = 32, 4096
HEADER = 64


class BlackBox:
    """`state` (1 while in a call), the call's start time, the number of calls, the process's CPU
    time at the start, then a ring of the last 32 calls: function, length of the input, its first
    4,088 bytes. The supervisor reads it without a lock: `begin` writes the state last and `end`
    first, so "in a call" never pairs with the previous call's start (that read as a hang)."""

    def __init__(self, path):
        size = HEADER + SLOTS * (SLOT + 8)
        with open(path, "wb") as f:
            f.truncate(size)
        self.file = open(path, "r+b")
        self.mm = mmap.mmap(self.file.fileno(), size)
        self.count = 0

    def begin(self, fn, data):
        slot = HEADER + (self.count % SLOTS) * (SLOT + 8)
        stored = data[:SLOT]
        struct.pack_into("<II", self.mm, slot, FUNCTIONS.index(fn), len(data))
        self.mm[slot + 8:slot + 8 + len(stored)] = stored
        self.count += 1
        struct.pack_into("<dId", self.mm, 4, time.time(), self.count, time.process_time())
        struct.pack_into("<I", self.mm, 0, 1)

    def end(self):
        struct.pack_into("<I", self.mm, 0, 0)


def read_blackbox(path):
    """`(in_call, started, count, cpu, calls)`, where calls are the last ones, oldest first:
    `(fn, length, bytes)`, and `cpu` the worker's CPU time when the call started."""
    with open(path, "rb") as f:
        data = f.read()
    state, started, count, cpu = struct.unpack_from("<IdId", data, 0)
    calls = []
    for i in range(max(0, count - SLOTS), count):
        slot = HEADER + (i % SLOTS) * (SLOT + 8)
        fn, length = struct.unpack_from("<II", data, slot)
        calls.append((FUNCTIONS[fn], length, data[slot + 8:slot + 8 + min(length, SLOT)]))
    return state == 1, started, count, cpu, calls


FETCH = ctypes.CFUNCTYPE(ctypes.c_bool, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint32, ctypes.c_bool)


class Engine:
    def __init__(self, lib, box):
        self.lib = ctypes.CDLL(lib)
        self.box = box
        L = self.lib
        for name, args in [("aequalis_init", [ctypes.c_char_p]), ("aequalis_eval", [ctypes.c_char_p]), ("aequalis_suggest", [ctypes.c_char_p, ctypes.c_uint32]),
                           ("aequalis_set", [ctypes.c_char_p, ctypes.c_char_p]), ("aequalis_sync", [ctypes.c_bool]), ("aequalis_seed", [ctypes.c_char_p, ctypes.c_char_p]),
                           ("aequalis_settings", []), ("aequalis_trivia", []), ("aequalis_measures", []), ("aequalis_currencies", [])]:
            getattr(L, name).argtypes = args
            getattr(L, name).restype = ctypes.c_void_p
        L.aequalis_free.argtypes = [ctypes.c_void_p]
        L.aequalis_set_fetcher.argtypes = [FETCH]
        self.calls = 0
        self.slowest = {}

    def _call(self, fn, label, f, *args):
        if self.box:
            self.box.begin(fn, label)
        started = time.perf_counter()
        p = f(*args)
        elapsed = time.perf_counter() - started
        if self.box:
            self.box.end()
        self.calls += 1
        text = ctypes.string_at(p).decode("utf-8", "replace") if p else None
        if p:
            self.lib.aequalis_free(p)
        return text, elapsed

    def init(self, config):
        text = json.dumps(config).encode()
        return self._call("init", text, self.lib.aequalis_init, text)

    def eval(self, request):
        b = request.encode()
        return self._call("eval", b, self.lib.aequalis_eval, b)

    def suggest(self, before):
        b = before.encode()
        return self._call("suggest", b, self.lib.aequalis_suggest, b, 3)

    def set(self, key, value):
        label = json.dumps([key, value]).encode()
        return self._call("set", label, self.lib.aequalis_set, key.encode(), None if value is None else value.encode())

    def list(self, which):
        return self._call("list", which.encode(), getattr(self.lib, f"aequalis_{which}"))

    def sync(self):
        return self._call("sync", b"", self.lib.aequalis_sync, False)

    def seed(self, rates, cities):
        label = json.dumps([len(rates or b""), cities]).encode()
        return self._call("seed", label, self.lib.aequalis_seed, rates, cities.encode() if cities else None)


# ---------------------------------------------------------------- the device the engine is told about


def zone_files():
    root = "/usr/share/zoneinfo"
    out = []
    for d, dirs, files in os.walk(root):
        dirs[:] = [x for x in dirs if x not in ("posix", "right")]
        for f in files:
            path = os.path.join(d, f)
            name = os.path.relpath(path, root)
            with open(path, "rb") as fh:
                head = fh.read(4)
            if head == b"TZif" and len(name.encode()) < 40:
                out.append((name, path))
    return sorted(out)


def corrupt(data, r):
    """A zone file damaged the way a bad copy or a bad download would be."""
    data = bytearray(data)
    op = r.randint(0, 4)
    if op == 0:
        return bytes(data[: r.randint(0, len(data))])
    if op == 1:
        for _ in range(r.randint(1, 8)):
            if data:
                data[r.randrange(len(data))] = r.randrange(256)
        return bytes(data)
    if op == 2 and len(data) > 44:
        # A header count (times, types, characters…) made huge.
        at = 20 + 4 * r.randint(0, 5)
        data[at:at + 4] = struct.pack(">I", r.choice([0, 1, 0x7FFFFFFF, 0xFFFFFFFF, r.randrange(1 << 32)]))
        return bytes(data)
    if op == 3:
        return bytes(data) + bytes(r.randrange(256) for _ in range(r.randint(1, 64)))
    i = data.rfind(b"\n", 0, len(data) - 1)
    return bytes(data[: i + 1]) + r.choice([b"CET-1CEST,M3.5.0,M10.5.0/3", b"<+0330>-3:30", b"XYZ", b"A" * 300, b"M99.9.9", b"\xff\xfe"]) + b"\n"


def make_zoneinfo(kind, work, r):
    if kind == "system":
        return "/usr/share/zoneinfo"
    if kind == "none":
        path = os.path.join(work, "zoneinfo-none")
        os.makedirs(path, exist_ok=True)
        return path
    zones = zone_files()
    if kind == "mutated":
        path = os.path.join(work, "zoneinfo-mutated")
        shutil.rmtree(path, ignore_errors=True)
        for name, src in zones:
            if r.random() < 0.15:
                continue
            os.makedirs(os.path.dirname(os.path.join(path, name)), exist_ok=True)
            with open(src, "rb") as f:
                data = f.read()
            with open(os.path.join(path, name), "wb") as f:
                f.write(corrupt(data, r) if r.random() < 0.4 else data)
        return path
    # Android's tzdata: every zone in one file (`tz.rs` `from_bundle`).
    entries, body = [], b""
    for name, src in zones:
        with open(src, "rb") as f:
            data = f.read()
        if kind == "bundle-mutated" and r.random() < 0.3:
            data = corrupt(data, r)
        entries.append((name, len(body), len(data)))
        body += data
    index = 12 + 12
    data_at = index + 52 * len(entries)
    head = b"tzdata2026c\0" + struct.pack(">III", index, data_at, 0)
    table = b"".join(name.encode().ljust(40, b"\0") + struct.pack(">II", off, n) + b"\0" * 4 for name, off, n in entries)
    bundle = bytearray(head + table + body)
    if kind == "bundle-mutated":
        for _ in range(r.randint(0, 3)):
            at = r.randrange(12, data_at)
            bundle[at] = r.randrange(256)
    path = os.path.join(work, "tzdata")
    with open(path, "wb") as f:
        f.write(bundle)
    return path


# ---------------------------------------------------------------- downloads, as the fetcher serves them


class Fetcher:
    """Serves the engine's downloads from the fuzzer: real answers, damaged ones, garbage, or none."""

    def __init__(self, r, samples):
        self.r = r
        self.samples = samples
        self.callback = FETCH(self.fetch)

    def body(self, url):
        r = self.r
        sample = self.samples.get("rates" if "er-api" in url else "cities" if "cities" in url else "other", b"")
        choice = r.random()
        if choice < 0.3:
            return sample
        if choice < 0.6 and sample:
            data = bytearray(sample)
            for _ in range(r.randint(1, 20)):
                data[r.randrange(len(data))] = r.choice(b'{}[],:"0123456789-.eE \\\xff')
            return bytes(data)
        if choice < 0.8 and sample:
            return sample[: r.randint(0, len(sample))]
        return r.choice([b"", b"null", b"[]", b"{}", b'{"result":"success","rates":{"EUR":-1,"USD":1e999}}', b"<html>502</html>",
                         b'{"format":1,"count":99,"cities":[]}', b"\x1f\x8b\x08\x00", bytes(r.randrange(256) for _ in range(200))])

    def fetch(self, url, to, seconds, strict):
        try:
            if self.r.random() < 0.15:
                return False
            with open(to.decode(), "wb") as f:
                f.write(self.body(url.decode()))
            return True
        except Exception:
            return False


# ---------------------------------------------------------------- the loop


class Worker:
    def __init__(self, a):
        self.config = json.load(open(a.config))
        self.r = random.Random(a.seed)
        self.work = a.work
        self.box = BlackBox(a.blackbox) if a.blackbox else None
        self.engine = Engine(a.lib, self.box)
        with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "work", "vocab.json")) as f:
            self.vocab = json.load(f)
        self.gen = generate.Generator(self.vocab, a.seed)
        self.sent = set()
        self.found = {}
        self.plain = self.config.get("decimal", ".") == "." and not self.config.get("thousands") and self.config.get("settings", {}).get("precision") == 15
        self.start()

    def start(self):
        c = self.config
        data = os.path.join(self.work, "data")
        os.makedirs(os.path.join(data, "aequalis"), exist_ok=True)
        with open(os.path.join(data, "aequalis", "settings.json"), "w") as f:
            json.dump(c.get("settings", {}), f)
        init = {"dir": data, "zone": c["zone"], "zoneinfo": make_zoneinfo(c.get("zoneinfo", "system"), self.work, self.r),
                "decimal": c.get("decimal", "."), "thousands": c.get("thousands", ""), "grouping": c.get("grouping", [3]),
                "imperial": c.get("imperial", False), "now": c.get("now", 1792222391)}
        self.engine.init(init)
        samples = {}
        for key, path in self.vocab["samples"].items():
            with open(path, "rb") as f:
                samples[key] = f.read()
        self.fetcher = Fetcher(self.r, samples)
        self.engine.lib.aequalis_set_fetcher(self.fetcher.callback)
        kind = c.get("data", "none")
        if kind != "none":
            cities = self.vocab["archive_file"] if kind == "shipped" and self.vocab.get("archive_file") else self.vocab["samples"]["cities"]
            self.engine.seed(samples.get("rates"), cities)

    def report(self, kind, key, detail, fn, text):
        k = f"{kind}:{key}"
        self.found[k] = self.found.get(k, 0) + 1
        if k in self.sent:
            return
        self.sent.add(k)
        print(json.dumps({"type": "finding", "kind": kind, "key": key, "detail": detail, "fn": fn, "input": text, "config": self.config["id"]}), flush=True)

    def reports(self, found, fn, text):
        for kind, key, detail in found:
            self.report(kind, key, detail, fn, text)

    def timed(self, fn, text, elapsed):
        if elapsed > 0.05 and fn in ("eval", "suggest"):
            # Timed again: a busy machine (the other workers) makes any call slow once.
            call = self.engine.eval if fn == "eval" else self.engine.suggest
            elapsed = min(elapsed, call(text)[1], call(text)[1])
        if elapsed > 0.05:
            shape = "long" if len(text) > 512 else "short"
            self.report("slow", f"{fn}:{shape}:{int(math.log2(elapsed * 1000))}", f"{elapsed * 1000:.1f} ms, {len(text)} characters", fn, text)

    def eval(self, text):
        reply, elapsed = self.engine.eval(text)
        self.timed("eval", text, elapsed)
        doc, found = oracles.eval_reply(text, reply)
        self.reports(found, "eval", text)
        return doc, reply

    def typing(self, text):
        """As the app: each key, the answer and the suggestions for the text so far."""
        n = len(text)
        cuts = range(1, n + 1) if n <= 80 else sorted({self.r.randint(1, n) for _ in range(20)} | {n})
        for i in cuts:
            before = text[:i]
            reply, elapsed = self.engine.suggest(before)
            self.timed("suggest", before, elapsed)
            _, found = oracles.suggest_reply(before, reply)
            self.reports(found, "suggest", before)
            self.eval(before)

    def request(self):
        text = self.gen.request().replace("\0", "")
        self.typing(text)
        doc, reply = self.eval(text)
        if self.r.random() < 0.05:
            again, _ = self.engine.eval(text)
            if oracles.strip(again) != oracles.strip(reply):
                self.report("determinism", "differs", f"{oracles.strip(reply)[:200]} / {oracles.strip(again)[:200]}", "eval", text)
        # A question: each choice must answer, or ask something else (not the same question).
        amb = doc.get("ambiguous") if isinstance(doc, dict) else None
        if isinstance(amb, dict) and isinstance(amb.get("choices"), list) and self.r.random() < 0.3:
            for choice in amb["choices"][:3]:
                if isinstance(choice, dict) and isinstance(choice.get("request"), str):
                    d, _ = self.eval(choice["request"])
                    a2 = d.get("ambiguous") if isinstance(d, dict) else None
                    if isinstance(a2, dict) and a2.get("text") == amb.get("text") and a2.get("kind") == amb.get("kind"):
                        self.report("choices", f"asks-again:{amb.get('kind')}", f"{choice['request']!r} asks again about {amb.get('text')!r}", "eval", text)

    def calculation(self):
        text, expected, tree = self.gen.calculation()
        doc, _ = self.eval(text)
        if self.gen.overflowed and isinstance(doc, dict) and doc.get("ok"):
            # A step past the largest double, yet an answer (`10^800` → infinity, `1/∞` → 0): Aequalis
            # computes as IEEE does; whether it should say "out of range" instead is a choice.
            self.report("semantics", "intermediate-overflow", f"{text!r} → {doc.get('result')!r}", "eval", text)
        if self.plain:
            shape = ",".join(sorted({n[0] if n[0] != "fn" else n[1] for n in walk(tree)}))
            for kind, key, detail in oracles.differential(text, expected, self.gen.scale(tree), doc):
                self.report(kind, f"{key}:{shape}", detail, "eval", text)
            # Spelling changes nothing: spaces, brackets.
            spaced = "".join(f" {c} " if c in "+/^()[]{}" else c for c in text)
            for variant in [text.replace(" ", ""), text.replace("(", "[").replace(")", "]"), spaced]:
                if " x " in text and variant == text.replace(" ", ""):
                    continue
                d2, _ = self.eval(variant)
                if isinstance(doc, dict) and isinstance(d2, dict) and (doc.get("ok"), doc.get("result")) != (d2.get("ok"), d2.get("result")):
                    self.report("inconsistency", f"spelling:{shape}", f"{text!r} → {doc.get('result') or doc.get('error')!r}; {variant!r} → {d2.get('result') or d2.get('error')!r}", "eval", text)

    def units(self):
        """Round trips and the ways to write a conversion."""
        cat = self.gen.category()
        if len(cat["units"]) < 2:
            return
        a, b = self.r.sample(cat["units"], 2)
        x = self.r.choice(["1", "10", "2.5", "1000", "0.001", "42"])
        doc, _ = self.eval(f"{x} {a['symbol']} {b['symbol']}")
        if not (isinstance(doc, dict) and doc.get("ok")):
            return
        result = doc["result"]
        back, _ = self.eval(f"{result} {a['symbol']}")
        # Results have a fixed number of decimals: a tiny one keeps few significant digits.
        digits = len(result.split(" ")[0].replace("-", "").replace(".", "").lstrip("0").split("e")[0])
        if self.plain and digits >= 10 and isinstance(back, dict) and back.get("ok"):
            got = oracles.number_of(back["result"].split(" ")[0])
            if got is not None and abs(got - float(x)) > 1e-6 * max(1.0, abs(float(x))):
                self.report("inconsistency", f"round-trip:{cat['name']}", f"{x} {a['symbol']} → {result} → {back['result']}", "eval", f"{x} {a['symbol']} {b['symbol']}")
        for connector in ["to", "in", "->"]:
            d2, _ = self.eval(f"{x} {a['symbol']} {connector} {b['symbol']}")
            if not (isinstance(d2, dict) and d2.get("result") == result):
                self.report("inconsistency", f"connector:{connector}", f"`{x} {a['symbol']} {b['symbol']}` → {result}; with `{connector}` → {(d2 or {}).get('error')}", "eval", f"{x} {a['symbol']} {connector} {b['symbol']}")
        if not cat.get("case_sensitive"):
            d3, _ = self.eval(f"{x} {a['symbol'].upper()} {b['symbol'].upper()}")
            if not (isinstance(d3, dict) and d3.get("result") == result):
                self.report("inconsistency", f"case:{cat['name']}", f"`{x} {a['symbol']} {b['symbol']}` → {result}; upper case → {(d3 or {}).get('result') or (d3 or {}).get('error')}", "eval", f"{x} {a['symbol'].upper()} {b['symbol'].upper()}")

    def settings(self):
        """A burst of settings changes, then requests, then the configuration's settings back."""
        reply, _ = self.engine.list("settings")
        self.reports(oracles.list_reply("settings", reply), "list", "settings")
        try:
            listed = json.loads(reply).get("settings", [])
        except ValueError:
            return
        for _ in range(self.r.randint(1, 4)):
            s = self.r.choice(listed)
            value = self.r.choice([*s.get("options", []), s.get("default", ""), "", "0", "-1", "99999999999999999999", "30d", "1m", "abc", "true",
                                   self.gen.pick(generate.NASTY) * 3, "[[[", "]]]", "[MMMM", "DD MMM YYYY [at] HH:mm tz ZZ dddd", "YYYYYYYYYYYYYY", "ddd" * 50])
            self.engine.set(s["key"], value)
            for _ in range(3):
                self.eval(self.gen.request(simple=True))
            self.eval("now")
            self.eval("12 Sept 2026 10:00 PM")
        for s in listed:
            self.engine.set(s["key"], None)
        for key, value in self.config.get("settings", {}).items():
            self.engine.set(key, json.dumps(value) if not isinstance(value, str) else value)

    def lists(self):
        for which in ["measures", "currencies", "trivia", "settings"]:
            reply, elapsed = self.engine.list(which)
            self.timed("list", which, elapsed)
            self.reports(oracles.list_reply(which, reply), "list", which)

    def ingest(self):
        self.engine.sync()
        for _ in range(5):
            self.eval(self.gen.request(simple=True))
        self.eval("12 usd eur")
        self.eval("Rome Paris")
        self.lists()

    def run(self):
        last = time.time()
        while True:
            r = self.r.random()
            if r < 0.80:
                self.request()
            elif r < 0.90:
                self.calculation()
            elif r < 0.96:
                self.units()
            elif r < 0.975:
                self.settings()
            elif r < 0.99:
                self.lists()
            elif self.config.get("ingest"):
                self.ingest()
            if time.time() - last > 5:
                last = time.time()
                print(json.dumps({"type": "stats", "calls": self.engine.calls, "rss_kb": rss_kb(), "found": self.found}), flush=True)

    def replay(self, path):
        """Each call of the file (JSON lines: `{"fn", "input"}`), in order; the replies on stdout."""
        for line in open(path):
            call = json.loads(line)
            fn, text = call["fn"], call["input"]
            if fn == "eval":
                reply, _ = self.engine.eval(text)
            elif fn == "suggest":
                reply, _ = self.engine.suggest(text)
            elif fn == "set":
                reply, _ = self.engine.set(*json.loads(text))
            elif fn == "list":
                reply, _ = self.engine.list(text)
            elif fn == "sync":
                reply, _ = self.engine.sync()
            else:
                continue
            print(json.dumps({"fn": fn, "input": text, "reply": reply}), flush=True)


def walk(tree):
    yield tree
    for child in tree[1:]:
        if isinstance(child, tuple):
            yield from walk(child)


def rss_kb():
    with open("/proc/self/statm") as f:
        return int(f.read().split()[1]) * os.sysconf("SC_PAGE_SIZE") // 1024


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--lib", required=True)
    p.add_argument("--config", required=True)
    p.add_argument("--seed", type=int, default=0)
    p.add_argument("--blackbox")
    p.add_argument("--work", required=True)
    p.add_argument("--replay")
    a = p.parse_args()
    # When a call hangs, the supervisor takes its stack (`eu-stack`) before killing it: Yama
    # (`ptrace_scope` 1) lets only an ancestor attach, unless the process allows any.
    try:
        ctypes.CDLL(None).prctl(0x59616D61, ctypes.c_ulong(2**64 - 1), 0, 0, 0)  # PR_SET_PTRACER_ANY
    except (OSError, AttributeError):
        pass
    w = Worker(a)
    if a.replay:
        w.replay(a.replay)
    else:
        w.run()


if __name__ == "__main__":
    main()
