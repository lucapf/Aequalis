"""The fuzzer's requests: built from the engine's own vocabulary (`fuzz.py build` writes it to
`fuzz/work/vocab.json`), by feature, then mutated; and a calculator whose value Python knows, for
the differential oracle. Standard library only.
"""

import math
import random

# Characters that break code counting bytes, case or width: 2, 3 and 4 bytes in UTF-8, combining
# marks, zero-width and direction marks, spaces that are not spaces, look-alike digits and signs,
# letters whose case changes their length (ß → SS, İ → i̇), emoji with modifiers.
NASTY = [
    "é", "ß", "İ", "ı", "ﬁ", "ς", "Σ", "°", "²", "³", "µ", "·", "€", "£", "¥", "₹", "円", "−", "–", "—",
    "́", "̈", "​", "‍", "‎", "‏", "‮", "⁦", "﻿",
    " ", " ", " ", "　", "\t", "\r", "\x0b", "\x1b", "\x7f",
    "１", "٣", "۳", "०", "½", "⅓", "∞", "√", "π", "×", "÷", "′", "″", "„", "’", "«",
    "🙂", "👍🏽", "🇮🇹", "👨‍👩‍👧", "𝟘", "𐍈", "\U0010ffff", "Ⅻ", "ǅ", "ᾈ",
]

NUMBERS = [
    "0", "-0", "1", "-1", "2", "10", "12", "100", "0.5", ".5", "5.", "1e3", "2.5E-3", "1e", "1e-400", "1e400",
    "1e308", "5e-324", "9" * 40, "0" * 30 + "1", "1." + "0" * 30 + "1", "3.14159", "-40", "1792222391",
    "007", "1_000", "1,5", "1.000", "1,000", "2,5", "0x10", "inf", "nan", "+5",
]

DATE_WORDS = ["now", "today", "date", "local", "num", "n", "number"]
FORMATS = [
    "iso", "iso-utc", "iso-basic", "iso-date", "iso-time", "iso-ordinal", "iso-week",
    "unix", "unix-ms", "unix-us", "unix-ns", "local",
]
ORDERS = ["YYYY/MM/DD", "YY/MM/DD", "DD/MM/YY", "DD/MM/YYYY", "MM/DD/YYYY", "MM/DD/YY"]
PATTERNS = ORDERS + [o + v for o in ORDERS for v in ["-24h", "-24h-tz", "-12h", "-12h-tz"]]
MONTHS = [
    "January", "Jan", "Sept", "Sep", "September", "Sptember", "Febuary", "enero", "septiembre", "setiembre",
    "janvier", "février", "fevrier", "févr", "juil", "gennaio", "giugno", "gen", "März", "Marz", "Mrz", "Okt",
    "Dezember", "Jänner", "janeiro", "março", "out", "dez", "jui",
]
FUNCTIONS = ["sqrt", "log", "log10", "ln", "loge", "sin", "cos", "tan", "sinh", "cosh", "tanh"]
CONNECTORS = ["to", "in", "->", "of", "de", "the", "distance", "a", "da"]
ZONE_WORDS = ["GMT", "UTC", "GMT+2", "UTC-5", "GMT+05:30", "UTC+0530", "GMT-14", "GMT+15", "GMT+abc", "gmt+1", "utc-05:30"]
TIMES = ["10:00", "10:00AM", "10:00 PM", "10 pm", "22:00", "9:30:15 p.m.", "24:00", "23:59:60", "3:20PMUTC-5", "15:30GMT+2"]
SHAPES = ["12/04/2026", "2026/12/04", "2026-12-04", "2026-W49-5", "2026-338", "20261204T1504Z", "2026-12-04T10:00:00+02:00",
          "25/12/2026", "31/02/2026", "2026/13/40", "12-Sep-26", "1er janvier 2026", "12 de septiembre de 2026", "12. März 2026"]
DURATIONS = ["d", "wd", "h", "min", "s", "ms", "wk", "mo", "yr", "days", "hours", "workdays"]


def trig(name, x):
    """`sin`, `cos`, `tan` with Tally's rule: a result below 1e-14 of the argument is 0."""
    r = getattr(math, name)(x)
    return 0.0 if abs(r) < 1e-14 * abs(x) else r


class Generator:
    def __init__(self, vocab, seed):
        self.r = random.Random(seed)
        self.v = vocab
        self.units = [u for cat in vocab["units"] for u in cat["units"]]
        self.words = [w for u in self.units for w in [u["symbol"], *u["aliases"]]]

    # ---------------------------------------------------------------- pieces

    def pick(self, items):
        return self.r.choice(items)

    def number(self):
        r = self.r.random()
        if r < 0.4:
            return self.pick(NUMBERS)
        if r < 0.7:
            return str(self.r.randint(0, 10 ** self.r.randint(1, 12)))
        if r < 0.9:
            return f"{self.r.uniform(-1e4, 1e4):.{self.r.randint(0, 8)}f}"
        return f"{self.r.uniform(1, 10):.3f}e{self.r.randint(-30, 30)}"

    def sp(self):
        return self.pick([" ", " ", " ", "", "  "])

    def category(self):
        return self.pick(self.v["units"])

    def unit(self, cat=None):
        cat = cat or self.category()
        u = self.pick(cat["units"])
        return self.pick([u["symbol"], u["symbol"], *u["aliases"]]) if u["aliases"] else u["symbol"]

    def currency(self):
        c = self.pick(self.v["currencies"])
        return self.pick([c["code"], c["code"].lower(), c["name"], *c["words"], *c["symbols"]])

    def country(self):
        c = self.pick(self.v["countries"])
        return self.pick([c["name"], *c.get("aliases", [])])

    def city(self):
        return self.pick(self.v["cities"]) if self.r.random() < 0.6 else self.pick(self.v["archive"] or self.v["cities"])

    def place(self):
        r = self.r.random()
        if r < 0.5:
            return self.city()
        if r < 0.75:
            return self.country()
        lat, lon = self.r.uniform(-95, 95), self.r.uniform(-190, 190)
        return self.pick([f"{lat:.4f},{lon:.4f}", f"{lat:.2f}, {lon:.2f}", f"{abs(lat):.1f}{'N' if lat > 0 else 'S'} {abs(lon):.1f}{'E' if lon > 0 else 'W'}",
                          f"{lat:.1f}°N {lon:.1f}°E", f"{self.city()}, {self.country()}"])

    def date(self):
        r = self.r.random()
        if r < 0.3:
            return self.pick(SHAPES)
        if r < 0.55:
            d, y = self.r.randint(0, 32), self.r.choice([2026, 26, 1999, 9999, 10000, 0])
            m = self.pick(MONTHS)
            return self.pick([f"{d} {m} {y}", f"{m} {d}, {y}", f"{y} {m} {d}", f"{d}-{m}-{y}", f"{d}st of {m} {y}"])
        if r < 0.7:
            return self.pick(["now", "today", "now", f"{self.r.randint(-10**12, 10**12)} unix", f"{self.number()} unix-ms"])
        y, m, d = self.r.randint(1, 9999), self.r.randint(0, 13), self.r.randint(0, 32)
        sep = self.pick(["/", "-", ".", " "])
        return self.pick([f"{y:04}{sep}{m:02}{sep}{d:02}", f"{d}{sep}{m}{sep}{y}", f"{m}/{d}/{y % 100:02}"])

    def time(self):
        if self.r.random() < 0.6:
            return self.pick(TIMES)
        return f"{self.r.randint(0, 25)}:{self.r.randint(0, 61):02}{self.pick(['', 'AM', ' pm', ':00', ':59.123456789'])}"

    # ---------------------------------------------------------------- features

    def conversion(self):
        cat = self.category()
        return f"{self.number()}{self.sp()}{self.unit(cat)}{self.sp()}{self.pick(['', 'to ', 'in ', '-> '])}{self.unit(cat if self.r.random() < 0.8 else None)}"

    def compound(self):
        def term():
            return f"{self.unit()}{self.pick(['', '^2', '2', '²', '³', '^-1', '^99'])}"

        def compound():
            return term() + "".join(f"{self.pick(['/', '*', '·'])}{self.pick(['', '100', '1'])}{term()}" for _ in range(self.r.randint(1, 3)))

        return f"{self.number()} {compound()} {self.pick([compound(), self.unit()])}"

    def money(self):
        a = self.number()
        return self.pick([f"{a} {self.currency()} {self.currency()}", f"{a}{self.currency()} {self.currency()}", f"{self.currency()}{a} to {self.currency()}",
                          f"{a} {self.country()} {self.country()}", f"{a} {self.currency()}", f"{self.currency()} {a} in {self.currency()}"])

    def dates(self):
        d = self.date()
        math_ = "".join(f" {self.pick(['+', '-'])}{self.sp()}{self.number()}{self.sp()}{self.pick(DURATIONS)}" for _ in range(self.r.randint(0, 3)))
        return self.pick([f"{d}{math_}", f"{d}{math_} {self.pick(FORMATS + PATTERNS)}", f"{d} {self.pick(PATTERNS)} {self.pick(FORMATS)}",
                          f"{d} - {self.date()} {self.pick(['', 'd', 'wd', 'h'])}", f"{d} {self.pick(ZONE_WORDS)} {self.pick(FORMATS)}", f"{d} {self.time()}"])

    def times(self):
        return self.pick([f"{self.time()} {self.place()}", f"{self.time()} {self.place()} to {self.place()}", f"now {self.place()}", f"{self.place()}",
                          f"{self.time()} {self.pick(ZONE_WORDS)} {self.place()}", f"{self.date()} {self.time()} {self.place()}"])

    def distance(self):
        return self.pick([f"distance {self.place()} to {self.place()}", f"{self.place()} {self.place()}", f"{self.place()} to {self.place()} {self.unit()}",
                          f"{self.place()} to {self.place()} in {self.pick(['km', 'mi', 'm', 'ft', 'au'])}", f"distance {self.place()}"])

    def percent(self):
        a, b = self.number(), self.number()
        return self.pick([f"{a} + {b}%", f"{a} - {b}%", f"{b}% of {a}", f"{a} * {b}%", f"{b}%", f"{a} + {b}% + {b}%"])

    def soup(self):
        pool = [self.number, self.unit, self.currency, self.place, self.date, self.time, lambda: self.pick(CONNECTORS + DATE_WORDS + FORMATS + ZONE_WORDS),
                lambda: self.pick(["+", "-", "*", "/", "^", "(", ")", "[", "]", "{", "}", "%", ":", ",", "."]), lambda: self.pick(NASTY)]
        return self.pick([" ", "", " "]).join(self.pick(pool)() for _ in range(self.r.randint(1, 8)))

    def random_text(self):
        n = self.r.randint(0, 40)
        return "".join(chr(self.pick([self.r.randint(32, 126), self.r.randint(0x80, 0x7ff), self.r.randint(0x800, 0xd7ff), self.r.randint(0x10000, 0x10ffff)]))
                       for _ in range(n))

    # ---------------------------------------------------------------- mutations

    def mutate(self, s):
        for _ in range(self.r.randint(1, 3)):
            op = self.r.randint(0, 9)
            i = self.r.randint(0, len(s))
            if op == 0:
                s = s[:i] + self.pick(NASTY) + s[i:]
            elif op == 1 and s:
                s = s[:i] + s[i + 1:]
            elif op == 2:
                s = s.replace(" ", "", 1)
            elif op == 3:
                s = s.upper() if self.r.random() < 0.5 else s.lower()
            elif op == 4:
                words = s.split(" ")
                self.r.shuffle(words)
                s = " ".join(words)
            elif op == 5:
                s = s[:i] + self.pick(self.words + CONNECTORS + FORMATS + ZONE_WORDS) + s[i:]
            elif op == 6 and s:
                j = self.r.randint(i, len(s))
                s = s[:j] + s[i:j] + s[j:]
            elif op == 7:
                s = s[:i] + self.pick(["(", ")", "[", "{", "-", "+", "°", "%", ",", ".", ":", "/"]) + s[i:]
            elif op == 8:
                s = s[:i] + " " * self.r.randint(1, 3) + s[i:]
            else:
                s = s[:i] + self.number() + s[i:]
        return s

    def long(self):
        """Near the 512-character limit, or far past it."""
        base = self.request(simple=True)
        n = self.pick([510, 511, 512, 513, 514, 600, 2048, 2049, 100_000])
        filler = self.pick([" ", "1+", "é", "🙂", base + " "])
        text = (base + filler * n)
        return text[:n] if len(text) >= n else text

    # ---------------------------------------------------------------- one request

    def request(self, simple=False):
        """A request and the name of the strategy that made it."""
        features = [self.conversion, self.compound, self.money, self.dates, self.times, self.distance, self.percent]
        r = self.r.random()
        if simple or r < 0.45:
            text = self.pick(features)()
        elif r < 0.65:
            text = self.mutate(self.pick(features)())
        elif r < 0.8:
            text = self.mutate(self.pick(self.v["seeds"]))
        elif r < 0.92:
            text = self.soup()
        elif r < 0.97:
            text = self.random_text()
        else:
            text = self.long()
        return text

    # ---------------------------------------------------------------- the calculator, with its value

    def calculation(self):
        """`(text, value)`: an expression in one of Tally's spellings, and its value as Python
        computes it (`None`: an error, which Tally must give too)."""
        tree = self._tree(self.r.randint(1, 4))
        text = self._render(tree, 0)
        self.overflowed = False
        try:
            value = self._eval(tree)
            if not math.isfinite(value):
                value = None
        except (ArithmeticError, ValueError):
            value = None
        return text, value, tree

    def _tree(self, depth):
        if depth == 0 or self.r.random() < 0.25:
            r = self.r.random()
            if r < 0.1:
                return ("const", self.pick(["e", "pi"]))
            n = self.pick(["0", "1", "2", "3", "10", "0.5", ".25", "1e3", "2.5E-2", "7", "100"]) if r < 0.6 else str(self.r.randint(0, 999))
            return ("num", n)
        r = self.r.random()
        if r < 0.55:
            return (self.pick(["+", "-", "*", "/"]), self._tree(depth - 1), self._tree(depth - 1))
        if r < 0.7:
            return ("^", self._tree(depth - 1), ("num", self.pick(["2", "3", "0.5", "-1", "0"])) if self.r.random() < 0.7 else self._tree(depth - 1))
        if r < 0.8:
            return ("neg", self._tree(depth - 1))
        if r < 0.9:
            return ("fn", self.pick(FUNCTIONS), self._tree(depth - 1))
        return ("deg", self.pick(["sin", "cos", "tan"]), self.pick(["0", "30", "45", "60", "90", "180"]), self.pick(["°", "deg", " deg"]))

    # Precedence: sum 1, product 2, unary minus 3, power 4, atom 5.
    def _render(self, t, outer):
        kind = t[0]
        sp = self.sp
        if kind == "num":
            return t[1]
        if kind == "const":
            return t[1]
        if kind in "+-":
            text, own = f"{self._render(t[1], 1)}{sp()}{kind}{sp()}{self._render(t[2], 2)}", 1
        elif kind in "*/":
            op = kind if kind == "/" else self.pick(["*", " x "])
            text, own = f"{self._render(t[1], 2)}{sp()}{op}{sp()}{self._render(t[2], 3)}", 2
        elif kind == "^":
            # Right-associative and tighter than a unary minus: the base needs brackets unless it
            # is an atom; `2^-1` is allowed.
            exp = t[2]
            exp_text = exp[1] if exp[0] == "num" and exp[1].startswith("-") else self._render(exp, 4)
            text, own = f"{self._render(t[1], 5)}{self.pick(['^', '**'])}{exp_text}", 4
        elif kind == "neg":
            text, own = f"-{self._render(t[1], 4)}", 3
        elif kind == "fn":
            o, c = self.pick([("(", ")"), ("[", "]"), ("{", "}")])
            text, own = f"{t[1]}{o}{self._render(t[2], 0)}{c}", 5
        else:  # deg
            text, own = f"{t[1]}({t[2]}{t[3]})", 5
        # Left operands are rendered at their own level, right ones one above (left-associative);
        # brackets only where the precedence needs them.
        if own < outer:
            o, c = self.pick([("(", ")"), ("[", "]"), ("{", "}")])
            return f"{o}{text}{c}"
        return text

    def _eval(self, t):
        """IEEE doubles, as Tally computes: a value too large is infinity (noted in `overflowed`),
        not an error; only the final result must be finite."""
        r = self._value(t)
        if math.isinf(r):
            self.overflowed = True
        if math.isnan(r):
            raise ValueError("not a real number")
        return r

    def _value(self, t):
        kind = t[0]
        if kind == "num":
            return float(t[1])
        if kind == "const":
            return math.e if t[1] == "e" else math.pi
        if kind == "neg":
            return -self._eval(t[1])
        if kind == "deg":
            return trig(t[1], math.radians(float(t[2])))
        if kind == "fn":
            x = self._eval(t[2])
            name = t[1]
            if name in ("sin", "cos", "tan"):
                return trig(name, x)
            if name == "sqrt":
                if x < 0:
                    raise ValueError
                return math.sqrt(x)
            if name in ("log", "log10", "ln", "loge"):
                if x <= 0:
                    raise ValueError
                return math.log10(x) if name.startswith("log") and name != "loge" else math.log(x)
            try:
                return getattr(math, name)(x)
            except OverflowError:
                return math.copysign(math.inf, x) if name == "sinh" else math.inf
        a, b = self._eval(t[1]), self._eval(t[2])
        if kind in "+-":
            # Tally's rule (SPEC §3): a sum below 1e-14 of its larger operand is rounding noise, 0.
            r = a + b if kind == "+" else a - b
            return 0.0 if abs(r) < 1e-14 * max(abs(a), abs(b)) else r
        if kind == "*":
            return a * b
        if kind == "/":
            if b == 0:
                raise ZeroDivisionError
            return a / b
        try:
            return math.pow(a, b)
        except OverflowError:
            # Too large: infinity, negative for a negative base and an odd whole exponent.
            odd = b == int(b) and int(b) % 2 == 1
            return -math.inf if a < 0 and odd else math.inf
        except ValueError:
            # 0 to a negative power: infinity (IEEE), which Tally reports as not finite.
            if a == 0:
                return math.inf
            raise

    def scale(self, t):
        """The largest magnitude met while computing `t`: the tolerance's scale (Tally rounds sums
        below 1e-14 of their operands to 0)."""
        try:
            values = [abs(self._eval(t))]
        except (ArithmeticError, ValueError):
            return math.inf
        for child in t[1:]:
            if isinstance(child, tuple):
                values.append(self.scale(child))
        return max(values + [1.0])
